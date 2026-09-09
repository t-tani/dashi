# コーパスに現れた見出し語を Haiku(DSPy 経由)で中項目へ分類し、
# semantic-class-table.tsv と同じ形式の表を書き出す。
import datetime
import json
import os
import random
import re
import sys
import threading
from concurrent.futures import ThreadPoolExecutor

import dspy

from classifier import load_env
from classifier_v2 import WORK, entry_of, make_lm, make_predictor

CHUNK = 20
WORKERS = 8
KANJI = re.compile(r"[一-鿿]")
KATA = re.compile(r"[ァ-ヶー]")
LATIN = re.compile(r"[0-9A-Za-z_.\-]")


def context_of(word, text):
    """語が単独で立つ最初の箇所の前後を返す。見つからなければ None。"""
    if re.fullmatch(r"[ァ-ヶー]+", word):
        same = KATA
    elif re.search(r"[0-9A-Za-z]", word):
        same = LATIN
    else:
        same = KANJI
    at = 0
    for _ in range(20):
        at = text.find(word, at)
        if at < 0:
            return None
        before = text[at - 1] if at else ""
        after = text[at + len(word) : at + len(word) + 1]
        if not (same.fullmatch(before) or same.fullmatch(after)):
            line_start = text.rfind("\n", 0, at) + 1
            line_end = text.find("\n", at)
            line = text[line_start : line_end if line_end > 0 else len(text)]
            pos = at - line_start
            return line[max(0, pos - 40) : pos + len(word) + 40].strip().replace("==", "")
        at += len(word)
    return None


def main():
    dspy.configure(lm=make_lm())
    classify = make_predictor()

    words = [w for w in open(WORK + "/corpus-headwords.txt", encoding="utf-8").read().split("\n") if w]
    text = open(WORK + "/corpus-text.txt", encoding="utf-8").read()
    print(f"見出し語 {len(words)}")

    pairs = [(w, context_of(w, text)) for w in words]
    with_ctx = sum(1 for _, c in pairs if c)
    print(f"用例を添えられた語 {with_ctx}")

    # 塊の中身を散らす。並びのままだと、ラテン文字の断片だけの塊ができて
    # モデルが何も返さない。種は 694 に固定する。
    random.Random(694).shuffle(pairs)
    chunks = [pairs[i : i + CHUNK] for i in range(0, len(pairs), CHUNK)]
    done = {}
    failed = []
    lock = threading.Lock()
    counter = [0]

    def run(chunk):
        entries = [entry_of(w, c) for w, c in chunk]
        for attempt in range(3):
            try:
                result = classify(entries=entries)
                got = {}
                for item in result.classifications:
                    cats = sorted({c for c in item.categories if 10 <= c <= 57})
                    if cats:
                        got[item.word] = cats
                with lock:
                    done.update(got)
                    counter[0] += 1
                    if counter[0] % 20 == 0:
                        print(f"{counter[0]}/{len(chunks)} chunks", flush=True)
                return
            except Exception as error:
                if attempt == 2:
                    with lock:
                        failed.append((chunk, f"{type(error).__name__}: {error}"[:200]))

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        list(pool.map(run, chunks))

    print(f"分類できた語 {len(done)} / 失敗した塊 {len(failed)}")
    for chunk, why in failed[:5]:
        print("  失敗:", [w for w, _ in chunk][:5], why)

    with open(WORK + "/haiku-v2-classifications.json", "w", encoding="utf-8") as sink:
        json.dump(done, sink, ensure_ascii=False, indent=1)

    today = datetime.date.today().isoformat()
    model = load_env(os.path.expanduser("~/.env"))["ANTHROPIC_MODEL"]
    header = [
        "# 意味分類表(見出し語と分類番号の対応)。検証用に生成した表である。",
        f"# 生成に使ったモデルは {model} である。",
        "# 生成の方式は、用例文を添えた語を Claude Haiku へ渡し、意味の中項目を",
        "# 答えさせる分類である。呼び出しは DSPy の Predict 1 段で、few-shot の実例を",
        "# 25 語ぶん積んである。分類番号は 1.mm00 の形で、mm が中項目の 2 桁である。",
        "#",
        "# 生成の方針は次のとおりである。語義として挙げるのは、現代の日本語で日常的に",
        "# 使われると広く認められるものだけである。比喩や転用に由来する語義も、辞書に",
        "# 載る程度に定着していれば挙げる。攻撃の対象を指す「標的」がこれに当たる。",
        "# 挙げないのは 3 つで、まだ定着していない直訳や転用、特定の文脈や界隈でしか",
        "# 通じない用法、文学的・修辞的な言い回しにだけ現れる語義である。迷ったら",
        "# 挙げない側に倒す。",
        "#",
        "# この方針は、定着した術語への指摘を出さないために置いてある。「鍵」は錠を",
        "# 開ける道具の語義と手がかりを指す語義の両方を持ち、どちらも表に載る。",
        "#",
        "# 小数点以下第 3 位と第 4 位は 0 で埋めてあり、意味を持たない。",
        f"# 生成した日は {today} である。",
        "# 対象は、評価コーパスに現れた見出し語だけである。",
        "# 版: haiku-mid-2",
    ]
    with open(WORK + "/haiku-v2-semantic-class-table.tsv", "w", encoding="utf-8") as sink:
        for line in header:
            print(line, file=sink)
        for word in sorted(done):
            numbers = "\t".join(f"1.{c}00" for c in sorted(done[word]))
            print(f"{word}\t{numbers}", file=sink)
    print("表を書き出した")


if __name__ == "__main__":
    main()
