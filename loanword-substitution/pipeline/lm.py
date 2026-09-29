# ゲートウェイを呼ぶ LM の組み立て。semantic-class/pipeline/lm.py と同じ形である。
# モデル名は ~/.env の ANTHROPIC_MODEL か OPENAI_MODEL、接続先は LLM_BASE_URL、
# 認証は LLM_API_KEY を読む。
import os

import dspy


def load_env(path=None):
    """KEY=VALUE の並びを読む。引用符と前後の空白は落とす。"""
    path = path or os.path.expanduser("~/.env")
    env = {}
    with open(path, encoding="utf-8") as source:
        for line in source:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            env[key.strip()] = value.strip().strip('"').strip("'")
    return env


def make_lm(model_key, max_tokens=512, temperature=1.0, cache=False):
    """ゲートウェイを呼ぶ LM を組む。

    `cache` の既定を偽にするのは、同じプロンプトを繰り返して候補の揺れを見るためである。
    DSPy の既定(真)では 2 回目以降が保存された応答を返すので、サンプルが 1 つに潰れる。
    """
    env = load_env()
    return dspy.LM(
        "openai/" + env[model_key],
        api_base=env["LLM_BASE_URL"].rstrip("/") + "/v1",
        api_key=env["LLM_API_KEY"],
        max_tokens=max_tokens,
        temperature=temperature,
        cache=cache,
        # 応答が返らない呼び出しで走行全体が止まらないよう、待ち時間と再試行を限る。
        timeout=120,
        num_retries=2,
    )


def model_name(model_key):
    return load_env()[model_key]
