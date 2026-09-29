"""集めた候補から、配布する外来語の置き換え表を書く。

行は (置き換えの語, カタカナ語, 語義, モデル) の組である。語義は JMdict の `<sense>` であり、
英訳の列はその語義の英訳を `;` でつないだものである。`ティア` の `tier` と `tear; teardrop shape` は
別の行になる。載せるのは、`--min-votes` 回以上の応答に現れ、JMdict に見出しがあり、カタカナでなく、
一般のコーパスで `--min-noun-count` 回以上現れる名詞の候補である。LLM が作った語と、カタカナ語のまま
返した候補を落とすための条件である。

各行に、応答に現れた回数と応答の中での順位、置き換えの語の情報技術と一般での単独の出現の頻度、
カタカナ語の情報技術での全出現の頻度を添える。順位は、確からしい順に挙げさせた候補の並びの位置であり、
現れた応答の数が同じ候補の間の強弱を持つ。検査する側は、文書に現れた語をこの表で引き、その語が分野で
稀でカタカナ語が頻出なら候補に挙げる。
"""
import argparse
import json
import re
from collections import Counter, defaultdict
from pathlib import Path

from build_headwords import load_counts

KATAKANA = re.compile(r"^[ァ-ヴー]+$")


def read_candidates(directory, axis):
    """(カタカナ語, 語義の番号, 英語, モデル) ごとの集計と、応答の総数を返す。

    集計は候補ごとに (現れた応答の数, 順位の合計) である。順位は応答の中の位置で、先頭を 1 とする。
    同じ応答に同じ候補が 2 回現れたら先の位置を採る。
    """
    tallies = defaultdict(lambda: defaultdict(lambda: [0, 0]))
    samples = Counter()
    for path in sorted(Path(directory).glob("*.jsonl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            record = json.loads(line)
            if record["axis"] != axis or record.get("error"):
                continue
            key = (record["loanword"] or "", record.get("sense", ""), record["english"], record["model"])
            samples[key] += 1
            ranks = {}
            for position, candidate in enumerate(record["candidates"], start=1):
                candidate = candidate.strip()
                if candidate:
                    ranks.setdefault(candidate, position)
            for candidate, rank in ranks.items():
                tally = tallies[key][candidate]
                tally[0] += 1
                tally[1] += rank
    return tallies, samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidates-dir", default="candidates")
    parser.add_argument("--counts-dir", default="counts")
    parser.add_argument("--glosses", required=True, help="jmdict_glosses.py の出力")
    parser.add_argument("--readings", required=True, help="jmdict_readings.py の出力")
    parser.add_argument("--field", default="it-docs,it-agency")
    parser.add_argument("--field-wiki", default="wiki-computing")
    parser.add_argument("--general", default="wiki-all")
    parser.add_argument("--min-votes", type=int, default=3, help="候補が現れた応答の数の下限")
    parser.add_argument("--min-noun-count", type=int, default=100, help="一般のコーパスでの名詞の回数の下限")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    known = set(json.loads(Path(args.glosses).read_text(encoding="utf-8")))
    known |= set(json.loads(Path(args.readings).read_text(encoding="utf-8")))
    field_names = args.field.split(",") + [args.field_wiki]
    # 置き換えの語は単独の出現で、カタカナ語は全出現で数える。書き手が `門` を単独で立てる形が
    # 問題なのに対し、分野が `ゲート` を使うことは複合語の中の出現にも表れるためである。
    field_alone, field_tokens = load_counts(args.counts_dir, field_names, "alone")
    field_total, _ = load_counts(args.counts_dir, field_names, "total")
    general_alone, general_tokens = load_counts(args.counts_dir, [args.general], "alone")
    general_total, _ = load_counts(args.counts_dir, [args.general], "total")

    def rate(counts, tokens, word):
        return counts.get(word, 0) * 1_000_000 / tokens

    def accepted(candidate):
        return (
            candidate in known
            and not KATAKANA.match(candidate)
            and general_total.get(candidate, 0) >= args.min_noun_count
        )

    rows = []
    for axis in ("loanword", "english"):
        tallies, samples = read_candidates(args.candidates_dir, axis)
        for key, counted in tallies.items():
            loanword, sense, english, model = key
            for candidate, (votes, rank_sum) in counted.items():
                if votes < args.min_votes or not accepted(candidate):
                    continue
                rows.append(
                    (
                        candidate,
                        axis,
                        loanword,
                        sense,
                        english,
                        model,
                        votes,
                        samples[key],
                        rank_sum / votes,
                        rate(field_alone, field_tokens, candidate),
                        rate(general_alone, general_tokens, candidate),
                        rate(field_total, field_tokens, loanword) if loanword else 0.0,
                    )
                )
    rows.sort(key=lambda r: (r[0], r[1], r[2], r[4], r[5]))
    with open(args.out, "w", encoding="utf-8") as sink:
        sink.write(
            "word\taxis\tloanword\tsense\tenglish\tmodel\tvotes\tsamples\tmean_rank\t"
            "word_field_alone_per_million\tword_general_alone_per_million\tloanword_field_per_million\n"
        )
        for r in rows:
            sink.write(
                f"{r[0]}\t{r[1]}\t{r[2]}\t{r[3]}\t{r[4]}\t{r[5]}\t{r[6]}\t{r[7]}\t{r[8]:.2f}\t"
                f"{r[9]:.1f}\t{r[10]:.1f}\t{r[11]:.1f}\n"
            )
    words = {r[0] for r in rows}
    print(f"rows {len(rows)} words {len(words)}")


if __name__ == "__main__":
    main()
