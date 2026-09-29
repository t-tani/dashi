"""表の行ごとに、その意味でその語を書くのが日本語として自然かを LLM に尋ねる。

表の行 (語, カタカナ語, 英語) は、語が覆う意味の 1 つである。`段`←ステップ は「処理の順」の意味で
`段` と書く形を指す。この形が日本語として自然かは、語が実際に使われる回数とは別の事柄であり、
回数からは決まらない。行ごとに尋ね、答えを JSONL に追記する。

1 行につき `--samples` 回サンプルする。プロンプトの文面は PROMPT_VERSION でバージョンを持つ。
`--dry-run` はプロンプトを組み立てて標準出力に書くだけで、ゲートウェイを呼ばない。
"""
import argparse
import concurrent.futures
import datetime
import json
import sys
import threading
from pathlib import Path
from typing import Literal

import dspy

from lm import make_lm, model_name

PROMPT_VERSION = "naturalness-v3"

# プロンプトの形が choice のときのバージョン。
CHOICE_VERSION = "choice-v2"


class Choice(dspy.Signature):
    """情報技術の文書を日本語で書くとき、ある英語の意味を表すのに、カタカナ語と漢字の語の
    どちらが自然かを選ぶ。

    2 つの候補のどちらも自然でなく、別の語が自然なら、その語を答える。カタカナ語と漢字の語の
    どちらかを必ず選ぶ形ではない。

    どちらも単独の名詞として使う形で比べる。単独の名詞とは、複合語の一部ではなく、助詞を伴って
    それだけで立つ形である。その語を含む複合語が自然に書けても、単独では意味が通らない語がある。

    書き手がその語を単独で書いたとき、読み手が英語の意味と同じものを思い浮かべるかを基準にする。"""

    english: str = dspy.InputField(desc="表したい意味の英語")
    loanword: str = dspy.InputField(desc="カタカナ語の候補")
    word: str = dspy.InputField(desc="漢字の語の候補")
    choice: Literal[
        "カタカナ語", "漢字の語", "どちらも自然", "どちらも不自然", "別の語が自然"
    ] = dspy.OutputField(
        desc="2 つの候補のどちらでもない語が自然なら 別の語が自然 を選ぶ"
    )
    better: str = dspy.OutputField(desc="別の語が自然 を選んだときの、その語。それ以外は空")
    reason: str = dspy.OutputField(desc="選んだ理由。1 文")


class Naturalness(dspy.Signature):
    """情報技術の文書を日本語で書くとき、あるカタカナ語が指す意味を、漢字の語を単独の名詞として
    書いて表すのが日本語として自然かを判断する。

    単独の名詞とは、複合語の一部ではなく、助詞を伴ってそれだけで立つ形である。その語を含む
    複合語が自然に書けても、単独では意味が通らない語がある。判断するのは単独の形だけである。

    書き手がその語を単独でその意味で使ったとき、読み手が同じものを思い浮かべるかを基準にする。
    その意味を語が単独で持たなければ不自然である。"""

    loanword: str = dspy.InputField(desc="カタカナ語")
    english: str = dspy.InputField(desc="その語義の英語")
    word: str = dspy.InputField(desc="漢字の語。単独の名詞として使う形で判断する")
    judgment: Literal["自然", "不自然", "どちらとも言えない"] = dspy.OutputField()
    reason: str = dspy.OutputField(desc="判断の理由。1 文")


def read_rows(path, words, limit):
    rows = []
    seen = set()
    with open(path, encoding="utf-8") as source:
        header = source.readline().rstrip("\n").split("\t")
        for line in source:
            row = dict(zip(header, line.rstrip("\n").split("\t")))
            if row["axis"] != "loanword":
                continue
            key = (row["word"], row["loanword"], row["english"])
            if key in seen or (words and row["word"] not in words):
                continue
            seen.add(key)
            rows.append(row)
            if limit and len(rows) >= limit:
                break
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--table", required=True)
    parser.add_argument("--out-dir", default="naturalness")
    parser.add_argument("--model-key", default="ANTHROPIC_MODEL")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--temperature", type=float, default=1.0)
    parser.add_argument("--words", help="この語の行だけに絞る。読点区切り")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--form", choices=("naturalness", "choice"), default="naturalness",
                        help="プロンプトの形。naturalness は漢字の語が自然かを尋ね、choice は\n"
                             "カタカナ語と漢字の語のどちらが自然かを選ばせる")
    args = parser.parse_args()
    words = set(args.words.split("、")) if args.words else None
    rows = read_rows(args.table, words, args.limit)
    if args.dry_run:
        for row in rows:
            print(json.dumps({"word": row["word"], "loanword": row["loanword"],
                              "english": row["english"]}, ensure_ascii=False))
        print(f"{len(rows)} 行 x {args.samples} 回 = {len(rows) * args.samples} 回の呼び出し",
              file=sys.stderr)
        return
    lm = make_lm(args.model_key, temperature=args.temperature)
    dspy.configure(lm=lm)
    predictor = dspy.Predict(Choice if args.form == "choice" else Naturalness)
    model = model_name(args.model_key)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    today = datetime.date.today().isoformat()
    jobs = [(row, k) for row in rows for k in range(args.samples)]
    lock = threading.Lock()

    def ask(job):
        row, index = job
        try:
            result = predictor(loanword=row["loanword"], english=row["english"], word=row["word"])
            judgment = result.choice if args.form == "choice" else result.judgment
            better = getattr(result, "better", "") if args.form == "choice" else ""
            reason, error = result.reason, None
        except Exception as exception:  # noqa: BLE001 - 1 件の失敗で走行を止めない
            judgment, better, reason, error = "", "", "", f"{type(exception).__name__}: {exception}"
        return {"word": row["word"], "loanword": row["loanword"], "english": row["english"],
                "judgment": judgment, "better": better, "reason": reason,
                "sample_index": index, "model": model,
                "temperature": args.temperature, "prompt_version": CHOICE_VERSION if args.form == "choice" else PROMPT_VERSION,
                "date": today, "error": error}

    failed = 0
    with (out / f"{model}.jsonl").open("a", encoding="utf-8") as sink:
        with concurrent.futures.ThreadPoolExecutor(args.workers) as pool:
            futures = [pool.submit(ask, job) for job in jobs]
            for future in concurrent.futures.as_completed(futures):
                record = future.result()
                failed += bool(record["error"])
                with lock:
                    sink.write(json.dumps(record, ensure_ascii=False) + "\n")
                    sink.flush()
    print(f"{len(jobs)} 回の呼び出し、失敗 {failed} 件", file=sys.stderr)


if __name__ == "__main__":
    main()
