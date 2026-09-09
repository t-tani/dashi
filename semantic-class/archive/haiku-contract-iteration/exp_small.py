# 小規模の精度調査: Haiku(DSPy 経由)の中項目分類を旧表(WLSP)と突き合わせる。
# 旧表は測定にだけ使い、プロンプトには入れない。中項目の番号と名前は、公開されて
# いる分類体系(部立て)の記述であり、語と分類の対応(データの行)ではない。
import json
import os
import random
import re
import sys
from collections import defaultdict

import dspy
from pydantic import BaseModel, Field

WORK = os.environ["WORK_DIR"]


def load_env(path):
    env = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            env[key.strip()] = value.strip().strip('"').strip("'")
    return env


def load_table(path):
    mids = defaultdict(set)
    for line in open(path, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        parts = line.rstrip("\n").split("\t")
        word = parts[0]
        for num in parts[1:]:
            m = re.match(r"\d+\.(\d\d)", num)
            if m:
                mids[word].add(int(m.group(1)))
    return mids


class WordCategories(BaseModel):
    """1 語の中項目分類。"""

    word: str
    categories: list[int] = Field(
        min_length=1, description="あてはまる中項目番号(10〜57)すべて"
    )


class ClassifyWords(dspy.Signature):
    """日本語の名詞を、意味の中項目(2 桁の番号)へ分類する。中項目は次の 43 個である。

    10 事柄・真偽 / 11 種類・例 / 12 存在・出没 / 13 様相・情勢・価値 / 14 力・勢い /
    15 作用・変化・移動・開始・終了 / 16 時間・時期 / 17 空間・場所・方向 /
    18 形・型・姿 / 19 量・数・程度 /
    20 人間・人称 / 21 家族・親族 / 22 相手・仲間・他人 / 23 人種・民族 /
    24 成員・職業の人 / 25 公私・君臣 / 26 社会・世界・国・地域 / 27 機関・組織 /
    30 心・感情・思考・方法 / 31 言語活動 / 32 創作・著述・芸術 /
    33 文化・歴史・風俗・生活 / 34 義務・権利 / 35 交わり・応対 / 36 支配・政治 /
    37 取得・金銭・売買 / 38 事業・業務・産業 /
    40 物品 / 41 資材・材料 / 42 衣料 / 43 食料 / 44 住居・建物 / 45 道具・機械 /
    46 灯火 / 47 土地の利用 /
    50 自然・現象 / 51 物体・物質 / 52 宇宙・天体・空 / 53 生物 / 54 植物 /
    55 動物 / 56 身体 / 57 生命

    多義語は、名詞として普通に使う語義の中項目をすべて挙げる。
    比喩や転用でだけ生まれる語義は挙げない。用例が添えてある語は、用例の使い方を優先する。"""

    entries: list[str] = dspy.InputField(desc="「語」または「語 || 用例文」の一覧")
    classifications: list[WordCategories] = dspy.OutputField()


def find_context(word, lines):
    kata = re.fullmatch(r"[ァ-ヶー]+", word)
    if kata:
        pat = re.compile(r"(?<![ァ-ヶー])" + re.escape(word) + r"(?![ァ-ヶー])")
    else:
        pat = re.compile(r"(?<![一-鿿])" + re.escape(word) + r"(?![一-鿿])")
    for line in lines:
        if pat.search(line):
            line = line.strip().replace("==", "")
            pos = line.find(word)
            start = max(0, pos - 40)
            return line[start : pos + 40]
    return None


def main():
    env = load_env(os.path.expanduser("~/.env"))
    raw = env["ANTHROPIC_API_TOKEN"]
    token = raw[7:] if raw.startswith("Bearer ") else raw
    lm = dspy.LM(
        "openai/" + env["ANTHROPIC_MODEL"],
        api_base=env["ANTHROPIC_BASE_URL"].rstrip("/") + "/v1",
        api_key=token,
        max_tokens=8192,
        temperature=0.0,
    )
    dspy.configure(lm=lm)

    wlsp = load_table(os.environ["AKUNUKI_DIR"] + ".aku/reference/semantic-class-table.tsv")
    lines = open(WORK + "/corpus-text.txt", encoding="utf-8").read().splitlines()

    content_words = [
        w for w in wlsp if len(w) >= 2 and not re.fullmatch(r"[ぁ-ゖー]+", w)
    ]
    random.seed(694)

    corpus_pairs = []
    for w in random.sample(sorted(content_words), 4000):
        ctx = find_context(w, lines)
        if ctx:
            corpus_pairs.append((w, ctx))
        if len(corpus_pairs) >= 100:
            break
    set_a = corpus_pairs
    set_b = [(w, None) for w in random.sample(sorted(content_words), 100)]

    classify = dspy.Predict(ClassifyWords)
    results = {}
    for name, pairs in [("corpus", set_a), ("random", set_b)]:
        preds = {}
        for i in range(0, len(pairs), 20):
            chunk = pairs[i : i + 20]
            entries = [w if ctx is None else f"{w} || {ctx}" for w, ctx in chunk]
            try:
                out = classify(entries=entries)
            except Exception as e:
                print("NG:", type(e).__name__, str(e)[:200])
                sys.exit(1)
            for item in out.classifications:
                preds[item.word] = sorted(
                    set(c for c in item.categories if 10 <= c <= 57)
                )
            print(f"{name}: {min(i + 20, len(pairs))}/{len(pairs)} classified")
        results[name] = {"pairs": pairs, "preds": preds}

    with open(WORK + "/exp_small_mid_results.json", "w", encoding="utf-8") as f:
        json.dump(
            {
                name: {
                    "preds": r["preds"],
                    "pairs": [[w, ctx] for w, ctx in r["pairs"]],
                }
                for name, r in results.items()
            },
            f,
            ensure_ascii=False,
            indent=1,
        )

    def divs(mids):
        return {m // 10 for m in mids}

    def concrete(s):
        return bool(s) and s <= {4, 5}

    def has_abstract(s):
        return bool(s & {1, 3})

    for name, r in results.items():
        rows = []
        for w, _ in r["pairs"]:
            if w not in r["preds"]:
                continue
            rows.append((w, set(wlsp[w]), set(r["preds"][w])))
        n = len(rows)
        mid_exact = sum(1 for _, t, p in rows if t == p)
        mid_overlap = sum(1 for _, t, p in rows if t & p)
        div_rows = [(w, divs(t), divs(p)) for w, t, p in rows]
        div_exact = sum(1 for _, t, p in div_rows if t == p)
        div_overlap = sum(1 for _, t, p in div_rows if t & p)
        conc = sum(1 for _, t, p in div_rows if concrete(t) == concrete(p))
        abst = sum(1 for _, t, p in div_rows if has_abstract(t) == has_abstract(p))
        print(f"\n=== {name} (n={n}, 未返答 {len(r['pairs']) - n}) ===")
        print(f"中項目の集合一致:   {mid_exact}/{n} = {100 * mid_exact / n:.1f}%")
        print(f"中項目の交わりあり: {mid_overlap}/{n} = {100 * mid_overlap / n:.1f}%")
        print(f"部門の集合一致:     {div_exact}/{n} = {100 * div_exact / n:.1f}%")
        print(f"部門の交わりあり:   {div_overlap}/{n} = {100 * div_overlap / n:.1f}%")
        print(f"具体条件の一致:     {conc}/{n} = {100 * conc / n:.1f}%")
        print(f"抽象含みの一致:     {abst}/{n} = {100 * abst / n:.1f}%")
        print("部門の不一致の例(語 / 旧表 / Haiku、中項目で表示):")
        shown = 0
        for (w, t, p), (_, dt, dp) in zip(rows, div_rows):
            if dt != dp and shown < 12:
                print(f"  {w} / {sorted(t)} / {sorted(p)}")
                shown += 1


if __name__ == "__main__":
    main()
