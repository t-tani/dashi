"""単独で立つ漢字 1 字の名詞の判定モデルを学び、JSON に書き出す。

正例は akunuki のリポジトリの文章で、直すと判定した字の出現である。負例は人の IT 文書の抽出の
出現と、人の技術書の出現である。位置・関係の字(分類番号 1.17)は、どちらからも外す。

得点の線は、人の技術書を 1 冊ずつ除いて学んだモデルで除いた本を採点し、9 冊分の得点を
高い順に並べて、技術書 1 MB あたり 2 件目、5 件目、10 件目、20 件目の得点に置く。

使い方(single-kanji-noun/ の中で):
    uv run python pipeline/train.py --human <人の抽出の JSONL> --repo <リポジトリの JSONL> \\
        --techbook <人の技術書の JSONL> --techbook-bytes <技術書の本文のバイト数> \\
        --counts <回数の表のディレクトリ> --novelty <珍しさの表> --out <モデルの JSON>
JSONL はどれも corpus-tool の extract-subtrees が書いたものである。
"""
import argparse
import json
import math
import re

import lightgbm as lgb
import numpy as np
import pandas as pd

import features as F
import model as M
import tables

BOOKS = [
    "build-web-application-with-golang", "Go-SCP-jaJP", "Hatena-Textbook",
    "Introduction-to-Addon-Development-in-Blender-Web", "JavaScript-Plugin-Architecture", "js-primer",
    "progit", "The-Little-Book-on-CoffeeScript", "what-is-maven",
]
LINES_PER_MB = (2, 5, 10, 20)
PARAMS = dict(objective="binary", feature_fraction=0.8, bagging_fraction=0.8, bagging_freq=1, lambda_l2=1.0,
              verbose=-1, seed=1643, deterministic=True, force_col_wise=True, num_threads=8)
# 2 つのブースター(葉の数・学習率・木の数)と、得点へ足す重み。
BOOSTERS = [(dict(num_leaves=7, learning_rate=0.03, min_child_samples=20), 300, 0.7),
            (dict(num_leaves=31, learning_rate=0.1, min_child_samples=20), 800, 0.3)]
WEEKDAYS = set("月火水木金土日")


def center(e, w):
    """前後を切り出した本文の中で、中心の語の位置(文字)。切り出しは語の前 120 バイトから始まる。"""
    t = len(e.encode()[:120].decode("utf-8", "ignore"))
    ps = [i for i in range(len(e)) if e.startswith(w, i)]
    return min(ps, key=lambda i: abs(i - t)) if ps else None


def char_list(e, j):
    """`(全・各・本・再・非)` のように、括弧の中に 1 字の漢字だけを 4 つ以上「・」で並べた字の言及か。"""
    o = max(e.rfind("(", 0, j), e.rfind("（", 0, j))
    c = [x for x in (e.find(")", j), e.find("）", j)) if x >= 0]
    if o < 0 or not c:
        return False
    items = e[o + 1:min(c)].split("・")
    return len(items) >= 4 and all(re.fullmatch(r"[一-龥]", x.strip()) for x in items)


def excluded(r):
    """人の技術書の負例と線から外す出現。語が見つからない、ひらがなを含まない文、日付の後と
    見出しの曜日、1 字の漢字の列挙である。"""
    e, w = r["excerpt"], r["word"]
    j = center(e, w)
    if j is None:
        return True
    a = max(e.rfind("。", 0, j), e.rfind("\n", 0, j)) + 1
    b = [x for x in (e.find("。", j), e.find("\n", j)) if x >= 0]
    b = min(b) if b else len(e)
    if not re.search(r"[ぁ-ゟ]", e[a:b]):
        return True
    if w in WEEKDAYS and (re.search(r"\d+\s*[(（]\s*$", e[:j]) and re.match(r"\s*[)）]", e[j + 1:])
                          or re.match(r"^\s*[-*・]?\s*$", e[a:j]) and re.match(r"\s*[:：]", e[j + 1:])):
        return True
    return char_list(e, j)


def book_of(path):
    name = path.rsplit("/", 1)[-1]
    return next((b for b in BOOKS if name.startswith(b)), None)


def read(path):
    return [r for r in (json.loads(line) for line in open(path, encoding="utf-8")) if F.usable(r)]


def frame(rows, levels):
    df = pd.DataFrame(rows, columns=F.FEATURES)
    for c in F.CATEGORICAL:
        df[c] = pd.Categorical(df[c].astype(str), categories=levels[c])
    return df


def fit(pos, human, extra):
    """正例・人の抽出・追加の負例で 2 つのブースターを学び、得点の関数とブースターを返す。"""
    neg = pd.concat([human, extra], ignore_index=True)
    X = pd.concat([pos, neg], ignore_index=True)
    y = np.r_[np.ones(len(pos)), np.zeros(len(neg))]
    ds = lgb.Dataset(X, y, weight=np.where(y == 1, len(neg) / len(pos), 1.0), categorical_feature=F.CATEGORICAL,
                     free_raw_data=False)
    parts = []
    for params, rounds, weight in BOOSTERS:
        b = lgb.train(dict(PARAMS, **params), ds, rounds)
        h = b.predict(human, raw_score=True)
        parts.append((b, weight, float(h.mean()), float(h.std())))
    return (lambda D: sum(w * (b.predict(D, raw_score=True) - m) / s for b, w, m, s in parts)), parts


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    for name in ("human", "repo", "techbook", "counts", "novelty", "out"):
        ap.add_argument(f"--{name}", required=True)
    ap.add_argument("--techbook-bytes", type=int, required=True)
    ap.add_argument("--oob", help="1 冊を除いて学んだモデルで付けた人の技術書の得点を書く JSONL(評価に使う)")
    ap.add_argument("--fix-chars", default="fix-chars.txt")
    ap.add_argument("--semantic-class", default="../semantic-class")
    ap.add_argument("--substitution-table", default="../loanword-substitution/substitution-table.tsv")
    a = ap.parse_args()
    lex = F.Lexicon(f"{a.semantic_class}/noun/semantic-class-table.tsv",
                    f"{a.semantic_class}/verb/verb-semantic-class-table.tsv", a.substitution_table, a.counts)
    nov = tables.read_novelty(a.novelty)
    fix = {line.strip() for line in open(a.fix_chars, encoding="utf-8") if line.strip() and not line.startswith("#")}

    human = [r for r in read(a.human) if not lex.positional(r["word"])]
    pos = [r for r in read(a.repo) if r["word"] in fix and not lex.positional(r["word"])]
    tb = [r for r in read(a.techbook) if not lex.positional(r["word"])]
    if any(book_of(r["file"]) is None for r in tb):
        raise SystemExit("人の技術書の出現に、本の名前が分からないファイルがある")
    Xh = [F.row(r, lex, nov, own=1) for r in human]
    Xp = [F.row(r, lex, nov) for r in pos]
    Xt = [F.row(r, lex, nov) for r in tb]
    levels = {c: sorted({str(d[c]) for d in Xh + Xp + Xt}) for c in F.CATEGORICAL}
    Dh, Dp, Dt = frame(Xh, levels), frame(Xp, levels), frame(Xt, levels)
    ok = np.array([not excluded(r) for r in tb])
    book = np.array([book_of(r["file"]) for r in tb])
    print(f"正例 {len(pos)}(字 {len({r['word'] for r in pos})})、人の抽出 {len(human)}、人の技術書 {len(tb)}"
          f"(負例と線に使う {int(ok.sum())})", flush=True)

    oob = np.zeros(len(tb))
    for bk in BOOKS:
        f, _ = fit(Dp, Dh, Dt[(book != bk) & ok])
        m = book == bk
        oob[m] = f(Dt[m])
    if a.oob:
        with open(a.oob, "w", encoding="utf-8") as w:
            for r, s, k in zip(tb, oob, ok):
                w.write(json.dumps(dict(file=r["file"], excerpt=r["excerpt"], word=r["word"], score=float(s), used=bool(k)),
                                   ensure_ascii=False) + "\n")
    mb = a.techbook_bytes / 1e6
    order = np.sort(oob[ok])[::-1]
    lines = {str(d): float(order[int(d * mb)]) for d in LINES_PER_MB}

    f, parts = fit(Dp, Dh, Dt[ok])
    out = dict(features=F.FEATURES, categorical=levels, lines=lines, boosters=[
        dict(weight=w, mean=m, std=s, tree_info=b.dump_model()["tree_info"]) for b, w, m, s in parts])
    # 書き出した木を JSON のまま採点し、LightGBM の得点と一致することを確かめる。
    want = f(Dt)
    got = np.array([M.score(x, out) for x in Xt])
    worst = float(np.max(np.abs(want - got)))
    if not worst < 1e-9:
        raise SystemExit(f"書き出したモデルの得点が LightGBM と {worst} ずれる")
    with open(a.out, "w", encoding="utf-8") as w:
        json.dump(out, w, ensure_ascii=False, separators=(",", ":"))
    print("線", {k: round(v, 4) for k, v in lines.items()}, "書き出しの誤差", worst)


if __name__ == "__main__":
    main()
