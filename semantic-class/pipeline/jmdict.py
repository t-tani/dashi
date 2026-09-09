"""JMdict の XML を読み、分類にかける見出しを抜き出す。

品詞で採る語義を選ぶ規則だけが分類対象ごとに違い、target.is_target_pos が持つ。
残りの絞りは名詞と動詞で同じである。第 1 に、採る品詞を持ち `exp`(成句)を
併せ持たない語義が 1 つ以上ある項目だけを採る。「気を付ける」のような句は 1 語の
分類になじまない。第 2 に、その語義がどれも古語・廃語・稀語の印を持つ項目を
落とす。生成の方針が求めるのは現代の日本語で日常的に使われる語義だからである。
第 3 に、表記に日本語の文字以外(ラテン文字・数字・記号・空白)が混じる見出しと、
13 字以上の見出しを落とす。

これに 2 つの追加の絞りを重ねられる。drop_names は、採る語義がどれも固有名の印
(会社名・地名・人名・作品名など)を持つ項目を落とす。priority は、JMdict が
頻度の印(ichi・news・spec・gai)を付けた見出しだけを残す。頻度の印は国語辞典と
新聞のコーパスに由来し、日常的に使われる語の目安になる。

見出しは項目ごとに 1 つだけ採る。漢字表記があればその先頭、無ければ読みの先頭
である。表記の揺れをすべて採ると、同じ語の分類を何度も課金することになる。

XML の実体参照(`&n;` など)は DTD で定義されており、ElementTree は展開できない。
1 行 1 タグの形で書かれているので、行を読んで組み立てる。
"""

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


def headword(kebs, rebs, senses, pris, is_target_pos, drop_names=False, priority=False):
    """分類にかける見出しを返す。採らない項目では None を返す。"""
    kept = [
        (pos, misc) for pos, misc in senses if is_target_pos(pos) and "exp" not in pos
    ]
    if not kept:
        return None
    if all(misc & STALE for _, misc in kept):
        return None
    if drop_names and all(misc & NAMES for _, misc in kept):
        return None
    if priority and not any(p.startswith(PRIORITY) for p in pris):
        return None
    word = (kebs or rebs or [None])[0]
    if word is None or len(word) > MAX_LENGTH or not JAPANESE.fullmatch(word):
        return None
    return word


def collect(path, is_target_pos, drop_names=False, priority=False):
    """見出しの一覧と、読んだ項目の総数を返す。"""
    seen, words = set(), []
    total = 0
    for kebs, rebs, senses, pris in entries(path):
        total += 1
        word = headword(kebs, rebs, senses, pris, is_target_pos, drop_names, priority)
        if word and word not in seen:
            seen.add(word)
            words.append(word)
    return words, total


def priority_words(path):
    """JMdict が頻度の印を付けた見出しの集合を返す。

    品詞の絞りは掛けない。評価コーパスの語にも印の有無を付けたいので、項目の
    見出しをすべて見る。
    """
    words = set()
    for kebs, rebs, _senses, pris in entries(path):
        if any(p.startswith(PRIORITY) for p in pris):
            words.update(kebs)
            words.update(rebs)
    return words
