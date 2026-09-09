# Haiku 表の精度を層ごとの適合率から推計し、旧表と比べる。
import os
import json
import math

WORK = os.environ["WORK_DIR"]


def wilson(k, n):
    p, z = k / n, 1.96
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0, c - h), min(1, c + h)


strata = json.load(open(WORK + "haiku-strata.json", encoding="utf-8"))["strata"]
# 各層の適合率は、その層の母集団から抜いた標本の見立てである。
rates = {
    "旧だけ": (52, 60),
    "WordNet だけ": (14, 60),
    "両方": (12, 20),
    "Haiku だけ": (13, 30),
}
print("層\t件数\t標本の適合率\t95% 区間\t妥当の概算")
total = ok = 0
for name, count in strata.items():
    k, n = rates[name]
    lo, hi = wilson(k, n)
    print(f"{name}\t{count}\t{k}/{n} = {k/n:.1%}\t{lo:.1%}〜{hi:.1%}\t{count*k/n:.0f}")
    total += count
    ok += count * k / n
print(f"\nHaiku 表の指摘 {total} 件のうち妥当は概算 {ok:.0f} 件 ({ok/total:.1%})")
print("旧表の指摘 1314 件のうち妥当は概算 1038 件 (79.0%)")
print(f"妥当な指摘の絶対数: Haiku {ok:.0f} 件 / 旧表 1038 件")
