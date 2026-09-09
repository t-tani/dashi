# ゲートウェイへの疎通の試し

判定に使う LLM のゲートウェイへ、3 通りの呼び方で疎通するかを試したスクリプトを置く。どのスクリプトも、接続先と API キーを `~/.env` から読む。呼び方の内訳は次のとおり。

| スクリプト | クライアント | ゲートウェイのルート |
|---|---|---|
| `test_gateway.py` | anthropic SDK | Anthropic 互換 |
| `test_gateway_dspy.py` | DSPy | OpenAI 互換 |
| `test_gateway_pydantic.py` | pydantic-ai | OpenAI 互換 |

`test_gateway_dspy.py` と `test_gateway_pydantic.py` では、型を宣言した構造化出力も確かめた。2026-09-09 時点で意味分類表の生成は DSPy 経由なので、`test_gateway.py` と `test_gateway_pydantic.py` は経路を比べたときの記録である。
