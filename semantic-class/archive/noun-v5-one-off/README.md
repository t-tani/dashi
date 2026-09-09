# 名詞の意味分類表で 1 回だけ使ったスクリプト

配布した名詞の意味分類表の判定を作るときに、1 回だけ使ったスクリプトを置く。`import_votes.py` は、評価コーパスに現れた語の判定を JSON から JSONL へ取り込む。取り込み元と取り込み先は `../../noun/votes/README.md` にある。`measure_tokens.py` は、1 バッチ 20 語の分類に要るトークン数を測る。この数は費用の概算に使う。

どちらもそのままでは動かない。`import_votes.py` は、`../../pipeline/` にない `jmdict_nouns` を import する。`measure_tokens.py` は `classifier.make_predictor` を引数なしで呼ぶが、`../../pipeline/classifier.py` の `make_predictor` は分類の対象を引数に取る。
