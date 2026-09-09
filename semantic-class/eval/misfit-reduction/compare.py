"""比べる条件をすべて採点し、1 つの表にまとめる。

条件は、閾値 `abstract_ratio` の値と、後段で落とす免除の語の一覧の組で決まる。
免除は検出器の外で当てる。この実験では検出器を書き換えないので、指摘を語で
落とす形で条件を再現する。

母数は正解集合の人手裁定の行に固定してある。閾値を下げると母数の外の指摘が
増えるので、適合率を 2 通りで出す。母数の中だけで数えた値と、母数の外の指摘を
すべて偽陽性と置いた下限である。真の適合率はこの 2 つの間にある。
"""

import argparse
import pathlib

import score

RUNS = pathlib.Path(__file__).parent / "runs"
EXEMPT = pathlib.Path(__file__).parent / "exemptions"

# (行の名前, 閾値, 免除の一覧の名前)
CONDITIONS = [
    ("閾値 0.50", "0.50", []),
    ("閾値 0.55", "0.55", []),
    ("閾値 0.60", "0.60", []),
    ("閾値 0.65", "0.65", []),
    ("閾値 0.70", "0.70", []),
    ("閾値 0.75", "0.75", []),
    ("閾値 0.80", "0.80", []),
    ("閾値 0.85(既定)", "0.85", []),
    ("閾値 0.90", "0.90", []),
    ("既定 + 中項目 48 の免除", "0.85", ["class48"]),
    ("既定 + comp タグの免除(全語義)", "0.85", ["comp-any"]),
    ("既定 + comp タグの免除(第 1 義)", "0.85", ["comp-first"]),
    ("既定 + 英訳の抽象の免除", "0.85", ["wordnet-abstract"]),
    ("既定 + 英訳の抽象の免除(広い)", "0.85", ["wordnet-abstract-wide"]),
    ("既定 + 他コーパスの指摘語の免除", "0.85", ["other-corpus"]),
    ("既定 + 他コーパスの裁定済みの免除", "0.85", ["other-corpus-judged"]),
    ("既定 + カタカナ語の免除", "0.85", ["katakana"]),
    ("閾値 0.70 + 中項目 48 の免除", "0.70", ["class48"]),
    ("閾値 0.70 + comp タグの免除(全語義)", "0.70", ["comp-any"]),
    ("閾値 0.70 + 英訳の抽象の免除", "0.70", ["wordnet-abstract"]),
    ("既定 + 中項目 48 + 英訳の抽象", "0.85", ["class48", "wordnet-abstract"]),
    ("既定 + comp + 英訳の抽象", "0.85", ["comp-any", "wordnet-abstract"]),
    ("既定 + comp + 英訳の抽象(広い)", "0.85", ["comp-any", "wordnet-abstract-wide"]),
    ("既定 + comp + 英訳の抽象 + 他コーパスの裁定済み", "0.85",
     ["comp-any", "wordnet-abstract", "other-corpus-judged"]),
    ("既定 + 英訳の抽象 + 他コーパスの裁定済み", "0.85", ["wordnet-abstract", "other-corpus-judged"]),
    ("閾値 0.70 + 英訳の抽象 + 中項目 48", "0.70", ["wordnet-abstract", "class48"]),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--queue", required=True)
    parser.add_argument("--unit", choices=["occurrence", "word"], default="word")
    parser.add_argument("--tsv", required=True)
    args = parser.parse_args()
    judged = score.load_truth(args.queue)
    truth = score.by_occurrence(judged) if args.unit == "occurrence" else score.by_word(judged)
    header = ["条件", "指摘", "TP", "FP", "FN", "TN", "適合率", "適合率の下限", "再現率", "母数外"]
    lines = ["\t".join(header)]
    print(f"人手裁定 {len(judged)} 行 / 単位 {args.unit} の母数 {len(truth)}")
    for name, ratio, lists in CONDITIONS:
        exempt = score.load_exempt([EXEMPT / f"{n}.txt" for n in lists])
        findings = score.apply_exempt(score.load_findings(RUNS / f"lint-{ratio}.json"), exempt)
        tp, fp, fn, tn, outside = score.score(findings, truth, args.unit)
        prec = tp / (tp + fp) if tp + fp else 0.0
        low = tp / (tp + fp + len(outside)) if tp + fp + len(outside) else 0.0
        rec = tp / (tp + fn) if tp + fn else 0.0
        row = [name, str(len(findings)), str(tp), str(fp), str(fn), str(tn),
               f"{prec:.1%}", f"{low:.1%}", f"{rec:.1%}", str(len(outside))]
        lines.append("\t".join(row))
        print("\t".join(row))
    pathlib.Path(args.tsv).write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
