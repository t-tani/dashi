"""評価コーパスの本文から、語の用例を切り出す。

分類にかける語へ用例を添えるのは、語だけを見せると語義を取り違えるためである。
どのパスでも同じ用例を添えないと、バッチの組み方だけを変えた比較にならない。

切り出しは、語が単独で立つ最初の箇所を探して前後 40 字を返す。単独で立つとは、
語の直前と直後が同じ字種でないことを指す。同じ字種が続く箇所は、より長い語の
一部を拾っている見込みが高い。
"""

import re

KANJI = re.compile(r"[一-鿿]")
KATA = re.compile(r"[ァ-ヶー]")
LATIN = re.compile(r"[0-9A-Za-z_.\-]")


def context_of(word, text):
    """語が単独で立つ最初の箇所の前後を返す。見つからなければ None を返す。"""
    if re.fullmatch(r"[ァ-ヶー]+", word):
        same = KATA
    elif re.search(r"[0-9A-Za-z]", word):
        same = LATIN
    else:
        same = KANJI
    at = 0
    for _ in range(20):
        at = text.find(word, at)
        if at < 0:
            return None
        before = text[at - 1] if at else ""
        after = text[at + len(word) : at + len(word) + 1]
        if not (same.fullmatch(before) or same.fullmatch(after)):
            line_start = text.rfind("\n", 0, at) + 1
            line_end = text.find("\n", at)
            line = text[line_start : line_end if line_end > 0 else len(text)]
            pos = at - line_start
            return line[max(0, pos - 40) : pos + len(word) + 40].strip().replace("==", "")
        at += len(word)
    return None
