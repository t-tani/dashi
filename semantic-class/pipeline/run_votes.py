"""語の一覧を分類し、保存形式 v5 の JSONL へ追記する。

再実行しても、既に判定済みの語は飛ばして続きから走る。判定済みかどうかは
出力の JSONL を読んで決めるので、走行の途中で止まっても、次の走行が残りだけを
拾う。塊の中の 1 語でも返れば、その語は判定済みになる。

塊の中身は種 694 で混ぜる。並びのままだと、字種の偏った塊ができてモデルが
空の配列を返すことがある。

バッチの来歴をレコードに残す。chunk_id はその走行でのバッチの通し番号に語順の種を
添えた識別子で、chunk_index はバッチ内の位置である。同じバッチに載る語が判定へ
及ぼす影響を、後から統計で検査できるようにするためである。
"""

import argparse
import datetime
import json
import os
import random
import threading
from concurrent.futures import ThreadPoolExecutor

import dspy

import load_target
import votes
from classifier import entry_of, make_predictor
from corpus import context_of
from jmdict import priority_words
from lm import make_lm, model_name

CHUNK = 20
WORKERS = 8


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--words", required=True, help="1 行 1 語の一覧")
    parser.add_argument("--out-dir", required=True, help="votes のディレクトリ")
    parser.add_argument("--model-key", required=True, choices=("ANTHROPIC_MODEL", "OPENAI_MODEL"))
    parser.add_argument("--tier", required=True, choices=votes.TIERS)
    parser.add_argument(
        "--target",
        required=True,
        help="分類対象のディレクトリ。Signature と実例と、許す中項目番号の集合が変わる",
    )
    parser.add_argument(
        "--corpus-words",
        help="この一覧にある語の tier は corpus にする。層をまたぐ語の一覧を"
        "処理するときに要る",
    )
    parser.add_argument("--jmdict", required=True)
    parser.add_argument("--limit", type=int, help="先頭からこの語数だけ処理する(試走用)")
    parser.add_argument("--seed", type=int, default=694, help="語順を混ぜる種")
    parser.add_argument("--tag", default="", help="chunk_id の接頭。対照実験で走行を分ける")
    parser.add_argument("--workers", type=int, default=WORKERS, help="同時に投げる塊の数")
    parser.add_argument(
        "--corpus-text",
        help="用例を切り出す本文。渡すと、本文に現れる語へ用例を添える。"
        "1 回目の判定と同じ入力でパスを重ねるために要る",
    )
    args = parser.parse_args()

    target = load_target.load(args.target)
    model = model_name(args.model_key)
    out = votes.path_for(args.out_dir, model, args.seed)
    done = votes.words_of(out)
    words = [
        w
        for w in open(args.words, encoding="utf-8").read().split("\n")
        if w and w not in done
    ]
    if args.limit:
        words = words[: args.limit]
    print(
        f"モデル {model} / 判定済み {len(done)} / これから {len(words)}"
        f" / 種 {args.seed} / 同時 {args.workers}",
        flush=True,
    )
    if not words:
        print("残りが無いので何もしない")
        return

    common = priority_words(args.jmdict)
    # 1 回目の判定と入力をそろえる。用例の有無が違うと、バッチの組み方だけを変えた比較に
    # ならない。
    corpus_words = set()
    if args.corpus_words:
        corpus_words = {
            w
            for w in open(args.corpus_words, encoding='utf-8').read().split('\n')
            if w
        }
    contexts = {}
    if args.corpus_text:
        text = open(args.corpus_text, encoding='utf-8').read()
        contexts = {w: context_of(w, text) for w in words}
        print(f"用例を添えられた語 {sum(1 for c in contexts.values() if c)}", flush=True)
    today = datetime.date.today().isoformat()
    dspy.configure(lm=make_lm(args.model_key))
    classify = make_predictor(target)
    allowed = target.CATEGORIES

    random.Random(args.seed).shuffle(words)
    chunks = [words[i : i + CHUNK] for i in range(0, len(words), CHUNK)]
    stamp = f"{args.tag}s{args.seed}-" if args.tag else f"s{args.seed}-"
    lock = threading.Lock()
    counter, failed = [0, 0], []

    def run(numbered):
        number, chunk = numbered
        chunk_id = f"{stamp}{number:05d}"
        at = {word: index for index, word in enumerate(chunk)}
        for attempt in range(3):
            try:
                result = classify(
                    entries=[entry_of(w, contexts.get(w)) for w in chunk]
                )
                wanted = set(chunk)
                rows, seen = [], set()
                for item in result.classifications:
                    # モデルが同じ語を 2 度返すことがある。先に返った分を採る。
                    if item.word not in wanted or item.word in seen:
                        continue
                    seen.add(item.word)
                    cats = sorted({c for c in item.categories if c in allowed})
                    if cats:
                        rows.append(
                            votes.record(
                                item.word,
                                cats,
                                model,
                                today,
                                context=contexts.get(item.word),
                                tier="corpus"
                                if item.word in corpus_words
                                else args.tier,
                                jmdict_priority=item.word in common,
                                source="api",
                                chunk_id=chunk_id,
                                chunk_index=at[item.word],
                                pass_seed=args.seed,
                            )
                        )
                if not rows:
                    raise ValueError("返った語が無い")
                with lock:
                    votes.append(out, rows)
                    counter[0] += 1
                    counter[1] += len(rows)
                    if counter[0] % 50 == 0:
                        print(
                            f"{counter[0]}/{len(chunks)} 塊 / {counter[1]} 語",
                            flush=True,
                        )
                return
            except Exception as error:
                if attempt == 2:
                    with lock:
                        failed.append((chunk, f"{type(error).__name__}: {error}"[:160]))

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        list(pool.map(run, enumerate(chunks)))

    print(f"通った塊 {counter[0]}/{len(chunks)} / 書いた語 {counter[1]} / 失敗した塊 {len(failed)}")
    for chunk, why in failed[:5]:
        print("  失敗:", chunk[:4], why)
    if failed:
        report = os.path.join(args.out_dir, f"failed-{model.replace('/', '-')}-s{args.seed}.json")
        with open(report, "w", encoding="utf-8") as sink:
            json.dump(
                [{"words": c, "error": w} for c, w in failed], sink, ensure_ascii=False
            )
        print("失敗した塊を書き出した:", report)


if __name__ == "__main__":
    main()
