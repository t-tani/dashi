"""人手確認用の一覧を作る。

候補の出現ごとに、自動の一次付けと検出器の実際の出力を並べる。確認は、2 つが
食い違う行だけを見ればよい。一致する行は、どちらの見方でも同じ結論なので後回しに
できる。

該当文の列には、対象の出現を含む文をそのまま入れる。切り詰めない。確認者が原文を
開かずに判断できるようにするためである。対象の出現は【】で囲む。同じ語が文中に
何度も出ても、囲むのは対象の 1 つだけである。

確認欄は空で置く。人が「はい」か「いいえ」を書き入れると、その行が正解になる。
自動の一次付けは、確認を経るまで正解ではない。
"""

import argparse
import csv
import json
import re

WORD = re.compile(r"'([^']+)'")
SPACE = re.compile(r"[\s　]+")
# 表の行・見出し・引用は句点を持たないことがあるので、行の全体を文として扱う。
WHOLE_LINE = ("|", "#", ">")


def read_lines(path, cache):
    if path not in cache:
        try:
            cache[path] = open(path, encoding="utf-8").read().split("\n")
        except OSError:
            cache[path] = []
    return cache[path]


def surfaces(word):
    """本文に現れうる表記を、長いものから順に返す。

    検出器は長音符の揺れを吸収して見出しの形で語を報告する。本文が「プロセッサ」
    でも報告は「プロセッサー」になる。そのままでは本文の中に見つからない。
    """
    forms = {word, word.rstrip("ー"), word.replace("ー", ""), word + "ー"}
    return sorted((f for f in forms if f), key=len, reverse=True)


def sentence_at(path, line_no, col, word, cache):
    """対象の出現を含む文を返し、その出現を【】で囲む。

    文の区切りは句点か行末である。表のセルと見出しの行は行の全体を返す。
    """
    lines = read_lines(path, cache)
    if not 0 < line_no <= len(lines):
        return ""
    raw = lines[line_no - 1]
    at, word = -1, word
    for form in surfaces(word):
        start = col - 1
        if 0 <= start < len(raw) and raw.startswith(form, start):
            at, word = start, form
            break
    if at < 0:
        for form in surfaces(word):
            found = raw.find(form)
            if found >= 0:
                at, word = found, form
                break
    if at < 0:
        return SPACE.sub(" ", raw).strip()

    if raw.lstrip().startswith(WHOLE_LINE):
        start, end = 0, len(raw)
    else:
        start = raw.rfind("。", 0, at) + 1
        stop = raw.find("。", at + len(word))
        end = stop + 1 if stop >= 0 else len(raw)
    piece = raw[start:end]
    rel = at - start
    marked = piece[:rel] + "【" + word + "】" + piece[rel + len(word) :]
    return SPACE.sub(" ", marked).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidates", required=True)
    parser.add_argument("--labels", required=True)
    parser.add_argument("--findings", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    data = json.load(open(args.candidates, encoding="utf-8"))
    labels = {r["word"]: r for r in json.load(open(args.labels, encoding="utf-8"))}
    findings = [
        f
        for f in json.load(open(args.findings, encoding="utf-8"))["findings"]
        if f["rule"] == "concrete-noun-misfit"
    ]
    flagged = {}
    for f in findings:
        m = WORD.search(f["message"])
        flagged[(f["path"], f["line"], m.group(1) if m else "")] = f

    cache = {}
    rows = []
    for c in data["candidates"]:
        label = labels.get(c["word"])
        key = (c["path"], c["line"], c["word"])
        auto = "はい" if label and label["flag"] else "いいえ" if label else "未付与"
        detector = "はい" if key in flagged else "いいえ"
        rows.append(
            {
                "文書": c["path"].rsplit("/", 1)[-1],
                "行": c["line"],
                "語": c["word"],
                "該当文": sentence_at(c["path"], c["line"], c["col"], c["word"], cache),
                "自動判定": auto,
                "理由": label["reason"] if label else "",
                "検出器": detector,
                "食い違い": "はい" if auto != detector else "いいえ",
                "確認": "",
            }
        )

    # 検出器が指摘したのに候補の列挙が拾えなかった出現を足す。列挙は意味分類表の
    # 見出しによる最長一致で、検出器の形態素解析とは切り方が違う。1 字の語と、表や
    # 見出しの行に出た語がこぼれる。この 2 つを落とすと混同行列の陽性側が欠ける。
    seen = {(c["path"], c["line"], c["word"]) for c in data["candidates"]}
    for key, f in flagged.items():
        if key in seen:
            continue
        word = key[2]
        label = labels.get(word)
        rows.append(
            {
                "文書": f["path"].rsplit("/", 1)[-1],
                "行": f["line"],
                "語": word,
                "該当文": sentence_at(f["path"], f["line"], f["col"], word, cache),
                "自動判定": "はい" if label and label["flag"] else "いいえ" if label else "未付与",
                "理由": label["reason"] if label else "候補の列挙の外",
                "検出器": "はい",
                "食い違い": "はい",
                "確認": "",
            }
        )

    # 食い違う行を先頭に置く。確認はここだけ見ればよい。
    rows.sort(key=lambda r: (r["食い違い"] != "はい", r["文書"], r["行"], r["語"]))
    fields = ["文書", "行", "語", "該当文", "自動判定", "理由", "検出器", "食い違い", "確認"]
    with open(args.out, "w", encoding="utf-8", newline="") as sink:
        writer = csv.DictWriter(sink, fieldnames=fields, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)

    split = [r for r in rows if r["食い違い"] == "はい"]
    print(f"候補 {len(rows)} 件 / 食い違い {len(split)} 件")
    for label, auto, det in (
        ("自動も検出器も指摘すべき", "はい", "はい"),
        ("自動だけが指摘すべき", "はい", "いいえ"),
        ("検出器だけが指摘した", "いいえ", "はい"),
        ("どちらも指摘すべきでない", "いいえ", "いいえ"),
    ):
        n = sum(1 for r in rows if r["自動判定"] == auto and r["検出器"] == det)
        print(f"  {label:<24} {n}")
    print(f"書き出した: {args.out}")


if __name__ == "__main__":
    main()
