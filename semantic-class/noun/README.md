# 名詞の意味分類表

名詞の意味分類表は `semantic-class-table.tsv` にあり、akunuki はこの表を埋め込みます。利用条件は CC BY 4.0 です。生成には claude-haiku-4-5-20251001 を使い、判定は 2026-08-30 に取りました。

## 構成

配布する表と、生成に使った入力と判定、判定が割れた語の一覧を、このフォルダに置きます。表と `review/` の一覧が載せる確定値は、同じ語に取った判定の多数決で決めた中項目です。

| パス | 中身 |
|---|---|
| `semantic-class-table.tsv` | 見出し語 169,238 語に分類番号(`1.mm00`)を対応させた、配布する表 |
| `target.py` | 指示文(生成の方針と 43 中項目の一覧)と few-shot の実例、分類番号の規約、採る品詞、来歴の注記を持つ分類対象の定義 |
| `demos-rationale.txt` | few-shot の実例をどう選んだか |
| `v5-candidate-semantic-class-table.tsv` | 冒頭の注記だけが配布する表と違う、研究用の候補の表 |
| `inputs/corpus-headwords.txt` | 用例を添えて判定した、評価コーパスに現れた 7,743 語 |
| `inputs/jmdict-noun-headwords.txt` | JMdict の名詞の見出しから評価コーパスの語を除いた 161,855 語 |
| `inputs/daily-subset.txt` | 3 パスの判定を取る日常語 27,962 語(評価コーパスの語と、JMdict が頻度のタグを付けた語の和集合) |
| `votes/` | 判定の JSONL と確定値と来歴 |
| `review/` | 判定が割れた語(モデル間で食い違った語と、確定しなかった語) |

確定値の決め方と `votes/` のファイルの分かれ方は `votes/README.md` に、`review/` の詳細は `review/README.md` にあります。

## 再現手順

コマンドは、`../pipeline/README.md` の準備を済ませたうえで、`semantic-class/` の中で実行します。`<JMdict_e.xml>` は JMdict の XML を指し、`<コーパスの本文>` は評価コーパスの本文を 1 つにまとめたテキストを指します。どちらも dashi には含まれていません。

見出しの一覧は JMdict のバージョンに依ります。`inputs/` の一覧は JMdict created 2026-08-23 から抜いたもので、2026-09-03 のバージョンで抜き直すと、161,855 語のうち 152 語が入れ替わります。一覧を作り直さず `inputs/` のものを使えば、この差は出ません。

```
# 1. JMdict の名詞の見出しを抜く。評価コーパスの語は除き、固有名を落とす
uv run python pipeline/headwords.py --target noun \
    --jmdict <JMdict_e.xml> --exclude noun/inputs/corpus-headwords.txt \
    --drop-names --out noun/inputs/jmdict-noun-headwords.txt

# 2. 3 パスの判定を取る日常語の一覧を作る。頻度のタグを持つ名詞と、評価コーパスの語の和集合を取る
uv run python pipeline/headwords.py --target noun \
    --jmdict <JMdict_e.xml> --drop-names --priority --out <頻度のタグを持つ名詞>
cat noun/inputs/corpus-headwords.txt <頻度のタグを持つ名詞> \
    | sort -u > noun/inputs/daily-subset.txt

# 3. 1 パス目の判定を取る。評価コーパスの語には用例を添え、JMdict の語には添えない
#    --model-key を OPENAI_MODEL に替えると、もう一方のモデルで同じ判定を取る
uv run python pipeline/run_votes.py --target noun \
    --words noun/inputs/corpus-headwords.txt \
    --out-dir noun/votes --model-key ANTHROPIC_MODEL --tier corpus \
    --corpus-text <コーパスの本文> --jmdict <JMdict_e.xml>
uv run python pipeline/run_votes.py --target noun \
    --words noun/inputs/jmdict-noun-headwords.txt \
    --out-dir noun/votes --model-key ANTHROPIC_MODEL --tier jmdict \
    --jmdict <JMdict_e.xml>

# 4. 日常語に 2 パス目と 3 パス目の判定を足す。種を変えるとバッチの組み方が変わる
uv run python pipeline/run_votes.py --target noun \
    --words noun/inputs/daily-subset.txt \
    --out-dir noun/votes --model-key ANTHROPIC_MODEL --tier jmdict \
    --corpus-words noun/inputs/corpus-headwords.txt \
    --corpus-text <コーパスの本文> --jmdict <JMdict_e.xml> --seed 1002
#    3 パス目は、同じコマンドを --seed 1003 で実行する

# 5. 判定の JSONL を検証する。フィールドの有無と、中項目が範囲に収まるかと、語の重複を見る
uv run python pipeline/verify.py --target noun \
    noun/votes/*.jsonl

# 6. 3 パスの判定から多数決で確定値を作る。確定しなかった語と落ちた中項目は review/ へ書き出す
uv run python pipeline/finalize.py --votes-dir noun/votes \
    --model claude-haiku-4-5-20251001 --seeds 694,1002,1003 \
    --out-dir noun/votes/final --eval-dir noun/review

# 7. 生成の来歴を書き出す。指示文の全文と中項目の一覧とコマンドが入る
uv run python pipeline/manifest.py --target noun \
    --out-dir noun/votes --jmdict-version "JMdict created 2026-08-23"

# 8. 配布する表を書く。冒頭の注記(方針・バージョン・利用条件)を manifest から取る
uv run python pipeline/build_release_table.py --target noun \
    --votes-dir noun/votes/final --model claude-haiku-4-5-20251001 \
    --manifest noun/votes/manifest.json --version semantic-class-v1 \
    --out noun/semantic-class-table.tsv

# 9. 手順 6 をもう一方のモデルでも済ませてから、2 つのモデルの確定値が割れた語を並べる
uv run python pipeline/disagreement.py --final-dir noun/votes/final \
    --model-a claude-haiku-4-5-20251001 --model-b gpt-5.6-luna \
    --out noun/review/disagreement-v5.tsv
```

手順 3 と 4 は冪等です。出力の JSONL を読んで判定済みの語を飛ばすので、途中で止めても、同じコマンドを出し直せば残りだけを拾います。`--limit <語数>` を足すと、残りの先頭からその語数だけを処理できます。試しに動かすときに使ってください。

手順 6 と 8 と 9 は API を呼びません。`votes/` の JSONL から決定的に作れるので、出し直しても表と `review/` の一覧は同じ内容になります。ただし、手順 8 が書く注記の日付の行だけは、実行した日に変わります。

`votes/` にある評価コーパスの語の判定は、手順 3 の 1 つ目のコマンドで取ったものではありません。保存形式を変える前に取ってあった判定は、`../archive/haiku-contract-iteration/` と `../archive/luna-comparison/` に JSON で残っています。これを `../archive/noun-v5-one-off/import_votes.py` で取り込みました。判定の条件は同じなので、作り直すなら手順 3 のコマンドで取り直せます。

## akunuki の写しとの差

akunuki が埋め込む写しは、表の本文が同一です。違うのは冒頭の注記だけなので、表を作り直して akunuki へ渡すときは、注記の差を確かめてください。
