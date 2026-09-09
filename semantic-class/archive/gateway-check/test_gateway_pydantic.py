# pydantic-ai を litellm ゲートウェイ(OpenAI 互換ルート)に通す疎通テスト。
# ~/.env の値は表示しない。
import os
import sys

from pydantic import BaseModel, Field
from pydantic_ai import Agent
from pydantic_ai.models.openai import OpenAIChatModel
from pydantic_ai.providers.openai import OpenAIProvider


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


class WordDivisions(BaseModel):
    """1 語の部門分類。部門は 1 関係、2 主体、3 活動、4 生産物、5 自然物。"""

    word: str
    divisions: list[int] = Field(min_length=1, description="あてはまる部門番号すべて")


class Classification(BaseModel):
    words: list[WordDivisions]


def main():
    env = load_env(os.path.expanduser("~/.env"))
    base_url = env["ANTHROPIC_BASE_URL"].rstrip("/") + "/v1"
    raw = env["ANTHROPIC_CUSTOM_HEADERS"].partition(":")[2].strip()
    token = raw[7:] if raw.startswith("Bearer ") else raw
    model_name = env["ANTHROPIC_SMALL_FAST_MODEL"]

    print("base_url host:", base_url.split("//")[-1].split("/")[0])
    print("model:", model_name)

    model = OpenAIChatModel(
        model_name, provider=OpenAIProvider(base_url=base_url, api_key=token)
    )
    agent = Agent(
        model,
        output_type=Classification,
        system_prompt=(
            "日本語の名詞を意味の部門へ分類する。部門は 1 関係(抽象的な関係・性質)、"
            "2 主体(人・組織)、3 活動(行為・出来事)、4 生産物(人工物)、"
            "5 自然物 の 5 つである。多義語はあてはまる部門をすべて挙げる。"
        ),
    )

    try:
        result = agent.run_sync("サーバー、透明性、踏み台")
    except Exception as e:
        print("NG:", type(e).__name__, str(e)[:300])
        sys.exit(1)

    for w in result.output.words:
        print(w.word, "->", w.divisions)
    usage = result.usage()
    print("usage: in", usage.input_tokens, "out", usage.output_tokens)
    print("OK")


if __name__ == "__main__":
    main()
