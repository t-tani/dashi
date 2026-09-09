# 日本語 WordNet 写像の測定

akunuki の意味分類表は、名詞ごとに意味の区分を並べた参照データである。この表の生成元を、非営利限定の分類語彙表から日本語 WordNet の写像へ移せるかを測った記録である。移せれば、ライセンスの制約なしに表を配れる。

移せない。語を 5 つの大きな区分へ振り分けた結果は、両方の表に載る語で 56.3% しか一致せず、検出器が実際に問う形に絞っても 87.5% にとどまった。評価コーパス 300 ファイルへの指摘は 967 件から 1,063 件へ動くだけだったが、同じ位置に同じ語で出た指摘は 278 件しかなかった。`aku_morph` の既存のテストも、405 件のうち 18 件が落ちた。

置き換えの見込みを潰しているのは、部門の粒度の粗さでも写像の作り方でもなく、2 つの言語資源が語に付ける語義の集合が違うことである。日本語 WordNet は英語の synset へ訳語を貼って作る。そのため、英語を字義どおり写した語を見つけるという `concrete-noun-misfit` の狙いに、まさにその写した語義を与えてしまう。

## 入力データ

測定に使ったデータを並べる。パスの列は、このフォルダからの相対パスである。このフォルダにないものは、取得元の URL か置き場所を書く。

| データ | パス | バージョン・日付 | 件数 |
|---|---|---|---|
| 日本語 WordNet の語と synset の対応 | `https://github.com/bond-lab/wnja/releases/download/v1.1/wnjpn-ok.tab.gz` | 1.1 | 語義の対応がおよそ 15 万行 |
| Princeton WordNet の名詞データ | `https://wordnetcode.princeton.edu/3.0/WNdb-3.0.tar.gz` の `dict/data.noun` | 3.0 | synset 82,115 |
| Princeton WordNet の lexname 一覧 | `https://wordnetcode.princeton.edu/3.0/WordNet-3.0.tar.gz` の `dict/lexnames` | 3.0 | 45 |
| 分類語彙表由来の表 | akunuki の `.aku/reference/semantic-class-table.tsv` | 2026-08-29 時点 | 55,680 語 |
| WordNet 由来の表(狭い写像) | `wnja-semantic-class-table.tsv` | 2026-08-29 | 65,784 語 |
| WordNet 由来の表(広い写像) | `wnja-semantic-class-table-wide.tsv` | 2026-08-29 | 65,784 語 |
| 評価コーパス | 外部 | — | Markdown 300 ファイル |
| docs A | 外部 | — | Markdown 42 ファイル |
| docs B | 外部 | — | Markdown 9 ファイル |

## 前提条件

3 つの設定 `configs/cfg-old.toml`・`configs/cfg-new.toml`・`configs/cfg-wide.toml` は、1 バイトも違わない。どれも意味分類表を `reference/semantic-class-table.tsv` という相対パスで指すので、3 つの表の違いは、設定と並べて置いた `reference/` の中身が持っていた。`cfg-old` の実行では分類語彙表由来の表を、`cfg-new` では狭い写像の表を、`cfg-wide` では広い写像の表を、そこへ置いた。`abstract_ratio` はどれも 0.7 で、これが `結果` の数字を出したときの値である。3 つの実行で変えたのは、表だけである。

指摘を出した akunuki のバイナリのバージョンは記録がない。`測定の手順` の `aku lint` はバージョンを書き出さず、出力の JSON もバージョンを持たない。`aku_morph` の単体テストを実行した akunuki の複製についても、どの commit から取ったかの記録がない。

このフォルダに入っていないのは、3 つのコーパスと、設定と並べて置いた `reference/` である。コーパスは手元の Markdown へ差し替えれば lint 自体は動くが、件数は `結果` と比べられない。日本語 WordNet と Princeton WordNet は `入力データ` の URL から誰でも取れ、生成した 2 つの表もこのフォルダにある。そのため、表の生成と一致率の測定は、それだけで同じ数字を再現できる。分類語彙表由来の表は非営利限定で、akunuki を持っていれば手元にある。この測定は API を呼ばないため、API キーは要らない。

この測定は、検出器が出した指摘のうち妥当だったものの割合、すなわち適合率だけを見る。指摘すべきだったもののうち実際に指摘したものの割合、すなわち再現率は出せない。検出器が指摘しなかった候補を見ていないため、指摘すべきだったのに出なかった偽陰性と、指摘すべきでなく出てもいない真陰性を数えていないからである。評価の枠組みは `../../eval/methodology.md` にある。

## 測定の手順

2 通りの写像の表を作り、分類語彙表由来の表との突き合わせ、lint、指摘の差分の抽出まで、次のコマンドで実行した。実行には、`../../../pyproject.toml` の依存を入れた venv と、`入力データ` の 3 つのコーパスが手元にあることが要る。ただし `jmdict_bridge.py` は `build_table` の名前でモジュールを取り込むので、そのままでは動かない。このフォルダのファイル名は `build_wnja_table.py` である。コマンドの中の `sub` は評価コーパスに付けた名前であり、docs A と docs B にも同じ形で当てた。

```
# 表を作る(狭い写像と広い写像)
python build_wnja_table.py
python build_wnja_table.py wide

# 分類語彙表由来の表と突き合わせる
python compare.py            > compare-out.txt
python lexname_stats.py      > lexname-stats.txt

# 3 つのコーパスへ lint を掛ける(cfg-old と cfg-new で 1 回ずつ)
aku lint --format json --config cfg-old/config.toml  --dict .aku/dict <コーパス> > <コーパス>-old.json
aku lint --format json --config cfg-new/config.toml  --dict .aku/dict <コーパス> > <コーパス>-new.json

# 指摘の入れ替わりを拾う
python diff_findings.py sub-old.json sub-new.json sub > diff-sub.txt
```

## 結果

3 つの実行から得た主な数値を並べる。

| 指標 | 値 |
|---|---|
| 部門の集合の一致(両方の表に載る語) | 56.3% |
| 具体物の条件の一致 | 87.5% |
| 評価コーパスの指摘(分類語彙表由来) | 967 件 |
| 評価コーパスの指摘(WordNet 由来) | 1,063 件 |
| 同じ位置に同じ語で出た指摘 | 278 件 |
| `aku_morph` の既存テストの失敗 | 405 件中 18 件 |

## WordNet 由来の表の生成

入手元は 2 つある。日本語 WordNet 1.1 と Princeton WordNet 3.0 で、次の 3 つのファイルを取る。

- `https://github.com/bond-lab/wnja/releases/download/v1.1/wnjpn-ok.tab.gz`(語と synset の対応)
- `https://wordnetcode.princeton.edu/3.0/WNdb-3.0.tar.gz`(名詞データ `dict/data.noun`)
- `https://wordnetcode.princeton.edu/3.0/WordNet-3.0.tar.gz`(lexname の一覧 `dict/lexnames`)

日本語 WordNet の synset の ID は、Princeton WordNet 3.0 のオフセットと桁が一致する。そのため `00001740-n` の語は、`data.noun` の `00001740` の行で引ける。

lexname から部門への写像は、lexfile の定義と収める synset の中身を読んで手で書いたもので、分類語彙表由来の表から対応を写してはいない。各部門が受ける lexname は次の 5 組である。

- 部門 1(関係): `noun.attribute`・`noun.event`・`noun.location`・`noun.quantity`・`noun.relation`・`noun.shape`・`noun.state`・`noun.time`
- 部門 2(主体): `noun.group`・`noun.person`
- 部門 3(活動): `noun.act`・`noun.cognition`・`noun.communication`・`noun.feeling`・`noun.motive`・`noun.possession`
- 部門 4(生産物): `noun.artifact`・`noun.food`
- 部門 5(自然物): `noun.animal`・`noun.body`・`noun.object`・`noun.phenomenon`・`noun.plant`・`noun.process`・`noun.substance`

`noun.Tops` の 51 synset は 1 つの部門にまとまらない。そこで synset ごとに、その synset を最上位に置く lexfile の部門を継がせて書いた。`entity` と `abstraction` は部門 1、`person` と `group` は部門 2 である。`act` と `cognition` は部門 3、`artifact` と `food` は部門 4、`animal` と `substance` は部門 5 である。

表は、名詞の synset に付いた日本語の語ごとに、全語義の部門を集めて 1 行にまとめている。分類番号は `1.d000` の 6 字とし、小数点以下第 1 位だけが意味を持つ。残る 3 桁は 0 で埋める。`role-conflict` の主語の部門の検査も `concrete-noun-misfit` も、実装が読むのは `ClassNumber::division` だけである。すなわち小数点以下第 1 位だけなので、この形で両方の検出器が動く。

狭い写像の表では、部門を 1 つだけ持つ語が 57,434、2 つ持つ語が 7,162、3 つ以上持つ語が 1,188 である。生成は 100 行ほどの Python で書いた。手順は 3 段である。まず `data.noun` の各行の第 2 列、すなわち lex_filenum を読み、オフセットから部門を引く表を作る。次に `wnjpn-ok.tab` の `-n` で終わる行だけを残し、語ごとに部門を集める。最後に、空白を含む語を落として見出し語の昇順で書き出す。

## 日本語 WordNet の利用条件

利用条件は `https://bond-lab.github.io/wnja/license.txt` にあり、写しをこのフォルダの `wnja-license.txt` に置いた。原文は、目的を問わず、無償で、使用と複製と改変と頒布を許すと書いている。条件は、著作権表示と許諾文と免責を複製・派生物・文書のすべてに添えることである。配布ページはこれに加えて、成果を公開する場合に次の表示とリンクを求める。

```
日本語ワードネット(XX 版)© 2009-2011 NICT, 2012-2015 Francis Bond and 2016-2024 Francis Bond, Takayuki Kuribayashi
```

再頒布ではライセンスも共に配る。営利利用を妨げる条項はない。

## 5 部門の一致率

両方の表に載るのは 22,711 語しかない。分類語彙表由来の側だけにある語が 32,969、WordNet 由来の側だけにある語が 43,073 ある。語彙の重なりは分類語彙表由来の表の 41% にとどまる。

両方に載る 22,711 語で測ると、部門の集合が一致するのは 12,792 語(56.3%)である。部門が 1 つでも重なる語なら 18,064 語(79.5%)ある。

検出器が実際に問う形に直すと数字は上がる。`concrete-noun-misfit` が指摘を出す条件、すなわち分類がすべて生産物か自然物にあるかは、19,863 語で一致し、割合にして 87.5% である。段落の判定の条件、すなわち関係か活動を 1 つでも含むかは、19,384 語(85.4%)で一致する。`role-conflict` の主語の部門の検査は 5 つの部門それぞれについて「その部門を含むか」を問うので、集合の一致と同じ 56.3% になる。

分類語彙表由来の表で部門を 1 つしか持たない 20,588 語に絞ると、集合の一致は 60.5% へ上がる。部門を 2 つ持つ 1,962 語では 16.2% へ落ちる。表記で切ると差はわずかで、漢字だけの語が 56.5%、カタカナだけの語が 59.3%、混じりの語が 51.5% である。一方、評価コーパスに現れる 4,174 語に絞ると集合の一致は 43.2% へ落ち、これは現れない語の 59.3% を下回る。よく使う語ほど語義が多く、語義が多いほど 2 つの言語資源は割れる。

どちらの表でも部門が 1 つの語だけを取り、分類語彙表由来の部門を行、WordNet 由来の部門を列にして数えた。

| 分類語彙表由来 \ WordNet 由来 | 関係 | 主体 | 活動 | 生産物 | 自然物 | 計 |
|---|---|---|---|---|---|---|
| 関係 | 1727 | 97 | 633 | 145 | 129 | 2731 |
| 主体 | 176 | 2203 | 54 | 211 | 27 | 2671 |
| 活動 | 761 | 186 | 4630 | 204 | 38 | 5819 |
| 生産物 | 61 | 27 | 135 | 2387 | 218 | 2828 |
| 自然物 | 625 | 40 | 118 | 88 | 1510 | 2381 |

対角は 12,457 語で、この表が数えた 16,430 語の 75.8% にあたる。崩れが大きい組み合わせは 3 つある。最も多いのは活動と関係の取り違えで、分類語彙表由来の表が活動とする 761 語を WordNet 由来の表は関係と読み、逆向きにも 633 語ある。自然物を関係と読む語も 625 語ある。

写像そのものの当たり具合も測った。日本語 WordNet で lexname が 1 つに定まり、分類語彙表由来の表でも部門が 1 つの語を取り、lexname ごとに分類語彙表由来の部門の分布を見た。

最頻の部門の占有率が高いのは `noun.animal` の 94%、`noun.body` の 93%、`noun.feeling` の 92% である。`noun.person` は 90%、`noun.quantity` と `noun.communication` は 89% である。一方で `noun.event` は 38%、`noun.location` は 41% しかなく、`noun.attribute` は 46%、`noun.state` は 48%、`noun.process` は 53% にとどまる。

占有率の低いこの 5 つに共通するのは、lexfile の中身が部門をまたいでいることである。たとえば `noun.location` には場所の抽象名と国名と地形が同居し、`noun.state` には「経済状態」と「喘息」が、`noun.process` には「処理」と「電気分解」が同居している。ところが分類語彙表由来の表は、これらを別の部門に置く。そのため lexname を 1 つの部門へ写す限り、この割れは埋まらない。

割れる lexname は部門を 1 つに絞らず並べる、という変種も作って測った。`noun.attribute` と `noun.event` を関係と活動に、`noun.location` を関係と主体と自然物に広げた表である。同じく `noun.process` を活動と自然物に、`noun.state` を関係と自然物に、`noun.substance` を生産物と自然物にする。部門が 1 つでも重なる語は 79.5% から 86.0% へ上がった。しかし集合の一致は 56.3% から 50.1% へ落ち、`concrete-noun-misfit` の指摘を出す条件の一致は 87.5% から 87.4% へわずかに下がった。検出器の判定が動かないため、この変種に利点はない。

## 3 つのコーパスでの指摘の差分

参照データのパスだけを差し替えた設定を 2 つ作り、`aku lint --format json` を同じ入力へ 2 度当てた。設定は同梱の雛形のままである。つまり `standalone-calque` と `role-conflict` のコピュラ文の検査は無効で、`concrete-noun-misfit` と `role-conflict` の主語の部門の検査は有効である。指摘の文面には分類番号が入って必ず変わるため、突き合わせの鍵はファイル・行・桁と語だけにした。

評価コーパス(300 ファイル)では、`concrete-noun-misfit` が 967 件から 1,063 件へ増えた。ただし両方に出た指摘は 278 件で、WordNet 由来の表だけが出した指摘が 785 件、分類語彙表由来の表だけが出した指摘が 689 件だった。`role-conflict` は 8 件のまま動かなかった。

WordNet 由来の表だけが出す指摘の語は、多い順に「アクセス」102 件、「インターネット」64 件、「処理」64 件である。以下「スキャン」32 件、「自動」28 件、「モジュール」28 件と続くが、これらはいずれも誤検出である。誤検出になる理由は語義の付き方にある。日本語 WordNet で「アクセス」が持つ語義は `02671224-n` だけで、その lexname は `noun.artifact` になっている。同じように「処理」は `13541167-n` だけで `noun.process` に入り、「自動」は自動小銃を指す `02760855-n` だけを持つ。語義が英語の側から 1 つだけ付いた語では、日本語での主な用法が表に現れない。

分類語彙表由来の表だけが出す指摘の語は、多い順に「標的」99 件、「パッチ」69 件、「背景」58 件である。以下、「鍵」44 件、「バイパス」32 件、「キー」31 件と続く。これらは、英語の語を写した語を段落の中で見つけるというこの検出器の狙いに正面から当たる。失うと痛手になる。消える理由は言語資源の作り方にある。日本語 WordNet の「パッチ」は、`noun.artifact` の `03897943-n` と同時に、`noun.communication` の `06573223-n` も持つ。「鍵」も `noun.artifact` に加えて `noun.cognition` と `noun.communication` を持つ。この検出器は、分類が 1 つでも抽象の部門にあれば報告しない。そのため抽象の語義が付いた時点で当たらない。日本語 WordNet は英語の synset へ日本語の訳語を貼って作る。つまり、写した語義そのものを語に登録する。写した語義を持つ言語資源で、写した語を見つけることはできない。

docs A と docs B でも傾向は同じだった。docs A の `docs`(42 ファイル)では `concrete-noun-misfit` が 186 件から 147 件へ減り、両方に出るのは 52 件である。同じ入力で `role-conflict` は 35 件から 29 件へ減り、両方に出るのは 28 件である。分類語彙表由来の表だけが出す 7 件は「機械」2 件、「側」2 件、「候補」「被害」「片側」である。docs B の `docs`(9 ファイル)では `concrete-noun-misfit` が 161 件から 141 件へ減り、両方に出るのは 47 件である。`role-conflict` は 23 件から 25 件へ増え、この表だけが出す指摘はない。どちらのコーパスでも、この表だけが出す語には「キー」「タグ」「ラベル」「標的」が並ぶ。

部門を並べる変種でも結果は変わらなかった。評価コーパスで `concrete-noun-misfit` は 962 件になり、両方に出た指摘は 265 件だった。

## seeds と既存のテスト

akunuki の `seeds/` の 3 ファイルに、意味分類表を引くケースはない。`dict-specs.yaml`・`domain-terms-seed.yaml`・`protected-corpus.yaml` のどれにもない。同じリポジトリの `tests/generated/` も空である。そのため seeds では WordNet 由来の表の可否を測れない。

測れるのは `aku_morph` の単体テストである。akunuki の複製を作り、`.aku/reference/semantic-class-table.tsv` だけを WordNet 由来の表に置き換えたうえで、`cargo test -p aku_morph --features morph --lib` を実行した。置き換える前は 405 件すべてが通る。一方、置き換えた後は 18 件が落ち、内訳は 3 つに分かれる。

第 1 は、分類番号を文字列で照合する 3 件である。`semantic_class_table` の「`門` の行から分類番号を引ける」は `1.4420` を期待する。WordNet 由来の表が返すのは `1.1000`・`1.2000`・`1.4000` である。`concrete_noun_misfit` の「抽象が支配する段落に立つ具体物の名詞を報告する」は、`床` に `1.4270` ほか 3 つを期待する。`subject_division` の「主体を求める述語に生産物の主語が立つと報告する」は `装置` に `1.4630` を期待する。これは表が部門しか持たないことの帰結で、言語資源の質の問題ではない。

第 2 は、語が表にないことを前提にした 3 件である。`adverb_metaphor`・`copula_division`・`subject_division` がそれぞれ持つ「意味分類表にない語は判定しない」が当たり、どれも `lookup("検出器")` が空であることを確かめる。ところが日本語 WordNet は `検出器` を `noun.artifact` として持つため、この前提が崩れる。

第 3 は、語の部門が変わることによる 12 件である。`concrete_noun_misfit` の 5 件では、テストが共有する段落の前半にある `処理` が WordNet 由来の表で自然物になる。そのため意図しない指摘が 1 件増えて落ちる。`copula_division` の 6 件と `adverb_metaphor` の 3 件と `subject_division` の 1 件は、`担当者` がこの表にないことが主な原因である。日本語 WordNet は `担当者` を持たない。`装置` は分類語彙表由来の表で生産物だけだが、WordNet 由来の表では活動と生産物の 2 つを持つ。そのため主語の部門の検査に当たらなくなる。

`standalone-calque` の単体テスト 11 件は WordNet 由来の表でも通る。`網` と `段` はこの表でも関係と生産物の両方の部門を持つ。そのため部門をまたぐという条件を満たす。

`aku_cli` の結合テストのうち意味分類表を使うものは、`crates/aku_cli/tests/fixtures/config/` に置いたフィクスチャの表を読む。そのため既定の表を替えても結果は動かない。フィクスチャの表は、検出器が表を読んで指摘を出すまでの経路を検査するためのものである。言語資源の質は測れない。

## 対訳表を経由した補完

対訳表の英訳から Princeton WordNet を引いて、日本語 WordNet にない語を埋められるかも測った。対訳表は JMdict の英訳を収めた akunuki の参照データで、使うのは第 1 義の英訳だけである。その英訳が Princeton WordNet の名詞の見出しに当たれば、全語義の lexname から部門を集める。対訳表の 48,355 語のうち、この経路で部門が付いたのは 18,750 語だった。しかし分類語彙表由来の表にあって日本語 WordNet にない 32,969 語のうち、埋まったのは 1,224 語、割合にして 3.7% しかない。埋まった語での部門の集合の一致は 26.7%、部門が 1 つでも重なるのは 80.1% だった。穴の 96% は埋まらず、埋まった語での一致率も本体を下回る。この経路に見込みはない。

## aku report の集計への影響

部門だけを持つ表は、`aku report` の同じ英訳を持つ語の集計を壊す。この集計は、設定の `shared_gloss_class_digits` で分類番号の先頭 2 桁を見て語を分ける。雛形の値は 2 である。部門だけの表では小数点以下第 2 位が常に 0 なので、絞り込みが 1 桁分しか効かなくなる。日本語 WordNet を採るなら、この設定の値と、それが依存する桁の意味を決め直す必要がある。

## 日本語 WordNet の見出しの表記の揺れ

見出しには「騒き」「涌泉」のような表記や、「イギリスの首都」のような句が入る。akunuki は正規化形で表を引くため、この揺れの分だけ引けない語が出る。

## ファイル一覧

このフォルダのファイルと、その中身を並べる。

| ファイル | 中身 |
|---|---|
| `build_wnja_table.py` | 日本語 WordNet から部門だけの意味分類表を作る |
| `wnja-semantic-class-table.tsv` | 狭い写像の表 |
| `wnja-semantic-class-table-wide.tsv` | 広い写像の表 |
| `wnja-license.txt` | 日本語 WordNet の利用条件の写し |
| `configs/` | 3 つの実行で使った設定 |
| `compare.py` | 2 つの表の部門と具体物の条件の一致を数える |
| `compare-out.txt` | 狭い写像での一致率 |
| `compare-wide-out.txt` | 広い写像での一致率 |
| `lexname_stats.py` | lexname ごとに分類語彙表由来の部門の分布を数える |
| `lexname-stats.txt` | lexname ごとの部門の分布 |
| `diff_findings.py` | 表を入れ替えたときに増減する指摘を拾う |
| `diff-sub.txt` | 評価コーパスでの指摘の差分 |
| `diff-docs-a.txt` | docs A での指摘の差分 |
| `diff-docs-b.txt` | docs B での指摘の差分 |
| `diff-sub-wide.txt` | 広い写像での評価コーパスの差分 |
| `coverage.txt` | 表ごとの見出し語の被覆 |
| `cause-table.txt` | 入れ替わった指摘の上位語が、両方の表で持つ部門 |
| `cause-split.txt` | 入れ替わりの原因ごとの件数 |
| `overlap-words.txt` | 入れ替わった指摘の語を件数の多い順に並べた一覧 |
| `override-estimate.txt` | 分類を直して入れ替わりをどこまで戻せるかの見積り |
| `jmdict_bridge.py` | 対訳表の英訳を介して部門を補えるかを測る |
| `jmdict-bridge.txt` | 対訳表を介した補完の測定 |
