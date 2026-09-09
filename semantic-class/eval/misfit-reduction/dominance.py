"""現行の既定より良い構成があるかを判定し、フィルタの選択性を測る。

良いとは、現行の既定(閾値 0.85、除外なし)に対して、真陽性を 1 件も落とさず、
偽陰性を増やさず、指摘の総数を減らすことである。正解集合が小さく区間が広い
ので、区間の重なりではなくこの関係で白黒を付ける。

母数の外に出た指摘には人手の裁定が無い。落とすと閾値を下げた構成が有利に
見えるので、この判定では母数内の偽陽性と母数外の指摘を足した数を、指摘の
総数として比べる。

フィルタの選択性は、そのフィルタが落とした指摘のうち偽陽性だった数と、
巻き添えにした真陽性の数で測る。真陽性を落とすフィルタは重ね掛けの候補に
しない。
"""

import argparse
import pathlib

import score

HERE = pathlib.Path(__file__).parent
RUNS = HERE / "runs"
EXEMPT = HERE / "exemptions"

FILTERS = [
    "class48",
    "comp-any",
    "comp-first",
    "wordnet-abstract",
    "wordnet-abstract-wide",
    "other-corpus",
    "other-corpus-judged",
    "katakana",
]

# 30 文書の外で妥当と裁定された語。推奨する構成が、これを免除で消さないことを
# 確かめる。出所は dashi の eval の各 verdicts である。
RETENTION = {
    "野生": "finding-verdicts/verdicts_v2.py OK old:1、luna-comparison B:13",
    "ボード": "finding-verdicts/verdicts_v2.py OK old:38",
    "導線": "finding-verdicts/verdicts_v2.py OK old:55",
    "バケット": "finding-verdicts/verdicts_v2.py OK new:58、shipped-default-diff AFTER:21",
    "窓": "finding-verdicts/verdicts_v2.py OK both:15、shipped-default-diff AFTER:22",
    "原子": "finding-verdicts/verdicts_v2.py OK both:17",
    "死": "finding-verdicts/verdicts_v2.py FINAL_OK:26",
    "機械": "finding-verdicts/verdicts_v2.py FINAL_OK:30(同じ語を REASON は定着と裁定)",
    "レバー": "shipped-default-diff/verdicts.py BEFORE:28",
    "透かし": "shipped-default-diff/verdicts.py AFTER:23",
    "口": "luna-comparison/verdicts_v3.py A:21",
    "ルアー": "luna-comparison/verdicts_v3.py U:24",
}


def run(ratio, lists, truth, unit):
    exempt = score.load_exempt([EXEMPT / f"{n}.txt" for n in lists])
    findings = score.apply_exempt(score.load_findings(RUNS / f"lint-{ratio}.json"), exempt)
    tp, fp, fn, tn, outside = score.score(findings, truth, unit)
    return findings, tp, fp, fn, tn, len(outside)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--queue", required=True)
    parser.add_argument("--unit", choices=["occurrence", "word"], default="occurrence")
    args = parser.parse_args()
    judged = score.load_truth(args.queue)
    truth = score.by_occurrence(judged) if args.unit == "occurrence" else score.by_word(judged)
    base = run("0.85", [], truth, args.unit)
    _, btp, bfp, bfn, _, bout = base
    print(f"現行の既定: 指摘 {len(base[0])} TP {btp} FP {bfp} FN {bfn} 母数外 {bout}")

    print("\n== 閾値だけを動かしたとき、偽陰性を取り戻せるか")
    for ratio in ["0.50", "0.55", "0.60", "0.65", "0.70", "0.75", "0.80", "0.85", "0.90"]:
        findings, tp, fp, fn, _, out = run(ratio, [], truth, args.unit)
        print(f"  {ratio}: 指摘 {len(findings)} TP {tp} FN {fn} 母数内 FP {fp} 母数外 {out} "
              f"/ 出力の総数 {fp + out}")

    print("\n== フィルタの選択性(閾値 0.85 の 68 件に当てる)")
    for name in FILTERS:
        findings, tp, fp, fn, _, out = run("0.85", [name], truth, args.unit)
        print(f"  {name}: 残り {len(findings)} 件 / 刈った {len(base[0]) - len(findings)} 件 "
              f"(うち偽陽性 {bfp - fp} 件、真陽性 {btp - tp} 件) "
              f"/ 重ね掛けの候補 {'はい' if tp == btp else 'いいえ'}")

    print("\n== 30 文書の外で妥当と裁定された語を、フィルタが消すか")
    header = "  語\t" + "\t".join(FILTERS)
    print(header)
    for word, source in RETENTION.items():
        marks = []
        for name in FILTERS:
            exempt = score.load_exempt([EXEMPT / f"{name}.txt"])
            marks.append("消す" if word in exempt else "残す")
        print(f"  {word}\t" + "\t".join(marks) + f"\t({source})")


if __name__ == "__main__":
    main()
