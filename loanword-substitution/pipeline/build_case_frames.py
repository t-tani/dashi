"""置き換えの語ごとに、付いた格助詞と係り先の述語の組を、分野と一般と法令の回数を添えて 1 つの TSV に書く。

行は (語, 格助詞, 述語) である。回数は 3 つ持つ。分野は技術文書と情報技術の機関と技術雑誌・技術書と政府の文書と
Wikipedia の `STEM.Computing` の記事の合計、一般は Wikipedia の全記事から `STEM.Computing` の記事を
引いた残り、法令は e-Gov の法令 API から取った条文である。同じダンプを同じ実装で数えているので、
一般の引き算は情報技術以外の記事の回数になる。法令は文体と語法が他と違うので、分野にも一般にも
混ぜずに分けて数える。

数えるのは単独で立ち、かつ述語に係った出現だけである。単独とは、前に名詞・形状詞・接頭辞がなく、
後ろに名詞・形状詞・接尾辞がない出現である。回数は corpus-tool の `count-case-frames` が書く。

載せる語は置き換え表の語である。分野で稀な語が一般では豊富な組を持つ、といった見方をするための表で、
語の意味がどの述語のどの格で分かれるかを読む。
"""
import argparse
import json
from collections import defaultdict
from pathlib import Path

FIELD = ("it-docs", "it-agency", "it-press", "jp-gov", "wiki-computing")
LAW = "jp-law"


def read_counts(path):
    """`<語>\t<格助詞>\t<述語>\t<回数>` の TSV を読む。"""
    counts = defaultdict(int)
    with path.open(encoding="utf-8") as source:
        for line in source:
            word, case, predicate, count = line.rstrip("\n").split("\t")
            counts[(word, case, predicate)] += int(count)
    return counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case-frames-dir", required=True, help="count-case-frames の出力の親")
    parser.add_argument("--map", required=True, help="build_word_map.py の出力")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    words = set(json.loads(Path(args.map).read_text(encoding="utf-8")))
    directory = Path(args.case_frames_dir)
    field = defaultdict(int)
    for name in FIELD:
        for key, count in read_counts(directory / name / "case-frame-counts.tsv").items():
            field[key] += count
    computing = read_counts(directory / "wiki-computing" / "case-frame-counts.tsv")
    general = read_counts(directory / "wiki-all" / "case-frame-counts.tsv")
    for key, count in computing.items():
        general[key] -= count
    law = read_counts(directory / LAW / "case-frame-counts.tsv")
    rows = []
    for key in set(field) | set(general) | set(law):
        if key[0] not in words:
            continue
        rows.append((key, field.get(key, 0), max(general.get(key, 0), 0), law.get(key, 0)))
    rows.sort(key=lambda row: (row[0][0], -row[1], -row[2], -row[3], row[0][1], row[0][2]))
    with open(args.out, "w", encoding="utf-8") as sink:
        sink.write("word\tcase\tpredicate\tfield\tgeneral\tlaw\n")
        for (word, case, predicate), in_field, in_general, in_law in rows:
            sink.write(f"{word}\t{case}\t{predicate}\t{in_field}\t{in_general}\t{in_law}\n")
    print(f"語 {len({row[0][0] for row in rows})}、組 {len(rows)}、"
          f"分野の延べ {sum(row[1] for row in rows)}、一般の延べ {sum(row[2] for row in rows)}、"
          f"法令の延べ {sum(row[3] for row in rows)}")


if __name__ == "__main__":
    main()
