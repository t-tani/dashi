"""判定モデルが引く 2 つの表と、corpus-tool に渡す語の一覧を作る。表は珍しさの表と、対象の字の表である。

珍しさの表は、人の IT 文書の抽出の出現から、部分木の断片と組み合わせの鍵を、字ごとと意味分類
ごとに数えたものである。行は `<種類>\t<グループ>\t<字か分類>\t<鍵>\t<回数>` で、種類は次の 4 つである。

    wf  字と鍵の組の回数
    cf  意味分類(分類番号の先頭 4 字)と鍵の組の回数
    wn  字の出現数(グループと鍵は空)
    cn  意味分類の出現数(グループと鍵は空)

対象の字の表は、置き換え表の漢字 1 字の語ごとに、使うかどうかと外す理由を持つ。

corpus-tool に渡す語の一覧は、置き換え表の漢字 1 字の語と、それに対応する外来語である。外来語の
回数は、字の組を外来語の組と比べる特徴に使う。

使い方(single-kanji-noun/ の中で):
    uv run python pipeline/tables.py novelty --human <人の抽出の JSONL> --out <珍しさの表>
    uv run python pipeline/tables.py targets --counts <回数の表のディレクトリ> --out <対象の字の表>
    uv run python pipeline/tables.py words --out <語の一覧>
"""
import argparse
import csv
import io
import json
import re

import zstandard

import features as F

# 人の IT 文書で単独の名詞として 1 MB あたりこの回数以上出る字は、技術文書で本来の意味の用法が
# 多く、使い方の違いでは判定できないので対象から外す。
COMMON_PER_MB = 5.0
# 文書の種類によって普通に使う字。`癖` は文体を論じる文書で、`章` は本の中で使う。
GENRE_CHARS = {"癖": "文体を論じる文書で普通に使う", "章": "本の中で普通に使う"}


def open_text(path, mode):
    """`.zst` で終わるパスは zstd で圧縮して読み書きする。"""
    if path.endswith(".zst"):
        raw = open(path, mode + "b")
        if mode == "w":
            return _TextWriter(zstandard.ZstdCompressor(level=19).stream_writer(raw))
        return _TextReader(zstandard.ZstdDecompressor().stream_reader(raw))
    return open(path, mode, encoding="utf-8")


class _TextWriter:
    def __init__(self, stream):
        self.stream = stream

    def write(self, s):
        self.stream.write(s.encode("utf-8"))

    def close(self):
        self.stream.close()


class _TextReader:
    def __init__(self, stream):
        self.text = io.TextIOWrapper(stream, encoding="utf-8")

    def __iter__(self):
        return iter(self.text)

    def close(self):
        self.text.close()


def build_novelty(human_path, lex):
    nov = F.Novelty()
    for line in open(human_path, encoding="utf-8"):
        r = json.loads(line)
        if F.usable(r):
            nov.add(r["word"], lex.word_class(r["word"]), F.novelty_keys(r, lex))
    return nov


def write_novelty(nov, path):
    rows = []
    for g in F.NOVELTY_GROUPS:
        rows += [("wf", g, w, k, n) for (w, k), n in nov.wf[g].items()]
        rows += [("cf", g, c, k, n) for (c, k), n in nov.cf[g].items()]
    # 字と分類の出現数は、どのグループでも同じなので 1 度だけ書く。
    g0 = F.NOVELTY_GROUPS[0]
    rows += [("wn", "", w, "", n) for w, n in nov.wn[g0].items()]
    rows += [("cn", "", c, "", n) for c, n in nov.cn[g0].items()]
    out = open_text(path, "w")
    for r in sorted(rows):
        out.write("\t".join(map(str, r)) + "\n")
    out.close()


def read_novelty(path):
    nov = F.Novelty()
    src = open_text(path, "r")
    for line in src:
        kind, g, a, k, n = line.rstrip("\n").split("\t")
        n = int(n)
        if kind == "wf":
            nov.wf[g][(a, k)] = n
        elif kind == "cf":
            nov.cf[g][(a, k)] = n
        else:
            for g in F.NOVELTY_GROUPS:
                (nov.wn if kind == "wn" else nov.cn)[g][a] = n
    src.close()
    return nov


def write_targets(lex, counts_dir, substitution_table, path):
    stats = json.load(open(f"{counts_dir}/case-frame-stats.json", encoding="utf-8"))
    mb = stats["text_bytes"] / 1e6
    chars = sorted({r["word"] for r in csv.DictReader(open(substitution_table, encoding="utf-8"), delimiter="\t")
                    if re.fullmatch(r"[一-龥々]", r["word"])})
    with open(path, "w", encoding="utf-8") as out:
        out.write("char\tclass\thuman_per_mb\tloanwords\tuse\treason\n")
        for c in chars:
            rate = lex.Mw[c] / mb
            if lex.positional(c):
                use, reason = 0, "位置・関係の字"
            elif rate >= COMMON_PER_MB:
                use, reason = 0, f"人の IT 文書で 1 MB あたり {COMMON_PER_MB:g} 回以上"
            elif c in GENRE_CHARS:
                use, reason = 0, GENRE_CHARS[c]
            else:
                use, reason = 1, "-"
            out.write(f"{c}\t{lex.word_class(c)}\t{rate:.4f}\t{','.join(sorted(lex.loan.get(c, ())))}\t{use}\t{reason}\n")
    return len(chars)


def write_words(substitution_table, path):
    chars, loans = set(), set()
    for r in csv.DictReader(open(substitution_table, encoding="utf-8"), delimiter="\t"):
        if re.fullmatch(r"[一-龥々]", r["word"]):
            chars.add(r["word"])
            if r["loanword"]:
                loans.add(r["loanword"])
    with open(path, "w", encoding="utf-8") as out:
        for w in sorted(chars | loans):
            out.write(w + "\n")
    return len(chars), len(loans)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("table", choices=["novelty", "targets", "words"])
    ap.add_argument("--human")
    ap.add_argument("--counts")
    ap.add_argument("--out", required=True)
    ap.add_argument("--semantic-class", default="../semantic-class")
    ap.add_argument("--substitution-table", default="../loanword-substitution/substitution-table.tsv")
    a = ap.parse_args()
    if a.table == "words":
        print("字 %d、外来語 %d" % write_words(a.substitution_table, a.out))
        return
    lex = F.Lexicon(f"{a.semantic_class}/noun/semantic-class-table.tsv",
                    f"{a.semantic_class}/verb/verb-semantic-class-table.tsv", a.substitution_table, a.counts)
    if a.table == "novelty":
        write_novelty(build_novelty(a.human, lex), a.out)
    else:
        print(write_targets(lex, a.counts, a.substitution_table, a.out), "字")


if __name__ == "__main__":
    main()
