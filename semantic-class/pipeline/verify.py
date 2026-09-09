"""votes の JSONL を検証し、要るなら重複を落とす。

追記だけで書き足す形なので、走行が同じ語を 2 度書く余地がある。モデルが
1 回の応答で同じ語を 2 度返した場合がこれにあたる。読み込み側は後の行で
上書きするため実害は出ないが、行数と語数がずれると集計を誤る。

`--fix` を渡すと、語ごとに最初の行だけを残して書き直す。
"""

import argparse
import json
import os
from collections import Counter

import load_target
import votes

REQUIRED = (
    "word",
    "categories",
    "model",
    "date",
    "contract",
    "context_used",
    "tier",
    "jmdict_priority",
    "source",
    "chunk_id",
    "chunk_index",
)


def check(path, allowed, fix=False):
    rows = []
    with open(path, encoding="utf-8") as source:
        for number, line in enumerate(source, 1):
            line = line.strip()
            if not line:
                continue
            rows.append((number, json.loads(line)))

    problems = []
    for number, row in rows:
        missing = [key for key in REQUIRED if key not in row]
        if missing:
            problems.append(f"{number} 行目: 欄が無い {missing}")
        cats = row.get("categories") or []
        if not cats:
            problems.append(f"{number} 行目: categories が空")
        if cats != sorted(set(cats)):
            problems.append(f"{number} 行目: categories が昇順でないか重複する {cats}")
        if any(c not in allowed for c in cats):
            problems.append(f"{number} 行目: 範囲の外の中項目 {cats}")
        if row.get("tier") not in votes.TIERS:
            problems.append(f"{number} 行目: tier が {row.get('tier')}")

    counts = Counter(row["word"] for _, row in rows)
    duplicated = {word: n for word, n in counts.items() if n > 1}

    print(f"{os.path.basename(path)}: 行 {len(rows)} / 異なり語 {len(counts)}")
    print(f"  層ごと: {dict(Counter(row['tier'] for _, row in rows))}")
    print(f"  重複した語 {len(duplicated)} 件" + (f" {list(duplicated)[:5]}" if duplicated else ""))
    for problem in problems[:10]:
        print("  " + problem)
    if len(problems) > 10:
        print(f"  ほか {len(problems) - 10} 件")

    if fix and duplicated:
        seen, kept = set(), []
        for _, row in rows:
            if row["word"] in seen:
                continue
            seen.add(row["word"])
            kept.append(row)
        with open(path, "w", encoding="utf-8") as sink:
            for row in kept:
                print(json.dumps(row, ensure_ascii=False), file=sink)
        print(f"  重複を落として書き直した: {len(rows)} 行 → {len(kept)} 行")
    return len(problems) == 0 and not duplicated


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+")
    parser.add_argument("--fix", action="store_true")
    parser.add_argument(
        "--target", required=True, help="分類対象のディレクトリ。許す中項目番号の集合が変わる"
    )
    args = parser.parse_args()
    allowed = load_target.load(args.target).CATEGORIES
    ok = all([check(path, allowed, args.fix) for path in args.files])
    print("\n検証を通った" if ok else "\n直すところがある")


if __name__ == "__main__":
    main()
