"""免除の語の一覧を作る。concrete-noun-misfit の指摘を後段で落とすのに使う。

作る一覧は 6 つある。

- `class48`: 44 分類の表で中項目 48(無形の人工物)を持つ語。
- `comp-any`: 対訳表で、どれかの英訳に計算機分野のタグが付く語。
- `comp-first`: 対訳表で、第 1 義の英訳に計算機分野のタグが付く語。
- `wordnet-abstract`: 対訳表の第 1 義の英訳が、Princeton WordNet で抽象の側の
  分類ファイルに載る名詞の語義を持つ語。
- `other-corpus`: 評価コーパス以外の 2 つの文書群で、既定の設定の検出器が
  指摘した語。
- `other-corpus-judged`: 上のうち、既存の裁定の記録で誤検出とされた語。

WordNet の分類ファイルの名前は、分類語彙表の部門に写せる。動作・認識・伝達・
属性・関係・状態・時間・量・出来事・感情・動機・過程は、関係と活動の部門に
当たる。人工物・物体・物質・動物・植物・身体・食物・人・集団は、生産物と自然物
の部門に当たる。前者を抽象の側とする。場所・所有・形状・自然現象は部門の対応が
割れるので、広い一覧のほうにだけ入れる。
"""

import argparse
import pathlib
import re

# WordNet の名詞の分類ファイルのうち、抽象の側に写すもの。
ABSTRACT_LEXNAMES = {
    "noun.act",
    "noun.attribute",
    "noun.cognition",
    "noun.communication",
    "noun.event",
    "noun.feeling",
    "noun.motive",
    "noun.process",
    "noun.quantity",
    "noun.relation",
    "noun.state",
    "noun.time",
}
# 部門の対応が割れるもの。広い一覧にだけ足す。
WIDE_LEXNAMES = {"noun.location", "noun.possession", "noun.shape", "noun.phenomenon"}


def class48(path):
    """44 分類の表から、中項目 48 を持つ語を返す。"""
    words = set()
    for line in open(path, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        parts = line.rstrip("\n").split("\t")
        if "1.4800" in parts[1:]:
            words.add(parts[0])
    return words


def gloss_rows(path):
    """対訳表の行を (見出し語, 分野タグ, 第 1 義の数, 英訳の列) で返す。"""
    for line in open(path, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        f = line.rstrip("\n").split("\t")
        if len(f) < 7:
            continue
        try:
            first = int(f[4])
        except ValueError:
            continue
        yield f[0], f[3], first, f[6:]


def comp_tagged(path, first_only):
    """計算機分野のタグが付く語を返す。`first_only` なら第 1 義に限る。"""
    words = set()
    for word, tags, first, _ in gloss_rows(path):
        if tags == "-":
            continue
        for item in tags.split(","):
            at, _, names = item.partition(":")
            if "comp" not in names.split(";"):
                continue
            if not first_only or int(at) < first:
                words.add(word)
                break
    return words


def load_lexnames(dict_dir):
    """WordNet の分類ファイルの番号から名前への対応を返す。"""
    names = {}
    for line in open(pathlib.Path(dict_dir) / "lexnames", encoding="utf-8"):
        number, name, _ = line.split("\t")
        names[int(number)] = name
    return names


def load_noun_lexnames(dict_dir):
    """WordNet の名詞の見出しから、その語義が載る分類ファイルの名前の集合を返す。"""
    names = load_lexnames(dict_dir)
    at_offset = {}
    for line in open(pathlib.Path(dict_dir) / "data.noun", encoding="utf-8", errors="replace"):
        if line.startswith("  "):
            continue
        f = line.split(" ", 2)
        at_offset[f[0]] = names[int(f[1])]
    lemma_names = {}
    for line in open(pathlib.Path(dict_dir) / "index.noun", encoding="utf-8", errors="replace"):
        if line.startswith("  "):
            continue
        f = line.split()
        lemma, count = f[0], int(f[2])
        offsets = f[-count:]
        lemma_names[lemma] = {at_offset[o] for o in offsets if o in at_offset}
    return lemma_names


def wordnet_abstract(gloss_path, dict_dir, wide):
    """第 1 義の英訳が抽象の側の分類ファイルに載る語を返す。"""
    lemma_names = load_noun_lexnames(dict_dir)
    target = ABSTRACT_LEXNAMES | (WIDE_LEXNAMES if wide else set())
    words = set()
    for word, _, first, glosses in gloss_rows(gloss_path):
        for gloss in glosses[:first]:
            lemma = gloss.strip().replace(" ", "_")
            if lemma_names.get(lemma, set()) & target:
                words.add(word)
                break
    return words


QUOTED = re.compile(r"^'([^']+)'")


def flagged_words(paths):
    """lint の JSON から、concrete-noun-misfit が指摘した語を返す。"""
    import json

    words = set()
    for path in paths:
        for f in json.load(open(path, encoding="utf-8"))["findings"]:
            if f["rule"] != "concrete-noun-misfit":
                continue
            m = QUOTED.match(f["message"])
            if m:
                words.add(m.group(1))
            if f.get("excerpt"):
                words.add(f["excerpt"])
    return words


def write(path, words):
    pathlib.Path(path).write_text("\n".join(sorted(words)) + "\n", encoding="utf-8")
    print(f"{path}: {len(words)} 語")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--table48", required=True)
    parser.add_argument("--gloss", required=True)
    parser.add_argument("--wordnet-dict", required=True)
    parser.add_argument("--other-corpus-findings", nargs="+", required=True)
    parser.add_argument("--out-dir", required=True)
    args = parser.parse_args()
    out = pathlib.Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    write(out / "class48.txt", class48(args.table48))
    write(out / "comp-any.txt", comp_tagged(args.gloss, first_only=False))
    write(out / "comp-first.txt", comp_tagged(args.gloss, first_only=True))
    write(out / "wordnet-abstract.txt", wordnet_abstract(args.gloss, args.wordnet_dict, wide=False))
    write(out / "wordnet-abstract-wide.txt", wordnet_abstract(args.gloss, args.wordnet_dict, wide=True))
    write(out / "other-corpus.txt", flagged_words(args.other_corpus_findings))


if __name__ == "__main__":
    main()
