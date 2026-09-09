# 旧世代のモジュール

`archive/` のスクリプトが import するモジュールのうち、`../../pipeline/` にないものを置く。`../../pipeline/` が持つのは生成に使う実装だけなので、`archive/` の記録を読み直すときと、記録の数を出し直すときには `legacy/` のモジュールを使う。コードは `archive/` のスクリプトが呼ぶ実装のままだが、コメントの記録済みの語(`束`・`票`・`生成契約`)は `../../pipeline/` と同じ語に置き換えてある。

各モジュールの中身と import する側は次のとおりである。

| ファイル | 中身 | import する側 |
|---|---|---|
| `classifier.py` | 語義を絞る方針の分類器 | `haiku-contract-iteration/` |
| `demos.py` | 語義を絞る方針の few-shot の実例 | `classifier.py` |
| `classifier_v2.py` | 訂正後の方針の分類器 | `haiku-contract-iteration/`・`luna-comparison/`・`batch-effect/` |
| `demos_v2.py` | 訂正後の方針の few-shot の実例 | `classifier_v2.py` |
| `lm_v3.py` | ゲートウェイを叩く LM の組み立て | `luna-comparison/`・`batch-effect/` |
| `votes_v5.py` | 判定の保存形式の読み書き | `batch-effect/` |
| `jmdict_nouns.py` | JMdict から名詞の見出しを抜く | `noun-v5-one-off/` |

`classifier.py` は `WORK`・`load_env`・`find_context`・`make_lm` を持つ。
