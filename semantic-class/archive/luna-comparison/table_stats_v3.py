# 表そのものの統計。被覆と、旧表と重なる語での一致と、具体物だけを持つ語の割合。
# 旧表は採点にだけ使い、値を写さない。
# 使い方: python table_stats_v3.py <表の TSV> [<表の TSV> ...]
import os
import sys

from score_bench import load_table

OLD = os.environ["AKUNUKI_DIR"] + ".aku/reference/semantic-class-table.tsv"
WORK = os.environ["WORK_DIR"]


def divs(mids):
    return {m // 10 for m in mids}


def main(paths):
    old = load_table(OLD)
    heads = [
        w
        for w in open(WORK + "corpus-headwords.txt", encoding="utf-8").read().split("\n")
        if w
    ]
    print(f"コーパスの見出し語 {len(heads)} / うち旧表に載る {sum(1 for w in heads if w in old)}")
    for path in paths:
        table = load_table(path)
        covered = [w for w in heads if w in table]
        both = [w for w in covered if w in old]
        div_exact = sum(1 for w in both if divs(table[w]) == divs(old[w]))
        conc = sum(
            1
            for w in both
            if (divs(old[w]) <= {4, 5}) == (divs(table[w]) <= {4, 5})
        )
        only_conc = sum(1 for w in table.values() if w and divs(w) <= {4, 5})
        print(
            f"{path.rsplit('/', 1)[-1]}: 行 {len(table)} / 見出し語を覆う {len(covered)}"
            f" / 旧表と重なる {len(both)}"
            f" / 部門の集合一致 {100 * div_exact / len(both):.1f}%"
            f" / 具体条件の一致 {100 * conc / len(both):.1f}%"
            f" / 具体物だけの語 {100 * only_conc / len(table):.1f}%"
        )


if __name__ == "__main__":
    main(sys.argv[1:])
