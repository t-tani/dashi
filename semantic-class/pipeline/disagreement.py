"""確定値の不一致を並べる。上書き表を育てる作業の待ち行列になる。

2 種類の食い違いを分けて持つ。1 つはモデル間の不一致で、2 つのモデルが同じ語に
違う中項目を与えた場合である。もう 1 つは同じモデルのパス間の揺れで、バッチの組み方を
変えると答えが変わった場合である。前者はモデルの言語判断の差、後者はバッチ起因の揺れ
なので、直し方が違う。パス間で揺れた語は上書き表に書くより先に、判定を増やすか
バッチを固定する方が効く。

割れ方は 3 つに分ける。交わりが無い対は、どちらかが明らかに誤っている見込みが
高く、先に見るべきものである。片方がもう片方を含む対は語義の数の差でしかない。
残りは部分的に重なる対である。

検出器が読むのは「引けた分類がすべて生産物か自然物の部門にあるか」の 1 ビット
なので、その判定が割れたかどうかも欄に持つ。表の差し替えに効くのはこの列である。
"""

import argparse
import csv
import json
import os
from collections import Counter

FIELDS = (
    "word",
    "kind",
    "a",
    "b",
    "concrete_split",
    "a_pass",
    "b_pass",
    "a_votes",
    "b_votes",
    "tier",
    "jmdict_priority",
)


def read_final(path):
    rows = {}
    with open(path, encoding="utf-8") as source:
        for line in source:
            line = line.strip()
            if line:
                row = json.loads(line)
                rows[row["word"]] = row
    return rows


def divisions(categories):
    return {c // 10 for c in categories}


def concrete(categories):
    divs = divisions(categories)
    return bool(divs) and divs <= {4, 5}


def kind(first, second):
    a, b = set(first), set(second)
    if a == b:
        return "一致"
    if not a & b:
        return "交わり無し"
    if a <= b or b <= a:
        return "包含"
    return "部分的に重なる"


def pass_state(row):
    """パス間の揺れを 3 つの値で言う。判定が 1 回しかない語は判定できない。"""
    if row["votes"] < 2:
        return "判定 1 回のみ"
    return "一致" if row["unanimous"] else "割れた"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--final-dir", required=True)
    parser.add_argument("--model-a", required=True)
    parser.add_argument("--model-b", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    a = read_final(os.path.join(args.final_dir, args.model_a.replace("/", "-") + ".jsonl"))
    b = read_final(os.path.join(args.final_dir, args.model_b.replace("/", "-") + ".jsonl"))
    both = sorted(set(a) & set(b))
    print(f"{args.model_a} {len(a)} 語 / {args.model_b} {len(b)} 語 / 両方 {len(both)} 語")

    rows, model_same = [], 0
    for word in both:
        first, second = a[word]["categories"], b[word]["categories"]
        how = kind(first, second)
        if how == "一致":
            model_same += 1
            continue
        rows.append(
            {
                "word": word,
                "kind": how,
                "a": " ".join(map(str, first)),
                "b": " ".join(map(str, second)),
                "concrete_split": "はい" if concrete(first) != concrete(second) else "いいえ",
                "a_pass": pass_state(a[word]),
                "b_pass": pass_state(b[word]),
                "a_votes": a[word]["votes"],
                "b_votes": b[word]["votes"],
                "tier": a[word]["tier"],
                "jmdict_priority": "はい" if a[word]["jmdict_priority"] else "いいえ",
            }
        )

    split = sum(1 for r in rows if r["concrete_split"] == "はい")
    print(f"モデル間で割れた語 {len(rows)} / {len(both)} = {len(rows) / len(both):.1%}")
    print("  割れ方:", dict(Counter(r["kind"] for r in rows)))
    print(f"  具体条件まで割れた語 {split} = 両方に載る語の {split / len(both):.1%}")
    print("  層ごと:", dict(Counter(r["tier"] for r in rows)))

    # 2 回以上の判定がある語だけで、モデル差とバッチ起因の揺れを分けて数える。
    multi = [w for w in both if a[w]["votes"] >= 2 and b[w]["votes"] >= 2]
    if multi:
        wobble = sum(1 for w in multi if not (a[w]["unanimous"] and b[w]["unanimous"]))
        differ = sum(1 for w in multi if kind(a[w]["categories"], b[w]["categories"]) != "一致")
        steady = sum(
            1
            for w in multi
            if a[w]["unanimous"]
            and b[w]["unanimous"]
            and kind(a[w]["categories"], b[w]["categories"]) != "一致"
        )
        print(f"\n2 回以上の判定がある {len(multi)} 語の内訳")
        print(f"  どちらかのパスが揺れた       {wobble} = {wobble / len(multi):.1%}")
        print(f"  モデル間で割れた             {differ} = {differ / len(multi):.1%}")
        print(f"  両モデルとも安定なのに割れた {steady} = {steady / len(multi):.1%}")
        print("  最後の行が、バッチ起因の揺れでは説明できないモデルの言語判断の差である")

    order = {"交わり無し": 0, "部分的に重なる": 1, "包含": 2}
    rows.sort(key=lambda r: (order[r["kind"]], r["concrete_split"] != "はい", r["word"]))

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8", newline="") as sink:
        writer = csv.DictWriter(sink, fieldnames=list(FIELDS), delimiter="\t")
        writer.writeheader()
        writer.writerows(rows)
    print("\n書き出した:", args.out)

    with open(args.out.rsplit(".", 1)[0] + "-summary.json", "w", encoding="utf-8") as sink:
        json.dump(
            {
                "model_a": args.model_a,
                "model_b": args.model_b,
                "judged_by_both": len(both),
                "model_agree": model_same,
                "model_disagree": len(rows),
                "kinds": dict(Counter(r["kind"] for r in rows)),
                "concrete_split": split,
            },
            sink,
            ensure_ascii=False,
            indent=1,
        )


if __name__ == "__main__":
    main()
