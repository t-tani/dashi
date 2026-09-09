# 訂正後の方針のまま、シード 694 の同じ 200 語を任意のモデルで測る(参考値)。
# 使い方: python exp_bench_v3.py <OPENAI_MODEL|ANTHROPIC_MODEL> <出力 JSON>
import json
import sys

import dspy

from classifier_v2 import WORK, entry_of, make_predictor
from lm_v3 import make_lm


def main(model_key, out_path):
    dspy.configure(lm=make_lm(model_key))
    base = json.load(open(WORK + "/exp_small_mid_results.json", encoding="utf-8"))
    classify = make_predictor()
    out = {}
    for name, r in base.items():
        pairs = [(w, c) for w, c in r["pairs"]]
        preds = {}
        for i in range(0, len(pairs), 20):
            chunk = pairs[i : i + 20]
            result = classify(entries=[entry_of(w, c) for w, c in chunk])
            for item in result.classifications:
                preds[item.word] = sorted({c for c in item.categories if 10 <= c <= 57})
            print(f"{name}: {min(i + 20, len(pairs))}/{len(pairs)}", flush=True)
        out[name] = {"preds": preds, "pairs": [[w, c] for w, c in pairs]}
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
