# 判定基準 v2 での、表ごとの精度の推計。
import json
import math

import sample_keys
import verdicts_v2 as v2
from score_haiku import load

CORPORA = ("sub", "docs A", "docs B")


def wilson(k, n):
    p, z = k / n, 1.96
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0, c - h), min(1, c + h)


old = set().union(*(load("old", c) for c in CORPORA))
new = set().union(*(load("new", c) for c in CORPORA))
haiku = set().union(*(load("haiku", c) for c in CORPORA))

pools = {
    "old": len(old - new),
    "new": len(new - old),
    "both": len(old & new),
}
rates = {"old": (3, 60), "new": (1, 60), "both": (2, 20), "fresh": (0, 30)}

print("== 層ごとの適合率(基準 v2)==")
for name, (k, n) in rates.items():
    lo, hi = wilson(k, n)
    print(f"{name}: {k}/{n} = {k/n:.1%} (95% 区間 {lo:.1%}〜{hi:.1%})")

print("\n== 表ごとの精度 ==")
for label, parts in (
    ("旧表", [("old", pools["old"]), ("both", pools["both"])]),
    ("WordNet 表", [("new", pools["new"]), ("both", pools["both"])]),
):
    total = sum(c for _, c in parts)
    ok = sum(c * rates[s][0] / rates[s][1] for s, c in parts)
    print(f"{label}: 指摘 {total} 件 / 妥当の概算 {ok:.0f} 件 ({ok/total:.1%})")

strata = json.load(open("haiku-strata.json", encoding="utf-8"))["strata"]
key_of = {"旧だけ": "old", "WordNet だけ": "new", "両方": "both", "Haiku だけ": "fresh"}
total = sum(strata.values())
ok = sum(c * rates[key_of[k]][0] / rates[key_of[k]][1] for k, c in strata.items())
print(f"Haiku 表: 指摘 {total} 件 / 妥当の概算 {ok:.0f} 件 ({ok/total:.1%})")

print("\n== 妥当に残った 6 件を、どの表が出すか ==")
picks = [("old", 1), ("old", 38), ("old", 55), ("new", 58), ("both", 15), ("both", 17)]
for side, index in picks:
    key = next(k for i, _, k in sample_keys.sample(side) if i == index)
    tables = [n for n, s in (("旧", old), ("WordNet", new), ("Haiku", haiku)) if key in s]
    print(f"{key[3]}\t{key[0].rsplit('/', 1)[-1]}:{key[1]}\t出す表: {'・'.join(tables)}")
