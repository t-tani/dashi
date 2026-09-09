# 任意の表・任意の閾値の指摘を、判定基準 v2 で採点する。
# score_v2_final.py を、表の印を引数で受ける形に広げただけである。
#
# 使い方: python score_v3.py <印> <abstract_ratio> [除外する語の一覧ファイル]
# 除外する語の一覧を渡すと、その語への指摘を落としてから数える。
# 後段の除外は、検出器へ免除規則を足した場合のシミュレーションであり、
# 実測ではない。
import os
import json
import math
import sys
from collections import Counter

import sample_keys
import verdicts_v2 as v2

WORK = os.environ["WORK_DIR"]
RULE = "concrete-noun-misfit"
CORPORA = ("sub", "docs A", "docs B")
VALID = [("old", 1), ("old", 38), ("old", 55), ("new", 58), ("both", 15), ("both", 17)]


def rows_of(path, drop):
    with open(path, encoding="utf-8") as source:
        found = [f for f in json.load(source)["findings"] if f["rule"] == RULE]
    keys = [sample_keys.key(f) for f in found]
    return [k for k in keys if k[3] not in drop]


def wilson(k, n):
    p, z = k / n, 1.96
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0, c - h), min(1, c + h)


def judged():
    """判定済み 170 件を (鍵, 見立て) で返す。"""
    import random

    out = []
    for side in ("old", "new", "both"):
        for index, _, key in sample_keys.sample(side):
            out.append((key, v2.verdict(side, index)))
    fresh = json.load(open(WORK + "haiku-strata.json", encoding="utf-8"))["fresh"]
    rows = [r.split("\t") for r in fresh]
    random.seed(694)
    for index, row in enumerate(sorted(random.sample(sorted(rows), 30)), 1):
        key = (row[0], int(row[1]), int(row[2]), row[3])
        out.append((key, v2.verdict("fresh", index)))
    return out


def main(tag, ratio, drop_path=None):
    drop = set()
    if drop_path:
        drop = {
            w
            for w in open(drop_path, encoding="utf-8").read().split("\n")
            if w and not w.startswith("#")
        }
        print(f"後段で除外する語 {len(drop)} 種")

    full = set()
    for corpus in CORPORA:
        full |= set(rows_of(f"{WORK}{corpus}-{tag}-{ratio}.json", drop))
    # 判定済みの標本はファイル名と語だけで突き合わせる(桁がずれても拾う)。
    loose = {(k[0].rsplit("/", 1)[-1], k[3]) for k in full}

    print(f"== {tag} / abstract_ratio {ratio} ==")
    print(f"指摘の総数 {len(full)}")

    good = [next(k for i, _, k in sample_keys.sample(s) if i == n) for s, n in VALID]
    kept = [k for k in good if (k[0].rsplit("/", 1)[-1], k[3]) in loose]
    print(f"(a) 妥当 6 件の保持 {len(kept)}/6: {'・'.join(k[3] for k in kept) or 'なし'}")

    bad = [k for k, v in judged() if v == "ng"]
    gone = [k for k in bad if (k[0].rsplit("/", 1)[-1], k[3]) not in loose]
    print(
        f"(b) 判定済みの誤検出 {len(bad)} 件のうち消えた {len(gone)} 件"
        f" ({len(gone) / len(bad):.1%})"
    )
    left = Counter(k[3] for k in bad if (k[0].rsplit("/", 1)[-1], k[3]) in loose)
    print("    残った語:", "、".join(f"{w} {c}" for w, c in left.most_common(12)) or "なし")

    counts = Counter(k[3] for k in full)
    print(f"(c) 残った指摘 {len(full)} 件 / 語の種類 {len(counts)}")
    print("    多い語:", "、".join(f"{w} {c}" for w, c in counts.most_common(15)))

    out = f"{WORK}findings-{tag}-{ratio}{'-filtered' if drop_path else ''}.json"
    with open(out, "w", encoding="utf-8") as sink:
        json.dump(sorted("\t".join(map(str, k)) for k in full), sink, ensure_ascii=False)
    print("指摘の一覧を書き出した:", out.rsplit("/", 1)[-1])


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
