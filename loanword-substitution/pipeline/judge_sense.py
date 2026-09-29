"""出現ごとに、書き手が使っている意味を決め、その意味を語が日本語で持つかを判定する。

表の行 (語, カタカナ語, 英語) は語が覆う意味の 1 つである。`段` なら `段`←コラム(段組み)、
`段`←ステップ(処理の順)、`段`←ロー(行)が並ぶ。判定は 2 つを見る。

1. 適合。出現の文脈が、どの行のカタカナ語の文脈にいちばん合うか。文脈の語ごとの対数確率の平均で
   測る。和にすると文脈の語数で大きくなるので平均にする。
2. 裏づけ。その意味を、語が一般の日本語で持つか。カタカナ語の文脈と、一般での語の文脈の
   コサインで測る。`層` はレイヤーの意味を一般でも持つが、`段` はステップの意味を持たない。

適合が語自身の一般の用法より高く、かつ裏づけが低い出現を、置き換えとして報告する。
"""
import argparse
import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path


def read_profiles(directories, wanted):
    """語ごとの文脈の回数と、語の出現数を返す。`wanted` の語だけを読む。"""
    profiles = defaultdict(Counter)
    totals = Counter()
    for directory in directories:
        with (Path(directory) / "cooccurrence-counts.tsv").open(encoding="utf-8") as source:
            for line in source:
                key, count = line.rstrip("\n").rsplit("\t", 1)
                word, _, context = key.partition("\t")
                if word in wanted:
                    profiles[word][context] += int(count)
        with (Path(directory) / "cooccurrence-targets.tsv").open(encoding="utf-8") as source:
            for line in source:
                word, count = line.rstrip("\n").split("\t")
                if word in wanted:
                    totals[word] += int(count)
    return profiles, totals


def cosine(a, b):
    keys = set(a) & set(b)
    if not keys:
        return 0.0
    norm_a = math.sqrt(sum(v * v for v in a.values()))
    norm_b = math.sqrt(sum(v * v for v in b.values()))
    if not norm_a or not norm_b:
        return 0.0
    return sum(a[k] * b[k] for k in keys) / (norm_a * norm_b)


class Judge:
    def __init__(self, field, general, vocabulary, smoothing):
        self.field = field
        self.general = general
        self.vocabulary = vocabulary
        self.smoothing = smoothing
        self.field_totals = {w: sum(c.values()) for w, c in field.items()}
        self.general_totals = {w: sum(c.values()) for w, c in general.items()}
        self.support_cache = {}

    def mean_log_probability(self, profiles, totals, word, context_words):
        """文脈の語ごとの対数確率の平均。文脈が空なら None を返す。"""
        if not context_words:
            return None
        total = totals.get(word, 0)
        contexts = profiles.get(word, {})
        return sum(
            math.log(
                (contexts.get(c, 0) + self.smoothing)
                / (total + self.smoothing * self.vocabulary)
            )
            for c in context_words
        ) / len(context_words)

    def support(self, word, loanword):
        """その意味を語が一般の日本語で持つか。カタカナ語の分野の文脈と、語の一般の文脈の近さ。"""
        key = (word, loanword)
        if key not in self.support_cache:
            self.support_cache[key] = cosine(
                self.field.get(loanword, {}), self.general.get(word, {})
            )
        return self.support_cache[key]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--field", nargs="+", required=True)
    parser.add_argument("--general", nargs="+", required=True)
    parser.add_argument("--pairs", required=True, help="列は 語、カタカナ語、英語")
    parser.add_argument("--occurrences", required=True, help="extract-occurrences の JSONL")
    parser.add_argument("--min-loanword-occurrences", type=int, default=50)
    parser.add_argument("--vocabulary", type=int, default=51313)
    parser.add_argument("--smoothing", type=float, default=0.5)
    parser.add_argument("--fit-margin", type=float, default=0.0, help="適合が語自身の用法を上回る差の下限")
    parser.add_argument("--max-support", type=float, default=1.0, help="裏づけの上限")
    args = parser.parse_args()
    senses = defaultdict(list)
    wanted = set()
    with open(args.pairs, encoding="utf-8") as source:
        for line in source:
            fields = line.rstrip("\n").split("\t")
            word, loanword = fields[0], fields[1]
            english = fields[2] if len(fields) > 2 else ""
            senses[word].append((loanword, english))
            wanted.update((word, loanword))
    field, field_totals = read_profiles(args.field, wanted)
    general, general_totals = read_profiles(args.general, wanted)
    judge = Judge(field, general, args.vocabulary, args.smoothing)
    judged = flagged = skipped = 0
    for line in Path(args.occurrences).read_text(encoding="utf-8").splitlines():
        record = json.loads(line)
        word, context = record["word"], record["context"]
        own = judge.mean_log_probability(general, general_totals, word, context)
        if own is None:
            skipped += 1
            continue
        best = None
        for loanword, english in senses[word]:
            if field_totals.get(loanword, 0) < args.min_loanword_occurrences:
                continue
            fit = judge.mean_log_probability(field, field_totals, loanword, context)
            if fit is not None and (best is None or fit > best[2]):
                best = (loanword, english, fit)
        if best is None:
            skipped += 1
            continue
        loanword, english, fit = best
        support = judge.support(word, loanword)
        judged += 1
        hit = fit - own > args.fit_margin and support < args.max_support
        flagged += hit
        print(json.dumps({**record, "loanword": loanword, "english": english,
                          "fit": round(fit, 3), "own": round(own, 3),
                          "margin": round(fit - own, 3), "support": round(support, 3),
                          "flagged": hit}, ensure_ascii=False))
    print(f"判定 {judged} 件、報告 {flagged} 件、判定しない {skipped} 件", file=sys.stderr)


if __name__ == "__main__":
    main()
