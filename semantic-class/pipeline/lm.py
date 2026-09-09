# 第 3 期の測定で使う LM の組み立て。
# luna は ~/.env の OPENAI_MODEL、Haiku は ANTHROPIC_MODEL を読む。
# どちらも同じ litellm ゲートウェイ(LLM_BASE_URL)を叩く。認証は LLM_API_KEY である。
import os

import dspy


def load_env(path):
    """KEY=VALUE の並びを読む。引用符と前後の空白は落とす。"""
    env = {}
    with open(path, encoding="utf-8") as source:
        for line in source:
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            env[key.strip()] = value.strip().strip('"').strip("'")
    return env


def make_lm(model_key, max_tokens=8192, temperature=0.0):
    env = load_env(os.path.expanduser("~/.env"))
    return dspy.LM(
        "openai/" + env[model_key],
        api_base=env["LLM_BASE_URL"].rstrip("/") + "/v1",
        api_key=env["LLM_API_KEY"],
        max_tokens=max_tokens,
        temperature=temperature,
    )


def model_name(model_key):
    return load_env(os.path.expanduser("~/.env"))[model_key]
