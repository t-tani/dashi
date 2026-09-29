# 単独で立つ漢字 1 字の名詞の判定モデル

このモデルは、漢字 1 字の名詞が単独で立つ出現ごとに得点を出し、LLM が外来語の代わりに書いた字や比喩で立てた字(`欄`・`版`・`窓`)を見分けます。akunuki の検出器は、得点が線を超えた出現を指摘します。

これらの字は字面が正しい日本語なので、造語や表記の検査には掛かりません。一方、人の IT 文書では、これらの字は単独の名詞としてほとんど使われず、使われる場合も前後の語との係り受けの形が違います。モデルはこの形の違いを見ます。

字そのものは特徴に入れません。同じ字でも、暗号の `鍵をローテートする` や周波数の `13.56MHz 帯` のように、技術文書で正しく使う出現があるからです。字は、人の IT 文書での回数と、同じ字の人の用法を引く鍵にだけ使います。

## 構成

| パス | 中身 |
|---|---|
| `pipeline/features.py` | 出現 1 件から特徴 89 列を作る処理 |
| `pipeline/tables.py` | 珍しさの表、対象の字の表、corpus-tool に渡す語の一覧を作る処理 |
| `pipeline/train.py` | モデルを学び、線を決めて JSON に書き出す処理 |
| `pipeline/model.py` | 書き出した JSON の木をたどって得点を出す処理 |
| `pipeline/make_sample.py` | 人の IT 文書の抽出を、日本語コーパスから一覧どおりに写す処理 |
| `pipeline/prepare_text.py` | 部分木を書き出す前に、平文の表とリンクを整える処理 |
| `fix-chars.txt` | 人手の判定で直すとした字の一覧 |

正例は、`fix-chars.txt` の字の出現から選びます。出現の部分木と修飾と格と述語の組は、`compound-frequency/corpus-tool/` の `extract-subtrees` が書き出し、回数の表は同じ corpus-tool の `count-case-frames` が数えます。どちらも akunuki の `aku_morph` の同じ関数で解析するので、検査する側の akunuki と同じ部分木が出ます。

## 成果物

成果物は、ほかの成果物と同じ GitHub のリリースに、次の名前のアセットとして載せます。来歴は `single-kanji-noun-manifest.json`(このディレクトリの `manifest.json` と同じ内容)が持ちます。

| ファイル | 中身 |
|---|---|
| `single-kanji-noun-model.json` | 判定モデル |
| `single-kanji-noun-novelty.tsv.zst` | 珍しさの表 |
| `single-kanji-noun-targets.tsv` | 対象の字の表 |
| `single-kanji-noun-case-frame-counts.tsv` | 人の IT 文書での、字と格と述語の組の回数 |
| `single-kanji-noun-noun-modifier-counts.tsv` | 人の IT 文書での、字に係る修飾の回数 |

### モデル

`single-kanji-noun-model.json` は、LightGBM の 2 つのブースターを書き出したものです。得点は、ブースターごとの生の得点を人の抽出での平均と標準偏差で標準化し、0.7 と 0.3 の重みで足した値です。

| キー | 中身 |
|---|---|
| `features` | 特徴の列の名前の並び |
| `categorical` | カテゴリの列ごとの水準 |
| `boosters` | ブースターごとの `weight`・`mean`・`std` と、LightGBM の `dump_model()` の `tree_info` |
| `lines` | 人の技術書 1 MB あたりの件数ごとの線 |

木の `split_feature` は、`features` の並びの番号を指します。カテゴリの値は `categorical` の水準の番号にして木へ渡し、水準にない値は欠損にします。`lines` は、1 MB あたり 2 件、5 件、10 件、20 件が超える得点を持ちます。

木のたどり方は LightGBM と同じです。`pipeline/model.py` がその実装で、`train.py` は書き出した JSON での得点が LightGBM の得点と一致することを確かめてから書き出します。

### 表

`single-kanji-noun-novelty.tsv.zst` は zstd で圧縮した TSV で、人の IT 文書の抽出の出現から、部分木の断片と組み合わせの鍵を数えたものです。行は `<種類>\t<グループ>\t<字か分類>\t<鍵>\t<回数>` で、種類は次の 4 つです。

| 種類 | 数える単位 |
|---|---|
| `wf` | 字と鍵の組 |
| `cf` | 意味分類(分類番号の先頭 4 字)と鍵の組 |
| `wn` | 字の出現 |
| `cn` | 意味分類の出現 |

`wn` と `cn` の行は、グループと鍵が空です。グループは 17 あり、鍵の作り方で 3 つに分かれます。`nov:T<文節数>:<水準>` の 6 グループの鍵は、中心の文節を字の意味分類で書いた断片です。`frag:<水準>` の 5 グループの鍵は、中心を伏せた断片で、`combo:<型>` の 7 グループの鍵は、格と述語の分類のような 1 つの組み合わせです。作り方の詳細は `pipeline/features.py` の `novelty_keys` にあります。

`single-kanji-noun-targets.tsv` は、置き換え表(`loanword-substitution/substitution-table.tsv`)の漢字 1 字の語ごとに、次の列を持ちます。

| 列 | 中身 |
|---|---|
| `char` | 字 |
| `class` | 意味分類表の先頭の分類番号の 4 字 |
| `human_per_mb` | 人の IT 文書で単独の名詞として出る、1 MB あたりの回数 |
| `loanwords` | 置き換え表でこの字に対応する外来語の、`,` 区切りの並び |
| `use` | 検出器の対象にするかどうか(1 か 0) |
| `reason` | 対象にしない理由 |

対象にしない字は 3 種類です。1 つ目は位置・関係(分類番号 1.17)の字で、`側`・`先`・`外` のように、使い方の違いでは判定できません。2 つ目は、人の IT 文書で 1 MB あたり 5 回以上出る字です。`鍵` と `行` がこの例で、技術文書で本来の意味の用法が多い字です。3 つ目は `癖` と `章` で、文体を論じる文書と本の中で普通に使う字です。

回数の 2 つの表は、`count-case-frames` の出力をそのまま載せます。数えた語は、置き換え表の漢字 1 字の語 753 と、それに対応する外来語 647 です。

## 作り方

コマンドは `single-kanji-noun/` の中で実行します。`corpus-tool` は `compound-frequency/corpus-tool/target/release/corpus-tool` を指します。

まず、corpus-tool に渡す語の一覧を作り、人の IT 文書で回数を数えます。数えるのは日本語コーパスのうち情報技術の文書のソースで、それ以外のソースは `--exclude` に並べて外します。

```
uv run python pipeline/tables.py words --out words.txt
corpus-tool count-case-frames --corpus-dir <日本語コーパスの data/> \
    --exclude <情報技術でないソース> \
    --targets words.txt --out-dir counts
```

次に、学習の入力の平文を整え、出現を書き出します。人の IT 文書の抽出は `make_sample.py` で写し、`prepare_text.py` で整えます。人の技術書は、textlint-ja が集めた技術書 9 冊の本文を平文にしたもので、ファイル名の頭に本の名前を付けて 1 つのディレクトリに置きます。リポジトリの文章は、akunuki のリポジトリの Markdown とコメントと文字列を平文にしたものです。どちらも `prepare_text.py` で整えてから書き出します。

```
uv run python pipeline/make_sample.py <文書の一覧> <日本語コーパスの data/> human-text
uv run python pipeline/prepare_text.py human-text human-prep
corpus-tool extract-subtrees --text-dir human-prep --targets words.txt --out human.jsonl
corpus-tool extract-subtrees --text-dir <リポジトリの平文> --exclude dict,tests --targets words.txt --out repo.jsonl
corpus-tool extract-subtrees --text-dir <人の技術書の平文> --targets words.txt --out techbook.jsonl
```

最後に、表を作ってモデルを学びます。`--techbook-bytes` には、人の技術書の平文の `.txt` の合計のバイト数を渡します。

```
uv run python pipeline/tables.py novelty --human human.jsonl --counts counts --out novelty.tsv.zst
uv run python pipeline/tables.py targets --counts counts --out targets.tsv
uv run python pipeline/train.py --human human.jsonl --repo repo.jsonl --techbook techbook.jsonl \
    --techbook-bytes 4119049 --counts counts --novelty novelty.tsv.zst --out model.json
```

## 学習と評価

正例は、akunuki のリポジトリの文章で、人手の判定が直すとした字の出現 1,461 件(34 字)です。負例は、人の IT 文書の抽出の出現 59,194 件と、人の技術書の出現 6,662 件です。人の技術書の出現からは、ひらがなを含まない文、日付の後と見出しに並べた曜日、1 字の漢字の列挙を外します。学習の入力は配りません。

人の技術書は学習にも使うので、線は、1 冊を除いて学んだモデルで除いた本を採点して決めます。人の技術書 9 冊は 4.12 MB です。判定の正解は、LLM が書いた外部のリポジトリの出現に人手で付けた直す・残すです。akunuki の検出器は出現を入力の条件で外してから採点するので、表の件数も外した後の数です。

| 線 | 判定済みの直す(81 件) | 判定済みの残す(13 件) | 無作為に抜いた直す(28 件) | 無作為に抜いた残す(2 件) | 人の技術書で超える出現 |
|---|---|---|---|---|---|
| 10 回/MB | 51 | 4 | 16 | 2 | 3.9 回/MB |
| 20 回/MB | 65 | 7 | 26 | 2 | 7.8 回/MB |

無作為に抜いた 30 件は、マスク言語モデルの予測確率を足したモデルの線の上から抜いたものです。このモデルはその確率を使わないので、拾う数は低めに出ます。

## 利用条件

表とモデルは CC BY 4.0 です。回数は最上位の README の参考文献に挙げた資料を含む文書から数え、意味分類は `semantic-class/`、字と外来語の対応は `loanword-substitution/` の表から引いています。
