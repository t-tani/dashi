"""第 1 層の既存の判定を、保存形式 v5 の JSONL へ取り込む。

評価コーパスに現れた 7,743 語は、訂正後の方針で Haiku と luna の判定が既にある。
API を呼び直さずに形式だけを変える。用例文は、そのとき渡したものを同じ手順で
組み立て直す。分類の入力を再現できないと、レコードの categories を後から
作り直せないためである。

来歴として、元の JSON のファイル名を source に書く。
"""

import argparse
import json
import os

import votes
from corpus import context_of
from jmdict_nouns import priority_words

# 取り込む元。(元ファイル、モデル名、判定した日)
SOURCES = {
    "haiku": ("haiku-v2-classifications.json", "claude-haiku-4-5-20251001", "2026-08-30"),
    "luna": ("luna-v3-classifications.json", "gpt-5.6-luna", "2026-08-30"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help="semantic-class のディレクトリ")
    parser.add_argument("--corpus-text", required=True, help="用例を切り出す本文")
    parser.add_argument("--jmdict", required=True)
    parser.add_argument("--which", choices=sorted(SOURCES), required=True)
    args = parser.parse_args()

    name, model, date = SOURCES[args.which]
    loaded = json.load(open(os.path.join(args.root, "votes", name), encoding="utf-8"))
    text = open(args.corpus_text, encoding="utf-8").read()
    common = priority_words(args.jmdict)
    print(f"元の判定 {len(loaded)} 語 / 頻度の印を持つ見出し {len(common)}")

    out = votes.path_for(os.path.join(args.root, "votes", "v5"), model)
    done = votes.words_of(out)
    rows = [
        votes.record(
            word,
            categories,
            model,
            date,
            context=context_of(word, text),
            tier="corpus",
            jmdict_priority=word in common,
            source=name,
        )
        for word, categories in sorted(loaded.items())
        if word not in done
    ]
    votes.append(out, rows)
    print(f"取り込んだ {len(rows)} 語 / 既にあった {len(done)} 語 / 書き先 {out}")


if __name__ == "__main__":
    main()
