# コーパスに現れた見出し語を任意のモデル・任意の分類器で中項目へ分類し、
# semantic-class-table.tsv と同じ形式の表を書き出す。
# 使い方: python build_table_v3.py <OPENAI_MODEL|ANTHROPIC_MODEL> <分類器モジュール> <印>
import datetime
import importlib
import json
import random
import sys
import threading
from concurrent.futures import ThreadPoolExecutor

import dspy

from build_haiku_table_v2 import context_of
from classifier_v2 import WORK, entry_of
from lm_v3 import make_lm, model_name

CHUNK = 20
WORKERS = 8


def main(model_key, module_name, tag):
    module = importlib.import_module(module_name)
    dspy.configure(lm=make_lm(model_key))
    classify = module.make_predictor()

    words = [
        w
        for w in open(WORK + "/corpus-headwords.txt", encoding="utf-8").read().split("\n")
        if w
    ]
    text = open(WORK + "/corpus-text.txt", encoding="utf-8").read()
    print(f"見出し語 {len(words)}", flush=True)

    pairs = [(w, context_of(w, text)) for w in words]
    print(f"用例を添えられた語 {sum(1 for _, c in pairs if c)}", flush=True)

    # 塊の中身を散らす。並びのままだと、ラテン文字の断片だけの塊ができて
    # モデルが何も返さない。種は 694 に固定する。
    random.Random(694).shuffle(pairs)
    chunks = [pairs[i : i + CHUNK] for i in range(0, len(pairs), CHUNK)]
    done = {}
    failed = []
    lock = threading.Lock()
    counter = [0]
    lo, hi = module.RANGE

    def run(chunk):
        entries = [entry_of(w, c) for w, c in chunk]
        for attempt in range(3):
            try:
                result = classify(entries=entries)
                got = {}
                for item in result.classifications:
                    cats = sorted({c for c in item.categories if lo <= c <= hi})
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

    with open(f"{WORK}/{tag}-classifications.json", "w", encoding="utf-8") as sink:
        json.dump(done, sink, ensure_ascii=False, indent=1)

    header = module.header(model_name(model_key), datetime.date.today().isoformat())
    with open(f"{WORK}/{tag}-semantic-class-table.tsv", "w", encoding="utf-8") as sink:
        for line in header:
            print(line, file=sink)
        for word in sorted(done):
            numbers = "\t".join(f"1.{c}00" for c in sorted(done[word]))
            print(f"{word}\t{numbers}", file=sink)
    print("表を書き出した")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3])
