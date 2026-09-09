"""既定の前後で、意味分類表を引く 2 検出器の指摘を突き合わせる。

鍵はパスと行と桁と語である。指摘の文面には分類番号が埋まって表ごとに変わるので、
鍵に入れると同じ箇所が消えた側と増えた側の両方に並ぶ。
"""
import os
import json, re, sys
from collections import Counter

W = os.environ["WORK_DIR"]
WORD = re.compile(r"'([^']+)'")
TARGETS = ("sub", "docs A", "docs B", "akunuki-docs")


def load(side, rule):
    out = {}
    for t in TARGETS:
        for f in json.load(open(f"{W}{t}-{side}.json", encoding="utf-8"))["findings"]:
            if f["rule"] != rule:
                continue
            m = WORD.search(f["message"])
            out[(f["path"], f["line"], f["col"], m.group(1) if m else "")] = (t, f["message"])
    return out


def main(rule):
    before, after = load("before", rule), load("after", rule)
    gone = sorted(set(before) - set(after))
    new = sorted(set(after) - set(before))
    both = set(before) & set(after)
    print(f"== {rule}")
    print(f"  前 {len(before)} / 後 {len(after)} / 両方 {len(both)} / 消えた {len(gone)} / 増えた {len(new)}")
    for label, keys in (("消えた", gone), ("増えた", new)):
        c = Counter(k[3] for k in keys)
        print(f"  {label} {len(keys)} 件 / 語の種類 {len(c)}")
        print("    上位:", "、".join(f"{w} {n}" for w, n in c.most_common(12)) or "なし")
        per = Counter(before[k][0] if label == "消えた" else after[k][0] for k in keys)
        print("    対象ごと:", dict(per))
    return gone, new, before, after


if __name__ == "__main__":
    main(sys.argv[1])
