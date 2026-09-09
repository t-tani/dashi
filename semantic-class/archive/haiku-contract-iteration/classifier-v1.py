# 中項目分類器の共通部分。exp_fewshot.py と build_haiku_table.py が読む。
import os
import re

import dspy
from pydantic import BaseModel, Field

from demos import DEMOS

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


def make_lm(max_tokens=8192):
    env = load_env(os.path.expanduser("~/.env"))
    raw = env["ANTHROPIC_API_TOKEN"]
    token = raw[7:] if raw.startswith("Bearer ") else raw
    return dspy.LM(
        "openai/" + env["ANTHROPIC_MODEL"],
        api_base=env["ANTHROPIC_BASE_URL"].rstrip("/") + "/v1",
        api_key=token,
        max_tokens=max_tokens,
        temperature=0.0,
    )


def entry_of(word, context):
    return word if context is None else f"{word} || {context}"


def make_demos():
    examples = []
    for group in DEMOS:
        entries = [entry_of(w, ctx) for w, ctx, _ in group]
        answers = [WordCategories(word=w, categories=cats) for w, _, cats in group]
        examples.append(
            dspy.Example(entries=entries, classifications=answers).with_inputs("entries")
        )
    return examples


def make_predictor(with_demos=True):
    predictor = dspy.Predict(ClassifyWords)
    if with_demos:
        predictor.demos = make_demos()
    return predictor


def find_context(word, lines):
    if re.fullmatch(r"[ァ-ヶー]+", word):
        pat = re.compile(r"(?<![ァ-ヶー])" + re.escape(word) + r"(?![ァ-ヶー])")
    else:
        pat = re.compile(r"(?<![一-鿿])" + re.escape(word) + r"(?![一-鿿])")
    for line in lines:
        if pat.search(line):
            line = line.strip().replace("==", "")
            pos = line.find(word)
            return line[max(0, pos - 40) : pos + 40]
    return None
