"""表の語のうち、意味が割れているものを選ぶ。

語が持つカタカナ語の文脈どうしの近さを測る。`段` は `段組み` の意味(コラム)と処理の順の意味
(ステップ)を持ち、この 2 つの文脈は離れている。`項目` は `アイテム` と `フィールド` を持つが、
どちらも同じものを指し、文脈は近い。離れた組を持つ語だけが、意味の取り違えが起きうる語である。

出力は 1 行 1 語で、最も離れた 2 つのカタカナ語と、その近さを持つ。
"""
import argparse
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path


def read_profiles(directories, wanted):
    profiles = defaultdict(Counter)
    totals = Counter()
    for directory in directories:
        with (Path(directory) / "cooccurrence-counts.tsv").open(encoding="utf-8") as source:
            for line in source:
                key, count = line.rstrip("\n").rsplit("\t", 1)
                word, _, context = key.partition("\t")
                if word in wanted:
                    profiles[word][context] += int(count)
        with (Path(directory) / "cooccurrence-targets.tsv").open(encoding="utf-8") as source:
            for line in source:
                word, count = line.rstrip("\n").split("\t")
                if word in wanted:
                    totals[word] += int(count)
    return profiles, totals


def cosine(a, b):
    keys = set(a) & set(b)
    if not keys:
        return 0.0
    norm_a = math.sqrt(sum(v * v for v in a.values()))
    norm_b = math.sqrt(sum(v * v for v in b.values()))
    if not norm_a or not norm_b:
        return 0.0
    return sum(a[k] * b[k] for k in keys) / (norm_a * norm_b)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--field", nargs="+", required=True)
    parser.add_argument("--table", required=True)
    parser.add_argument("--min-occurrences", type=int, default=100, help="カタカナ語の出現の下限")
    parser.add_argument("--max-similarity", type=float, default=0.15, help="離れていると見なす近さの上限")
    args = parser.parse_args()
    senses = defaultdict(set)
    with open(args.table, encoding="utf-8") as source:
        next(source)
        for line in source:
            fields = line.rstrip("\n").split("\t")
            if fields[1] == "loanword":
                senses[fields[0]].add(fields[2])
    wanted = set(senses) | {j for v in senses.values() for j in v}
    profiles, totals = read_profiles(args.field, wanted)
    rows = []
    for word, loanwords in senses.items():
        known = sorted(j for j in loanwords if totals.get(j, 0) >= args.min_occurrences)
        if len(known) < 2:
            continue
        worst = min(
            ((cosine(profiles[a], profiles[b]), a, b)
             for i, a in enumerate(known) for b in known[i + 1:]),
            default=None,
        )
        if worst and worst[0] <= args.max_similarity:
            rows.append((word, worst[1], worst[2], worst[0], len(known)))
    rows.sort(key=lambda r: r[3])
    print("word\tloanword_a\tloanword_b\tsimilarity\tsenses")
    for word, a, b, sim, n in rows:
        print(f"{word}\t{a}\t{b}\t{sim:.3f}\t{n}")
    print(f"語 {len(senses)} のうち、意味が割れる語 {len(rows)}", file=sys.stderr)


if __name__ == "__main__":
    main()
