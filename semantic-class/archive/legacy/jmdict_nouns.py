"""JMdict の XML から、分類にかける名詞の見出しを抜き出す。

基本の絞り方は 4 段である。第 1 に、品詞に `n`(名詞)を持つ語義が 1 つ以上ある
項目だけを採る。`n-suf`(接尾辞的名詞)・`ctr`(助数詞)・`pn`(代名詞)しか
持たない項目は、単独で立つ名詞ではないので採らない。第 2 に、その語義が
`exp`(成句)を併せ持つ場合は落とす。「気を付ける」のような句は 1 語の分類に
なじまない。第 3 に、名詞の語義がどれも古語・廃語・稀語の印を持つ項目を落とす。
生成の方針が求めるのは現代の日本語で日常的に使われる語義だからである。第 4 に、
表記に日本語の文字以外(ラテン文字・数字・記号・空白)が混じる見出しと、13 字
以上の見出しを落とす。

これに 2 つの追加の絞りを重ねられる。`--drop-names` は、名詞の語義がどれも
固有名の印(会社名・地名・人名・作品名など)を持つ項目を落とす。`--priority`
は、JMdict が頻度の印(ichi・news・spec・gai)を付けた見出しだけを残す。
頻度の印は国語辞典と新聞のコーパスに由来し、日常的に使われる語の目安になる。

見出しは項目ごとに 1 つだけ採る。漢字表記があればその先頭、無ければ読みの先頭
である。表記の揺れをすべて採ると、同じ語の分類を何度も課金することになる。

XML の実体参照(`&n;` など)は DTD で定義されており、ElementTree は展開できない。
1 行 1 タグの形で書かれているので、行を読んで組み立てる。
"""

import argparse
import re

POS = re.compile(r"<pos>&([a-z0-9-]+);</pos>")
MISC = re.compile(r"<misc>&([a-z0-9-]+);</misc>")
KEB = re.compile(r"<keb>(.+?)</keb>")
REB = re.compile(r"<reb>(.+?)</reb>")
PRI = re.compile(r"<(?:ke|re)_pri>(.+?)</(?:ke|re)_pri>")

# 日本語の文字だけでできた見出しを通す。長音符と繰り返し記号を含める。
JAPANESE = re.compile(r"\A[ぁ-ゟ゠-ヿ一-鿿々ーー]+\Z")
MAX_LENGTH = 12

# 現代の日本語で日常的に使われる語義ではない印。
STALE = {"arch", "obs", "obsc", "rare", "dated", "hist", "poet"}

# 固有名の印。JMdict_e は固有名の大半を持たないが、混じる分を落とせる。
NAMES = {
    "company", "organization", "product", "place", "person", "given",
    "surname", "char", "work", "station", "serv", "group", "creat",
    "dei", "doc", "ev", "fem", "male", "myth", "obj", "relig", "tradem",
}

# 頻度の印の接頭。ichi1・news2・spec1・gai1 のような形で付く。
PRIORITY = ("ichi", "news", "spec", "gai")


def entries(path):
    """項目ごとに(漢字表記、読み、語義、頻度の印)を返す。"""
    kebs, rebs, senses, pris = [], [], [], set()
    pos, misc = set(), set()
    in_sense = False
    with open(path, encoding="utf-8") as source:
        for line in source:
            if line.startswith("<entry>"):
                kebs, rebs, senses, pris = [], [], [], set()
            elif line.startswith("<keb>"):
                kebs.append(KEB.search(line).group(1))
            elif line.startswith("<reb>"):
                rebs.append(REB.search(line).group(1))
            elif line.startswith("<ke_pri>") or line.startswith("<re_pri>"):
                pris.add(PRI.search(line).group(1))
            elif line.startswith("<sense>"):
                in_sense, pos, misc = True, set(), set()
            elif in_sense and line.startswith("<pos>"):
                pos.add(POS.search(line).group(1))
            elif in_sense and line.startswith("<misc>"):
                misc.add(MISC.search(line).group(1))
            elif line.startswith("</sense>"):
                senses.append((pos, misc))
                in_sense = False
            elif line.startswith("</entry>"):
                yield kebs, rebs, senses, pris


def noun_headword(kebs, rebs, senses, pris, drop_names=False, priority=False):
    """名詞として採る見出しを返す。採らない項目では None を返す。"""
    nouns = [(pos, misc) for pos, misc in senses if "n" in pos and "exp" not in pos]
    if not nouns:
        return None
    if all(misc & STALE for _, misc in nouns):
        return None
    if drop_names and all(misc & NAMES for _, misc in nouns):
        return None
    if priority and not any(p.startswith(PRIORITY) for p in pris):
        return None
    word = (kebs or rebs or [None])[0]
    if word is None or len(word) > MAX_LENGTH or not JAPANESE.fullmatch(word):
        return None
    return word


def collect(path, drop_names=False, priority=False):
    seen, words = set(), []
    total = 0
    for kebs, rebs, senses, pris in entries(path):
        total += 1
        word = noun_headword(kebs, rebs, senses, pris, drop_names, priority)
        if word and word not in seen:
            seen.add(word)
            words.append(word)
    return words, total


def priority_words(path):
    """JMdict が頻度の印を付けた見出しの集合を返す。

    名詞の絞りは掛けない。第 1 層(評価コーパスの語)にも印の有無を付けたいので、
    項目の見出しをすべて見る。
    """
    words = set()
    for kebs, rebs, _senses, pris in entries(path):
        if any(p.startswith(PRIORITY) for p in pris):
            words.update(kebs)
            words.update(rebs)
    return words


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jmdict", required=True)
    parser.add_argument("--out")
    parser.add_argument("--exclude", help="この一覧にある語を除く(第 1 層など)")
    parser.add_argument("--drop-names", action="store_true")
    parser.add_argument("--priority", action="store_true")
    args = parser.parse_args()

    words, total = collect(args.jmdict, args.drop_names, args.priority)
    print(f"JMdict の項目 {total} / 採った見出し {len(words)}")
    if args.exclude:
        known = {
            w for w in open(args.exclude, encoding="utf-8").read().split("\n") if w
        }
        words = [w for w in words if w not in known]
        print(f"既知の {len(known)} 語を除いた残り {len(words)}")
    if args.out:
        with open(args.out, "w", encoding="utf-8") as sink:
            for word in sorted(words):
                print(word, file=sink)
        print(f"書き出した: {args.out}")


if __name__ == "__main__":
    main()
