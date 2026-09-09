# few-shot の実例を積んで、シード 694 の同じ 200 語を測り直す。
# 対象の 200 語は exp_small_mid_results.json に保存された組をそのまま使う。
import json

import dspy

from classifier import WORK, entry_of, make_lm, make_predictor


def main():
    dspy.configure(lm=make_lm())
    base = json.load(open(WORK + "/exp_small_mid_results.json", encoding="utf-8"))
    classify = make_predictor(with_demos=True)

    out = {}
    for name, r in base.items():
        pairs = [(w, ctx) for w, ctx in r["pairs"]]
        preds = {}
        for i in range(0, len(pairs), 20):
            chunk = pairs[i : i + 20]
            result = classify(entries=[entry_of(w, c) for w, c in chunk])
            for item in result.classifications:
                preds[item.word] = sorted(
                    {c for c in item.categories if 10 <= c <= 57}
                )
            print(f"{name}: {min(i + 20, len(pairs))}/{len(pairs)}")
        out[name] = {"preds": preds, "pairs": [[w, c] for w, c in pairs]}

    with open(WORK + "/exp_fewshot_results.json", "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
