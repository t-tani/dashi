# 後段で除外する語の一覧を書き出す。
#
# 案 A: 44 分類の表で中項目 48(無形の人工物)を得た語。
# 案 B: 対訳表 gloss-table.tsv で、どれかの語義に分野タグ comp が付く語。
#
# どちらも、検出器へ免除規則を足した場合のシミュレーションに使う。
import os
import json
import sys

WORK = os.environ["WORK_DIR"]
GLOSS = os.environ["AKUNUKI_DIR"] + ".aku/reference/gloss-table.tsv"


def from_48():
    data = json.load(open(WORK + "haiku48-v3-classifications.json", encoding="utf-8"))
    return sorted(w for w, cats in data.items() if 48 in cats)


def from_comp():
    words = set()
    with open(GLOSS, encoding="utf-8") as source:
        for line in source:
            if line.startswith("#"):
                continue
            parts = line.rstrip("\n").split("\t")
            if len(parts) < 4:
                continue
            tags = {t.partition(":")[2] for t in parts[3].split(",") if ":" in t}
            if "comp" in tags:
                words.add(parts[0])
    return sorted(words)


if __name__ == "__main__":
    which = sys.argv[1]
    words = from_48() if which == "48" else from_comp()
    path = f"{WORK}drop-{which}.txt"
    with open(path, "w", encoding="utf-8") as sink:
        for word in words:
            print(word, file=sink)
    print(f"{which}: {len(words)} 語を {path.rsplit('/', 1)[-1]} へ書き出した")
