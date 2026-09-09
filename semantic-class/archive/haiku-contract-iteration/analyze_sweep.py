# abstract_ratio の掃引を読み、指摘の総数と妥当 6 件の保持数を出す。
import json
import re

import sample_keys

WORD = re.compile(r"'([^']+)'")
RULE = "concrete-noun-misfit"
CORPORA = ("sub", "docs A", "docs B")
RATIOS = ("0.50", "0.55", "0.60", "0.65", "0.70", "0.75", "0.80", "0.85", "0.90")

VALID = [("old", 1), ("old", 38), ("old", 55), ("new", 58), ("both", 15), ("both", 17)]


def keys(path):
    """指摘を(ファイル、語)で数える。この検出器は 1 文書 1 語につき初出だけを
    報告するので、段落の判定が変わると行と桁がずれる。ファイルと語で照合する。"""
    with open(path, encoding="utf-8") as source:
        return {
            (f["path"].rsplit("/", 1)[-1], sample_keys.key(f)[3])
            for f in json.load(source)["findings"]
            if f["rule"] == RULE
        }


def valid_keys():
    out = []
    for side, index in VALID:
        out.append(next(k for i, _, k in sample_keys.sample(side) if i == index))
    return out


def main():
    good = valid_keys()
    print("妥当 6 件:", "、".join(k[3] for k in good))
    print("\nratio\t指摘の総数\t妥当 6 件の保持\t保持した語")
    for ratio in RATIOS:
        found = set()
        for corpus in CORPORA:
            found |= keys(f"{corpus}-haiku2-{ratio}.json")
        kept = [k for k in good if (k[0].rsplit("/", 1)[-1], k[3]) in found]
        print(f"{ratio}\t{len(found)}\t{len(kept)}/6\t{'・'.join(k[3] for k in kept)}")


if __name__ == "__main__":
    main()
