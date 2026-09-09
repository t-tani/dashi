"""分類器の組み立て。分類対象の定義から Signature と few-shot の実例を作る。

指示文と実例は分類対象ごとに違い、target.py が持つ。ここにあるのは、どの分類対象
でも同じ入出力の形と、DSPy の Predict へ実例を積む手順である。
"""

import dspy
from pydantic import BaseModel, Field


class WordCategories(BaseModel):
    """1 語の中項目分類。"""

    word: str
    categories: list[int] = Field(
        min_length=1, description="あてはまる中項目番号(10〜57)すべて"
    )


def entry_of(word, context):
    return word if context is None else f"{word} || {context}"


def signature_of(target):
    """分類対象の指示文を持つ Signature を返す。"""
    return dspy.Signature(
        {
            "entries": (
                list[str],
                dspy.InputField(desc="「語」または「語 || 用例文」の一覧"),
            ),
            "classifications": (list[WordCategories], dspy.OutputField()),
        },
        target.INSTRUCTIONS,
    )


def make_predictor(target):
    predictor = dspy.Predict(signature_of(target))
    predictor.demos = [
        dspy.Example(
            entries=[entry_of(w, c) for w, c, _ in group],
            classifications=[
                WordCategories(word=w, categories=cats) for w, _, cats in group
            ],
        ).with_inputs("entries")
        for group in target.DEMOS
    ]
    return predictor
