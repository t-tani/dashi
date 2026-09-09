"""評価コーパスから 30 文書を抜き、判定単位の候補を全列挙する。

候補は、意味分類が引ける単独で立つ名詞の出現である。単独で立つとは、より長い語の
一部ではないことを指す。

検出器は形態素解析で語を切るが、ここでは意味分類表の見出し語による最長一致で切る。
同じ切り方にはならないので、食い違いは人手の確認で拾う。近似であることを承知の上で
使うのは、外から候補を数え上げる手段がこれしかないためである。

意味分類表の見出しには 1 字の漢字やラテン文字の断片も入っている。日本語の文字だけで
できた 2 字以上の見出しに限るのは、1 字の語を拾うと候補が名詞の出現ではなく文字の
出現に近づくためである。
"""

import argparse
import json
import random
import re

JAPANESE = re.compile(r"\A[ぁ-ゟ゠-ヿ一-鿿々ー]+\Z")
KANJI = re.compile(r"[一-鿿々]")
KATA = re.compile(r"[ァ-ヶー]")
HIRA = re.compile(r"[ぁ-ゟ]")
# 見出し語を拾わない行。Markdown の記法と、コードとリンクの中身である。
SKIP = re.compile(r"^\s*(```|\||#{1,6}\s|>\s)")
CODE = re.compile(r"`[^`]*`|\[[^\]]*\]\([^)]*\)|https?://\S+")


def load_table(path, min_len=2):
    words = set()
    with open(path, encoding="utf-8") as source:
        for line in source:
            if line.startswith("#") or not line.strip():
                continue
            word = line.split("\t", 1)[0]
            if len(word) >= min_len and JAPANESE.fullmatch(word):
                words.add(word)
    return words


def same_class(a, b):
    """2 文字が同じ字種なら真。境界の判定に使う。"""
    for pat in (KANJI, KATA, HIRA):
        if pat.fullmatch(a) and pat.fullmatch(b):
            return True
    return False


def candidates(text, words, longest):
    """1 文書から候補の出現を返す。最長一致で切り、境界を確かめる。"""
    found = []
    for number, line in enumerate(text.split("\n"), 1):
        if SKIP.match(line):
            continue
        masked = CODE.sub(lambda m: " " * len(m.group(0)), line)
        at = 0
        while at < len(masked):
            hit = None
            for size in range(min(longest, len(masked) - at), 1, -1):
                piece = masked[at : at + size]
                if piece in words:
                    hit = piece
                    break
            if hit is None:
                at += 1
                continue
            before = masked[at - 1] if at else ""
            after = masked[at + len(hit) : at + len(hit) + 1]
            alone = not (
                (before and same_class(before, hit[0]))
                or (after and same_class(hit[-1], after))
            )
            if alone:
                found.append((number, at + 1, hit, line.strip()))
            at += len(hit)
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", required=True, help="評価コーパスのディレクトリ")
    parser.add_argument("--table", required=True)
    parser.add_argument("--sample", type=int, default=30)
    parser.add_argument("--seed", type=int, default=694)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    import pathlib

    docs = sorted(str(p) for p in pathlib.Path(args.corpus).rglob("*.md"))
    random.seed(args.seed)
    picked = sorted(random.sample(docs, args.sample))
    words = load_table(args.table)
    longest = max(len(w) for w in words)
    print(f"文書 {len(docs)} 件から {len(picked)} 件を抜いた")
    print(f"意味分類表の見出しのうち、日本語 2 字以上 {len(words)} 語")

    rows = []
    for path in picked:
        text = open(path, encoding="utf-8").read()
        for line, col, word, sentence in candidates(text, words, longest):
            rows.append(
                {
                    "path": path,
                    "line": line,
                    "col": col,
                    "word": word,
                    "sentence": sentence,
                }
            )
    with open(args.out, "w", encoding="utf-8") as sink:
        json.dump({"documents": picked, "candidates": rows}, sink, ensure_ascii=False)
    print(f"候補 {len(rows)} 件 / 異なり語 {len({r['word'] for r in rows})}")
    print(f"書き出した: {args.out}")


if __name__ == "__main__":
    main()
