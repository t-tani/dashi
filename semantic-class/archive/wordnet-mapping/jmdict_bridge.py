#!/usr/bin/env python3
"""JMdict の英訳を経由して、日本語 WordNet に無い語の部門を補えるかを測る。"""

import os
import compare
import build_table

GLOSS = os.environ["AKUNUKI_DIR"] + ".aku/reference/gloss-table.tsv"
INDEX = os.environ["CORPUS_DIR"] + "/tmp-compare/wordnet694/dict/index.noun"
DATA = os.environ["CORPUS_DIR"] + "/tmp-compare/wordnet694/dict/data.noun"


def read_lemma_offsets():
    lemmas = {}
    with open(INDEX, encoding="latin-1") as source:
        for line in source:
            if line.startswith("  "):
                continue
            fields = line.split()
            lemma = fields[0]
            count = int(fields[2])
            pointers = int(fields[3])
            offsets = fields[4 + pointers + 2 :]
            lemmas[lemma] = offsets[:count]
    return lemmas


def read_first_glosses():
    glosses = {}
    with open(GLOSS, encoding="utf-8") as source:
        for line in source:
            if line.startswith("#") or not line.strip():
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 7:
                continue
            try:
                first = int(fields[4])
            except ValueError:
                continue
            glosses[fields[0]] = fields[6 : 6 + first]
    return glosses


def main():
    offset_divisions = build_table.read_offset_divisions(DATA, build_table.LEX_DIVISION)
    lemma_offsets = read_lemma_offsets()
    glosses = read_first_glosses()
    old = compare.read(compare.OLD)
    new = compare.read(compare.NEW)

    filled = {}
    for word, first in glosses.items():
        divisions = set()
        for gloss in first:
            for offset in lemma_offsets.get(gloss.replace(" ", "_"), []):
                divisions |= offset_divisions.get(offset, set())
        if divisions:
            filled[word] = divisions

    gap = set(old) - set(new)
    covered = gap & set(filled)
    print(f"対訳表の見出し語 {len(glosses)} 語のうち、英訳が英語 WordNet の名詞に当たる語 {len(filled)}")
    print(f"日本語 WordNet に無く旧表にある語 {len(gap)} のうち、この経路で埋まる語 {len(covered)}")
    pairs = [(old[word], filled[word]) for word in sorted(covered)]
    compare.report("埋まった語での一致", pairs)

    outside = set(filled) - set(new) - set(old)
    print(f"どちらの表にも無く、この経路だけが持つ語 {len(outside)}")


if __name__ == "__main__":
    main()
