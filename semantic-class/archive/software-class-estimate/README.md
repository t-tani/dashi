# 無形の人工物を受ける中項目の効果の推定

無形の人工物を受ける中項目 48 は採用していない。`concrete-noun-misfit` は、抽象的な語の多い段落を対象とし、そこに具体物の名詞が単独で立っている箇所を指摘する。ところがこの検出器は、`ファイル` や `モジュール` のようなソフトウェアの語まで具体物とみなし、誤検出を出す。名詞ごとに中項目を並べた意味分類表へ、無形の人工物を受ける中項目を足し、誤検出がどれだけ減るかを見積もった。足した中項目は、分類語彙表の 43 中項目の外に自前で作った 48 番である。あわせて、対訳表が語義ごとに持つ分野タグ `comp` で代用しても同じ効果が出るかを測った。

中項目 48 を足すと誤検出はよく減った。44 分類の表が出した指摘 812 件のうち、48 を得た語への指摘を落とすと 202 件が残り、4 分の 1 になった。基準にした 43 分類の表は 735 件だったので、それに対しては 27.5% にあたる。判定済みの誤検出 164 件のうち 96.3% が消えた。ただし妥当な指摘も道連れになり、妥当な指摘の推定件数は 43 分類の 49 件から、48 を除外した後の 7 件へ落ちた。分野タグ `comp` での代用は 561 件までしか減らず、残る誤検出の上位を 1 つも落とさなかった。

## 入力データ

表の生成と lint と採点に使った入力は次のとおりである。

| データ | パス | バージョン・日付 | 件数 |
|---|---|---|---|
| コーパスの見出し語 | `../../noun/inputs/corpus-headwords.txt` | 2026-08-30 | 7,743 語 |
| 44 分類の分類器 | `classifier_v3_48.py` | — | — |
| 44 分類の few-shot | `demos_v3_48.py` | — | 実例 29 語 |
| few-shot の選定理由 | `demos-v3-48-rationale.txt` | — | — |
| 44 分類の表 | `haiku48-v3-semantic-class-table.tsv` | 2026-08-30 | 7,228 行 |
| 43 分類の表(比較の基準) | `../haiku-contract-iteration/haiku-v2-semantic-class-table.tsv` | 2026-08-30 | 7,693 行 |
| 対訳表 | akunuki の `.aku/reference/gloss-table.tsv`(外部) | JMdict created 2026-08-24 | 48,462 行 |
| 中項目 48 で除外する語の一覧 | `drop-48.txt` | 2026-08-30 | 722 語 |
| 分野タグ `comp` で除外する語の一覧 | `drop-comp.txt` | 2026-08-30 | 13,758 語 |
| 判定済みの標本 | `../finding-verdicts/verdicts_v2.py` | — | 170 件 |
| 評価コーパス | 外部 | — | Markdown 300 ファイル |
| docs A | 外部 | — | Markdown 42 ファイル |
| docs B | 外部 | — | Markdown 9 ファイル |
| 44 分類の設定 | `configs/cfg-haiku48.toml` | 実験時のスナップショット | — |
| 43 分類の設定 | `../haiku-contract-iteration/configs/cfg-haiku2.toml` | 実験時のスナップショット | — |

## 前提条件

同梱の `configs/cfg-haiku48.toml` と、比較の基準の `../haiku-contract-iteration/configs/cfg-haiku2.toml` を diff すると、違うのは `abstract_ratio` の 1 行だけで、前者が 0.85、後者が 0.90 である。ただしこの値は実行時に残らない。`../luna-comparison/lint_v3.sh` が `abstract_ratio` の行を第 3 引数で上書きするため、どちらの実行も 0.85 で揃えた。どちらの設定も意味分類表を `reference/semantic-class-table.tsv` という相対パスで指すので、43 分類の表と 44 分類の表の違いは、設定と並べて置いた `reference/` の中身が持っていた。その `reference/` はこのフォルダに入っていない。2 つの実行で変えたのは、この表だけである。

指摘を出した akunuki のバイナリの版は記録がない。`../luna-comparison/lint_v3.sh` は環境変数 `AKUNUKI_DIR` の下の `target/release/aku` をそのまま呼び、版を書き出していない。

リポジトリに入っていないものは、3 つのコーパスと、設定と並べて置いた `reference/` である。akunuki の対訳表 `.aku/reference/gloss-table.tsv` は、akunuki を持っていれば手元にある。コーパスは手元の Markdown へ差し替えれば lint 自体は動くが、件数は `結果` と比べられない。44 分類の表の生成は API を呼ぶため、API キーがあっても同じ表は再び出ない。一方、その表は `haiku48-v3-semantic-class-table.tsv` に、除外した語の一覧は `drop-48.txt` と `drop-comp.txt` に、指摘は `findings-*.json` にある。除外と採点は、この 3 つだけで同じ数字を再現できる。

## 手順

検出器のコードは変えずに、次のコマンドで表を作り、除外する語の一覧を書き出し、lint と採点まで実行した。48 を持つ語への指摘は、後段のスクリプトで落としている。**これは除外の規則を足した場合のシミュレーションであり、実測ではない。**

```
# 44 分類で 7,743 語を分類して表を作る(API を呼ぶ)
python ../luna-comparison/build_table_v3.py ANTHROPIC_MODEL classifier_v3_48 haiku48-v3

# 除外する語の一覧を 2 通りで書き出す
python drop_words_v3.py 48    # 中項目 48 を得た語
python drop_words_v3.py comp  # comp の分野タグを持つ語

# 3 つのコーパスへ lint を掛ける
bash ../luna-comparison/lint_v3.sh cfg-haiku48 haiku48 0.85

# 除外の前と後で採点する
python ../luna-comparison/score_v3.py haiku48 0.85                  > score-v3-haiku48-raw.txt
python ../luna-comparison/score_v3.py haiku48 0.85 drop-48.txt      > score-v3-haiku48-filtered.txt
python ../luna-comparison/score_v3.py haiku2  0.85 drop-comp.txt    > score-v3-haiku2-comp.txt
```

## 結果

除外の前と後の数字は次のとおりである。適合率は、検出器が出した指摘のうち妥当だったものの割合であり、残った指摘から種 694 で 30 件を抜き、1 件ずつ当否を判定して数えた。判定は LLM による自動のもので、人手の確認を経ていない。妥当な指摘の推定件数は、この適合率に指摘の総数を掛けた概算である。妥当な 6 件は、判定済みの標本 170 件のうち妥当と判定した 6 件を指し、残る 164 件が誤検出である。

| 指標 | 43 分類(基準) | 44 分類・除外前 | 44 分類・48 除外 | 43 分類・comp 除外 |
|---|---|---|---|---|
| 指摘の総数 | 735 件 | 812 件 | 202 件 | 561 件 |
| 妥当な 6 件の保持 | 3 件 | 3 件 | 2 件 | 3 件 |
| 判定済みの誤検出 164 件の消滅 | 85.4% | 90.9% | 96.3% | 88.4% |
| 標本の適合率 | 2/30 = 6.7% | — | 1/30 = 3.3% | 1/30 = 3.3% |
| 妥当な指摘の推定件数 | 49 件 | — | 7 件 | 19 件 |

48 を得た語は 722 で、そのうち 204 語は 48 だけを持つ。`ファイル`・`ブラウザ`・`パッケージ` は 48 を得た。`モジュール`・`コンポーネント`・`システム` も同じで、語の側では狙いどおりに拾えた。

ただし 48 を部門 4 に置いたままでは、48 だけを持つ 204 語が具体物とみなされ、除外の規則を足す前の指摘はかえって増えた。43 分類の表の 735 件に対して、44 分類の表は 812 件だった。減りはすべて後段の除外から来ている。除外すると `バケット` も落ちるため、妥当な指摘の保持は 3 件から 2 件へ下がった。語の単位で落とす規則は、その語が計算機以外の文脈で転用された場合も落とす。

`comp` の案は被覆が足りない。タグは 13,758 語に付くが、残る誤検出の上位である `ブラウザ`・`パッケージ`・`モジュール`・`コンポーネント` にはどれも付かなかった。指摘は 735 件から 561 件へ、24% しか減らなかった。

## ファイル一覧

このフォルダに置いた成果物は次のとおりである。

| ファイル | 中身 |
|---|---|
| `classifier_v3_48.py` | 44 分類で語を分類させる分類器 |
| `demos_v3_48.py` | 分類器に渡す few-shot の実例 29 語 |
| `demos-v3-48-rationale.txt` | few-shot の各語に中項目を与えた理由 |
| `haiku48-v3-classifications.json` | 分類器が語ごとに返した中項目の一覧 7,228 語 |
| `haiku48-v3-semantic-class-table.tsv` | 44 分類の意味分類表 7,228 行 |
| `configs/cfg-haiku48.toml` | 44 分類の lint に使った設定 |
| `drop_words_v3.py` | 除外する語の一覧を 2 通りで書き出す |
| `drop-48.txt` | 中項目 48 を得た 722 語 |
| `drop-comp.txt` | 対訳表で `comp` の分野タグを持つ 13,758 語 |
| `findings-haiku48-0.85.json` | 44 分類の表での指摘 812 件 |
| `findings-haiku48-0.85-filtered.json` | 48 を除外した残り 202 件 |
| `findings-haiku2-0.85.json` | 43 分類の表での指摘 735 件 |
| `findings-haiku2-0.85-filtered.json` | `comp` を除外した残り 561 件 |
| `score-v3-haiku48-raw.txt` | 除外前の採点 |
| `score-v3-haiku48-filtered.txt` | 48 除外後の採点 |
| `score-v3-haiku2-comp.txt` | `comp` 除外後の採点 |

## 当否の判定の元データ

指摘 1 件ごとの当否と、その理由は `../luna-comparison/verdicts_v3.py` にある。
