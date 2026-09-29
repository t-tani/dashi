"""置き換え表を、日本語の語をキーにした対応表へ組み替えて JSON に書く。

検査する側は文書に現れた語から引くので、表の行を語でまとめ直す。値は、その語を挙げた
(軸, 元のカタカナ語, 意味, モデル) ごとの項目の並びである。項目は、現れた応答の数の多い順、
同数なら応答の中での平均の順位が前のものを先に置く。

`english` と `loanword` は組で 1 つの意味を指す。カタカナ語の軸では、その意味をカタカナ語が
覆っていて、日本語の語がその置き換えにあたる。英語の軸にはカタカナ語がないので `loanword` は空で、
英語 1 語を日本語 1 語で覆う型を表す。
"""
import argparse
import json
from collections import defaultdict


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--table", required=True, help="build_table.py の出力")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    index = defaultdict(list)
    with open(args.table, encoding="utf-8") as source:
        header = source.readline().rstrip("\n").split("\t")
        for line in source:
            row = dict(zip(header, line.rstrip("\n").split("\t")))
            index[row["word"]].append({
                "axis": row["axis"],
                "loanword": row["loanword"],
                "sense": int(row["sense"]) if row["sense"] else None,
                "english": row["english"],
                "model": row["model"],
                "votes": int(row["votes"]),
                "samples": int(row["samples"]),
                "mean_rank": float(row["mean_rank"]),
            })
    for entries in index.values():
        entries.sort(key=lambda e: (-e["votes"], e["mean_rank"], e["loanword"], e["english"], e["model"]))
    with open(args.out, "w", encoding="utf-8") as sink:
        json.dump({word: index[word] for word in sorted(index)}, sink, ensure_ascii=False)
    print(f"語 {len(index)}、項目 {sum(len(e) for e in index.values())}")


if __name__ == "__main__":
    main()
