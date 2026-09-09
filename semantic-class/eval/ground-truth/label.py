"""候補の語に、指摘すべきかの一次付けを自動で行う。

これは自動判定(LLM)であり、人手との一致率は未検証である。確認を経た行だけが
正解になる。

判定は語の単位で行い、同じ語のすべての出現へ広げる。出現ごとに問うと 1 万件を
超えて費用と時間が釣り合わないためである。語には、その語が実際に現れた文を
最大 2 つ添える。境界の語は文脈で変わるので、人手の確認でそこを見る。

基準は確定済みのものである。指摘すべきとするのは、定着していない転用や直訳で、
現代の技術文書の読み手が引っかかる使い方である。定着した語義と専門用語への指摘は、
語の意味分類が正しくても指摘すべきでないとする。
"""

import argparse
import json
import os
import sys
import threading
from concurrent.futures import ThreadPoolExecutor

import dspy
from pydantic import BaseModel, Field

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "pipeline"))
from lm import make_lm, model_name  # noqa: E402

CHUNK = 20
WORKERS = 8


class WordCall(BaseModel):
    """1 語の一次付け。"""

    word: str
    flag: bool = Field(description="指摘すべきなら true")
    reason: str = Field(description="理由を 1 行で。40 字以内")


class JudgeWords(dspy.Signature):
    """日本語の技術文書に現れた語について、linter が指摘すべきかを決める。

    この linter が狙うのは、英語の語をそのまま日本語へ写した結果、読み手が意味を
    取り違える書き方である。たとえば in the wild を「野生」、bucket を「バケット」、
    watermark を「透かし」と書く形がこれに当たる。

    指摘すべき(true)とするのは、定着していない転用や直訳で、現代の技術文書の
    読み手が引っかかる使い方だけである。

    指摘すべきでない(false)とするのは次である。第 1 に、計算機や情報技術で定着した
    専門用語。「パッケージ」「ブラウザ」「トークン」「シェル」「パッチ」「標的」
    「鍵」がこれに当たる。第 2 に、普通の日本語の用法。物や人をそのまま指す語、
    日本語として自然な比喩、助数詞、接尾辞がこれに当たる。第 3 に、辞書に載る
    程度に定着した転用。

    迷ったら false に倒す。この linter は指摘を出しすぎる側に偏っているので、
    定着した語を拾わないことを優先する。

    entries は「語 || 用例文」の形で渡す。用例は語の使われ方を知る手がかりである。"""

    entries: list[str] = dspy.InputField(desc="「語 || 用例文」の一覧")
    calls: list[WordCall] = dspy.OutputField()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidates", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--model-key", default="ANTHROPIC_MODEL")
    parser.add_argument("--chunk", type=int, default=CHUNK, help="1 回に渡す語数")
    args = parser.parse_args()

    data = json.load(open(args.candidates, encoding="utf-8"))
    examples = {}
    for row in data["candidates"]:
        examples.setdefault(row["word"], []).append(row["sentence"])
    words = sorted(examples)
    print(f"候補の出現 {len(data['candidates'])} 件 / 一次付けする語 {len(words)}", flush=True)

    done = {}
    if os.path.exists(args.out):
        done = {r["word"]: r for r in json.load(open(args.out, encoding="utf-8"))}
        print(f"既に付けた語 {len(done)}", flush=True)
    todo = [w for w in words if w not in done]
    if not todo:
        print("残りが無い")
        return

    model = model_name(args.model_key)
    dspy.configure(lm=make_lm(args.model_key))
    judge = dspy.Predict(JudgeWords)
    chunks = [todo[i : i + args.chunk] for i in range(0, len(todo), args.chunk)]
    lock = threading.Lock()
    counter = [0]

    def run(chunk):
        entries = []
        for w in chunk:
            sample = examples[w][0][:110]
            entries.append(f"{w} || {sample}")
        for attempt in range(3):
            try:
                result = judge(entries=entries)
                got = {}
                for item in result.calls:
                    if item.word in set(chunk):
                        got[item.word] = {
                            "word": item.word,
                            "flag": bool(item.flag),
                            "reason": item.reason.strip()[:80],
                            "judge": "自動(LLM)",
                            "model": model,
                        }
                if not got:
                    raise ValueError("返った語が無い")
                with lock:
                    done.update(got)
                    counter[0] += 1
                    if counter[0] % 20 == 0:
                        print(f"{counter[0]}/{len(chunks)} 塊", flush=True)
                return
            except Exception:
                if attempt == 2:
                    return

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(run, chunks))

    with open(args.out, "w", encoding="utf-8") as sink:
        json.dump([done[w] for w in sorted(done)], sink, ensure_ascii=False, indent=1)
    flagged = sum(1 for r in done.values() if r["flag"])
    print(f"一次付けした語 {len(done)} / 指摘すべき {flagged} / すべきでない {len(done) - flagged}")


if __name__ == "__main__":
    main()
