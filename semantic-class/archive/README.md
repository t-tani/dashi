# 終わった世代と実験の記録

意味分類表を作る過程で終わった実験と、使わなくなった世代のスクリプト・表・判定を、1 件 1 フォルダで置く。配布する表の生成には使わず、その手順は `../pipeline/` にある。それでも消さずに残してあるのは、`../eval/` の報告が数字の由来として archive のスクリプトと表を指すためである。

各フォルダには、その実験の README と、設定と、使ったスクリプトと、生成した表と判定が入る。パスは `archive/` からの相対で書く。

コマンドは、その記録のフォルダの中で実行する。だから `PYTHONPATH` の値だけは、記録のフォルダからの相対で書く。値に足すのは、`legacy/` と、import する側の実験のフォルダと、`../pipeline/` である。たとえば `shipped-default-diff/` では次のように呼ぶ。

```
PYTHONPATH=../legacy:../finding-verdicts python sample.py before
```

archive の Python スクリプトは、`__pycache__` を除いて 52 本ある。45 本はこの形で import が通り、残る 7 本は通らない。通らない理由は 2 通りある。5 本は `sample_keys` か `score_bench` を import し、2 本は `../../pyproject.toml` の依存に無いパッケージを import する。

`sample_keys` と `score_bench` は、どのコミットにも入っていない。だから次の 5 本は、書き直さない限り採点をやり直せない。

- `finding-verdicts/estimate_v2.py`
- `haiku-contract-iteration/analyze_sweep.py`
- `haiku-contract-iteration/score_haiku.py`
- `luna-comparison/score_v3.py`
- `luna-comparison/table_stats_v3.py`

依存が欠ける 2 本は `gateway-check/` にある。`test_gateway.py` が `anthropic` を、`test_gateway_pydantic.py` が `pydantic_ai` を import する。

import が通る側にも、実行の前提がある。判定を取り直すには API キーが、lint を掛け直すには評価コーパスが要る。

実験のフォルダと、その実験が答えた問いは次のとおり。

| フォルダ | 答えた問い |
|---|---|
| `wordnet-mapping/` | 意味分類表の生成元を分類語彙表から日本語 WordNet の写像へ移せるか |
| `haiku-contract-iteration/` | 表の生成を LLM に移すとき、どの語義を挙げよと指示すれば誤検出を出さない表になるか |
| `luna-comparison/` | 同じ方針で作った Haiku の表・luna の表・その和集合のうち、どれが最もよいか |
| `software-class-estimate/` | 無形の人工物を受ける中項目 48 を足すと誤検出はどれだけ減るか |
| `finding-verdicts/` | `concrete-noun-misfit` の指摘は、どれだけが直すべき文を指しているか |
| `detector-diff-scoring/` | 表を差し替えると `role-conflict` と `standalone-calque` の指摘はどう変わるか |
| `shipped-default-diff/` | 配布した既定どうしで検出はどう変わったか |
| `batch-effect/` | バッチの組み方は判定を動かすか |

実験ではないフォルダの中身は次のとおり。

| フォルダ | 中身 |
|---|---|
| `noun-v5-one-off/` | 配布した名詞の表を作るときに 1 回だけ使ったスクリプト |
| `gateway-check/` | LLM のゲートウェイへ疎通するかを試したスクリプト |
| `legacy/` | archive のスクリプトが import する旧世代のモジュール |

評価の枠組みは `../eval/methodology.md` にある。正解集合を作った記録は `../eval/ground-truth/`、誤検出を減らす手法を比べた記録は `../eval/misfit-reduction/` に置いてある。

## 生成を試した世代の表と判定

生成を試した世代の表と判定は、それを作った実験のフォルダに置いてある。ファイル名の接頭辞から、どの実験のものかがわかる。

| 接頭辞 | フォルダ |
|---|---|
| `haiku-` | `haiku-contract-iteration/` |
| `luna-` | `luna-comparison/` |
| `union-` | `luna-comparison/` |
| `haiku48-` | `software-class-estimate/` |
| `wnja-` | `wordnet-mapping/` |

別の実験がその表を使った場合は、使った側の README がフォルダをまたいで指す。どの実験がどの表を使ったかは、各 README の入力データの表で確かめる。

判定の JSON は、来歴をファイル名と生成スクリプトに預けている。ファイルを移すと判定の条件をたどれなくなるため、名前は変えていない。配布した名詞の表の判定のうち、評価コーパスに現れた語の分は `noun-v5-one-off/import_votes.py` で取り込んだ。取り込み元は `haiku-contract-iteration/haiku-v2-classifications.json` と `luna-comparison/luna-v3-classifications.json` である。
