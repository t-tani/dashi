"""前後それぞれの指摘から種 694 で 30 件を抜き、原文の断片つきで印字する。

既に裁いた語は、その裁定を添えて表示する。判定基準は dashi の
../finding-verdicts/verdicts_v2.py と同じで、定着した語義と専門用語への
指摘は誤検出、読み手が引っかかる未定着の転用や直訳への指摘を妥当とする。
"""
import os
import json, random, re, sys
sys.path.insert(0, os.environ["CORPUS_DIR"] + "/dashi/semantic-class/eval/finding-verdicts")
from verdicts_v2 import REASON

W = os.environ["WORK_DIR"]
WORD = re.compile(r"'([^']+)'")
TARGETS = ("sub", "docs A", "docs B", "akunuki-docs")


def window(path, line, col, width=95):
    lines = open(path, encoding="utf-8").read().split("\n")
    if not 0 < line <= len(lines):
        return ""
    text = lines[line - 1]
    at = col - 1
    head = "…" if at > width else ""
    tail = "…" if at + width < len(text) else ""
    return head + text[max(0, at - width): at + width].strip() + tail


def main(side):
    rows = []
    for t in TARGETS:
        for f in json.load(open(f"{W}{t}-{side}.json", encoding="utf-8"))["findings"]:
            if f["rule"] != "concrete-noun-misfit":
                continue
            m = WORD.search(f["message"])
            rows.append((f["path"], f["line"], f["col"], m.group(1) if m else ""))
    random.seed(694)
    picked = sorted(random.sample(sorted(rows), 30))
    print(f"== {side} / 母数 {len(rows)} 件から 30 件")
    for i, (p, line, col, w) in enumerate(picked, 1):
        known = REASON.get(w)
        tag = f"  [既裁定: {'定着' if known and '定着' in known else '普通の用法' if known else '新規'}]"
        print(f"--- {i} {w}{tag} @ {p.rsplit('/',1)[-1]}:{line}")
        print("   ", window(p, line, col))


if __name__ == "__main__":
    main(sys.argv[1])
