#!/usr/bin/env python3
"""新旧の表で入れ替わった concrete-noun-misfit の指摘を無作為に抽出する。

抽出の単位は 1 件の指摘(ファイル・行・桁・語)である。乱数の種は 694 に
固定する。
"""

import os
import json
import random
import re
import sys

RULE = "concrete-noun-misfit"
WORD = re.compile(r"'([^']+)'")
ROOT = os.environ["AKUNUKI_DIR"]
SOURCES = (
    ("sub", "sub-old.json", "sub-new.json", 48),
    ("docs A", "docs A-old.json", "docs A-new.json", 6),
    ("docs B", "docs B-old.json", "docs B-new.json", 6),
)


def load(path):
    with open(path, encoding="utf-8") as source:
        return [f for f in json.load(source)["findings"] if f["rule"] == RULE]


def key(finding):
    found = WORD.search(finding["message"])
    return (
        finding["path"],
        finding["line"],
        finding["col"],
        found.group(1) if found else "",
    )


def window(path, line, col, word):
    """語を中心に前後 90 字を切り出す。col は 1 から数えた文字位置である。"""
    full = path if path.startswith("/") else ROOT + path
    with open(full, encoding="utf-8") as source:
        lines = source.read().split("\n")
    if not 0 < line <= len(lines):
        return ""
    text = lines[line - 1]
    at = col - 1
    head = "…" if at > 90 else ""
    tail = "…" if at + len(word) + 90 < len(text) else ""
    return head + text[max(0, at - 90) : at + len(word) + 90].strip() + tail


def main():
    side = sys.argv[1]
    random.seed(694)
    picked = []
    for label, old_path, new_path, count in SOURCES:
        old = {key(f) for f in load(old_path)}
        new = {key(f) for f in load(new_path)}
        pool = sorted(new - old) if side == "new" else sorted(old - new)
        picked += [(label, item) for item in random.sample(pool, min(count, len(pool)))]
    for index, (label, (path, line, col, word)) in enumerate(picked, 1):
        name = path.rsplit("/", 1)[-1]
        print(f"--- {index} [{label}] {word} @ {name}:{line}")
        print(window(path, line, col, word))


if __name__ == "__main__":
    main()
