# Haiku 表だけが出す指摘から 30 件を無作為に抜き、前後の文脈を印字する。
import os
import json
import random

ROOT = os.environ["AKUNUKI_DIR"]
WORK = os.environ["WORK_DIR"]


def window(path, line, col, word):
    full = path if path.startswith("/") else ROOT + path
    lines = open(full, encoding="utf-8").read().split("\n")
    if not 0 < line <= len(lines):
        return ""
    text = lines[line - 1]
    at = col - 1
    head = "…" if at > 90 else ""
    tail = "…" if at + len(word) + 90 < len(text) else ""
    return head + text[max(0, at - 90) : at + len(word) + 90].strip() + tail


def main():
    data = json.load(open(WORK + "haiku-strata.json", encoding="utf-8"))
    rows = [r.split("\t") for r in data["fresh"]]
    rows = [(p, int(line), int(col), w) for p, line, col, w in rows]
    random.seed(694)
    for index, (path, line, col, word) in enumerate(
        sorted(random.sample(sorted(rows), min(30, len(rows)))), 1
    ):
        print(f"--- {index} {word} @ {path.rsplit('/', 1)[-1]}:{line}")
        print(window(path, line, col, word))


if __name__ == "__main__":
    main()
