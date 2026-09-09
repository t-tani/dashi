#!/usr/bin/env python3
"""日本語 WordNet から意味分類表(部門だけ)を生成する。

入力:
  dict/data.noun        Princeton WordNet 3.0 の名詞データ(lex_filenum を持つ)
  wnjpn-ok.tab          日本語 WordNet 1.1 の語と synset の対応
出力:
  wnja-semantic-class-table.tsv
"""

import sys
import unicodedata

# lexname 番号(data.noun の第 2 欄)から分類語彙表の部門へ。
# 1 関係 / 2 主体 / 3 活動 / 4 生産物 / 5 自然物
LEX_DIVISION = {
    "04": {3},  # noun.act        行為
    "05": {5},  # noun.animal     動物
    "06": {4},  # noun.artifact   人工物
    "07": {1},  # noun.attribute  属性
    "08": {5},  # noun.body       身体
    "09": {3},  # noun.cognition  認識
    "10": {3},  # noun.communication 伝達
    "11": {1},  # noun.event      出来事
    "12": {3},  # noun.feeling    感情
    "13": {4},  # noun.food       食物
    "14": {2},  # noun.group      集団
    "15": {1},  # noun.location   場所
    "16": {3},  # noun.motive     動機
    "17": {5},  # noun.object     自然の物体
    "18": {2},  # noun.person     人
    "19": {5},  # noun.phenomenon 自然現象
    "20": {5},  # noun.plant      植物
    "21": {3},  # noun.possession 所有
    "22": {5},  # noun.process    過程
    "23": {1},  # noun.quantity   数量
    "24": {1},  # noun.relation   関係
    "25": {1},  # noun.shape      形
    "26": {1},  # noun.state      状態
    "27": {5},  # noun.substance  物質
    "28": {1},  # noun.time       時間
}

# 収める synset が部門をまたぐ lexname は、部門を 1 つに絞らず並べる。
# 絞れない根拠は lexfile の中身にある。noun.location は「下側」のような抽象的な
# 場所と「ギニア」のような国と「カンブリア山地」のような地形を同居させる。
# noun.state は「経済状態」と「喘息」を、noun.process は「処理」と「電気分解」を、
# noun.event は「催物」と「受粉」を、noun.substance は「合板」と「岩塩」を、
# noun.attribute は「外形」と「明察」を同居させる。
WIDE_LEX_DIVISION = dict(LEX_DIVISION)
WIDE_LEX_DIVISION.update(
    {
        "07": {1, 3},  # noun.attribute
        "11": {1, 3},  # noun.event
        "15": {1, 2, 5},  # noun.location
        "22": {3, 5},  # noun.process
        "26": {1, 5},  # noun.state
        "27": {4, 5},  # noun.substance
    }
)

# noun.Tops(lexname 03)の 51 synset は、それが率いる lexfile の部門を継ぐ。
TOPS_DIVISION = {
    "00001740": 1,  # entity
    "00001930": 1,  # physical entity
    "00002137": 1,  # abstraction
    "00002452": 1,  # thing
    "00002684": 1,  # object
    "00003553": 1,  # whole, unit
    "00003993": 1,  # congener
    "00004258": 5,  # living thing
    "00004475": 5,  # organism
    "00005787": 5,  # benthos
    "00005930": 5,  # dwarf
    "00006024": 5,  # heterotroph
    "00006150": 5,  # parent (生物)
    "00006269": 5,  # life
    "00006400": 5,  # biont
    "00006484": 5,  # cell
    "00007347": 1,  # causal agent
    "00007846": 2,  # person
    "00015388": 5,  # animal
    "00017222": 5,  # plant
    "00019046": 5,  # native
    "00019128": 5,  # natural object
    "00019613": 5,  # substance
    "00020090": 5,  # substance
    "00020827": 5,  # matter
    "00021265": 4,  # food
    "00021734": 4,  # nutrient
    "00021939": 4,  # artifact
    "00022903": 4,  # article
    "00023100": 3,  # psychological feature
    "00023271": 3,  # cognition
    "00023773": 3,  # motivation
    "00024264": 1,  # attribute
    "00024720": 1,  # state
    "00026192": 3,  # feeling
    "00027167": 1,  # location
    "00027807": 1,  # shape
    "00028270": 1,  # time
    "00028651": 1,  # space
    "00029007": 1,  # absolute space
    "00029114": 1,  # phase space
    "00029378": 1,  # event
    "00029677": 5,  # process
    "00030358": 3,  # act
    "00031264": 2,  # group
    "00031921": 1,  # relation
    "00032613": 3,  # possession
    "00032823": 1,  # social relation
    "00033020": 3,  # communication
    "00033615": 1,  # measure, quantity
    "00034213": 5,  # phenomenon
}


def read_offset_divisions(path, lex_division):
    """synset のオフセットから部門の集合へ。"""
    divisions = {}
    with open(path, encoding="latin-1") as source:
        for line in source:
            if line.startswith("  "):
                continue
            columns = line.split(" ", 2)
            offset, lexname = columns[0], columns[1]
            if lexname == "03":
                found = TOPS_DIVISION.get(offset)
                found = None if found is None else {found}
            else:
                found = lex_division.get(lexname)
            if found is not None:
                divisions[offset] = found
    return divisions


def read_word_divisions(path, offset_divisions):
    """日本語の語から部門の集合へ。"""
    words = {}
    with open(path, encoding="utf-8") as source:
        for line in source:
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 2:
                continue
            synset, word = fields[0], fields[1].strip()
            if not synset.endswith("-n") or not word or " " in word:
                continue
            offset = synset[:-2]
            found = offset_divisions.get(offset)
            if found is None:
                continue
            words.setdefault(word, set()).update(found)
    return words


def main():
    wide = len(sys.argv) > 1 and sys.argv[1] == "wide"
    lex_division = WIDE_LEX_DIVISION if wide else LEX_DIVISION
    output = (
        "wnja-semantic-class-table-wide.tsv"
        if wide
        else "wnja-semantic-class-table.tsv"
    )
    offset_divisions = read_offset_divisions("dict/data.noun", lex_division)
    words = read_word_divisions("wnjpn-ok.tab", offset_divisions)

    header = [
        "# 意味分類表(見出し語と分類番号の対応)。部門だけを持つ検証用の表である。",
        "# 出典は日本語 WordNet (1.1) と Princeton WordNet 3.0 である。",
        "# 日本語 WordNet の取得元は",
        "# https://github.com/bond-lab/wnja/releases/download/v1.1/wnjpn-ok.tab.gz である。",
        "# Princeton WordNet 3.0 の取得元は",
        "# https://wordnetcode.princeton.edu/3.0/WNdb-3.0.tar.gz である。",
        "# 日本語 WordNet の利用条件は https://bond-lab.github.io/wnja/license.txt にある。",
        "# 分類番号の小数点以下第 1 位だけが意味を持ち、残りの 3 桁は 0 で埋めてある。",
        "# 部門は Princeton WordNet の lexname から写した。",
        "# 版: wnja-1.1+pwn-3.0",
    ]
    with open(output, "w", encoding="utf-8") as sink:
        for line in header:
            print(line, file=sink)
        for word in sorted(words):
            numbers = "\t".join(f"1.{d}000" for d in sorted(words[word]))
            print(f"{word}\t{numbers}", file=sink)

    sizes = {}
    for divisions in words.values():
        sizes[len(divisions)] = sizes.get(len(divisions), 0) + 1
    print(f"見出し語 {len(words)}", file=sys.stderr)
    print(f"部門の数の分布 {sorted(sizes.items())}", file=sys.stderr)


if __name__ == "__main__":
    main()
