# 標本の適合率から、指摘の総数に対する精度を推計する。
# 使い方: python estimate_v3.py <名前> <妥当の件数> <標本の件数> <指摘の総数>
import math
import sys


def wilson(k, n):
    p, z = k / n, 1.96
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0, c - h), min(1, c + h)


def main(label, ok, sample, total):
    rate = ok / sample
    lo, hi = wilson(ok, sample)
    print(
        f"{label}: 標本 {ok}/{sample} = {rate:.1%} (95% 区間 {lo:.1%}〜{hi:.1%}) / "
        f"指摘 {total} 件 / 妥当の概算 {rate * total:.0f} 件 "
        f"(区間 {lo * total:.0f}〜{hi * total:.0f} 件)"
    )


if __name__ == "__main__":
    main(sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]))
