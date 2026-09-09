# litellm 経由の Bedrock ゲートウェイへの疎通テスト。~/.env の値は表示しない。
import os
import sys

import anthropic


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


def parse_headers(raw):
    headers = {}
    for part in raw.replace("\\n", "\n").splitlines():
        if ":" in part:
            name, _, value = part.partition(":")
            headers[name.strip()] = value.strip()
    return headers


def main():
    env = load_env(os.path.expanduser("~/.env"))
    base_url = env["ANTHROPIC_BASE_URL"]
    model = env["ANTHROPIC_SMALL_FAST_MODEL"]
    headers = parse_headers(env.get("ANTHROPIC_CUSTOM_HEADERS", ""))

    print("base_url host:", base_url.split("//")[-1].split("/")[0])
    print("model:", model)
    for name, value in headers.items():
        print("header:", name, "(value length", len(value), ")")

    # .env のヘッダー値は "Bearer sk-..." の形で、litellm の仮想キーは
    # "Bearer " を剥がした sk-... の部分である。SDK の api_key に渡すと
    # x-api-key として送られ、litellm がそれを仮想キーとして受ける。
    raw = headers.get("x-litellm-api-key", "")
    token = raw[7:] if raw.startswith("Bearer ") else raw
    api_key = os.environ.get("ANTHROPIC_API_KEY") or token or "unused"

    client = anthropic.Anthropic(
        base_url=base_url, api_key=api_key, default_headers=headers or None
    )

    prompt = (
        "次の日本語の名詞を、意味の部門へ分類する。部門は 1 関係(抽象的な関係・性質)、"
        "2 主体(人・組織)、3 活動(行為・出来事)、4 生産物(人工物)、5 自然物 の 5 つである。"
        "多義語はあてはまる部門をすべて挙げる。\n"
        "出力は 1 語 1 行で「語: 部門番号(読点区切り)」だけを書く。\n\n"
        "サーバー\n透明性\n踏み台\n"
    )

    try:
        response = client.messages.create(
            model=model,
            max_tokens=256,
            messages=[{"role": "user", "content": prompt}],
        )
    except anthropic.NotFoundError as e:
        print("NG: model or path not found:", e.status_code, e.message)
        sys.exit(1)
    except anthropic.AuthenticationError as e:
        print("NG: authentication failed:", e.status_code)
        sys.exit(1)
    except anthropic.APIStatusError as e:
        print("NG: API error:", e.status_code, e.message)
        sys.exit(1)
    except anthropic.APIConnectionError as e:
        print("NG: connection failed:", e)
        sys.exit(1)

    print("stop_reason:", response.stop_reason)
    print("usage: in", response.usage.input_tokens, "out", response.usage.output_tokens)
    print("--- response ---")
    for block in response.content:
        if block.type == "text":
            print(block.text)
    print("OK")


if __name__ == "__main__":
    main()
