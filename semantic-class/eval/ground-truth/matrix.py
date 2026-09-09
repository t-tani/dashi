"""人手裁定の入った待ち行列から、混同行列と適合率・再現率を出す。

判定単位は語の出現である。厳密版は人手裁定の済んだ行だけで数え、未確認の行は
別枠に置く。暫定版は、自動の一次付けと検出器の出力が一致する未確認の行を暫定の
真陰性として数える。暫定版は自動判定に依存するので、注記なしに引用してはならない。
"""

import argparse
import csv
import math


def wilson(k, n):
    if n == 0:
        return 0.0, 0.0
    p, z = k / n, 1.96
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, c - h), min(1.0, c + h)


def report(name, tp, fp, fn, tn, note):
    total = tp + fp + fn + tn
    print(f"\n== {name}({note})")
    print(f"  真陽性 {tp} / 偽陽性 {fp} / 偽陰性 {fn} / 真陰性 {tn} / 合計 {total}")
    if tp + fp:
        lo, hi = wilson(tp, tp + fp)
        print(f"  適合率 {tp}/{tp + fp} = {tp / (tp + fp):.1%}(95% 区間 {lo:.1%}〜{hi:.1%})")
    if tp + fn:
        lo, hi = wilson(tp, tp + fn)
        print(f"  再現率 {tp}/{tp + fn} = {tp / (tp + fn):.1%}(95% 区間 {lo:.1%}〜{hi:.1%})")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--queue", required=True)
    args = parser.parse_args()
    rows = list(csv.DictReader(open(args.queue, encoding="utf-8"), delimiter="\t"))

    judged = [r for r in rows if (r.get("確認") or "").strip()]
    unjudged = [r for r in rows if not (r.get("確認") or "").strip()]
    agree = [r for r in unjudged if r["食い違い"] != "はい"]
    left = [r for r in unjudged if r["食い違い"] == "はい"]

    print(f"全 {len(rows)} 行 / 人手裁定 {len(judged)} 行 / 未確認 {len(unjudged)} 行")
    print(f"  未確認のうち、自動と検出器が一致 {len(agree)} 行 / 裁定待ち {len(left)} 行")

    def count(rows_):
        tp = sum(1 for r in rows_ if r["検出器"] == "はい" and r["確認"] == "はい")
        fp = sum(1 for r in rows_ if r["検出器"] == "はい" and r["確認"] == "いいえ")
        fn = sum(1 for r in rows_ if r["検出器"] == "いいえ" and r["確認"] == "はい")
        tn = sum(1 for r in rows_ if r["検出器"] == "いいえ" and r["確認"] == "いいえ")
        return tp, fp, fn, tn

    tp, fp, fn, tn = count(judged)
    report("厳密版", tp, fp, fn, tn, "人手裁定の行だけ。未確認は別枠")
    print(f"  別枠: 未確認 {len(unjudged)} 行(うち裁定待ち {len(left)} 行)")

    report(
        "暫定版",
        tp,
        fp,
        fn,
        tn + len(agree),
        "未確認の一致行を暫定の真陰性として加算",
    )
    print("  暫定の真陰性は自動判定に依る。人手の確認を経ていない")

    # 自動判定の較正。人手裁定の行だけで比べる。
    same = sum(1 for r in judged if r["自動判定"] == r["確認"])
    lo, hi = wilson(same, len(judged))
    print(f"\n== 自動判定と人手裁定の一致 {same}/{len(judged)} = {same / len(judged):.1%}"
          f"(95% 区間 {lo:.1%}〜{hi:.1%})")
    miss_yes = [r for r in judged if r["自動判定"] == "いいえ" and r["確認"] == "はい"]
    miss_no = [r for r in judged if r["自動判定"] == "はい" and r["確認"] == "いいえ"]
    unset = [r for r in judged if r["自動判定"] == "未付与"]
    print(f"  自動が見落とし(自動いいえ・人手はい) {len(miss_yes)} 件")
    print("    語:", "、".join(sorted({r["語"] for r in miss_yes})) or "なし")
    print(f"  自動が拾いすぎ(自動はい・人手いいえ) {len(miss_no)} 件")
    print("    語:", "、".join(sorted({r["語"] for r in miss_no})) or "なし")
    print(f"  自動が一次付けできなかった行 {len(unset)} 件")
    print("    語:", "、".join(sorted({r["語"] for r in unset})) or "なし")


if __name__ == "__main__":
    main()
