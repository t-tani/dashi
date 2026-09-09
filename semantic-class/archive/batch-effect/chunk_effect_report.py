"""バッチの組み替えで何が動いたかを、検出器が読む水準まで下ろして見る。

集合一致だけでは影響の大きさが分からない。concrete-noun-misfit が読むのは
「引けた分類がすべて生産物か自然物の部門にあるか」の 1 ビットだけなので、
その判定が変わった語の割合を測る。
"""

import argparse
import json
from collections import Counter


def divisions(categories):
    return {c // 10 for c in categories}


def concrete(categories):
    divs = divisions(categories)
    return bool(divs) and divs <= {4, 5}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+")
    args = parser.parse_args()

    for path in args.files:
        data = json.load(open(path, encoding="utf-8"))
        first, second = data["first"], data["second"]
        both = sorted(set(first) & set(second))
        rows = [(first[w]["categories"], second[w]["categories"]) for w in both]
        n = len(rows)

        same = sum(1 for a, b in rows if a == b)
        div_same = sum(1 for a, b in rows if divisions(a) == divisions(b))
        conc_same = sum(1 for a, b in rows if concrete(a) == concrete(b))
        subset = sum(1 for a, b in rows if a != b and (set(a) <= set(b) or set(b) <= set(a)))
        disjoint = sum(1 for a, b in rows if not (set(a) & set(b)))

        label = "同じバッチの組み方" if data.get("same_chunking") else "違うバッチの組み方"
        print(f"\n== {data['model']} / {label} / {n} 語")
        print(f"中項目の集合一致   {same:4d} = {same / n:.1%}")
        print(f"部門の集合一致     {div_same:4d} = {div_same / n:.1%}")
        print(f"具体条件の一致     {conc_same:4d} = {conc_same / n:.1%}")
        print(f"  食い違いのうち、片方がもう片方を含む {subset} 件")
        print(f"  食い違いのうち、交わりが無い         {disjoint} 件")

        flipped = [
            (w, first[w]["categories"], second[w]["categories"])
            for w in both
            if concrete(first[w]["categories"]) != concrete(second[w]["categories"])
        ]
        if flipped:
            print("  具体条件が反転した語:")
            for word, a, b in flipped[:10]:
                print(f"    {word}: {a} → {b}")
        sizes = Counter(len(a) - len(b) for a, b in rows)
        print("  語義の数の差(1 回目 - 2 回目):", dict(sorted(sizes.items())))


if __name__ == "__main__":
    main()
