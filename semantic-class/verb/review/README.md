# 分類が一致しなかった語の一覧(動詞)

`review/` には、動詞の意味分類表を作る過程で分類が一致しなかった語が入ります。一致しなかった相手は、2 つのモデルの間か、種を変えた 3 つのパスの間です。一致するかどうかを見る単位は中項目で、分類語彙表が意味の区分に与える 2 桁の番号です。

`review/` のファイルを書き出すのは、`../votes/` を読む 2 つのスクリプトです。`../../pipeline/finalize.py` は、3 つのパスの判定から確定値を作るときに、確定しなかった語と落ちた中項目を書き出します。できあがった確定値を 2 つのモデルで突き合わせるのが `../../pipeline/disagreement.py` です。コマンドは `../README.md` の手順 4 と 7 にあります。

`disagreement-v5-verbs.tsv` の列は、`../../pipeline/disagreement.py` の `FIELDS` と冒頭の docstring が定めます。残りの TSV の列を定めるのは `../../pipeline/finalize.py` です。

`review/` に置くファイルは次のとおりです。

| ファイル | 中身 |
|---|---|
| `disagreement-v5-verbs.tsv` | 2 つのモデルが違う中項目の集合を与えた語 |
| `disagreement-v5-verbs-summary.json` | `disagreement-v5-verbs.tsv` の件数の要約 |
| `unstable-claude-haiku-4-5-20251001.tsv` | 3 つのパスの答えがすべて割れて確定しなかった語(Haiku) |
| `unstable-gpt-5.6-luna.tsv` | 3 つのパスの答えがすべて割れて確定しなかった語(luna) |
| `dropped-senses-claude-haiku-4-5-20251001.tsv` | 2 つのパスに届かず確定値から落ちた中項目を持つ語(Haiku) |
| `dropped-senses-gpt-5.6-luna.tsv` | 2 つのパスに届かず確定値から落ちた中項目を持つ語(luna) |

`disagreement-v5-verbs.tsv` の行は、2 つのモデルの中項目の集合に交わりがない語を先頭に置いて並べてあります。
