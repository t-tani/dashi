"""配布する意味分類表を、確定値から 1 枚書き出す。

akunuki が同梱する semantic-class-table.tsv と同じ形式で書く。1 行が 1 語で、
第 1 欄が見出し語、第 2 欄より後が分類番号である。番号は体の類なら 1.mm00、
用の類なら 2.mm00 の形で、mm が中項目の 2 桁である。読み込み側は小数点以下
4 桁の形を求めるので、第 3 位と第 4 位は 0 で埋める。

多数決で確定しなかった語は載せない。3 回の判定がすべて割れた語であり、分類を
引けなければ検出器は黙るので、揺れた分類で指摘を出すより安全側に倒れる。

冒頭の注記は配布のためにある。表だけを受け取った読み手が、誰が何の方針で作り、
どの条件で使えるかを表の中だけで知れるようにする。研究用の候補表を書く
build_table.py とは別にしてあるのは、配布物の注記が法的な表示を含むためである。
"""

import argparse
import datetime
import hashlib
import json
import os
import re

import load_target
import votes


def wrap(text):
    """句点で文に割り、1 文を 1 行に置く。

    幅で折らないのは、日本語には語の切れ目に空白が無く、折ると語の途中で切れる
    ためである。akunuki は連続するコメント行を 1 つの段落として読むので、1 文が
    長くても読みは変わらない。
    """
    # 元の指示文は改行で折ってあり、その位置に空白が残る。日本語には語の切れ目に
    # 空白が無いので、両隣が日本語の空白だけを落とす。数の前後の空白は残す。
    text = re.sub(r"(?<=[^\x00-\x7f]) (?=[^\x00-\x7f])", "", text)
    lines = []
    for sentence in text.split("。"):
        sentence = sentence.strip()
        if sentence:
            lines.append("# " + sentence + "。")
    return lines


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--votes-dir", required=True, help="確定値のディレクトリ")
    parser.add_argument("--model", required=True)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--version", required=True, help="配布のバージョン。tag と合わせる")
    parser.add_argument("--out", required=True)
    parser.add_argument(
        "--target", required=True, help="分類対象のディレクトリ。分類番号の整数部が変わる"
    )
    args = parser.parse_args()
    target = load_target.load(args.target)
    prefix = target.CLASS_PREFIX

    rows = votes.read(votes.path_for(args.votes_dir, args.model))
    manifest = json.load(open(args.manifest, encoding="utf-8"))
    policy = manifest["contract"]["text"]
    fewshot = manifest["few_shot"]["words"]
    today = datetime.date.today().isoformat()

    header = [
        "# 意味分類表(見出し語と分類番号の対応)。読み込みは"
        " crates/aku_morph/src/semantic_class_table.rs が行う。",
        "# 出典は dashi(https://github.com/t-tani/dashi)が生成したこの表である。",
        "# Copyright 2026 Tomoaki Tani",
        f"# 生成に使ったモデルは {args.model} である。",
        f"# 生成した日は {today} である。",
        f"# 配布のバージョンは {args.version} である。",
        "# ライセンスは CC BY 4.0 である。",
        "# 出典を表示すれば、商用を含めて利用と改変と再頒布が許諾される。",
        "# https://creativecommons.org/licenses/by/4.0/",
        "#",
        "# 生成の方式は、語をモデルへ渡して意味の中項目を答えさせる分類である。"
        f"呼び出しは DSPy の Predict 1 段で、few-shot の実例を {fewshot} 語ぶん"
        "積んである。中項目の番号と名前の一覧だけを"
        "使い、どの語にどの中項目を与えるかはモデルの言語判断で決めた。",
        "#",
        "# 生成の方針は次のとおりである。",
    ]
    header += wrap(policy)
    header += [
        "#",
        "# 日常的に使われる語には、まとめ方の種を変えた 3 回の判定を取り、"
        "2 回以上が挙げた中項目だけを採った。3 回の判定がすべて割れて多数決で"
        "確定しなかった語は、この表に載せていない。分類を引けなければ検出器は"
        "黙るので、揺れた分類で指摘を出すより安全側に倒れる。",
        "#",
        f"# 分類番号は {prefix}.mm00 の形で、mm が中項目の 2 桁である。小数点以下第 3 位と"
        "第 4 位は 0 で埋めてあり、意味を持たない。",
        "# 生成の来歴の全文は、dashi の"
        f" {os.path.basename(target.DIRECTORY)}/votes/manifest.json にある。",
    ]

    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as sink:
        for line in header:
            print(line, file=sink)
        for word in sorted(rows):
            numbers = "\t".join(f"{prefix}.{c}00" for c in rows[word]["categories"])
            print(f"{word}\t{numbers}", file=sink)

    body = open(args.out, "rb").read()
    digest = hashlib.sha256(body).hexdigest()
    print(f"書き出した: {args.out}")
    print(f"見出し語: {len(rows)}")
    print(f"バイト数: {len(body)}")
    print(f"SHA-256: {digest}")


if __name__ == "__main__":
    main()
