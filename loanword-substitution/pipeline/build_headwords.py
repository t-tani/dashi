"""分野で頻出のカタカナの名詞の語義を 1 行 1 語義で書く。

行は (カタカナ語, 語義) の組である。語義は JMdict の `<sense>` であり、1 つの語義が複数の英訳を
持つ。`ティア` は `tier` の語義と `tear; teardrop shape` の語義の 2 行になる。英訳を平らに並べると
1 つの意味が 2 つに割れるので、語義を単位にする。

j は、分野の 2 種類の数え上げ(技術文書と機関、Wikipedia の話題別の記事)のどちらかで
`--min-rate` 件/100 万語以上のカタカナの名詞である。頻度は名詞トークン 100 万あたりの全出現数で、
一般の対照として Wikipedia の全記事の頻度も添える。数え上げは corpus-tool の `count-nouns` が書く。

読みは長音の有無の揺れ(ブラウザ/ブラウザー)も引く。JMdict にない語は、分野で
`--part-min-rate` 以上のカタカナ語へ最長一致で割れれば部品の第 1 語義をつなぎ(parts)、
割れなければ none とする。lsource は、その読みの項目がすべて英語以外の語源のときの言語である。
"""
import argparse
import json
import re
from collections import Counter
from pathlib import Path

KATAKANA = re.compile(r"^[ァ-ヴー]+$")


def load_counts(counts_dir, names, column="total"):
    """corpus-tool の `count-nouns` が書いた `<counts-dir>/<name>/noun-counts.tsv` と
    `noun-stats.json` を読み、語ごとの出現数と名詞トークンの総数を返す。複数の名前を渡せば足す。

    `column` が `total` なら全出現数、`alone` なら単独の出現数を読む。単独とは、前に名詞・形状詞・
    接頭辞がなく、後ろに名詞・形状詞・接尾辞がない出現である。`門` を `ゲート` の代わりに書く型は
    単独で立つ形なので、置き換えの語は単独で数える。
    """
    counts, tokens = Counter(), 0
    index = 1 if column == "total" else 2
    for name in names:
        directory = Path(counts_dir) / name
        with (directory / "noun-counts.tsv").open(encoding="utf-8") as source:
            for line in source:
                fields = line.rstrip("\n").split("\t")
                counts[fields[0]] += int(fields[index])
        tokens += json.loads((directory / "noun-stats.json").read_text(encoding="utf-8"))["noun_tokens"]
    return counts, tokens


class Readings:
    def __init__(self, path):
        self.index = json.loads(Path(path).read_text(encoding="utf-8"))

    def lookup(self, word):
        variants = [word, word[:-1]] if word.endswith("ー") else [word, word + "ー"]
        for variant in variants:
            if variant in self.index:
                return self.index[variant]
        return None

    def senses(self, word):
        """(語義の一覧, 語源の言語) を返す。項目がなければ None。

        語義は {"glosses": [英訳], "computing": 計算機分野のタグの有無} である。同じ読みの項目が
        複数あれば語義を並べ、英訳の並びが同じ語義は 1 つにまとめる。
        """
        entries = self.lookup(word)
        if not entries:
            return None
        senses, seen = [], set()
        for entry in entries:
            for sense in entry["senses"]:
                key = tuple(sense["glosses"])
                if key and key not in seen:
                    seen.add(key)
                    senses.append(sense)
        foreign = all(e["lsource"] and e["lsource"] != ["eng"] for e in entries)
        languages = set().union(*(set(e["lsource"]) for e in entries)) if foreign else set()
        return senses, languages


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--counts-dir", required=True)
    parser.add_argument("--readings", required=True, help="jmdict_readings.py の出力")
    parser.add_argument("--field", default="it-docs,it-agency", help="技術文書と機関の数え上げの名前")
    parser.add_argument("--field-wiki", default="wiki-computing", help="Wikipedia の話題別の記事の数え上げの名前")
    parser.add_argument("--general", default="wiki-all", help="一般の対照の数え上げの名前")
    parser.add_argument("--min-rate", type=float, default=2.0)
    parser.add_argument("--part-min-rate", type=float, default=5.0)
    parser.add_argument("--english-only", action="store_true", help="JMdict に語義がある語だけを載せる")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    field, field_tokens = load_counts(args.counts_dir, args.field.split(","))
    wiki, wiki_tokens = load_counts(args.counts_dir, [args.field_wiki])
    general, general_tokens = load_counts(args.counts_dir, [args.general])
    readings = Readings(args.readings)

    def rate(counts, tokens, word):
        return counts.get(word, 0) * 1_000_000 / tokens

    def field_rate(word):
        return max(rate(field, field_tokens, word), rate(wiki, wiki_tokens, word))

    def first_gloss(word):
        """部品の第 1 語義の先頭の英訳。語義が無ければ None。"""
        found = readings.senses(word)
        if not found or not found[0]:
            return None
        return found[0][0]["glosses"][0]

    def split_parts(word):
        parts, i = [], 0
        while i < len(word):
            for length in range(len(word) - i, 1, -1):
                part = word[i : i + length]
                if field_rate(part) >= args.part_min_rate and first_gloss(part):
                    parts.append(part)
                    i += length
                    break
            else:
                return None
        return parts if len(parts) >= 2 else None

    rows = []
    for word in {w for w in set(field) | set(wiki) if KATAKANA.match(w)}:
        if field_rate(word) < args.min_rate:
            continue
        rates = (rate(field, field_tokens, word), rate(wiki, wiki_tokens, word),
                 rate(general, general_tokens, word))
        found = readings.senses(word)
        if found and found[0]:
            senses, languages = found
            language = ",".join(sorted(languages))
            for index, sense in enumerate(senses):
                rows.append((word, index, ";".join(sense["glosses"]),
                             "yes" if sense["computing"] else "no", "jmdict", language) + rates)
            continue
        if args.english_only:
            continue
        parts = split_parts(word)
        if parts:
            joined = " ".join(first_gloss(part) for part in parts)
            rows.append((word, 0, joined, "no", "parts:" + "+".join(parts), "") + rates)
        else:
            rows.append((word, 0, "", "no", "none", "") + rates)
    rows.sort(key=lambda r: (-(r[6] + r[7]), r[0], r[1]))
    with open(args.out, "w", encoding="utf-8") as sink:
        sink.write("j\tsense\tenglish\tcomputing\tenglish_source\tlsource\t"
                   "field_per_million\twiki_topic_per_million\tgeneral_per_million\n")
        for r in rows:
            sink.write(f"{r[0]}\t{r[1]}\t{r[2]}\t{r[3]}\t{r[4]}\t{r[5]}\t"
                       f"{r[6]:.1f}\t{r[7]:.1f}\t{r[8]:.1f}\n")
    sources = Counter(r[4].split(":")[0] for r in rows)
    print(f"行 {len(rows)}、語 {len({r[0] for r in rows})}、"
          f"jmdict {sources['jmdict']}、parts {sources['parts']}、none {sources['none']}")


if __name__ == "__main__":
    main()
