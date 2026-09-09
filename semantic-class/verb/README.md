# 動詞の意味分類表

`verb-semantic-class-table.tsv` は、akunuki が埋め込む動詞の意味分類表で、利用条件は CC BY 4.0 です。入力の見出し語と判定の元データと再現手順も同じフォルダにあります。生成には claude-haiku-4-5-20251001 を使い、判定は 2026-09-04 に取りました。

この表は、配布のバージョンを付けていない候補の表です。バージョンを付ける工程は `build_release_table.py` が担いますが、メンテナーの裁定(2026-09-04)により、akunuki は候補の表をそのまま埋め込んでいます。そのため、akunuki の写しはこの表とバイト単位で同一です。

対象は、サ変を除く動詞です。「勉強する」のようなサ変は、「する」の前の形を名詞の表で判定できるため、この表には載せません。分類番号は用の類の `2.mm00` で、中項目は 23 個です。

名詞の表と違い、評価コーパスに現れた語の一覧は持ちません。動詞は活用するため、名詞で使った見出し語の部分文字列の走査では、コーパスに現れる語を拾えないからです。

判定は 3 パス取り、どのパスも全語を対象にします。母集団が小さいので、日常語に絞る必要がないからです。

## 構成

このフォルダが持つファイルとフォルダは、次のとおりです。

| パス | 中身 |
|---|---|
| `verb-semantic-class-table.tsv` | 見出し語と分類番号(`2.mm00`)の行を 6,768 語ぶん持つ候補の表 |
| `target.py` | 分類対象の定義 |
| `demos-rationale.txt` | few-shot の実例をどう選んだか |
| `inputs/jmdict-verb-headwords.txt` | JMdict のサ変を除く動詞の見出し 6,785 語 |
| `votes/` | 判定の JSONL と確定値と来歴 |
| `review/` | 判定が割れた語(モデル間で食い違った語と、確定しなかった語) |

`target.py` が持つのは、指示文(生成の方針と 23 中項目の一覧)、few-shot の実例、分類番号の規約、採る品詞、来歴の注記です。

`votes/` のファイルの分かれ方は `votes/README.md` に、`review/` の詳細は `review/README.md` にあります。

## 再現手順

コマンドは、`../pipeline/README.md` の準備を済ませたうえで、`semantic-class/` の中で実行します。 `<JMdict_e.xml>` は JMdict の XML を指しますが、dashi には含まれていません。`inputs/` の一覧は JMdict created 2026-09-03 から抜いたもので、同じバージョンなら手順 1 は同じ一覧に戻ります。

```
# 1. JMdict のサ変を除く動詞の見出しを抜く
uv run python pipeline/headwords.py --target verb \
    --jmdict <JMdict_e.xml> --out verb/inputs/jmdict-verb-headwords.txt

# 2. 判定を 3 回取る。種を変えるとバッチの組み方が変わる
#    --model-key を OPENAI_MODEL に替えると、もう一方のモデルで同じ判定を取る
uv run python pipeline/run_votes.py --target verb \
    --words verb/inputs/jmdict-verb-headwords.txt \
    --out-dir verb/votes --model-key ANTHROPIC_MODEL --tier jmdict \
    --jmdict <JMdict_e.xml> --seed 694
#    同じコマンドを --seed 1002 と --seed 1003 でもう 2 回

# 3. 判定の JSONL を検証する。フィールドと中項目の範囲と語の重複を見る
uv run python pipeline/verify.py --target verb \
    verb/votes/*.jsonl

# 4. 3 回の判定から多数決で確定値を作る。確定しなかった語と落ちた語義は review/ へ
uv run python pipeline/finalize.py --votes-dir verb/votes \
    --model claude-haiku-4-5-20251001 --seeds 694,1002,1003 \
    --out-dir verb/votes/final --eval-dir verb/review

# 5. 生成の来歴を書き出す。全パスの走行が終わってから実行する
uv run python pipeline/manifest.py --target verb \
    --out-dir verb/votes --jmdict-version "JMdict created 2026-09-03"

# 6. 候補の表を書く。冒頭の注記(方針・語の内訳)を manifest から取る
uv run python pipeline/build_table.py --target verb \
    --votes-dir verb/votes/final --model claude-haiku-4-5-20251001 \
    --manifest verb/votes/manifest.json \
    --out verb/verb-semantic-class-table.tsv

# 7. 2 つのモデルの確定値が割れた語を並べる。手順 4 をもう一方のモデルでも済ませてから
uv run python pipeline/disagreement.py --final-dir verb/votes/final \
    --model-a claude-haiku-4-5-20251001 --model-b gpt-5.6-luna \
    --out verb/review/disagreement-v5-verbs.tsv
```

手順 2 は冪等です。出力の JSONL を読んで判定済みの語を飛ばすので、途中で止めても、同じコマンドを出し直せば残りだけを拾います。ただし、モデルが空の分類を返す語を含むバッチは検証で弾かれます。その語は同じ種では同じバッチに落ちるため、未判定のまま残ります。残った語の一覧は `votes/failed-*.json` にあります。判定のない語は表に載らず、検出器は指摘を出さないので、安全側です。

手順 4 と 6 と 7 は API を呼びません。`votes/` の JSONL から決定的に作れるので、表と `review/` の一覧は同じ出力に戻ります。手順 6 で変わるのは、注記の日付の行だけです。

配布のバージョンを付けて出すときは、手順 6 の代わりに `build_release_table.py` を実行します。オプションには `--target verb` と `--version <配布のバージョン>` を付けます。
