"""名詞(体の類)の分類対象の定義。共通の pipeline が --target で読む。

持つのは、分類器へ渡す指示文と few-shot の実例、分類番号の規約、JMdict から
見出しを採る品詞、manifest に書く来歴の注記である。名前の一覧と読み込み方は
../pipeline/load_target.py にある。
"""

NAME = "noun"
POS_NAME = "名詞"

# 表へ書く分類番号の整数部。分類語彙表の類の番号で、1 が体の類である。
CLASS_PREFIX = "1"

# 許す中項目番号。10〜57 の全域を許して記録する。範囲の中の欠番(28 など)や
# 体系の拡張候補(48)も、行として残して後から検めるためである。
CATEGORIES = frozenset(range(10, 58))


def is_target_pos(pos):
    """品詞に n(名詞)を持つ語義を採る。

    n-suf(接尾辞的名詞)・ctr(助数詞)・pn(代名詞)しか持たない語義は、
    単独で立つ名詞ではないので採らない。
    """
    return "n" in pos


# 分類器の指示文(生成の方針 v2)。中項目の一覧と方針の全文を持つ。
# 挙げるのは、現代の日本語で日常的に使われると広く認められる語義だけである。
# 比喩・転用由来でも辞書に載る程度に定着していれば挙げる。迷ったら挙げない。
# 書き換えたら ../pipeline/votes.py の CONTRACT を上げる。
INSTRUCTIONS = """日本語の名詞を、意味の中項目(2 桁の番号)へ分類する。中項目は次の 43 個である。

10 事柄・真偽 / 11 種類・例 / 12 存在・出没 / 13 様相・情勢・価値 / 14 力・勢い /
15 作用・変化・移動・開始・終了 / 16 時間・時期 / 17 空間・場所・方向 /
18 形・型・姿 / 19 量・数・程度 /
20 人間・人称 / 21 家族・親族 / 22 相手・仲間・他人 / 23 人種・民族 /
24 成員・職業の人 / 25 公私・君臣 / 26 社会・世界・国・地域 / 27 機関・組織 /
30 心・感情・思考・方法 / 31 言語活動 / 32 創作・著述・芸術 /
33 文化・歴史・風俗・生活 / 34 義務・権利 / 35 交わり・応対 / 36 支配・政治 /
37 取得・金銭・売買 / 38 事業・業務・産業 /
40 物品 / 41 資材・材料 / 42 衣料 / 43 食料 / 44 住居・建物 / 45 道具・機械 /
46 灯火 / 47 土地の利用 /
50 自然・現象 / 51 物体・物質 / 52 宇宙・天体・空 / 53 生物 / 54 植物 /
55 動物 / 56 身体 / 57 生命

語義として挙げるのは、現代の日本語で日常的に使われると広く認められるものだけ
である。比喩や転用に由来する語義も、辞書に載る程度に定着していれば挙げる。
攻撃の対象を指す「標的」がこれに当たる。

次の 3 つは挙げない。まだ定着していない直訳や転用、特定の分野や集団の中でしか
通じない用法、文学的・修辞的な言い回しにだけ現れる語義である。迷ったら挙げない
側に倒す。

たとえば「鍵」には、錠を開ける器具を指す語義に加えて、手がかりを指す語義が
辞書に載る。どちらも挙げる。一方「灯」は明かりを指す語義だけを挙げ、「希望の灯」
のような修辞にだけ現れる語義は挙げない。

用例は、その語がどう使われるかを知る手がかりである。用例の語義だけに絞らず、
その語が持つ定着した語義を挙げる。"""

# few-shot の実例(生成の方針 v2)。分類語彙表の行は見ていない。理由と選定の
# 経緯は demos-rationale.txt に書いた。ベンチマークの 200 語とも、採点に使う
# 6 語とも重ならない。
# (語, 用例文 or None, 中項目の一覧)
DEMOS = [
    [
        ("標的", "日本を主要標的とし、金融詐欺に移行している", [10, 40]),
        ("鍵", "SSH 鍵の追加を検知する", [30, 45]),
        ("都市", "沿岸の都市に人口が集まる", [26]),
        ("値引き", "在庫を値引きして売り切る", [37]),
        ("絨毯", "床に絨毯を敷く", [40]),
    ],
    [
        ("ファイル", "EPUB ファイルから外部参照を除く", [31, 40]),
        ("パッチ", "パッチが唯一の確実な対策である", [15, 41]),
        ("常連", None, [24]),
        ("口座", None, [37]),
        ("手順", None, [30]),
    ],
    [
        ("ノイズ", "警告のノイズが多くて見落とす", [10, 50]),
        ("パイプ", "パイプの中身をカーネル内で転送する", [17, 45]),
        ("県", None, [26]),
        ("使い手", None, [24]),
        ("対策", None, [30]),
    ],
    [
        ("ツール", "解析ツールを走らせて結果を読む", [30, 45]),
        ("スクリプト", "Python スクリプトで検査する", [31, 32]),
        ("大手", None, [27]),
        ("手数料", None, [37]),
        ("工夫", None, [30]),
    ],
    [
        ("蝶番", "扉の蝶番が緩む", [45]),
        ("灯", "軒の灯をともす", [46]),
        ("窓口", None, [27, 44]),
        ("担い手", None, [24]),
        ("利回り", None, [37]),
        ("金", "金の相場が上がる", [37, 51]),
    ],
]

# manifest.json のうち、名詞に固有の節。共通の節は ../pipeline/manifest.py が
# 組む。母集団の語数とパスごとの語数は、書き出すときに数えて埋まる。
MANIFEST = {
    "version": "v5",
    "purpose": "母集団の語彙に対する複数モデルの判定を、再利用できる形で保存する",
    "contract_note": (
        "訂正前の文言は「定着して持つ語義は比喩・転用由来でもすべて挙げる」"
        "だった。この形は語義の列挙を膨らませ、周辺的な比喩語義まで付いて"
        "検出器の免除が効きすぎる"
    ),
    "categories": {
        "scheme": (
            "意味の中項目を 2 桁の番号で表す。十の位が部門で、1 が関係、"
            "2 が主体、3 が活動、4 が生産物、5 が自然物である。"
            "表へ書くときは 1.mm00 の形にする"
        ),
        "note": (
            "中項目 48(無形の人工物)は未採用である。分類語彙表にない独自の"
            "拡張であり、採る判断が出ていない"
        ),
        "provenance": (
            "番号と名前の一覧だけを使い、どの語にどの中項目を与えるかは"
            "モデルの言語判断で決めた"
        ),
    },
    "few_shot_note": "実例の語は、採点に使う語ともベンチマークの 200 語とも重ならない",
    "population": {
        "corpus": {
            "source": "inputs/corpus-headwords.txt",
            "how": (
                "旧表と日本語 WordNet 由来の表の見出し語を合わせた集合を、"
                "評価コーパスの本文に対して部分文字列として走査して得た"
            ),
            "context": "コーパスから用例文を取れた語には用例を添えた",
        },
        "jmdict": {
            "source": "inputs/jmdict-noun-headwords.txt",
            "script": "pipeline/headwords.py",
            "how": [
                "品詞に n(名詞)を持つ語義が 1 つ以上ある項目だけを採る。"
                "n-suf(接尾辞的名詞)・ctr(助数詞)・pn(代名詞)しか持たない"
                "項目は、単独で立つ名詞ではないので採らない",
                "その語義が exp(成句)を併せ持つ場合は落とす。"
                "「気を付ける」のような句は 1 語の分類になじまない",
                "名詞の語義がどれも古語・廃語・稀語の印"
                "(arch・obs・obsc・rare・dated・hist・poet)を持つ項目は落とす。"
                "生成の方針が求めるのは現代の日本語で日常的に使われる語義である",
                "表記に日本語の文字以外(ラテン文字・数字・記号・空白)が混じる"
                "見出しと、13 字以上の見出しを落とす",
                "名詞の語義がどれも固有名の印(会社名・地名・人名・作品名など)"
                "を持つ項目を落とす",
            ],
            "headword": (
                "見出しは項目ごとに 1 つだけ採る。漢字表記があればその先頭、"
                "無ければ読みの先頭である。表記の揺れをすべて採ると、"
                "同じ語の分類を何度も課金することになる"
            ),
            "excluded": "corpus の語は既に判定があるので除いた",
            "context": "用例は添えない。コーパスに現れない語を含むためである",
            "caveat": (
                "稀語と俗語の混入は承知の上で採った。表に載っても文書に現れ"
                "なければ働かない。稀語の分類の質は検証していない"
            ),
        },
    },
    "commands": [
        "uv run python pipeline/headwords.py --target noun"
        " --jmdict <JMdict_e.xml> --exclude noun/inputs/corpus-headwords.txt"
        " --drop-names --out noun/inputs/jmdict-noun-headwords.txt",
        "uv run python pipeline/headwords.py --target noun"
        " --jmdict <JMdict_e.xml> --drop-names --priority --out <頻度の印を持つ名詞>",
        "cat noun/inputs/corpus-headwords.txt <頻度の印を持つ名詞>"
        " | sort -u > noun/inputs/daily-subset.txt",
        "uv run python pipeline/run_votes.py --target noun"
        " --words noun/inputs/corpus-headwords.txt"
        " --out-dir noun/votes --model-key <ANTHROPIC_MODEL|OPENAI_MODEL>"
        " --tier corpus --corpus-text <コーパスの本文> --jmdict <JMdict_e.xml>",
        "uv run python pipeline/run_votes.py --target noun"
        " --words noun/inputs/jmdict-noun-headwords.txt"
        " --out-dir noun/votes --model-key <ANTHROPIC_MODEL|OPENAI_MODEL>"
        " --tier jmdict --jmdict <JMdict_e.xml>",
        "uv run python pipeline/run_votes.py --target noun"
        " --words noun/inputs/daily-subset.txt"
        " --out-dir noun/votes --model-key <ANTHROPIC_MODEL|OPENAI_MODEL>"
        " --tier jmdict --corpus-words noun/inputs/corpus-headwords.txt"
        " --corpus-text <コーパスの本文> --jmdict <JMdict_e.xml> --seed <1002|1003>",
        "uv run python pipeline/finalize.py --votes-dir noun/votes"
        " --model <モデル> --seeds 694,1002,1003"
        " --out-dir noun/votes/final --eval-dir noun/review",
        "uv run python pipeline/disagreement.py --final-dir noun/votes/final"
        " --model-a claude-haiku-4-5-20251001 --model-b gpt-5.6-luna"
        " --out noun/review/disagreement-v5.tsv",
        "uv run python pipeline/manifest.py --target noun"
        " --out-dir noun/votes --jmdict-version <JMdict の版>",
        "uv run python pipeline/build_release_table.py --target noun"
        " --votes-dir noun/votes/final --model claude-haiku-4-5-20251001"
        " --manifest noun/votes/manifest.json --version <配布のバージョン>"
        " --out noun/semantic-class-table.tsv",
    ],
    "voting": {
        "design": "全語に 1 回、日常語の部分集合に 3 回の判定",
        "why": (
            "バッチの組み方が判定を動かすことが対照実験で分かった(下の"
            "chunk_effect)。判定を増やして多数決を取れば、バッチの中身に引かれた"
            "語義の増減を打ち消せる。全語に 3 回の判定を取ると費用が 3 倍になるので、"
            "日常的に使われる語に絞って判定を増やす"
        ),
        "subset": {
            "definition": "tier が corpus の語と、jmdict_priority が true の語の和集合",
            "file": "inputs/daily-subset.txt",
            "note": (
                "評価コーパスに現れた語と、JMdict が頻度の印を付けた"
                "名詞の見出しの和集合である"
            ),
        },
        "passes": {
            "seeds": [694, 1002, 1003],
            "independence": (
                "パスごとにバッチの組み方の種を変える。温度 0 ではバッチを固定すると"
                "同じ判定が出るので、判定の独立性はバッチの組み方の違いだけから来る"
            ),
            "input": (
                "3 つのパスは入力をそろえる。評価コーパスに現れる語には"
                "どのパスでも同じ用例文を添える。用例の有無が違うと、"
                "バッチの組み方だけを変えた比較にならない"
            ),
            "files": (
                "1 回目の判定は <モデル>.jsonl、追加のパスは <モデル>-s<種>.jsonl。"
                "冪等な再実行が判定済みの語を飛ばす作りなので、同じファイルに"
                "混ぜると 2 回目の判定が飛ばされる"
            ),
        },
        "rule": {
            "majority": "2 パス以上に現れた語は、2 パス以上が挙げた中項目だけを採る",
            "single": "1 パスにしか現れない裾の語は、その 1 回の判定をそのまま採る",
            "unstable": (
                "どの中項目も 2 回の判定に届かない語は確定させず、表へ載せない。"
                "載せなければ検出器は黙るので、揺れた分類で指摘を出すより"
                "安全側に倒れる。落とした語は review/unstable-*.tsv に残す"
            ),
            "dropped": (
                "2 回の判定に届かず落ちた中項目は review/dropped-senses-*.tsv に残す。"
                "1 パスだけが挙げた語義であり、モデルがバッチの中身に引かれて"
                "足したか落としたかの跡である"
            ),
            "fields": (
                "確定値は votes/final/ に置く。votes(現れたパスの数)と"
                "rule で、多数決と 1 回の判定のどちらで決めたかを区別できる"
            ),
        },
    },
    "extra": {
        "chunk_effect": {
            "question": "バッチに一緒に載る語が判定を動かすか",
            "method": (
                "JMdict の見出しから種 694 で 400 語を取り、違うバッチの組み方で"
                " 2 回分類して語ごとの categories を突き合わせる。対照として、"
                "同じバッチの組み方で 2 回分類した場合も測る"
            ),
            "script": "archive/batch-effect/chunk_effect.py",
            "findings": {
                "same_chunking_set_match": {
                    "claude-haiku-4-5-20251001": 1.0,
                    "gpt-5.6-luna": 1.0,
                },
                "different_chunking_set_match": {
                    "claude-haiku-4-5-20251001": 0.650,
                    "gpt-5.6-luna": 0.759,
                },
                "different_chunking_division_match": {
                    "claude-haiku-4-5-20251001": 0.810,
                    "gpt-5.6-luna": 0.815,
                },
                "different_chunking_concrete_match": {
                    "claude-haiku-4-5-20251001": 0.955,
                    "gpt-5.6-luna": 0.930,
                },
                "chunk_size_sweep_haiku_set_match": {"20": 0.650, "10": 0.645, "4": 0.708},
                "position": (
                    "バッチの前半と後半で語義の数に差は出ない。位置ごとの平均は"
                    "1.00〜1.45 の幅で散り、勾配を持たない"
                ),
            },
            "reading": (
                "バッチを固定すれば判定は完全に再現する。動くのはバッチの中身を変えたとき"
                "だけなので、揺れの原因は走行ごとの非決定性ではなくバッチの組み方で"
                "ある。ただし食い違いの大半は語義を 1 つ足すか落とすかで、"
                "検出器が読む「引けた分類がすべて生産物か自然物の部門にあるか」"
                "の判定は 93〜96% で保たれる。バッチを小さくしても直らない"
            ),
        },
    },
}
