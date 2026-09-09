"""確定値から、意味分類表の候補を semantic-class-table.tsv の形で書き出す。

主にするモデルを 1 つ選び、その判定を表にする。分類番号は体の類なら 1.mm00、
用の類なら 2.mm00 の形で、mm が中項目の 2 桁である。読み込み側は小数点以下
4 桁の形を求めるので、第 3 位と第 4 位は 0 で埋める。

冒頭に方針と来歴を注記する。表だけを受け取った読み手が、どの語義を挙げる
方針で作られた表かを知れるようにするためである。
"""

import argparse
import datetime
import json
import os
import sys

import load_target
import votes

# 注記の 1 行に入れる全角の文字数。
WIDTH = 34


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--votes-dir", required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--priority-only", action="store_true", help="頻度の印を持つ語だけ")
    parser.add_argument("--manifest", help="manifest.json の場所。省くと votes-dir の直下を見る")
    parser.add_argument(
        "--target",
        required=True,
        help="分類対象のディレクトリ。分類番号の整数部と注記の文言が変わる",
    )
    args = parser.parse_args()
    target = load_target.load(args.target)
    prefix = target.CLASS_PREFIX
    pos_name = target.POS_NAME

    rows = votes.read(votes.path_for(args.votes_dir, args.model))
    if args.priority_only:
        rows = {w: r for w, r in rows.items() if r["jmdict_priority"]}
    manifest_path = args.manifest or os.path.join(args.votes_dir, "manifest.json")
    manifest = json.load(open(manifest_path, encoding="utf-8"))
    contract = manifest["contract"]["text"]

    tiers, rules = {}, {}
    for row in rows.values():
        tiers[row["tier"]] = tiers.get(row["tier"], 0) + 1
        if "rule" in row:
            rules[row["rule"]] = rules.get(row["rule"], 0) + 1

    # 方針は 1 続きの文字列で持っているので、句点で文に割ってから幅で折る。
    # akunuki は連続するコメント行を 1 つの段落として読み、句点の無い行を
    # 次の行とつなぐ。
    wrapped = []
    for sentence in contract.replace(" ", "").split("。"):
        if not sentence:
            continue
        sentence += "。"
        while len(sentence) > WIDTH:
            wrapped.append("# " + sentence[:WIDTH])
            sentence = sentence[WIDTH:]
        wrapped.append("# " + sentence)

    header = [
        "# 意味分類表(見出し語と分類番号の対応)。候補として生成した表である。",
        f"# 生成に使ったモデルは {args.model} である。",
        "# 生成の方式は、語をモデルへ渡して意味の中項目を答えさせる分類である。",
        "# 呼び出しは DSPy の Predict 1 段で、few-shot の実例を"
        f" {manifest['few_shot']['words']} 語ぶん積んである。",
        "#",
        "# 生成の方針は次のとおりである。",
    ]
    header += wrapped
    header += [
        "#",
        f"# 分類番号は {prefix}.mm00 の形で、mm が中項目の 2 桁である。小数点以下第 3 位と",
        f"# 第 4 位は 0 で埋めてあり、意味を持たない。中項目は {manifest['categories']['count']} 個で、分類語彙表の",
        "# 番号と名前の一覧だけを使い、どの語にどの中項目を与えるか",
        "# はモデルの言語判断で決めた。",
        "#",
        f"# 語の内訳は、評価コーパスに現れた語が {tiers.get('corpus', 0)} 語で、"
        f"JMdict の{pos_name}の見出しが {tiers.get('jmdict', 0)} 語である。",
        (
            "# 日常的に使われる語は、バッチの組み方の種を変えた 3 つのパスで判定し、"
            f"2 パス以上が挙げた中項目だけを採った({rules.get('majority', 0)} 語)。"
            if rules
            else "# 判定は 1 パスである。"
        ),
        (
            f"# 残る {rules.get('single-vote', 0)} 語は 1 回の判定をそのまま採った。"
            "3 パスの答えがすべて割れて確定しなかった語は、この表に載せていない。"
            if rules
            else "#"
        ),
        "# 判定の元データと来歴は votes の JSONL と manifest.json にある。",
        f"# 生成した日は {datetime.date.today().isoformat()} である。",
        f"# 版: v5-{args.model}",
    ]

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as sink:
        for line in header:
            print(line, file=sink)
        for word in sorted(rows):
            numbers = "\t".join(f"{prefix}.{c}00" for c in rows[word]["categories"])
            print(f"{word}\t{numbers}", file=sink)
    print(f"書き出した: {args.out} / 行 {len(rows)} / 内訳 {tiers}", file=sys.stderr)


if __name__ == "__main__":
    main()
