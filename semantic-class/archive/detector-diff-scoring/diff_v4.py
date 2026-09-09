# 意味分類表を引く 2 検出器の指摘を、旧表と Haiku v2 表で突き合わせる。
# 設定は 2 つとも同一で、違うのは意味分類表だけである。
#
# 突き合わせの鍵は(パス、行、桁、語)である。メッセージには分類番号が
# 埋まっていて表ごとに変わるので、鍵に入れると同じ箇所が消えた側と増えた側の
# 両方に並んでしまう。
import os
import json
import re
import sys
from collections import Counter

WORK = os.environ["WORK_DIR"]
ROOT = os.environ["AKUNUKI_DIR"]
CORPORA = ("sub", "docs A", "docs B")
RULES = ("role-conflict", "standalone-calque")
WORD = re.compile(r"'([^']+)'")


def load(tag, corpus, rule):
    with open(f"{WORK}{corpus}-{tag}.json", encoding="utf-8") as source:
        rows = json.load(source)["findings"]
    out = {}
    for f in rows:
        if f["rule"] != rule:
            continue
        found = WORD.search(f["message"])
        key = (f["path"], f["line"], f["col"], found.group(1) if found else "")
        out[key] = f["message"]
    return out


def window(path, line, col, width=90):
    full = path if path.startswith("/") else ROOT + path
    lines = open(full, encoding="utf-8").read().split("\n")
    if not 0 < line <= len(lines):
        return ""
    text = lines[line - 1]
    at = col - 1
    head = "…" if at > width else ""
    tail = "…" if at + width < len(text) else ""
    return head + text[max(0, at - width) : at + width].strip() + tail


def report(rule):
    print(f"\n{'=' * 70}\n== {rule}\n{'=' * 70}")
    old_all, new_all = {}, {}
    print("コーパスごとの件数(旧表 / Haiku v2 表)")
    for corpus in CORPORA:
        old, new = load("v4old", corpus, rule), load("v4haiku", corpus, rule)
        print(f"  {corpus}: {len(old)} / {len(new)}")
        old_all.update(old)
        new_all.update(new)
    shared = set(old_all) & set(new_all)
    print(f"  合計: {len(old_all)} / {len(new_all)} / 両方が出す {len(shared)} 件")

    for label, keys, source in (
        ("消えた指摘(旧表だけが出す)", sorted(set(old_all) - set(new_all)), old_all),
        ("増えた指摘(Haiku v2 表だけが出す)", sorted(set(new_all) - set(old_all)), new_all),
    ):
        print(f"\n--- {label} {len(keys)} 件")
        counts = Counter(k[3] for k in keys)
        print("    語の内訳:", "、".join(f"{w} {c}" for w, c in counts.most_common()) or "なし")
        for index, key in enumerate(keys, 1):
            path, line, col, _ = key
            print(f"  [{index}] {path.rsplit('/', 1)[-1]}:{line}:{col} {source[key]}")
            print(f"      {window(path, line, col)}")

    # 両方が出す指摘のうち、引けた分類が変わった件を数える。
    changed = sum(1 for k in shared if old_all[k] != new_all[k])
    print(f"\n--- 両方が出す {len(shared)} 件のうち、message の分類番号が変わった {changed} 件")


def main():
    for rule in RULES:
        report(rule)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
