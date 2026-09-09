# 造語の例の区分

複合語の頻度フィルタの評価に使う造語の集合は、akunuki の記録から採った 207 語である。この 207 語(`coined.tsv` と `coined-unknown.tsv`)を、採り先の記録が持つ意味で 3 つに分ける。`造語` は語そのものが存在しない語である。実在する用語のうち、akunuki が別の表記へ寄せた語は `言い換え`、別の意味や曖昧な意味で使った語は `意味の転用` に置く。区分は 2026-09-07 に付けたもので、区分の判定者は 207 語とも `claude-fable-5-1` である。メンテナーが確かめた語は、TSV の区分の判定者を `maintainer` へ書き換える。

区分は、語を採った記録が何を記録しているかで決める。`.aku/dict/fix/wording/user.prh.yml` は実在する用語を選んだ表記へ寄せる記録なので、そこから採った 53 語は一律に `言い換え` にする。`seeds/dict-specs.yaml` の 6 語のうち 5 語も `aku fix` の置換の仕様を持つので、同じ区分へ置く。残る 149 語は、`.aku/dict/report/avoid/user.yaml` の hint、`docs/slop-inventory.yaml` の referent と note、akunuki の issue の本文を 1 語ずつ読んで決める。

理由の列が「迷った」で終わる語は、`造語` と `意味の転用` のどちらにも読める。記録が咎めるのが語ではなく形であるとき、また複合の部品だけが他の分野からの転用であるときに、この読みの割れが起きる。

次の表は、207 語を 1 行ずつ並べる。採り先の列は、語を採った akunuki のリポジトリのパスか issue の番号である。理由の列には、その語をその区分に置いた根拠を書く。`記録は` を含むセルからは、採り先の記録が語に求める直し先がわかる。

セルが挙げる Wikipedia の記事数と出現回数は、cirrussearch のダンプ `jawiki-20251229-cirrussearch-content.json.gz`(2025-12-29)を数えた値である。記事数は、形態素解析を挟まず表層形で照合し、その語を本文に含む記事を数えた。出現回数は、複合語の頻度フィルタが同じダンプから数えた回数で、10 回以上はフィルタが既存語と読む境界にあたる。

| 語 | 区分 | 採り先 | 理由 |
|---|---|---|---|
| `キュレーション資源` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `資源キュレーション` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `実悪用` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `悪用を確認` か `実際に悪用されている` へ開くよう書く |
| `痕跡検出` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `OPSEC の痕跡を洗い出す` へ開くよう書く |
| `端末悪用` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `POS 端末を悪用した事案` へ開くよう書く |
| `隔離描画` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `shadow DOM で隔離して描画する` へ開くよう書く |
| `指摘検知` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `XSS の指摘を検知する` へ開くよう書く |
| `連鎖感染` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `次々と感染が広がる` へ開くよう書く |
| `ブラウザ搾取` | 造語 | `.aku/dict/report/avoid/user.yaml` | 英語の直訳で、記録は `ブラウザの脆弱性を悪用する` か `ブラウザをフックする` へ開くよう書く |
| `成果側` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `拾えたもの` へ開くよう書く |
| `ノイズ側` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `宣伝として落としたもの` へ開くよう書く |
| `最高深刻度` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `高深刻度` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `深刻度が高い` か `重大な脆弱性` へ開くよう書く |
| `低複雑度` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `攻撃条件の複雑さが低い` へ開くよう書く |
| `低複雑性` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `攻撃条件の複雑さが低い` へ開くよう書く |
| `重大クラス` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `発見元` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `報告元` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `悪性版` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `改ざんされたバージョン` か `〜を改ざんして` へ開くよう書く |
| `脆弱版` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `脆弱性を含むバージョン` か `影響を受けるバージョン` へ開くよう書く |
| `インターネット露出` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `インターネットに公開されている` へ開くよう書く |
| `実被害` | 造語 | `.aku/dict/report/avoid/user.yaml` | 記録は実際の被害を詰めた形として扱うが、新聞の文にも現れる語なので意味の転用と迷った |
| `実攻撃` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `実際の攻撃` へ開くよう書く |
| `実環境悪用` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `実際の攻撃で悪用されている` へ開くよう書く |
| `攻撃主張` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `〜が犯行を主張した` か `サイバー攻撃だとする主張` へ開くよう書く |
| `影響デバイス` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `影響を受けたデバイス` へ開くよう書く |
| `影響アカウント` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `影響を受けたアカウント` へ開くよう書く |
| `多段感染` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `複数段階の感染チェーン` か `段階を踏んで感染させる` へ開くよう書く |
| `標的拡大` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `標的を広げる` へ開くよう書く |
| `不正実行` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `許可なく実行する` へ開くよう書く |
| `追加注入` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `追加で注入する` へ開くよう書く |
| `制御奪取` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `制御を奪う` か `乗っ取る` へ開くよう書く |
| `攻撃波` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `攻撃の波` か `一連の攻撃` へ開くよう書く |
| `悪用ハードル` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `悪用の難しさ` か `悪用に必要な前提` へ開くよう書く |
| `流出対象` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `侵害成功` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `侵害に成功した` へ開くよう書く |
| `連鎖悪用` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `複数の脆弱性を組み合わせる` へ開くよう書く |
| `自動悪用` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `自動的に攻撃を行う` か `自動で悪用を試みる` へ開くよう書く |
| `感染フロー` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `感染の流れ` へ開くよう書く |
| `帰属証拠` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `帰属の根拠` へ開くよう書く |
| `人間承認` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `人間による承認` へ開くよう書く |
| `蒸留疑惑` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `蒸留した疑い` へ開くよう書く |
| `操作攻撃` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `操作して攻撃する` へ開くよう書く |
| `自動集約ソース` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `自動で集めた情報源` へ開くよう書く |
| `フィッシング起点` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `フィッシングを起点にした` へ開くよう書く |
| `外部送出` | 造語 | `.aku/dict/report/avoid/user.yaml` | 動詞句を名詞で詰めた形で、記録は `外部へ送信する` へ開くよう書く |
| `侵入後活動` | 造語 | `.aku/dict/report/avoid/user.yaml` | 助詞を落として詰めた形で、記録は `侵入後の活動` へ開くよう書く |
| `環境変数窃取` | 造語 | `.aku/dict/report/avoid/user.yaml` | 助詞を落として詰めた形で、記録は `環境変数から窃取する` へ開くよう書く |
| `無害化不備` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `無害化の不備` か `サニタイズ不足` へ開くよう書く |
| `破壊的操作` | 造語 | `.aku/dict/report/avoid/user.yaml` | 記録は破壊的な操作を詰めた形として扱うが、取り置いた技術文書の平文にも現れるので意味の転用と迷った |
| `未認証リモートコード` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `未認証でコードを実行できる` へ開くよう書く |
| `未認証任意ファイル` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `未認証で任意のファイルを〜できる` へ開くよう書く |
| `未認証認可バイパス` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `未認証で認可をバイパスできる` へ開くよう書く |
| `リモート未認証` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `リモートから認証なしで` へ開くよう書く |
| `ローカル低権限` | 造語 | `.aku/dict/report/avoid/user.yaml` | 助詞を落として詰めた形で、記録は `ローカルの低権限〜` へ開くよう書く |
| `未認証ファイル` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `未認証でファイルを〜できる` へ開くよう書く |
| `未認証データ` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `未認証でデータを〜できる` へ開くよう書く |
| `未認証コード` | 造語 | `.aku/dict/report/avoid/user.yaml` | 述語を名詞で詰めた形で、記録は `未認証でコードを実行できる` へ開くよう書く |
| `高権限リモート` | 造語 | `.aku/dict/report/avoid/user.yaml` | 助詞を落として詰めた形で、記録は `高い権限を持つリモート〜` へ開くよう書く |
| `発見レポート` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `攻撃者エコシステム` | 造語 | `.aku/dict/report/avoid/user.yaml` | 記録は攻撃者側の何を指すかが読めない形として扱うが、曖昧なのは生物学から借りた部品の側なので意味の転用と迷った |
| `キュレーションコレクション` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は何を指すのかを書くよう求める |
| `ポスト侵害` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `侵入後の活動` へ開くよう書く |
| `中間者型` | 造語 | `.aku/dict/report/avoid/user.yaml` | 記録は `中間者(AiTM)型の` から助詞を落とした形として扱うが、中間者が定着語なので意味の転用と迷った |
| `命令インジェクション` | 造語 | `.aku/dict/report/avoid/user.yaml` | 記録は 2 つの定着語のどちらでもない名として扱うが、指す先が 2 つに割れるので意味の転用と迷った |
| `メモリ汚染` | 造語 | `.aku/dict/report/avoid/user.yaml` | 記録は 2 つの攻撃を 1 語で呼んだ形として扱うが、RAM への攻撃を指す用例があるので意味の転用と迷った |
| `脆弱性連鎖` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `複数の脆弱性を組み合わせる` か `脆弱性の連鎖` へ開くよう書く |
| `連鎖脆弱性` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `複数の脆弱性を組み合わせる` か `脆弱性の連鎖` へ開くよう書く |
| `野外悪用` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `実際の攻撃での悪用` へ開くよう書く |
| `国家系` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `国家支援型` へ開くよう書く |
| `安全ガード` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `安全のための制御` か `保護機構` へ開くよう書く |
| `セキュリティ媒体` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `セキュリティ系メディア` へ開くよう書く |
| `境界公開システム` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `境界機器` か `境界に置かれる製品` へ開くよう書く |
| `追記統合` | 造語 | `.aku/dict/report/avoid/user.yaml` | 句を詰めた形で、記録は `追記して統合した` へ開くよう書く |
| `生成癖` | 造語 | `docs/slop-inventory.yaml` | 記録の直し先は `LLM が出力する日本語の癖` であり、語そのものは辞書にも Wikipedia にもない |
| `幽霊参照` | 造語 | `docs/slop-inventory.yaml` | dangling reference の訳として作った語であり、記録が造語と書く |
| `貪欲分割` | 造語 | `docs/slop-inventory.yaml` | 分野の定着語は `最長一致法` であり、貪欲に分割を接いだ形は辞書にも Wikipedia にもない |
| `固定点判定` | 造語 | `docs/slop-inventory.yaml` | 数学の固定点を部品にして作った複合で、複合そのものは辞書にも Wikipedia にもないが、部品の側が転用なので迷った |
| `圧縮名詞` | 造語 | `docs/slop-inventory.yaml` | 解析辞書にない漢語の連なりを指すために作った語であり、同じものを漢語連結とも呼んでいた |
| `事故駆動` | 造語 | `docs/slop-inventory.yaml` | `事故` に `駆動` を接いだ形であり、記録の直し先は運用の書き下しである |
| `検出駆動` | 造語 | `docs/slop-inventory.yaml` | `検出` に `駆動` を接いだ形であり、記録の直し先は運用の書き下しである |
| `価値単位` | 造語 | `docs/slop-inventory.yaml` | 目的と対象を省いた抽象名詞句だが、経済学に同じ形の用例があるので意味の転用と迷った |
| `機能表` | 造語 | `docs/slop-inventory.yaml` | この名前の成果物がリポジトリになく、指す先のない名である |
| `算出不能` | 造語 | `docs/slop-inventory.yaml` | 記録の直し先は `計算できない` であり、語そのものは辞書にも Wikipedia にもない |
| `取得試行` | 造語 | `docs/slop-inventory.yaml` | 記録の直し先は `取りに行った` であり、動詞句を名詞で詰めた形である |
| `集計元` | 造語 | `docs/slop-inventory.yaml` | 実在する語に `元` を接いだ形であり、記録の直し先は `集計している提供元` である |
| `価格系列` | 造語 | `docs/slop-inventory.yaml` | `価格` に `系列` を接いだ形であり、記録は複合形だけが禁止語に入ったと書く |
| `業績系列` | 造語 | `docs/slop-inventory.yaml` | `業績` に `系列` を接いだ形であり、記録は複合形だけが禁止語に入ったと書く |
| `年次系列` | 造語 | `docs/slop-inventory.yaml` | `年次` に `系列` を接いだ形であり、記録は複合形だけが禁止語に入ったと書く |
| `月次化` | 造語 | `docs/slop-inventory.yaml` | `月次` に `化` を接いだ形であり、記録は何をどの間隔の値へ直したのかを動詞で書くよう求める |
| `日次化` | 造語 | `docs/slop-inventory.yaml` | `日次` に `化` を接いだ形であり、記録は何をどの間隔の値へ直したのかを動詞で書くよう求める |
| `提出値` | 造語 | `docs/slop-inventory.yaml` | 記録の直し先は `提出時の値` であり、いつ時点の値かを省いて詰めた形である |
| `投入ラダー` | 造語 | `docs/slop-inventory.yaml` | 金融のラダーを借りて作った複合で、複合そのものは辞書にも Wikipedia にもないが、部品の側が転用なので迷った |
| `取得キュー` | 造語 | `docs/slop-inventory.yaml` | 記録の直し先は `順番待ち` であり、同じ表がすでにその語を使っていた |
| `収集ユニバース` | 造語 | `docs/slop-inventory.yaml` | 重なり合う項目の集合を呼ぶために作った語であり、8 つの集合に 18 通りの呼び名があった |
| `分析ユニバース` | 造語 | `docs/slop-inventory.yaml` | 重なり合う項目の集合を呼ぶために作った語であり、8 つの集合に 18 通りの呼び名があった |
| `深掘りユニバース` | 造語 | `docs/slop-inventory.yaml` | 重なり合う項目の集合を呼ぶために作った語であり、8 つの集合に 18 通りの呼び名があった |
| `収集集合` | 造語 | `docs/slop-inventory.yaml` | 重なり合う項目の集合を呼ぶために作った語であり、指す先は他の呼び名と同じである |
| `収集対象外` | 造語 | `docs/slop-inventory.yaml` | 取りに行かない状態そのものが廃されたので、指す先のない名である |
| `集中ファンダ勢` | 造語 | `docs/slop-inventory.yaml` | `ファンダ` の略に `勢` を接いだ形であり、記録は置き換え先を作っていないと書く |
| `規則族` | 造語 | `docs/slop-inventory.yaml` | `規則` に `族` を接いだ形であり、同じものを `ルール族` とも呼んでいた |
| `ルール族` | 造語 | `docs/slop-inventory.yaml` | `ルール` に `族` を接いだ形であり、同じものを `規則族` とも呼んでいた |
| `審査待ち` | 造語 | `docs/slop-inventory.yaml` | 1 つの状態に 2 つの名が併存した側の名だが、普通の日本語として読める句なので意味の転用と迷った |
| `監査検出` | 造語 | `docs/slop-inventory.yaml` | `監査のときに点ける検出器` を 2 語の漢語へ潰した形であり、記録が造語と書く |
| `揮発ポインタ` | 造語 | `docs/slop-inventory.yaml` | `volatile pointer` の直訳であり、対訳表にも Wikipedia の見出しにもない |
| `分野印` | 造語 | `docs/slop-inventory.yaml` | リポジトリに先からある `分野タグ` に対して作った 2 つ目の呼び名であり、辞書にも Wikipedia にもない |
| `意味部門` | 造語 | `docs/slop-inventory.yaml` | 分類語彙表の資料が `部門` とだけ呼ぶものに `意味` を接いだ形であり、別の名を作っている |
| `攻撃面調査` | 造語 | `docs/slop-inventory.yaml` | 記録済みの `攻撃面` に `調査` を接いだ複合であり、部品の側も直しの対象である |
| `誤格上げ` | 造語 | `docs/slop-inventory.yaml` | 接頭辞の `誤` を `格上げ` に付けた形であり、記録が造語と書く |
| `作業ディレクトリ起点` | 造語 | `docs/slop-inventory.yaml` | `〜を起点とする` の動詞句を名詞の後置で詰めた形であり、コミット前の検査が指摘した |
| `定着語リスト` | 造語 | `docs/slop-inventory.yaml` | 説明のない漢語の複合が機能の名前になった形であり、記録の直し先は `ユーザーが認めた用語の一覧` である |
| `停止点` | 造語 | `docs/slop-inventory.yaml` | `stop point` の直訳で JMdict にも Wikipedia の見出しにもないが、Wikipedia の本文には 10 回以上現れるので意味の転用と迷った |
| `束雑音` | 造語 | `docs/slop-inventory.yaml` | 転用した `束` に `雑音` を接いだ複合であり、複合そのものは辞書にも Wikipedia にもない |
| `束効果` | 造語 | `docs/slop-inventory.yaml` | 転用した `束` に `効果` を接いだ複合であり、複合そのものは辞書にも Wikipedia にもない |
| `束内位置` | 造語 | `docs/slop-inventory.yaml` | 転用した `束` に `内位置` を接いだ複合であり、複合そのものは辞書にも Wikipedia にもない |
| `生成契約` | 造語 | `docs/slop-inventory.yaml` | 英語の contract を借りて作った複合で、複合そのものは辞書にも Wikipedia にもないが、部品の側が転用なので迷った |
| `不安定語` | 造語 | `docs/slop-inventory.yaml` | `不安定` に `語` を接いだ形であり、語と判定のどちらが不安定なのかが名前から読めない |
| `文体系` | 造語 | `docs/slop-inventory.yaml` | `文体` に `系` を接いだ形であり、記録の直し先は `文の長さと記号の規則` である |
| `全数分類` | 造語 | `docs/slop-inventory.yaml` | 評価の方法を名指すために作った形であり、書いた記録を検査に掛けると指摘が出た |
| `全件読了` | 造語 | `docs/slop-inventory.yaml` | 検証の範囲を実態より広く言う形であり、記録が造語と書く |
| `時況語` | 造語 | `docs/slop-inventory.yaml` | 国語学の定着語は時の名詞であり、`時況` に `語` を接いだ形は辞書にも Wikipedia にもない |
| `接続系` | 造語 | issue #1054 | 検出器のまとまりを `系` で名指すために作った語である |
| `上限系` | 造語 | issue #1077 | 検出器のまとまりを `系` で名指すために作った語である |
| `反復系` | 造語 | issue #297 | 検出器のまとまりを `系` で名指すために作った語である |
| `構造系` | 造語 | issue #312 | 検出器のまとまりを `系` で名指した 7 語の 1 つだが、建築の同じ形が Wikipedia の本文に現れるので意味の転用と迷った |
| `参照リソース` | 造語 | issue #491 | 検出器が引くデータを resource の直訳で呼んだ形であり、`.aku/dict/fix/` の記録は同じ形の語を資料へ寄せる |
| `走査系` | 造語 | issue #594 | 検出器のまとまりを `系` で名指すために作った語である |
| `意味論系` | 造語 | issue #747 | 検出器のまとまりを `系` で名指すために作った語である |
| `報告系` | 造語 | issue #842 | 検出器のまとまりを `系` で名指すために作った語である |
| `社会工学` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `側チャネル` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `供給連鎖` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `供給網` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `指揮統制` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `赤チーム` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `青チーム` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `紫チーム` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `逆解析` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `盲目的クロスサイトスクリプティング` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `盲目的XSS` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `攻撃的セキュリティ` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `資源集` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `資源ガイド` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `解析資源` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `学習資源` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `ベンチマーク資源` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `2要素認証` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `サービス妨害` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `遠隔コード実行` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `特権昇格` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `横方向移動` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `ラテラルムーブメント` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `アローリスト` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `自己ホスト` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `攻撃面` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `サンドボックス脱出` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `保存型` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `蓄積型` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `住宅用プロキシ` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `型混乱` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `悪用コード` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `レース条件` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `認証欠落` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `認可欠落` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `責任開示` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `脅威インテル` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `野良悪用` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `非影響` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `供給チェーン` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `自己拡散` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `設定注入` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `遠隔悪用` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `持続化機構` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `境界製品` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `未認証者` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `オブジェクト注入` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `遠隔実行` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `レスポンス密輸` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `リクエスト密輸` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `宙吊りレコード` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `クレデンシャル` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `デニリスト` | 言い換え | `.aku/dict/fix/wording/user.prh.yml` | prh の記録であり、実在する用語をプロジェクトが選んだ表記へ寄せる |
| `参照資源` | 言い換え | `seeds/dict-specs.yaml` | seeds の記録が `aku fix` の置換の仕様であり、`資源` を `資料` へ寄せる prh の行と同じ意味を持つ |
| `OSINT資源` | 言い換え | `seeds/dict-specs.yaml` | seeds の記録が `aku fix` の置換の仕様であり、`資源` を `資料` へ寄せる prh の行と同じ意味を持つ |
| `指紋DB` | 言い換え | `seeds/dict-specs.yaml` | seeds の記録が `aku fix` の置換の仕様であり、`指紋` を `フィンガープリント` へ寄せる prh の行と同じ意味を持つ |
| `指紋一致` | 言い換え | `seeds/dict-specs.yaml` | seeds の記録が `aku fix` の置換の仕様であり、`指紋` を `フィンガープリント` へ寄せる prh の行と同じ意味を持つ |
| `エージェント型ブラウザ` | 言い換え | `seeds/dict-specs.yaml` | seeds の記録が `aku fix` の置換の仕様であり、括弧の原語だけを落として語そのものは残す |
| `推進役` | 意味の転用 | `.aku/dict/report/avoid/user.yaml` | 実在する語を、何を進めた側なのかを書かずに使った |
| `樹形図` | 意味の転用 | `docs/slop-inventory.yaml` | 数学と統計と言語学で別の対象に定着した語を、ディレクトリ構成の節に当てた |
| `資格審査` | 意味の転用 | `docs/slop-inventory.yaml` | 公共調達の語を、辞書の行を置換の対象にしてよいかの判定に当てた |
| `ドリフト検出` | 意味の転用 | `docs/slop-inventory.yaml` | 機械学習の定着語を記事の検査に当てたが、語が Wikipedia の 5 記事にしか現れないので造語と迷った |
| `未知語` | 意味の転用 | `docs/slop-inventory.yaml` | 形態素解析の定着語を、読み手が前提にしない場で使った |
| `供給源` | 意味の転用 | `docs/slop-inventory.yaml` | 実在する語が、事業者と資料と公開ページの 3 つを同時に指していた |
| `未接続` | 意味の転用 | `docs/slop-inventory.yaml` | 実在する語を、取り込む仕組みがまだない状態に当てた |
| `未取得` | 意味の転用 | `docs/slop-inventory.yaml` | 実在する語を、取りに行ったが手元にない状態に当てた |
| `配布先` | 意味の転用 | `docs/slop-inventory.yaml` | 実在する語を、構成要素の一覧が載る公開ページに当てた |
| `配信元` | 意味の転用 | `docs/slop-inventory.yaml` | 配布先と同じ公開ページを、向きの逆の実在する語で呼んだ |
| `観測点` | 意味の転用 | `docs/slop-inventory.yaml` | 測量と統計の定着語を、履歴に含まれる値の個数に当てた |
| `共変動` | 意味の転用 | `docs/slop-inventory.yaml` | 経済の定着語を、2 つの並びが同じ向きに動いた度合いに当てた |
| `代理指標` | 意味の転用 | `docs/slop-inventory.yaml` | 統計の定着語を読み手が前提にしない場で使ったが、語が Wikipedia の 5 記事にしか現れないので造語と迷った |
| `監視対象` | 意味の転用 | `docs/slop-inventory.yaml` | 解析辞書に見出しのある語が、8 つの集合のうち 1 つの別名になっていた |
| `状態機械` | 意味の転用 | `docs/slop-inventory.yaml` | 計算機科学の用語を、方針が対象に下す判定の遷移に当てた |
| `参照資料` | 意味の転用 | `docs/slop-inventory.yaml` | 会議の配布物を指す実在する語を、機械が照合のために引くデータに当てた |
| `二本立て` | 意味の転用 | `docs/slop-inventory.yaml` | 映画の興行の語を、1 つの crate が 2 つの責務を持つことに当てた |
| `壁時計` | 意味の転用 | `docs/slop-inventory.yaml` | 壁に掛ける時計を指す実在する語を、wall-clock time の直訳として使った |
| `ファネル` | 意味の転用 | `docs/slop-inventory.yaml` | マーケティングの購買行動モデルの語を、多段の絞り込みの手続きに当てた |
| `不安全` | 意味の転用 | `seeds/dict-specs.yaml` | 記録が咎めるのは意味ではなく形だが、語そのものは実在するので造語には置けない |
