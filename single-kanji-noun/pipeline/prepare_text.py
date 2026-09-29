"""平文を部分木の書き出しの前に整えて写す。表の行をセルごとの行に分け、リンクを文字だけにする。

表の 1 行を 1 文として解析すると、セルをまたいだ係り受けができる。リンクの URL は本文ではない。
`.txt` 以外のファイル(corpus-tool が読む manifest.tsv と files.tsv)はそのまま写す。

使い方: uv run python pipeline/prepare_text.py <元のディレクトリ> <写し先>
"""
import os
import re
import shutil
import sys

LINK = re.compile(r"!?\[([^\]]*)\]\([^)]*\)")
SEPARATOR = re.compile(r"^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$")


def clean(text):
    out = []
    for line in text.split("\n"):
        line = LINK.sub(r"\1", line)
        if line.lstrip().startswith("|"):
            if SEPARATOR.match(line):
                continue
            out.extend(cell.strip() for cell in line.strip().strip("|").split("|") if cell.strip())
        else:
            out.append(line)
    return "\n".join(out)


def main():
    src, dst = sys.argv[1], sys.argv[2]
    for root, _, files in os.walk(src):
        rel = os.path.relpath(root, src)
        os.makedirs(os.path.join(dst, rel), exist_ok=True)
        for f in files:
            p, q = os.path.join(root, f), os.path.join(dst, rel, f)
            if f.endswith(".txt"):
                with open(p, encoding="utf-8") as i, open(q, "w", encoding="utf-8") as o:
                    o.write(clean(i.read()))
            else:
                shutil.copy(p, q)


if __name__ == "__main__":
    main()
