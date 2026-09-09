"""1 塊(20 語)の分類に要るトークン数を実測する。費用の概算に使う。

第 2 層の語彙は用例文を持たないので、語だけを渡した形で測る。
"""

import sys

import dspy

from classifier import make_predictor
from lm import make_lm, model_name

# 第 2 層から取った、用例を添えない 20 語。長さの分布をまねてある。
PROBE = [
    "竹細工", "差し支え", "真贋", "狙撃", "弁士", "年寄り", "ターボ", "官憲",
    "末路", "立て看板", "横断幕", "肉質", "防火", "マスタード", "幾重", "全軍",
    "イデオロギー", "序文", "再考", "面持ち",
]


def main(model_key):
    lm = make_lm(model_key)
    dspy.configure(lm=lm)
    classify = make_predictor()
    result = classify(entries=PROBE)
    got = sum(1 for _ in result.classifications)
    usage = lm.history[-1].get("usage") or {}
    prompt = usage.get("prompt_tokens") or usage.get("input_tokens") or 0
    completion = usage.get("completion_tokens") or usage.get("output_tokens") or 0
    print(f"モデル {model_name(model_key)}")
    print(f"返った語 {got} / 入力 {prompt} トークン / 出力 {completion} トークン")
    return prompt, completion


if __name__ == "__main__":
    main(sys.argv[1])
