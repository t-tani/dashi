"""LLM に置き換えの候補を挙げさせて candidates/<モデル名>.jsonl に追記する。軸は 2 つある。

`--axis loanword` は、カタカナ語と語源の英語を渡し、カタカナ語の代わりに書きうる漢字とかなの短い語を
挙げさせる。`--axis english` は、英語だけを渡し、指す先を決めずに日本語 1 語で書くなら何かを挙げさせる。
後者は、source を `出所` のように 1 語で覆う型を集める。

意味ごとに 1 つのプロンプトにする。`バケット` は bucket の意味と baguette の意味で別に尋ね、候補が
どの行から出たかをレコードに残す。1 つの意味を複数の英語で言える場合(`ティア` の
`tear; teardrop shape`)は、その英語を並べて 1 回で渡す。英語ごとに割ると 1 つの意味が 2 つになり、
`tear` だけを見たモデルが別の意味の答えを返す。

1 語につき `--samples` 回サンプルする。temperature は 1.0 を既定にし、同じプロンプトを繰り返して、候補ごとに
何回の応答に現れたかを数えられるようにする。プロンプトの文面は PROMPT_VERSION でバージョンを持ち、レコードに残す。
`--dry-run` はプロンプトを組み立てて標準出力に書くだけで、ゲートウェイを呼ばない。
"""
import argparse
import concurrent.futures
import datetime
import json
import sys
import threading
from pathlib import Path

import dspy

from lm import make_lm, model_name

# プロンプトの文面のバージョン。軸ごとに別に持つ。文面を変えたら必ず上げる。
PROMPT_VERSION = {"loanword": "sense-v2", "english": "v2"}


class EnglishRendering(dspy.Signature):
    """英語の語を、指す先を決めずに日本語 1 語で書くなら何と書くかを答える。
    漢字とかなからなる短い語(和語・漢語)を、思いつく順に挙げる。
    カタカナ語は挙げない。説明や記号は付けない。"""

    english: str = dspy.InputField(desc="英語の語")
    candidates: list[str] = dspy.OutputField(desc="漢字とかなからなる短い語の候補。思いつく順")


class Substitution(dspy.Signature):
    """あるカタカナ語の代わりに、漢字とかなからなる短い日本語(和語・漢語。`受け皿` のような
    送りがなつきの語を含む)で書くとしたら何と書くかを答える。

    英語は、そのカタカナ語が持つ意味のうち 1 つを指す。英語が複数並ぶときは、どれも同じ 1 つの
    意味を別の言い方にしたものである。その意味に限って答える。

    思いつく候補を確からしい順に挙げる。説明や記号は付けない。"""

    english: str = dspy.InputField(desc="カタカナ語の 1 つの意味を指す英語。複数あれば同じ意味の別の言い方")
    loanword: str = dspy.InputField(desc="カタカナ語")
    candidates: list[str] = dspy.OutputField(desc="漢字とかなからなる短い語の候補。確からしい順")


def read_headwords(path, words, limit, key):
    rows = []
    with open(path, encoding="utf-8") as source:
        header = source.readline().rstrip("\n").split("\t")
        for line in source:
            row = dict(zip(header, line.rstrip("\n").split("\t")))
            if words and row[key] not in words:
                continue
            if not row["english"]:
                continue
            if key == "english" and any(r["english"] == row["english"] for r in rows[-64:]):
                # 英語の一覧は 1 行 1 語なので、同じ英語が続けて並ぶ。1 つの英語に 1 プロンプトにする
                continue
            rows.append(row)
            if limit and len(rows) >= limit:
                break
    return rows


def prompts_of(row, axis):
    """1 行から作るプロンプトの入力の一覧。(loanword, english) の組で、english 軸では loanword が None。"""
    if axis == "english":
        return [(None, row["english"])]
    return [(row["j"], english) for english in english_of(row)]


def english_of(row):
    """プロンプトに渡す英語。1 行が 1 つの意味なので、その意味を指す英語をつないだ 1 つを返す。"""
    return [row["english"]] if row["english"] else []


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--headwords", required=True, help="build_headwords.py か build_english_headwords.py の出力")
    parser.add_argument("--axis", choices=("loanword", "english"), default="loanword")
    parser.add_argument("--out-dir", default="candidates")
    parser.add_argument("--model-key", default="ANTHROPIC_MODEL")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--temperature", type=float, default=1.0)
    parser.add_argument("--words", help="この語だけに絞る。読点区切り。english 軸では英語")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--dry-run", action="store_true", help="プロンプトを組み立てて書くだけで、ゲートウェイを呼ばない")
    parser.add_argument("--workers", type=int, default=4, help="同時に送る呼び出しの数")
    args = parser.parse_args()
    words = set(args.words.split("、")) if args.words else None
    key = "english" if args.axis == "english" else "j"
    rows = read_headwords(args.headwords, words, args.limit, key)
    if args.dry_run:
        prompts = 0
        for row in rows:
            for loanword, english in prompts_of(row, args.axis):
                prompts += 1
                print(json.dumps({"axis": args.axis, "loanword": loanword, "english": english, "samples": args.samples,
                                  "temperature": args.temperature, "prompt_version": PROMPT_VERSION[args.axis]}, ensure_ascii=False))
        print(f"{len(rows)} 行、{prompts} プロンプト x {args.samples} 回 = {prompts * args.samples} 回の呼び出し", file=sys.stderr)
        return
    lm = make_lm(args.model_key, temperature=args.temperature)
    dspy.configure(lm=lm)
    predictor = dspy.Predict(EnglishRendering if args.axis == "english" else Substitution)
    model = model_name(args.model_key)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    today = datetime.date.today().isoformat()
    jobs = [(row, loanword, english, k) for row in rows
            for loanword, english in prompts_of(row, args.axis) for k in range(args.samples)]
    lock = threading.Lock()

    def ask(job):
        row, loanword, english, k = job
        try:
            result = predictor(english=english) if loanword is None else predictor(english=english, loanword=loanword)
            candidates, error = list(result.candidates), None
        except Exception as exception:  # noqa: BLE001 - 1 件の失敗で全体を止めず、レコードに残す
            candidates, error = [], f"{type(exception).__name__}: {exception}"
        return {
            "axis": args.axis,
            "loanword": loanword,
            "sense": row.get("sense", ""),
            "english": english,
            "candidates": candidates,
            "sample_index": k,
            "model": model,
            "temperature": args.temperature,
            "prompt_version": PROMPT_VERSION[args.axis],
            "date": today,
            "error": error,
        }

    failed = 0
    with (out / f"{model}.jsonl").open("a", encoding="utf-8") as sink:
        with concurrent.futures.ThreadPoolExecutor(args.workers) as pool:
            futures = [pool.submit(ask, job) for job in jobs]
            for future in concurrent.futures.as_completed(futures):
                record = future.result()
                if record["error"]:
                    failed += 1
                with lock:
                    sink.write(json.dumps(record, ensure_ascii=False) + "\n")
                    sink.flush()
    print(f"{len(jobs)} 回の呼び出し、失敗 {failed} 件", file=sys.stderr)


if __name__ == "__main__":
    main()
