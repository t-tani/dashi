# 第 3 期の測定で使う LM の組み立て。
# luna は ~/.env の OPENAI_MODEL、Haiku は ANTHROPIC_MODEL を読む。
# どちらも同じ litellm ゲートウェイ(ANTHROPIC_BASE_URL)を叩く。
import os

import dspy

from classifier import load_env


def make_lm(model_key, max_tokens=8192, temperature=0.0):
    env = load_env(os.path.expanduser("~/.env"))
    raw = env["ANTHROPIC_API_TOKEN"]
    token = raw[7:] if raw.startswith("Bearer ") else raw
    return dspy.LM(
        "openai/" + env[model_key],
        api_base=env["ANTHROPIC_BASE_URL"].rstrip("/") + "/v1",
        api_key=token,
        max_tokens=max_tokens,
        temperature=temperature,
    )


def model_name(model_key):
    return load_env(os.path.expanduser("~/.env"))[model_key]
