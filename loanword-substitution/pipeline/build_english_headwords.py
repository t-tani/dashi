"""情報技術の文書で頻出の語の英訳ごとに、その英語を訳に持つ語を 1 行 1 語で並べた表を書く。

行は (英語, 語) の組である。語は、技術文書と機関か Wikipedia の話題別の記事のどちらかで
`--min-rate` 件/100 万語以上で、JMdict に英訳を持つ名詞である。カタカナかどうかは問わない。
列に、語の文字種、情報技術での頻度、Wikipedia の全記事での頻度、その比、その語義が計算機分野の
タグつきか、同じ英語を持つ語の数を持つ。語が分野の用語かどうかは表では決めず、読む側がこれらの列で
判断する。JMdict は見出しの表(glosses)と読みの索引(readings)の両方で引く。
"""
import argparse
import json
import re
from collections import defaultdict
from pathlib import Path

from build_headwords import Readings, load_counts

KATAKANA = re.compile(r"^[ァ-ヴー]+$")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--counts-dir", required=True)
    parser.add_argument("--readings", required=True, help="jmdict_readings.py の出力")
    parser.add_argument("--glosses", required=True, help="jmdict_glosses.py の出力(見出しごとの英訳)")
    parser.add_argument("--field", default="it-docs,it-agency")
    parser.add_argument("--field-wiki", default="wiki-computing")
    parser.add_argument("--general", default="wiki-all")
    parser.add_argument("--min-rate", type=float, default=50.0)
    parser.add_argument("--single-word", action="store_true", help="空白を含まない英語だけを載せる")
    parser.add_argument("--min-fan-in", type=int, default=1, help="その英語を訳に持つ語の数の下限")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    field, field_tokens = load_counts(args.counts_dir, args.field.split(","))
    wiki, wiki_tokens = load_counts(args.counts_dir, [args.field_wiki])
    general, general_tokens = load_counts(args.counts_dir, [args.general])
    readings = Readings(args.readings)
    glosses = json.loads(Path(args.glosses).read_text(encoding="utf-8"))

    def rate(counts, tokens, word):
        return counts.get(word, 0) * 1_000_000 / tokens

    def field_rate(word):
        return max(rate(field, field_tokens, word), rate(wiki, wiki_tokens, word))

    def english_of(word):
        """(英訳, 計算機分野か) の一覧。カタカナ語は読みの索引、それ以外は見出しの表で引く。

        この軸のプロンプトは英語 1 語だけを渡すので、語義に属する英訳を平らに並べる。
        1 つの英訳が複数の語義に現れれば、計算機分野のタグはどれかが持てば持つとする。
        """
        if KATAKANA.match(word):
            senses = [s for entry in (readings.lookup(word) or []) for s in entry["senses"]]
        else:
            senses = glosses.get(word) or []
        return [(gloss, sense["computing"]) for sense in senses for gloss in sense["glosses"]]

    rows = defaultdict(dict)
    for word in set(field) | set(wiki):
        if field_rate(word) < args.min_rate:
            continue
        for english, computing in english_of(word):
            entry = rows[english].setdefault(word, {"rate": field_rate(word), "computing": False})
            entry["computing"] = entry["computing"] or computing
    if args.single_word:
        rows = {e: w for e, w in rows.items() if " " not in e}
    rows = {e: w for e, w in rows.items() if len(w) >= args.min_fan_in}
    with open(args.out, "w", encoding="utf-8") as sink:
        sink.write("english\tword\tscript\tfield_per_million\tgeneral_per_million\tfield_to_general\tcomputing\tfan_in\n")
        for english, words in sorted(rows.items(), key=lambda item: (-len(item[1]), item[0])):
            for word, value in sorted(words.items(), key=lambda item: -item[1]["rate"]):
                general_rate = rate(general, general_tokens, word)
                ratio = value["rate"] / general_rate if general_rate else float("inf")
                sink.write(
                    f"{english}\t{word}\t{'katakana' if KATAKANA.match(word) else 'other'}\t{value['rate']:.0f}\t"
                    f"{general_rate:.1f}\t{ratio:.1f}\t{'yes' if value['computing'] else 'no'}\t{len(words)}\n"
                )
    print(f"english {len(rows)} rows {sum(len(w) for w in rows.values())} fan_in>=2 {sum(1 for w in rows.values() if len(w) >= 2)}")


if __name__ == "__main__":
    main()
