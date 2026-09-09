# Haiku 表の指摘を、判定済み 140 件と突き合わせて採点する。
import os
import json
import re
from collections import Counter

import sample_keys
from verdicts import verdict

WORK = os.environ["WORK_DIR"]
WORD = re.compile(r"'([^']+)'")
RULE = "concrete-noun-misfit"
CORPORA = ("sub", "docs A", "docs B")


def load(tag, corpus):
    path = f"{WORK}{corpus}-{tag}.json"
    with open(path, encoding="utf-8") as source:
        return {
            sample_keys.key(f)
            for f in json.load(source)["findings"]
            if f["rule"] == RULE
        }


def main():
    old = set().union(*(load("old", c) for c in CORPORA))
    new = set().union(*(load("new", c) for c in CORPORA))
    haiku = set().union(*(load("haiku", c) for c in CORPORA))
    print(f"指摘の総数: 旧 {len(old)} / WordNet {len(new)} / Haiku {len(haiku)}")

    # (a) 妥当と判定された指摘を Haiku が保持したか。
    for side in ("old", "both"):
        rows = [(i, k) for i, _, k in sample_keys.sample(side)]
        kept = [(i, k) for i, k in rows if verdict(side, i) == "ok" and k in haiku]
        good = [(i, k) for i, k in rows if verdict(side, i) == "ok"]
        label = "旧表だけが出す妥当な指摘" if side == "old" else "両方が出す妥当な指摘"
        print(f"(a) {label} {len(good)} 件のうち Haiku が保持 {len(kept)} 件 ({len(kept)/len(good):.1%})")
        missing = [k[3] for i, k in good if k not in haiku]
        print("    落とした語:", "、".join(sorted(Counter(missing))) or "なし")

    # (b) 誤検出と判定された指摘を Haiku が再現したか。
    bad = []
    for side in ("new", "old", "both"):
        for i, _, k in sample_keys.sample(side):
            if verdict(side, i) == "ng":
                bad.append((side, i, k))
    repeated = [k for _, _, k in bad if k in haiku]
    print(f"(b) 誤検出と判定した {len(bad)} 件のうち Haiku が再現 {len(repeated)} 件 ({len(repeated)/len(bad):.1%})")
    print("    再現した語:", "、".join(sorted(Counter(k[3] for k in repeated))) or "なし")

    # (c) Haiku だけが出す指摘。
    fresh = haiku - old - new
    print(f"(c) Haiku 表だけが出す指摘 {len(fresh)} 件 / 異なり語 {len({k[3] for k in fresh})}")
    counts = Counter(k[3] for k in fresh)
    print("    多い語:", "、".join(f"{w} {c}" for w, c in counts.most_common(15)))

    # 層ごとの件数。精度の推計に使う。
    strata = {
        "旧だけ": len(haiku & (old - new)),
        "WordNet だけ": len(haiku & (new - old)),
        "両方": len(haiku & old & new),
        "Haiku だけ": len(fresh),
    }
    print("Haiku の指摘の層:", strata, "合計", sum(strata.values()))
    with open(WORK + "haiku-strata.json", "w", encoding="utf-8") as sink:
        json.dump(
            {"strata": strata, "fresh": sorted("\t".join(map(str, k)) for k in fresh)},
            sink,
            ensure_ascii=False,
        )


if __name__ == "__main__":
    main()
