#!/usr/bin/env python3
"""lexname ごとに、旧表の部門の分布を数える。

写像の当て方を測るための診断であり、この数から写像を作り直すことはしない。
"""

import os
import collections

OLD = os.environ["AKUNUKI_DIR"] + ".aku/reference/semantic-class-table.tsv"

lexnames = {}
for line in open("WordNet-3.0/dict/lexnames"):
    number, name, _ = line.split()
    lexnames[number] = name

offset_lexname = {}
for line in open("dict/data.noun", encoding="latin-1"):
    if line.startswith("  "):
        continue
    fields = line.split(" ", 2)
    offset_lexname[fields[0]] = fields[1]

word_lexnames = collections.defaultdict(set)
for line in open("wnjpn-ok.tab", encoding="utf-8"):
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 2 or not fields[0].endswith("-n"):
        continue
    lexname = offset_lexname.get(fields[0][:-2])
    if lexname:
        word_lexnames[fields[1].strip()].add(lexname)

old = {}
for line in open(OLD, encoding="utf-8"):
    if line.startswith("#") or not line.strip():
        continue
    fields = line.rstrip("\n").split("\t")
    old[fields[0]] = {int(number.split(".")[1][0]) for number in fields[1:]}

names = {1: "関係", 2: "主体", 3: "活動", 4: "生産物", 5: "自然物"}
rows = []
for lexname in sorted(lexnames):
    if not lexnames[lexname].startswith("noun."):
        continue
    counts = collections.Counter()
    for word, found in word_lexnames.items():
        if found != {lexname}:
            continue
        divisions = old.get(word)
        if divisions is None or len(divisions) != 1:
            continue
        counts[next(iter(divisions))] += 1
    total = sum(counts.values())
    if total == 0:
        continue
    top, hits = counts.most_common(1)[0]
    rows.append(
        (
            lexnames[lexname],
            total,
            names[top],
            hits / total,
            "、".join(f"{names[d]} {c}" for d, c in counts.most_common()),
        )
    )

print("lexname\t語数\t最頻の部門\t占有率\t内訳")
for name, total, top, share, breakdown in rows:
    print(f"{name}\t{total}\t{top}\t{share:.0%}\t{breakdown}")
