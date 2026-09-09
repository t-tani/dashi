"""votes/manifest.json を書き出す。

生成の方針の全文と中項目の一覧は、分類対象の指示文(target.py の INSTRUCTIONS)
から取る。手で写すと分類器と manifest が別々に古びるためである。母集団の作り方や
判定の重ね方など、分類対象に固有の節は target.py の MANIFEST から取り、母集団の
語数とパスごとの判定済みの語数は、入力の一覧と votes の JSONL を数えて埋める。
全パスの走行が終わってから実行する。
"""

import argparse
import datetime
import json
import os
import re

import dspy

import load_target
import votes
from lm import load_env

# 中項目の一覧。指示文は「10 事柄・真偽 / 11 種類・例 /」の形で並べ、行末にも
# 区切りのスラッシュが来る。スラッシュと改行で割ってから 1 件ずつ読む。
MID = re.compile(r"\A(\d\d)\s+(.+)\Z")

# 判定の保存形式の記述。分類対象によらず同じである。
RECORD_FORMAT = {
    "file": "モデルごとに 1 つの JSONL。1 行 1 語",
    "fields": {
        "word": "分類した語",
        "categories": "中項目番号の配列。許す集合は分類対象で決まる。昇順で重複を持たない",
        "model": "ゲートウェイへ渡したモデル名",
        "date": "判定した日",
        "contract": "生成の方針の版の識別子",
        "context_used": "用例文。添えなかったときは null",
        "tier": "語彙の母集団の層。corpus か jmdict",
        "jmdict_priority": "JMdict の頻度の印(ichi・news・spec・gai)を持つ見出しなら true",
        "source": "来歴。取り込んだ行は元ファイル名、この形式で取った行は api",
        "chunk_id": "その語を載せたバッチの識別子。取り込んだ行では null",
        "chunk_index": "バッチ内の位置(0 起点)。取り込んだ行では null",
        "pass_seed": "バッチの組み方を決めた種。パスの識別子でもある。取り込んだ行では null",
    },
    "regeneration": (
        "categories は後から作り直せる。判定の入力(word と context_used)と"
        "条件(model と contract)を 1 レコードに揃えて持たせてあるので、"
        "中項目 48(無形の人工物)のような体系の変更を採る場合は、対象の語"
        "だけを選んで取り直し、同じ形式の別ファイルへ書けばよい。"
        "既存の行は捨てずに残す"
    ),
    "subset": (
        "jmdict_priority が true の行だけを取れば、日常語だけの部分集合を"
        "判定の取り直しなしで切り出せる"
    ),
}

# モデル名を引く環境変数。manifest の models の節に、どの変数で指定したかを残す。
MODEL_KEYS = ("ANTHROPIC_MODEL", "OPENAI_MODEL")


def contract_text(instructions):
    """指示文から、方針の段落だけを取る。"""
    at = instructions.index("語義として挙げるのは")
    return " ".join(instructions[at:].split())


def categories(instructions):
    head = instructions[: instructions.index("語義として挙げるのは")]
    found = {}
    for piece in re.split(r"[/\n]", head):
        matched = MID.match(piece.strip())
        if matched:
            found[int(matched.group(1))] = matched.group(2).strip()
    return found


def count_lines(path):
    with open(path, encoding="utf-8") as source:
        return sum(1 for line in source if line.strip())


def pass_counts(out_dir):
    """モデルごと・パスごとの判定済みの語数を、votes の JSONL を数えて返す。"""
    counts = {}
    for name in sorted(os.listdir(out_dir)):
        if not name.endswith(".jsonl"):
            continue
        stem = name[: -len(".jsonl")]
        model, _, seed = stem.rpartition("-s")
        if not model or not seed.isdigit():
            model, seed = stem, str(votes.FIRST_SEED)
        counts.setdefault(model, {})[seed] = len(
            votes.words_of(os.path.join(out_dir, name))
        )
    return counts


def models(counts):
    """モデル名と、それを指定した環境変数の対応を返す。"""
    env = load_env(os.path.expanduser("~/.env"))
    by_name = {env[key]: key for key in MODEL_KEYS if key in env}
    return {
        model: {"env": by_name.get(model), "file": f"{model}.jsonl"} for model in counts
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, help="分類対象のディレクトリ")
    parser.add_argument("--out-dir", required=True, help="votes のディレクトリ")
    parser.add_argument("--jmdict-version", required=True)
    args = parser.parse_args()

    target = load_target.load(args.target)
    notes = target.MANIFEST
    mids = categories(target.INSTRUCTIONS)
    demo_words = [w for group in target.DEMOS for w, _, _ in group]
    counts = pass_counts(args.out_dir)

    population = json.loads(json.dumps(notes["population"]))
    for tier in ("corpus", "jmdict"):
        if tier in population:
            source = os.path.join(target.DIRECTORY, population[tier]["source"])
            population[tier]["count"] = count_lines(source)
    if "jmdict" in population:
        population["jmdict"]["jmdict_version"] = args.jmdict_version

    voting = json.loads(json.dumps(notes["voting"]))
    voting["passes"]["counts"] = counts

    manifest = {
        "version": notes["version"],
        "written": datetime.date.today().isoformat(),
        "purpose": notes["purpose"],
        "record_format": RECORD_FORMAT,
        "contract": {
            "id": votes.CONTRACT,
            "text": contract_text(target.INSTRUCTIONS),
            "note": notes["contract_note"],
        },
        "categories": {
            "count": len(mids),
            **notes["categories"],
            "items": {str(num): name for num, name in sorted(mids.items())},
        },
        "few_shot": {
            "module": "target.py",
            "groups": len(target.DEMOS),
            "words": len(demo_words),
            "rationale": "demos-rationale.txt",
            "note": notes["few_shot_note"],
        },
        "population": population,
        "generation": {
            "dspy": dspy.__version__,
            "target": target.NAME,
            "temperature": 0.0,
            "max_tokens": 8192,
            "predictor": "dspy.Predict 1 段",
            "chunk": 20,
            "chunk_order": (
                "語の一覧から判定済みの語を除き、パスの種で混ぜてから先頭より"
                " 20 語ずつ切る。並びのままだと字種の偏ったバッチができ、モデルが"
                "空の配列を返すことがある。バッチの通し番号と語順の種を chunk_id に、"
                "バッチ内の位置を chunk_index に残す"
            ),
            "workers": 8,
            "retries": 3,
            "gateway": "LLM_BASE_URL の litellm ゲートウェイを openai 互換で呼ぶ",
            "idempotent": (
                "出力の JSONL を読んで判定済みの語を飛ばす。走行の途中で止まっても、"
                "次の走行が残りだけを拾う"
            ),
            "commands": notes["commands"],
        },
        "voting": voting,
        **notes.get("extra", {}),
        "models": models(counts),
    }

    os.makedirs(args.out_dir, exist_ok=True)
    path = os.path.join(args.out_dir, "manifest.json")
    with open(path, "w", encoding="utf-8") as sink:
        json.dump(manifest, sink, ensure_ascii=False, indent=2)
        sink.write("\n")
    print(f"書き出した: {path} / 中項目 {len(mids)} / few-shot {len(demo_words)} 語")
    for model, by_seed in counts.items():
        print(f"  {model}: {by_seed}")


if __name__ == "__main__":
    main()
