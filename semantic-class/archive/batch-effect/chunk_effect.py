"""バッチの組み方が判定を動かすかを測る対照実験。

同じ 400 語を、異なる無作為のバッチの組み方で 2 回分類する。1 回目と 2 回目で、
ある語と同じバッチに載る語も、バッチ内の位置も変わる。判定がバッチの中身に影響
されないなら、2 回の categories は一致するはずである。

測るのは 2 つである。第 1 に、語ごとの categories の集合一致率。第 2 に、
バッチの前半 10 語と後半 10 語で、語義の数の平均に差が出るか。

`--same-chunking` を渡すと、2 回とも同じバッチの組み方で走る。この対照が要るのは、
一致率が下がる原因を切り分けるためである。バッチを変えたときだけ一致が下がるなら
原因はバッチの中身だが、バッチを変えなくても同じだけ下がるなら、原因は走行ごとの
揺れであり、バッチを小さくしても直らない。

本番の JSONL とは別のファイルへ書く。対照実験の判定を母集団の判定に混ぜない
ためである。
"""

import argparse
import datetime
import json
import os
import random
import statistics
import sys
import threading
from concurrent.futures import ThreadPoolExecutor

import dspy

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "pipeline"))

import votes_v5  # noqa: E402
from classifier_v2 import entry_of, make_predictor  # noqa: E402
from lm_v3 import make_lm, model_name  # noqa: E402

CHUNK = 20
WORKERS = 8
LOW, HIGH = 10, 57


def classify_all(classify, words, seed, model, today, chunk_size=CHUNK):
    """種を変えてバッチを割り直し、語ごとのレコードを返す。"""
    order = list(words)
    random.Random(seed).shuffle(order)
    chunks = [order[i : i + chunk_size] for i in range(0, len(order), chunk_size)]
    got = {}
    lock = threading.Lock()

    def run(numbered):
        number, chunk = numbered
        at = {word: index for index, word in enumerate(chunk)}
        for attempt in range(3):
            try:
                result = classify(entries=[entry_of(w, None) for w in chunk])
                rows = {}
                for item in result.classifications:
                    if item.word not in at:
                        continue
                    cats = sorted({c for c in item.categories if LOW <= c <= HIGH})
                    if cats:
                        rows[item.word] = votes_v5.record(
                            item.word, cats, model, today, tier="jmdict",
                            source="chunk-effect",
                            chunk_id=f"seed{seed}-{number:04d}",
                            chunk_index=at[item.word],
                        )
                if not rows:
                    raise ValueError("返った語が無い")
                with lock:
                    got.update(rows)
                return
            except Exception:
                if attempt == 2:
                    return

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(run, enumerate(chunks)))
    return got


def halves(rows, chunk_size=CHUNK):
    """バッチの前半と後半で、語義の数を集める。"""
    middle = chunk_size // 2
    front = [len(r["categories"]) for r in rows.values() if r["chunk_index"] < middle]
    back = [len(r["categories"]) for r in rows.values() if r["chunk_index"] >= middle]
    return front, back


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--words", required=True)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--model-key", required=True, choices=("ANTHROPIC_MODEL", "OPENAI_MODEL"))
    parser.add_argument("--sample", type=int, default=400)
    parser.add_argument("--same-chunking", action="store_true")
    parser.add_argument("--label", default="")
    parser.add_argument("--chunk", type=int, default=CHUNK, help="1 バッチの語数")
    args = parser.parse_args()

    pool = [w for w in open(args.words, encoding="utf-8").read().split("\n") if w]
    random.seed(694)
    words = sorted(random.sample(pool, args.sample))
    model = model_name(args.model_key)
    today = datetime.date.today().isoformat()
    dspy.configure(lm=make_lm(args.model_key))
    classify = make_predictor()

    print(f"モデル {model} / 標本 {len(words)} 語 / 1 バッチ {args.chunk} 語", flush=True)
    second_seed = 1001 if args.same_chunking else 2002
    print(f"バッチの組み方の種 1 回目 1001 / 2 回目 {second_seed}", flush=True)
    first = classify_all(classify, words, 1001, model, today, args.chunk)
    print(f"1 回目 {len(first)} 語", flush=True)
    second = classify_all(classify, words, second_seed, model, today, args.chunk)
    print(f"2 回目 {len(second)} 語", flush=True)

    both = sorted(set(first) & set(second))
    same = [w for w in both if first[w]["categories"] == second[w]["categories"]]
    overlap = [w for w in both if set(first[w]["categories"]) & set(second[w]["categories"])]
    rate = len(same) / len(both) if both else 0.0
    print(f"\n両方で判定が返った語 {len(both)}")
    print(f"集合一致 {len(same)} 語 = {rate:.1%}")
    print(f"交わりあり {len(overlap)} 語 = {len(overlap) / len(both):.1%}")

    print("\nバッチ内の位置ごとの語義の数(平均)")
    lines = []
    for label, rows in (("1 回目", first), ("2 回目", second)):
        front, back = halves(rows, args.chunk)
        line = (
            f"{label}: 前半 {statistics.mean(front):.3f} ({len(front)} 語) / "
            f"後半 {statistics.mean(back):.3f} ({len(back)} 語) / "
            f"差 {statistics.mean(front) - statistics.mean(back):+.3f}"
        )
        print(line)
        lines.append(line)

    differing = [
        {"word": w, "first": first[w]["categories"], "second": second[w]["categories"]}
        for w in both
        if first[w]["categories"] != second[w]["categories"]
    ]
    os.makedirs(args.out_dir, exist_ok=True)
    name = model.replace("/", "-") + (args.label or ("-same" if args.same_chunking else ""))
    with open(
        os.path.join(args.out_dir, f"chunk-effect-{name}.json"), "w", encoding="utf-8"
    ) as sink:
        json.dump(
            {
                "model": model,
                "same_chunking": args.same_chunking,
                "chunk": args.chunk,
                "sample": len(words),
                "judged_both": len(both),
                "set_match": len(same),
                "set_match_rate": rate,
                "overlap_rate": len(overlap) / len(both) if both else 0.0,
                "position": lines,
                "differing": differing,
                "first": first,
                "second": second,
            },
            sink,
            ensure_ascii=False,
            indent=1,
        )
    print(f"\n判定 {'は 95% 以上' if rate >= 0.95 else 'が 95% を下回った'}")


if __name__ == "__main__":
    main()
