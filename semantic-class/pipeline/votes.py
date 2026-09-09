"""判定の保存形式 v5 の読み書き。

1 レコード 1 語の JSONL で、モデルごとに 1 ファイルに収める。JSONL を選ぶのは、
追記だけで書き足せる形が冪等な再実行と相性がよいためである。走行の途中で
止まっても、書き終えた行はそのまま残る。

レコードの鍵は次のとおりである。

    word              分類した語
    categories        中項目番号の配列。許す番号の集合は分類対象の定義
                      (target.py の CATEGORIES)で決まる。昇順で重複を持たない
    model             ゲートウェイへ渡したモデル名
    date              判定した日(ISO 8601 の日付)
    contract          生成の方針の版の識別子
    context_used      用例文。添えなかったときは null
    tier              語彙の母集団の層。corpus は評価コーパスに現れた語、
                      jmdict は JMdict の見出し
    jmdict_priority   JMdict が頻度の印(ichi・news・spec・gai)を付けた
                      見出しなら true。日常語だけの部分集合を、判定を
                      取り直さずに切り出すために持つ
    source            来歴。既存の判定から取り込んだ行は元ファイル名、
                      この形式で新しく取った行は "api" とする
    chunk_id          その語を載せたバッチの識別子。1 回の呼び出しに載せた語の
                      まとまりを指す。取り込んだ行では null
    chunk_index       バッチ内の位置(0 起点)。同じバッチに載る語が判定へ及ぼす
                      影響を、後から統計で検査するために持つ
    pass_seed         バッチの組み方を決めた種。パスの識別子でもある。取り込んだ行では
                      null。温度 0 ではバッチを固定すると同じ判定が出るので、判定の
                      独立性はバッチの組み方の違いだけから来る

categories を後から作り直せるように、判定に要った入力(word と context_used)
と、判定の条件(model・contract)を 1 レコードの中に揃えて持たせてある。
中項目 48(無形の人工物)のような体系の変更を後から採る場合は、対象の語だけ
を選んで取り直し、同じ形式の別ファイルへ書けばよい。
"""

import json
import os

# 生成の方針の版。Signature の指示文を書き換えたら上げる。既存の行は書かれた
# ときの版を持ち続けるので、どの方針で出した判定かを行ごとに追える。
CONTRACT = "v3-reworded"
TIERS = ("corpus", "jmdict")

# 1 回目の判定の種。この種の判定を本体のファイルに置き、追加のパスは別ファイルにする。
FIRST_SEED = 694


def path_for(root, model, seed=None):
    """モデル名と種から JSONL のディレクトリを決める。

    1 回目の判定は <モデル>.jsonl、追加のパスは <モデル>-s<種>.jsonl に置く。
    パスごとにファイルを分けるのは、冪等な再実行が「判定済みの語を飛ばす」
    作りだからである。同じファイルに複数のパスを混ぜると、2 回目の判定を書こうと
    しても 1 回目の判定があるために飛ばされる。
    """
    name = model.replace("/", "-")
    if seed is not None and seed != FIRST_SEED:
        name += f"-s{seed}"
    return os.path.join(root, name + ".jsonl")


def record(
    word,
    categories,
    model,
    date,
    context=None,
    tier="corpus",
    jmdict_priority=False,
    source="api",
    chunk_id=None,
    chunk_index=None,
    pass_seed=None,
):
    return {
        "word": word,
        "categories": sorted(set(categories)),
        "model": model,
        "date": date,
        "contract": CONTRACT,
        "context_used": context,
        "tier": tier,
        "jmdict_priority": bool(jmdict_priority),
        "source": source,
        "chunk_id": chunk_id,
        "chunk_index": chunk_index,
        "pass_seed": pass_seed,
    }


def read(path):
    """判定済みの語を {語: レコード} で返す。ファイルが無ければ空を返す。"""
    if not os.path.exists(path):
        return {}
    done = {}
    with open(path, encoding="utf-8") as source:
        for line in source:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            done[row["word"]] = row
    return done


def append(path, rows):
    """レコードを追記する。書いた行はそのまま残るので、再実行で飛ばせる。"""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as sink:
        for row in rows:
            print(json.dumps(row, ensure_ascii=False), file=sink)


def words_of(path):
    """判定済みの語の集合を返す。冪等な再実行の飛ばし判定に使う。"""
    return set(read(path))
