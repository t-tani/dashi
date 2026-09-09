"""複数パスの判定から、語ごとの確定値を作る。

日常語の部分集合は 3 つのパスで判定してある。パスごとにバッチの組み方の種が違うので、
同じ語でも一緒のバッチに載る語が変わる。温度 0 ではバッチを固定すると同じ判定が出るため、
判定の独立性はバッチの組み方の違いだけから来る。

確定の規則は 2 つである。2 パス以上に現れた語は、2 パス以上が挙げた中項目だけを
採る。1 パスにしか現れない裾の語は、その 1 回の判定をそのまま採る。どちらで決めたかは
votes(その語が現れたパスの数)と rule で区別できる。

どの中項目も 2 回の判定に届かない語は確定しない。3 パスの答えが 3 通りに割れた場合で
ある。この語は確定表へ載せない。載せなければ検出器は黙るので、揺れた分類で
指摘を出すより安全側に倒れる。落とした語は eval の一覧へ書き、上書き表で個別に
裁く候補として残す。

2 回の判定に届かず落ちた中項目も別に記録する。1 パスだけが挙げた語義であり、モデルが
バッチの中身に引かれて足したか落としたかの跡である。
"""

import argparse
import csv
import json
import os
from collections import Counter

import votes


def load_passes(votes_dir, model, seeds):
    """パスごとの {語: レコード} を、種の順に返す。"""
    loaded = []
    for seed in seeds:
        path = votes.path_for(votes_dir, model, seed)
        rows = votes.read(path)
        print(f"  種 {seed}: {len(rows)} 語 ({os.path.basename(path)})")
        loaded.append((seed, rows))
    return loaded


def decide(sets):
    """(確定した中項目、落ちた中項目、規則、全パス一致か)を返す。

    確定しない語では、確定した中項目を空で返す。
    """
    cast = len(sets)
    unanimous = all(s == sets[0] for s in sets)
    if cast == 1:
        return sets[0], [], "single-vote", True
    counts = Counter(c for s in sets for c in s)
    kept = sorted(c for c, n in counts.items() if n >= 2)
    dropped = sorted(c for c, n in counts.items() if n < 2)
    if kept:
        return kept, dropped, "majority", unanimous
    return [], dropped, "unstable", unanimous


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--votes-dir", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--seeds", required=True, help="パスの種をコンマで区切る")
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--eval-dir", required=True)
    args = parser.parse_args()

    seeds = [int(s) for s in args.seeds.split(",")]
    print(f"{args.model} のパス:")
    passes = load_passes(args.votes_dir, args.model, seeds)

    words = sorted(set().union(*(set(rows) for _, rows in passes)))
    final, unstable, dropped_rows = [], [], []
    stats, agreement = Counter(), Counter()
    dropped_counts = Counter()

    for word in words:
        per_pass = [(seed, rows[word]) for seed, rows in passes if word in rows]
        sets = [row["categories"] for _, row in per_pass]
        kept, dropped, rule, unanimous = decide(sets)
        first = per_pass[0][1]
        cast = len(sets)
        stats[rule] += 1
        stats[f"votes={cast}"] += 1
        if cast >= 2:
            agreement["全パス一致" if unanimous else "パス間で割れた"] += 1

        common = {
            "word": word,
            "tier": first["tier"],
            "jmdict_priority": first["jmdict_priority"],
            "votes": cast,
            "pass_seeds": [seed for seed, _ in per_pass],
            "pass_categories": sets,
        }
        if dropped:
            dropped_counts[len(dropped)] += 1
            dropped_rows.append(
                {
                    "word": word,
                    "kept": " ".join(map(str, kept)),
                    "dropped": " ".join(map(str, dropped)),
                    "passes": " | ".join(" ".join(map(str, s)) for s in sets),
                    "tier": first["tier"],
                }
            )
        if rule == "unstable":
            unstable.append(
                {
                    "word": word,
                    "passes": " | ".join(" ".join(map(str, s)) for s in sets),
                    "tier": first["tier"],
                    "jmdict_priority": "はい" if first["jmdict_priority"] else "いいえ",
                }
            )
            continue
        final.append(
            {
                **common,
                "categories": kept,
                "model": args.model,
                "contract": first["contract"],
                "rule": rule,
                "unanimous": unanimous,
                "dropped_categories": dropped,
                "final": True,
            }
        )

    os.makedirs(args.out_dir, exist_ok=True)
    os.makedirs(args.eval_dir, exist_ok=True)
    name = args.model.replace("/", "-")
    out = os.path.join(args.out_dir, name + ".jsonl")
    with open(out, "w", encoding="utf-8") as sink:
        for row in final:
            print(json.dumps(row, ensure_ascii=False), file=sink)

    unstable_path = os.path.join(args.eval_dir, f"unstable-{name}.tsv")
    with open(unstable_path, "w", encoding="utf-8", newline="") as sink:
        writer = csv.DictWriter(
            sink, fieldnames=["word", "passes", "tier", "jmdict_priority"], delimiter="\t"
        )
        writer.writeheader()
        writer.writerows(unstable)

    dropped_path = os.path.join(args.eval_dir, f"dropped-senses-{name}.tsv")
    with open(dropped_path, "w", encoding="utf-8", newline="") as sink:
        writer = csv.DictWriter(
            sink, fieldnames=["word", "kept", "dropped", "passes", "tier"], delimiter="\t"
        )
        writer.writeheader()
        writer.writerows(dropped_rows)

    print(f"確定 {len(final)} 語 → {out}")
    print("  規則ごと:", {k: v for k, v in sorted(stats.items()) if not k.startswith("votes=")})
    print("  判定の数ごと:", {k: v for k, v in sorted(stats.items()) if k.startswith("votes=")})
    if agreement:
        total = sum(agreement.values())
        same = agreement["全パス一致"]
        print(f"  2 回以上の判定がある語 {total} のうち全パス一致 {same} = {same / total:.1%}")
    print(f"  確定しなかった語 {len(unstable)} → {os.path.basename(unstable_path)}")
    print(f"  2 回の判定に届かず落ちた語義を持つ語 {len(dropped_rows)} → {os.path.basename(dropped_path)}")
    print("    落ちた語義の数ごと:", dict(sorted(dropped_counts.items())))


if __name__ == "__main__":
    main()
