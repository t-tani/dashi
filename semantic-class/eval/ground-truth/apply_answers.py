"""人手裁定を待ち行列へ書き入れる。

照合の鍵は語と該当文の断片である。断片は手作業で整形されたものなので、記法の印と
空白を落として突き合わせる。1 行に定まらない照合は、推測せず未解決として報告する。
"""

import argparse
import csv
import re
import sys

NOISE = re.compile(r"[【】\*`_~\-—–#|>\s　。、,\.\(\)（）\[\]]+")


def flatten(text):
    return NOISE.sub("", text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--queue", required=True)
    parser.add_argument("--answers", required=True)
    parser.add_argument("--date", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    rows = list(csv.DictReader(open(args.queue, encoding="utf-8"), delimiter="\t"))
    answers = []
    with open(args.answers, encoding="utf-8") as source:
        for number, line in enumerate(source, 1):
            parts = line.rstrip("\n").split("\t")
            if number == 1 or len(parts) < 3:
                continue
            answers.append((number, parts[0].strip(), parts[1].strip(), parts[2].strip()))

    split = [r for r in rows if r["食い違い"] == "はい"]
    print(f"待ち行列の食い違い {len(split)} 行 / 回答 {len(answers)} 行")

    for r in rows:
        r.setdefault("裁定者", "")
        r.setdefault("裁定日", "")

    used, unresolved = set(), []
    for number, word, fragment, mark in answers:
        key = flatten(fragment)
        hits = [
            r
            for r in split
            if r["語"] == word and id(r) not in used and key and key in flatten(r["該当文"])
        ]
        if len(hits) != 1:
            unresolved.append((number, word, fragment, mark, len(hits)))
            continue
        row = hits[0]
        used.add(id(row))
        row["確認"] = "はい" if mark.upper() == "Y" else "いいえ"
        row["裁定者"] = "人手"
        row["裁定日"] = args.date

    # 第 2 段。断片で照合できなかった回答を、語で突き合わせる。断片は手作業で
    # 短くされており、原文の literal な接頭辞になっていない場合がある。語が残りの
    # 行の中で一意に定まるか、同じ語の残りの回答がすべて同じ答えなら、対応づけて
    # よい。後者は、どちらの行へ割り当てても結果が変わらないためである。
    second = []
    for item in list(unresolved):
        number, word, fragment, mark, _ = item
        rest = [r for r in split if r["語"] == word and id(r) not in used]
        same = [a for a in unresolved if a[1] == word]
        if len(rest) == 1 or (rest and len({a[3].upper() for a in same}) == 1):
            row = rest[0]
            used.add(id(row))
            row["確認"] = "はい" if mark.upper() == "Y" else "いいえ"
            row["裁定者"] = "人手"
            row["裁定日"] = args.date
            unresolved.remove(item)
            second.append((number, word, len(rest)))
    if second:
        print(f"語で突き合わせた {len(second)} 件")
        for number, word, n in second:
            note = "語が一意" if n == 1 else "同じ語の回答がすべて同じ答え"
            print(f"  回答 {number} 行目: {word}({note})")

    missing = [r for r in split if id(r) not in used]
    print(f"書き入れた {len(used)} 行 / 照合できなかった回答 {len(unresolved)} 件")
    for number, word, fragment, mark, n in unresolved:
        print(f"  回答 {number} 行目: {word} / 候補 {n} 件 / 断片「{fragment[:30]}」")
    print(f"回答が無い食い違い行 {len(missing)} 行")
    for r in missing:
        print(f"  {r['文書']}:{r['行']} [{r['語']}] {r['該当文'][:70]}")

    fields = list(rows[0].keys())
    with open(args.out, "w", encoding="utf-8", newline="") as sink:
        writer = csv.DictWriter(sink, fieldnames=fields, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    print(f"書き出した: {args.out}")
    return 0 if not unresolved else 1


if __name__ == "__main__":
    sys.exit(main())
