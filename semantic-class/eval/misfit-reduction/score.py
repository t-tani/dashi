"""正解集合の上で、concrete-noun-misfit の指摘の集合を採点する。

正解は待ち行列のうち人手裁定が入った 91 行だけで、母数はその行に固定する。
lint の指摘は、文書と行と語で待ち行列の行へ割り当てる。語は指摘の message に
ある正規化形と、excerpt の表層形の両方で照合する。

判定の単位を 2 通りで数える。出現の単位は methodology.md の定義そのままで、
配布中の既定の値を再現するのに使う。文書と語の組の単位は、条件を変えて比べる
のに使う。検出器は 1 文書につき同じ語を 1 度しか報告しないので、閾値を下げると
報告される出現が前の段落へ移り、同じ語を同じ文書で指摘しているのに出現の単位
では別の行に当たる。この移動は検出の良し悪しではないので、比較には組の単位を
使う。

母数の外に出た指摘は、人手の裁定が無いので混同行列に入れない。件数だけを別に
数え、適合率の下限を出すのに使う。
"""

import argparse
import csv
import json
import math
import pathlib
import re

QUOTED = re.compile(r"^'([^']+)'")


def wilson(k, n):
    if n == 0:
        return 0.0, 0.0
    p, z = k / n, 1.96
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, c - h), min(1.0, c + h)


def load_truth(path):
    """人手裁定の入った行を返す。"""
    rows = csv.DictReader(open(path, encoding="utf-8"), delimiter="\t")
    return [r for r in rows if (r.get("確認") or "").strip()]


def load_findings(path):
    """lint の JSON から concrete-noun-misfit の指摘を返す。

    1 件は (文書, 行, 正規化形, 表層形) である。
    """
    data = json.load(open(path, encoding="utf-8"))
    out = []
    for f in data["findings"]:
        if f["rule"] != "concrete-noun-misfit":
            continue
        m = QUOTED.match(f["message"])
        out.append(
            (
                pathlib.Path(f["path"]).name,
                f["line"],
                m.group(1) if m else "",
                f.get("excerpt", ""),
            )
        )
    return out


def by_occurrence(judged):
    """出現の単位の正解を、(文書, 行, 語) から「指摘すべきか」への辞書で返す。

    同じ鍵の行が複数あるときは、混同行列を行の数で数えられるよう、鍵ごとに
    行を並べて持つ。
    """
    truth = {}
    for r in judged:
        truth.setdefault((r["文書"], int(r["行"]), r["語"]), []).append(r["確認"] == "はい")
    return truth


def by_word(judged):
    """文書と語の組の単位の正解を返す。組の中に「はい」が 1 つでもあれば真とする。"""
    truth = {}
    for r in judged:
        key = (r["文書"], r["語"])
        truth[key] = [truth.get(key, [False])[0] or r["確認"] == "はい"]
    return truth


def match(findings, keys_of):
    """指摘を正解の鍵へ割り当てる。当たった鍵の集合と、外れた指摘を返す。"""
    flagged, outside = set(), []
    for doc, line, word, surface in findings:
        for key in keys_of(doc, line, word, surface):
            flagged.add(key)
            break
        else:
            outside.append((doc, line, word))
    return flagged, outside


def matrix(truth, flagged):
    """混同行列を数える。鍵に複数の行がぶら下がる場合は行の数で数える。"""
    tp = fp = fn = tn = 0
    for key, answers in truth.items():
        said = key in flagged
        for should in answers:
            if said and should:
                tp += 1
            elif said:
                fp += 1
            elif should:
                fn += 1
            else:
                tn += 1
    return tp, fp, fn, tn


def load_exempt(paths):
    """免除の語の一覧を読み、和集合を返す。"""
    words = set()
    for path in paths or []:
        words |= {
            line.strip()
            for line in open(path, encoding="utf-8")
            if line.strip() and not line.startswith("#")
        }
    return words


def apply_exempt(findings, exempt):
    """免除の語に当たる指摘を落とす。正規化形と表層形の両方で照合する。"""
    return [f for f in findings if f[2] not in exempt and f[3] not in exempt]


def score(findings, truth, unit):
    if unit == "occurrence":
        def keys_of(doc, line, word, surface):
            return [k for k in ((doc, line, word), (doc, line, surface)) if k in truth]
    else:
        def keys_of(doc, line, word, surface):
            return [k for k in ((doc, word), (doc, surface)) if k in truth]
    flagged, outside = match(findings, keys_of)
    return (*matrix(truth, flagged), outside)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--queue", required=True)
    parser.add_argument("--findings", nargs="+", required=True)
    parser.add_argument("--unit", choices=["occurrence", "word"], default="word")
    parser.add_argument("--exempt", nargs="*", help="免除の語の一覧。和集合を取る")
    parser.add_argument("--label", help="結果の行に付ける名前")
    parser.add_argument("--tsv")
    args = parser.parse_args()
    judged = load_truth(args.queue)
    exempt = load_exempt(args.exempt)
    truth = by_occurrence(judged) if args.unit == "occurrence" else by_word(judged)
    print(f"人手裁定 {len(judged)} 行 / 単位 {args.unit} の母数 {len(truth)}")
    header = ["条件", "指摘", "TP", "FP", "FN", "TN", "適合率", "再現率", "母数外"]
    lines = ["\t".join(header)]
    for path in args.findings:
        name = args.label or pathlib.Path(path).stem
        findings = apply_exempt(load_findings(path), exempt)
        tp, fp, fn, tn, outside = score(findings, truth, args.unit)
        prec = tp / (tp + fp) if tp + fp else 0.0
        rec = tp / (tp + fn) if tp + fn else 0.0
        plo, phi = wilson(tp, tp + fp)
        rlo, rhi = wilson(tp, tp + fn)
        print(
            f"{name}: 指摘 {len(findings)} / TP {tp} FP {fp} FN {fn} TN {tn} / "
            f"適合率 {prec:.1%}({plo:.1%}〜{phi:.1%}) "
            f"再現率 {rec:.1%}({rlo:.1%}〜{rhi:.1%}) / 母数外 {len(outside)}"
        )
        lines.append("\t".join([
            name, str(len(findings)), str(tp), str(fp), str(fn), str(tn),
            f"{prec:.1%}", f"{rec:.1%}", str(len(outside)),
        ]))
    if args.tsv:
        pathlib.Path(args.tsv).write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
