"""candidates/ の JSONL を集計し、語(または英語)ごとに候補が何回の応答に現れたかを Markdown に書く。

行は (モデル, 軸, カタカナ語, 英語) の組で、候補は現れた応答の数の多い順に並べる。空の応答の数と、
失敗の数も添える。JMdict の見出しにない候補には * を付ける。
"""
import argparse
import json
import re
from collections import Counter, defaultdict
from pathlib import Path

KATAKANA = re.compile(r"^[ァ-ヴー]+$")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidates-dir", default="candidates")
    parser.add_argument("--glosses", help="jmdict_glosses.py の出力。候補が JMdict の見出しかを印す")
    parser.add_argument("--readings", help="jmdict_readings.py の出力")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    known = set()
    if args.glosses:
        known |= set(json.loads(Path(args.glosses).read_text(encoding="utf-8")))
    if args.readings:
        known |= set(json.loads(Path(args.readings).read_text(encoding="utf-8")))
    tallies = defaultdict(Counter)
    samples = Counter()
    empty = Counter()
    errors = Counter()
    for path in sorted(Path(args.candidates_dir).glob("*.jsonl")):
        for line in path.read_text(encoding="utf-8").splitlines():
            record = json.loads(line)
            key = (record["model"], record["axis"], record["loanword"] or "", record["english"])
            samples[key] += 1
            if record.get("error"):
                errors[key] += 1
                continue
            seen = set()
            for candidate in record["candidates"]:
                candidate = candidate.strip()
                if not candidate or candidate in seen:
                    continue
                seen.add(candidate)
                tallies[key][candidate] += 1
            if not seen:
                empty[key] += 1
    with open(args.out, "w", encoding="utf-8") as sink:
        sink.write("| モデル | 軸 | カタカナ語 | 英語 | 応答 | 空 | 失敗 | 候補(現れた応答の数) |\n|---|---|---|---|---:|---:|---:|---|\n")
        for key in sorted(samples, key=lambda k: (k[1], k[2], k[3], k[0])):
            model, axis, loanword, english = key
            listed = "、".join(
                f"{c}{'' if (c in known or not known) else '*'}({n})" for c, n in tallies[key].most_common(12)
            )
            sink.write(f"| {model} | {axis} | {loanword} | {english} | {samples[key]} | {empty[key]} | {errors[key]} | {listed} |\n")
    print(f"keys {len(samples)} responses {sum(samples.values())} empty {sum(empty.values())} errors {sum(errors.values())}")


if __name__ == "__main__":
    main()
