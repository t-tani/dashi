#!/usr/bin/env python3
"""2 つの lint 結果(JSON)の指摘を検出器ごとに突き合わせる。

指摘の文面には分類番号が入るので、位置と語だけを鍵にして数える。
"""

import json
import re
import sys
from collections import Counter

WORD = re.compile(r"'([^']+)'")


def load(path):
    with open(path, encoding="utf-8") as source:
        return json.load(source)["findings"]


def word_of(finding):
    found = WORD.search(finding["message"])
    return found.group(1) if found else ""


def key(finding):
    return (
        finding["path"],
        finding["rule"],
        finding["line"],
        finding["col"],
        word_of(finding),
    )


def main(old_path, new_path, label):
    old = load(old_path)
    new = load(new_path)
    print(f"=== {label} ===")
    print(f"指摘の総数 旧 {len(old)} / 新 {len(new)}")
    old_counts = Counter(f["rule"] for f in old)
    new_counts = Counter(f["rule"] for f in new)
    changed = sorted(
        rule
        for rule in set(old_counts) | set(new_counts)
        if old_counts[rule] != new_counts[rule]
    )
    for rule in ("concrete-noun-misfit", "role-conflict", "standalone-calque"):
        print(f"{rule}: 旧 {old_counts[rule]} / 新 {new_counts[rule]}")
    print("件数が動いた検出器:", changed)

    for rule in changed:
        old_keys = {key(f) for f in old if f["rule"] == rule}
        new_keys = {key(f) for f in new if f["rule"] == rule}
        added = new_keys - old_keys
        removed = old_keys - new_keys
        both = old_keys & new_keys
        print(
            f"\n-- {rule}: 両方に出る {len(both)} 件"
            f" / 新だけ {len(added)} 件 / 旧だけ {len(removed)} 件 --"
        )
        for name, group in (("新だけ", added), ("旧だけ", removed)):
            words = Counter(item[4] for item in group)
            print(f"  {name}の語(上位 20 と異なり語数 {len(words)}):")
            print(
                "    "
                + "、".join(f"{word} {count}" for word, count in words.most_common(20))
            )


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
