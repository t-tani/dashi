# 差分の指摘から、種 694 で 30 件を抜いて原文の断片つきで印字する。
# 使い方: python sample_diff_v4.py <検出器> <lost|gained>
import random
import sys

from diff_v4 import CORPORA, load, window


def main(rule, side):
    old_all, new_all = {}, {}
    for corpus in CORPORA:
        old_all.update(load("v4old", corpus, rule))
        new_all.update(load("v4haiku", corpus, rule))
    if side == "lost":
        keys, source = sorted(set(old_all) - set(new_all)), old_all
        label = "消えた指摘(旧表だけが出す)"
    else:
        keys, source = sorted(set(new_all) - set(old_all)), new_all
        label = "増えた指摘(Haiku v2 表だけが出す)"

    random.seed(694)
    picked = sorted(random.sample(keys, min(30, len(keys))))
    print(f"== {rule} / {label} {len(keys)} 件から {len(picked)} 件")
    for index, key in enumerate(picked, 1):
        path, line, col, word = key
        print(f"--- {index} {word} @ {path.rsplit('/', 1)[-1]}:{line}:{col}")
        print(f"    {source[key]}")
        print(f"    {window(path, line, col)}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1], sys.argv[2])
