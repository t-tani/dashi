# 中項目分類器(生成の方針 v2)。
# 挙げるのは、現代の日本語で日常的に使われると広く認められる語義だけである。
# 比喩・転用由来でも辞書に載る程度に定着していれば挙げる。迷ったら挙げない。
import os

import dspy
from pydantic import BaseModel, Field

from classifier import find_context, load_env  # 用例の切り出しと .env の読みは共通
from demos_v2 import DEMOS

WORK = os.environ["WORK_DIR"]


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

    語義として挙げるのは、現代の日本語で日常的に使われると広く認められるものだけ
    である。比喩や転用に由来する語義も、辞書に載る程度に定着していれば挙げる。
    攻撃の対象を指す「標的」がこれに当たる。

    次の 3 つは挙げない。まだ定着していない直訳や転用、特定の文脈や界隈でしか
    通じない用法、文学的・修辞的な言い回しにだけ現れる語義である。迷ったら挙げない
    側に倒す。

    たとえば「鍵」には、錠を開ける道具を指す語義に加えて、手がかりを指す語義が
    辞書に載る。どちらも挙げる。一方「灯」は明かりを指す語義だけを挙げ、「希望の灯」
    のような修辞にだけ現れる語義は挙げない。

    用例は、その語がどう使われるかを知る手がかりである。用例の語義だけに絞らず、
    その語が持つ定着した語義を挙げる。"""

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


def make_predictor():
    predictor = dspy.Predict(ClassifyWords)
    predictor.demos = [
        dspy.Example(
            entries=[entry_of(w, c) for w, c, _ in group],
            classifications=[
                WordCategories(word=w, categories=cats) for w, _, cats in group
            ],
        ).with_inputs("entries")
        for group in DEMOS
    ]
    return predictor
