#!/usr/bin/env python3
"""旧表(分類語彙表由来)と新表(日本語 WordNet 由来)の部門の一致を測る。"""

import os
import sys
from collections import Counter

OLD = os.environ["AKUNUKI_DIR"] + ".aku/reference/semantic-class-table.tsv"
NEW = os.environ["CORPUS_DIR"] + "/tmp-compare/wordnet694/wnja-semantic-class-table.tsv"
CORPUS_LIST = os.environ["CORPUS_DIR"] + "/tmp-compare/wordnet694/corpus-text.txt"

ABSTRACT = {1, 3}
CONCRETE = {4, 5}


def read(path):
    table = {}
    with open(path, encoding="utf-8") as source:
        for line in source:
            if line.startswith("#") or not line.strip():
                continue
            fields = line.rstrip("\n").split("\t")
            table[fields[0]] = {int(number.split(".")[1][0]) for number in fields[1:]}
    return table


def script(word):
    kinds = set()
    for character in word:
        code = ord(character)
        if 0x4E00 <= code <= 0x9FFF:
            kinds.add("漢字")
        elif 0x30A0 <= code <= 0x30FF:
            kinds.add("カタカナ")
        elif 0x3040 <= code <= 0x309F:
            kinds.add("ひらがな")
        else:
            kinds.add("その他")
    if kinds == {"漢字"}:
        return "漢字だけ"
    if kinds == {"カタカナ"} or kinds == {"カタカナ", "その他"}:
        return "カタカナだけ"
    if kinds == {"ひらがな"}:
        return "ひらがなだけ"
    return "混じり"


def corpus_words(words):
    try:
        text = open(CORPUS_LIST, encoding="utf-8").read()
    except OSError:
        return None
    lengths = sorted({len(word) for word in words})
    found = set()
    limit = max(lengths)
    for start in range(len(text)):
        for length in lengths:
            if length > limit:
                break
            piece = text[start : start + length]
            if len(piece) < length:
                break
            if piece in words:
                found.add(piece)
    return found


def report(name, pairs):
    total = len(pairs)
    if total == 0:
        print(f"{name}: 0 語")
        return
    same = sum(1 for old, new in pairs if old == new)
    overlap = sum(1 for old, new in pairs if old & new)
    concrete_agree = sum(
        1 for old, new in pairs if (old <= CONCRETE) == (new <= CONCRETE)
    )
    abstract_agree = sum(
        1 for old, new in pairs if bool(old & ABSTRACT) == bool(new & ABSTRACT)
    )
    subject_agree = sum(
        1
        for old, new in pairs
        if all((d in old) == (d in new) for d in (1, 2, 3, 4, 5))
    )
    print(
        f"{name}: {total} 語 / 集合が一致 {same} ({same / total:.1%})"
        f" / 交わりあり {overlap} ({overlap / total:.1%})"
        f" / 具体物の判定が一致 {concrete_agree} ({concrete_agree / total:.1%})"
        f" / 抽象を含むかが一致 {abstract_agree} ({abstract_agree / total:.1%})"
        f" / 主語の部門の判定が一致 {subject_agree} ({subject_agree / total:.1%})"
    )


def main():
    old = read(OLD)
    new = read(NEW)
    shared = sorted(set(old) & set(new))
    print(f"旧表 {len(old)} 語 / 新表 {len(new)} 語 / 両方に載る語 {len(shared)} 語")
    print(f"旧表だけ {len(set(old) - set(new))} 語 / 新表だけ {len(set(new) - set(old))} 語")

    pairs = [(old[word], new[word]) for word in shared]
    report("全体", pairs)

    print("\n-- 旧表の部門の数で切る --")
    for size in (1, 2, 3):
        selected = [
            (old[word], new[word]) for word in shared if len(old[word]) == size
        ]
        report(f"旧表の部門が {size} つ", selected)
    selected = [(old[word], new[word]) for word in shared if len(old[word]) >= 4]
    report("旧表の部門が 4 つ以上", selected)

    print("\n-- 表記で切る --")
    for kind in ("漢字だけ", "カタカナだけ", "ひらがなだけ", "混じり"):
        selected = [
            (old[word], new[word]) for word in shared if script(word) == kind
        ]
        report(kind, selected)

    print("\n-- 単一部門どうしの取り違え(行が旧、列が新)--")
    matrix = Counter()
    for word in shared:
        if len(old[word]) == 1 and len(new[word]) == 1:
            matrix[(next(iter(old[word])), next(iter(new[word])))] += 1
    names = {1: "関係", 2: "主体", 3: "活動", 4: "生産物", 5: "自然物"}
    print("旧\\新\t" + "\t".join(names[d] for d in range(1, 6)) + "\t計")
    for row in range(1, 6):
        counts = [matrix[(row, column)] for column in range(1, 6)]
        print(f"{names[row]}\t" + "\t".join(str(c) for c in counts) + f"\t{sum(counts)}")

    found = corpus_words(set(shared))
    if found is not None:
        print(f"\n-- 評価コーパスに現れる語で切る(現れた {len(found)} 語)--")
        report("コーパスに現れる", [(old[w], new[w]) for w in shared if w in found])
        report("現れない", [(old[w], new[w]) for w in shared if w not in found])


if __name__ == "__main__":
    main()
