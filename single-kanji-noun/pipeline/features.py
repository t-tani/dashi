"""単独で立つ漢字 1 字の名詞の出現 1 件から、判定モデルの特徴 89 列を作る。

入力の出現は corpus-tool の extract-subtrees が書く 1 行で、係り受けの部分木と、名詞へ係る
修飾と、格と述語の組を持つ。字そのものは特徴に入れない。字は、人の IT 文書での回数と、
同じ字の人の用法を引く鍵にだけ使う。

特徴は 3 つのまとまりからなる。1 つ目は部分木の形で、中心の文節とその親、祖父母、子、兄弟、
並びの前後の文節の性質である。2 つ目は人の IT 文書での修飾と格と述語の組の回数、3 つ目は
同じ字の人の用法と比べた部分木の断片の珍しさである。akunuki の検出器は同じ計算を Rust で行うので、式を変えたら両方を直す。
"""
import collections
import csv
import math
import re

# 珍しさの式で、同じ意味分類の字の割合に与える重み(疑似の出現数)。
ALPHA = 20.0
# 部分木の中の文節で、並び順の隣というだけで含めた文節の距離。
SEQUENCE_ONLY = 10**6
# 断片の最大の文節数。
MAX_FRAGMENT = 4
# 数えない字。文法を説明する文書で普通に使う字である。
GRAMMAR = set("語文節形句格")

FEATURES = [
    "c_func", "c_rel", "c_nkids", "c_root", "c_kid_rels", "p_pos", "p_top", "p_mid", "p_func", "p_rel",
    "gp_pos", "gp_top", "n_sibs", "sib_rels", "sib_tops", "prev_pos", "prev_func", "next_pos", "next_func",
    "m_kind", "m_top", "m_mid", "m_cnt", "m_share", "m_freq", "m_exp", "m_loan",
    "f_case", "f_cnt", "f_share", "f_pfreq", "f_exp", "f_loan", "f_vtop", "f_vmid",
] + [f"nov_T{n}_{lv}_{s}" for n in (2, 3, 4) for lv in ("cls", "pos") for s in ("mean", "max", "zero")]
COMBOS = [
    "格・述語分類", "格・述語大分類・述語機能語", "修飾の種類・修飾語分類", "格・述語分類・兄弟の関係",
    "中心の関係・係り元の関係", "前後の機能語・格", "修飾語分類・格・述語分類",
]
FAMILIES = ["frag:cls", "frag:top", "frag:pos", "frag:func", "frag:rel"] + ["combo:" + k for k in COMBOS]
FEATURES += [f"n2_{fam.replace(':', '_')}_{s}" for fam in FAMILIES for s in ("mean", "max", "zero")]
CATEGORICAL = [
    "c_func", "c_rel", "c_kid_rels", "p_pos", "p_top", "p_mid", "p_func", "p_rel", "gp_pos", "gp_top",
    "sib_rels", "sib_tops", "prev_pos", "prev_func", "next_pos", "next_func", "m_kind", "m_top", "m_mid",
    "f_case", "f_vtop", "f_vmid",
]
NAN = float("nan")


class Lexicon:
    """意味分類表と外来語の置き換え表と、人の IT 文書での修飾・格と述語の組の回数。"""

    def __init__(self, noun_table, verb_table, substitution_table, counts_dir):
        # 名詞は全部の分類番号を持つ表と、先頭の分類番号の 4 字だけを持つ表の 2 つを使う。
        self.noun_codes, self.noun4, self.verb4 = {}, {}, {}
        for path, full, short in ((noun_table, self.noun_codes, self.noun4), (verb_table, None, self.verb4)):
            for line in open(path, encoding="utf-8"):
                if line.startswith("#"):
                    continue
                p = line.rstrip("\n").split("\t")
                if full is not None:
                    full[p[0]] = p[1:]
                short[p[0]] = p[1][:4] if len(p) > 1 else "?"
        self.loan = collections.defaultdict(set)
        for r in csv.DictReader(open(substitution_table, encoding="utf-8"), delimiter="\t"):
            if re.fullmatch(r"[一-龥々]", r["word"]) and r["loanword"]:
                self.loan[r["word"]].add(r["loanword"])
        self.M, self.Mw, self.F = collections.Counter(), collections.Counter(), collections.Counter()
        for line in open(f"{counts_dir}/noun-modifier-counts.tsv", encoding="utf-8"):
            w, k, m, n = line.rstrip("\n").split("\t")
            self.M[(w, k, m)] += int(n)
            self.Mw[w] += int(n)
        for line in open(f"{counts_dir}/case-frame-counts.tsv", encoding="utf-8"):
            w, c, v, n = line.rstrip("\n").split("\t")
            self.F[(w, c, v)] += int(n)
        self.Fwc, self.Fcv, self.Fc = collections.Counter(), collections.Counter(), collections.Counter()
        for (w, c, v), n in self.F.items():
            self.Fwc[(w, c)] += n
            self.Fcv[(c, v)] += n
            self.Fc[c] += n
        self.Mwk, self.Mkm, self.Mk = collections.Counter(), collections.Counter(), collections.Counter()
        for (w, k, m), n in self.M.items():
            self.Mwk[(w, k)] += n
            self.Mkm[(k, m)] += n
            self.Mk[k] += n

    def cls_of(self, node):
        """文節の主辞の意味分類の 4 字。表に無ければ品詞。"""
        if node["pos"].startswith(("動詞", "形容詞")):
            return self.verb4.get(node["lemma"], node["pos"])
        return self.noun4.get(node["lemma"], node["pos"])

    def word_class(self, w):
        """字の先頭の分類番号の 4 字。珍しさで同じ分類の字へ後退するときの鍵である。"""
        return (self.noun_codes.get(w) or ["-"])[0][:4]

    def positional(self, w):
        """位置・関係(分類番号 1.17)の字か。"""
        return any(c[:4] == "1.17" for c in self.noun_codes.get(w, []))


def top(c):
    """分類番号の部門(`1.4`)。分類番号でなければ `-`。"""
    return c[:3] if isinstance(c, str) and re.match(r"^\d\.\d", c) else "-"


def mid(c):
    """分類番号の中項目(`1.45`)。分類番号でなければ値をそのまま返す。"""
    return c[:4] if isinstance(c, str) and re.match(r"^\d\.\d\d", c) else (c if isinstance(c, str) else "-")


def top_or_value(c):
    """分類番号の部門。分類番号でなければ値をそのまま返す(断片と組み合わせの鍵で使う)。"""
    return c[:3] if isinstance(c, str) and re.match(r"^\d\.\d", c) else c


def lg(x):
    return math.log10(x + 1)


def is_mention(r):
    """中心の語が鉤括弧の中にある出現(語への言及)か。"""
    e, w = r["excerpt"], r["word"]
    j = e.find(w)
    if j < 0:
        return False
    before, after = e[:j], e[j + len(w):]
    return before.count("「") > before.count("」") and after.find("」") >= 0 and (
        after.find("「") < 0 or after.find("」") < after.find("「"))


def usable(r):
    """学習と表に使う出現か。漢字 1 字の語で、文法の字でなく、例文と辞書とテストの外にあり、言及でない。

    書き出しには外来語も入り、`ア` のような 1 字のカタカナもあるので、字の種類まで見る。"""
    f = r["file"]
    return (re.fullmatch(r"[一-龥々]", r["word"]) is not None and r["word"] not in GRAMMAR
            and not any(x in f for x in ("/demo/rules", "/tests/", "/dict/")) and not is_mention(r))


def shape(r, lex):
    """部分木の形の特徴。"""
    nodes = r["nodes"]
    by = {n["id"]: n for n in nodes}
    c = nodes[0]
    kids = [n for n in nodes if n["parent"] == 0]
    par = by.get(c["parent"]) if c["parent"] is not None else None
    gp = by.get(par["parent"]) if par and par["parent"] is not None else None
    sibs = [n for n in nodes if par and n["parent"] == par["id"] and n["id"] != 0]
    bypos = {n["position"]: n for n in nodes}
    prv, nxt = bypos.get(c["position"] - 1), bypos.get(c["position"] + 1)
    return dict(
        c_func=c["function"], c_rel=c["relation"], c_nkids=len(kids), c_root=int(c["parent"] is None),
        c_kid_rels="|".join(sorted({k["relation"] for k in kids})) or "-",
        p_pos=par["pos"] if par else "-", p_top=top(lex.cls_of(par)) if par else "-",
        p_mid=mid(lex.cls_of(par)) if par else "-", p_func=par["function"] if par else "-",
        p_rel=par["relation"] if par else "-",
        gp_pos=gp["pos"] if gp else "-", gp_top=top(lex.cls_of(gp)) if gp else "-",
        n_sibs=len(sibs), sib_rels="|".join(sorted({s["relation"] for s in sibs})) or "-",
        sib_tops="|".join(sorted({top(lex.cls_of(s)) for s in sibs})) or "-",
        prev_pos=prv["pos"] if prv else "-", prev_func=prv["function"] if prv else "-",
        next_pos=nxt["pos"] if nxt else "-", next_func=nxt["function"] if nxt else "-",
    )


def loan_ratio(lex, w, table, a, b, cnt):
    """置き換え表で字に対応する外来語が同じ組を取る率と、字が取る率の、対数の差の最大。"""
    best = None
    for L in lex.loan.get(w, ()):
        cl = lex.M[(L, a, b)] if table == "m" else lex.F[(L, a, b)]
        if cl < 3:
            continue
        x = math.log((cl + .5) / (lex.Mw[L] + 1)) - math.log((cnt + .5) / (lex.Mw[w] + 1))
        best = x if best is None else max(best, x)
    return best if best is not None else NAN


def counts(r, lex, own):
    """人の IT 文書での修飾と格と述語の組の回数の特徴。`own` は人の抽出の出現で 1 にし、
    自分の分を数から引く。"""
    w = r["word"]
    d = {}
    kind = r["kind"] or "-"
    d["m_kind"] = kind
    mo = r["modifier"]
    if kind in ("の", "連体", "連用") and mo:
        cnt = max(0, lex.M[(w, kind, mo)] - own)
        mc = (lex.noun_codes.get(mo) or lex.noun_codes.get(mo[-2:]) or lex.noun_codes.get(mo[-1:])
              or ([lex.verb4[mo]] if mo in lex.verb4 else ["-"]))
        d.update(m_top=top(mc[0]), m_mid=mid(mc[0]), m_cnt=lg(cnt), m_share=cnt / max(1, lex.Mw[w] - own),
                 m_freq=lg(lex.Mkm[(kind, mo)]),
                 m_exp=lg(lex.Mwk[(w, kind)] * lex.Mkm[(kind, mo)] / max(1, lex.Mk[kind])),
                 m_loan=loan_ratio(lex, w, "m", kind, mo, cnt))
    else:
        d.update(m_top="-", m_mid="-", m_cnt=NAN, m_share=NAN, m_freq=NAN, m_exp=NAN, m_loan=NAN)
    if r["case"]:
        cs, pv = r["case"], r["predicate"]
        cnt = max(0, lex.F[(w, cs, pv)] - own)
        d.update(f_case=cs, f_cnt=lg(cnt), f_share=cnt / max(1, lex.Mw[w] - own), f_pfreq=lg(lex.Fcv[(cs, pv)]),
                 f_exp=lg(lex.Fwc[(w, cs)] * lex.Fcv[(cs, pv)] / max(1, lex.Fc[cs])),
                 f_loan=loan_ratio(lex, w, "f", cs, pv, cnt),
                 f_vtop=top(lex.verb4.get(pv, "-")), f_vmid=mid(lex.verb4.get(pv, "-")))
    else:
        d.update(f_case="-", f_cnt=NAN, f_share=NAN, f_pfreq=NAN, f_exp=NAN, f_loan=NAN, f_vtop="-", f_vmid="-")
    return d


def fragments(r, label):
    """中心を含む連結な文節の集合(最大 4 文節)を、`label` で節点を書いた文字列にする。

    並び順の隣というだけで含めた文節と、その文節への辺は使わない。辺には子の関係ラベルを
    書く。返すのは、文節数を `T<数>:` の形で頭に付けた文字列の集合である。
    """
    nodes = [n for n in r["nodes"] if n["distance"] is not None and n["distance"] < SEQUENCE_ONLY]
    by = {n["id"]: n for n in nodes}
    adj = {n["id"]: set() for n in nodes}
    for n in nodes:
        if n["parent"] is not None and n["parent"] in by:
            adj[n["id"]].add(n["parent"])
            adj[n["parent"]].add(n["id"])
    seen, frontier = set(), [frozenset([0])]
    while frontier:
        s = frontier.pop()
        if s in seen:
            continue
        seen.add(s)
        if len(s) < MAX_FRAGMENT:
            for x in s:
                for y in adj[x]:
                    if y not in s:
                        frontier.append(s | {y})
    return {f"T{len(s)}:" + serialize(by, s, 0, None, label) for s in seen}


def serialize(by, members, at, came_from, label):
    n = by[at]
    parts = []
    p = n["parent"]
    if p is not None and p in members and p != came_from:
        parts.append(f"↑{n['relation']}:" + serialize(by, members, p, at, label))
    for m in sorted(members):
        c = by[m]
        if c["parent"] == at and m != came_from:
            parts.append(f"↓{c['relation']}:" + serialize(by, members, m, at, label))
    return label(n, at == 0) + ("(" + ",".join(sorted(parts)) + ")" if parts else "")


def novelty_keys(r, lex):
    """珍しさの鍵。`nov:<大きさ>:<水準>` の鍵は中心を字の意味分類で書き、`frag:<水準>` と
    `combo:<型>` の鍵は中心を伏せる。"""
    w = r["word"]
    keys = collections.defaultdict(set)
    center_cls = lex.noun4.get(w, "?")
    for lv in ("cls", "pos"):
        def lab(n, center, lv=lv):
            if center:
                return f"◎{center_cls}/{n['function']}"
            return f"{lex.cls_of(n) if lv == 'cls' else n['pos']}/{n['function']}"
        for f in fragments(r, lab):
            keys[f"nov:{f[:2]}:{lv}"].add(f"{f[:3]}{lv}:{f[3:]}")
    hidden = {
        "cls": lambda n: f"{lex.cls_of(n)}/{n['function']}",
        "pos": lambda n: f"{n['pos']}/{n['function']}",
        "top": lambda n: f"{top_or_value(lex.cls_of(n))}/{n['function']}",
        "func": lambda n: f"·/{n['function']}",
        "rel": lambda n: "·",
    }
    for lv, fn in hidden.items():
        prefix = "pos" if lv == "pos" else "cls"
        for f in fragments(r, lambda n, center, fn=fn: f"◎/{n['function']}" if center else fn(n)):
            if not f.startswith("T1:"):
                keys[f"frag:{lv}"].add(f"{f[:3]}{prefix}:{f[3:]}")
    for k, v in combo_keys(r, lex).items():
        keys["combo:" + k] = {v}
    return keys


def combo_keys(r, lex):
    nodes = r["nodes"]
    by = {n["id"]: n for n in nodes}
    c = nodes[0]
    par = by.get(c["parent"]) if c["parent"] is not None else None
    kids = [n for n in nodes if n["parent"] == 0]
    sibs = [n for n in nodes if par and n["parent"] == par["id"] and n["id"] != 0]
    bypos = {n["position"]: n for n in nodes}
    prv, nxt = bypos.get(c["position"] - 1), bypos.get(c["position"] + 1)
    mods = [n for n in kids if n["position"] < c["position"]]
    md = mods[-1] if mods else None
    mid_or_value = lambda x: x[:4] if isinstance(x, str) and re.match(r"^\d\.\d\d", x) else x
    pm = mid_or_value(lex.cls_of(par)) if par else "-"
    pt = top_or_value(lex.cls_of(par)) if par else "-"
    sr = "|".join(sorted({s["relation"] for s in sibs})) or "-"
    mdc = mid_or_value(lex.cls_of(md)) if md else "-"
    return {
        "格・述語分類": f"{c['function']}>{pm}",
        "格・述語大分類・述語機能語": f"{c['function']}>{pt}/{par['function'] if par else '-'}",
        "修飾の種類・修飾語分類": f"{md['function'] if md else '-'}:{mdc}",
        "格・述語分類・兄弟の関係": f"{c['function']}>{pm}[{sr}]",
        "中心の関係・係り元の関係": f"{c['relation']}<{'|'.join(sorted({k['relation'] for k in kids})) or '-'}",
        "前後の機能語・格": f"{prv['function'] if prv else '^'}_{c['function']}_{nxt['function'] if nxt else '$'}",
        "修飾語分類・格・述語分類": f"{mdc}>{c['function']}>{pm}",
    }


NOVELTY_GROUPS = [f"nov:T{n}:{lv}" for n in (2, 3, 4) for lv in ("cls", "pos")] + FAMILIES


class Novelty:
    """人の抽出の出現から数えた、字ごとと意味分類ごとの鍵の回数。"""

    def __init__(self):
        self.wf = {g: collections.Counter() for g in NOVELTY_GROUPS}
        self.cf = {g: collections.Counter() for g in NOVELTY_GROUPS}
        self.wn = {g: collections.Counter() for g in NOVELTY_GROUPS}
        self.cn = {g: collections.Counter() for g in NOVELTY_GROUPS}

    def add(self, w, c, keys):
        for g in NOVELTY_GROUPS:
            # 断片の大きさごとの字の出現数は、どの大きさでも出現 1 件につき 1 足す。
            self.wn[g][w] += 1
            self.cn[g][c] += 1
            for k in keys.get(g, ()):
                self.wf[g][(w, k)] += 1
                self.cf[g][(c, k)] += 1

    def features(self, w, c, keys, own):
        out = {}
        for g in NOVELTY_GROUPS:
            wf, cf, wn, cn = self.wf[g], self.cf[g], self.wn[g], self.cn[g]
            vals, zero = [], 0
            for k in keys.get(g, ()):
                pc = (cf[(c, k)] - own + 0.5) / (cn[c] - own + 1)
                num = wf[(w, k)] - own + ALPHA * pc
                if g.startswith("nov:"):
                    vals.append(-math.log(num / (wn[w] - own + ALPHA)))
                else:
                    vals.append(-math.log(max(1e-9, num) / (wn[w] - own + ALPHA)))
                zero += (wf[(w, k)] - own) <= 0
            if g.startswith("nov:"):
                name = "nov_" + g[4:].replace(":", "_")
            else:
                name = "n2_" + g.replace(":", "_")
            out[f"{name}_mean"] = sum(vals) / len(vals) if vals else NAN
            out[f"{name}_max"] = max(vals) if vals else NAN
            out[f"{name}_zero"] = zero / len(vals) if vals else NAN
        return out


def row(r, lex, nov, own=0, keys=None):
    """出現 1 件の特徴 89 列を、[`FEATURES`] の順の辞書で返す。"""
    keys = keys if keys is not None else novelty_keys(r, lex)
    d = shape(r, lex)
    d.update(counts(r, lex, own))
    d.update(nov.features(r["word"], lex.word_class(r["word"]), keys, own))
    return {k: d[k] for k in FEATURES}
