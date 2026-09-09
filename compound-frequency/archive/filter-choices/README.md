# 複合語の頻度フィルタの大きさと精度を決める選択

複合語の頻度フィルタの大きさと精度は、5 つの選択で決まる。解析辞書にない 1 形態素、隣り合う対の連結の検査、固有名詞の見出し、回数の下限、部品の頻度の下限である。2026-09-07 に、この 5 つを 1 つずつ変えて大きさと当たり方の動きを測り、5 つのうち 4 つでは変えた側を既定に採った。

回数の下限と部品の頻度の下限を上げ、固有名詞の見出しを外すと、成果物は 38.9 MB から 18.8 MB へ半分以下に縮む。それでも、LLM が作った語を集めた造語の例 204 語のうち、区分が `造語` の 129 語では拾える語が 107 語から 113 語へ増える。既存語の例 563 語のうち造語の候補へ誤って倒す語も、96 語から 106 語へ増えるにとどまる。配布中の成果物の当たり方は `../../eval/README.md` にある。

基準は、2026-09-07 時点の既定で組んだ成果物である。この既定は、解析辞書にない 1 形態素を数えず、固有名詞の見出しを登録し、部品の頻度の下限を 1 にし、対の連結の検査を挟む。併せて、部品の比の閾値をどこに置くか、造語の例の区分を 2 つから 3 つに分けると割合はどう動くかも測った。

## 入力データ

2026-09-07 の測定に使った入力は、Wikipedia のダンプ、解析辞書の配布物、技術文書の平文、正解つきの語の 4 つの TSV である。パスはリポジトリの根から見た形で挙げる。

| データ | パス | バージョン・日付 | 件数 |
|---|---|---|---|
| Wikipedia のダンプ | `jawiki-20251229-cirrussearch-content.json.gz`(外部) | 2025-12-29 | 全記事 1,484,267 件 |
| 解析辞書の配布物 | SudachiDict の `small_lex`・`core_lex`・`notcore_lex`(外部) | 2026-07-23 | — |
| 技術文書の平文 | `compound-frequency/corpus/text`(外部) | `manifest.tsv` の commit | 16 ソース 13,848 文書 |
| 造語の例 | `../../eval/coined.tsv` | — | 204 語 |
| 既存語の例 | `../../eval/established.tsv` | — | 563 語 |
| 解析辞書にない 1 形態素の造語の例 | `../../eval/coined-unknown.tsv` | — | 3 語 |
| 解析辞書にない 1 形態素の既存語の例 | `../../eval/established-unknown.tsv` | — | 269 語 |

成果物は Wikipedia の全記事と技術文書で組み、日本語コーパスは入れていない。

## 前提条件

どの行も、ダンプと解析辞書のバージョンと正解集合の作り方を固定し、基準から選択を 1 つだけ動かした。動かす場所は行によって違う。連結の扱いを変える行は基準の成果物をそのまま引き、`judge --concatenation` の値だけを変えた。固有名詞の見出し、回数の下限、部品の頻度の下限、組み合わせの行は、`dictionary-headwords` と `build` の引数を変えて成果物を組み直した。解析辞書にない 1 形態素の行では、`count` から `--count-known-only` を外して回数を数え直し、`judge` に `--unknown-morphemes` を足し、正解集合も広げた 2 つのファイルに差し替えた。この 3 つは同時に動く。

`corpus-tool` は akunuki の `aku_freq`・`aku_core`・`aku_morph` を、git の rev `a4d27caa92493d46cb60c29f975dca4d7526cc56` で固定して依存する。フィルタのキーは akunuki の分割単位 A で切るので、rev が違えばキーの数も当たり方も変わる。

部品の比の閾値は、掃引の行だけが `--threshold` で指定した値を使い、残りの行では `corpus-tool` の既定の 1000 が効いた。この値の根拠は `../../design.md` の「既定の根拠」にある。構成の分析に置き換える行は 2 つの閾値を渡していないので、既定の上 300・下 100 が効いた。3 形態素以上の語の検査は、2026-09-07 時点の既定だった `established` をコマンドで明示した。

同じ数を出すには 3 つの入力が要り、そのうち 2 つはリポジトリにない。Wikipedia のダンプ `jawiki-20251229-cirrussearch-content.json.gz` は、古いバージョンが手に入らなくなる。解析辞書は 2026-07-23 の SudachiDict である。技術文書のソースごとの commit は取得のたびに書き直される `compound-frequency/corpus/raw/manifest.tsv` にしかなく、リポジトリにないので、同じ本文を取り直せるとは限らない。正解つきの語の 4 つの TSV はリポジトリにある。日本語コーパスは使っていないので、この記録の再現に手元の日本語コーパスは要らない。

## 手順

`compound-frequency/corpus-tool` で実行した。`<ダンプ>` はダンプのディレクトリである。

基準の入力と成果物を作り、元の正解集合で測った。

```
cargo run --release -- count --count-known-only \
    --dump <ダンプ>/jawiki-20251229-cirrussearch-content.json.gz --out-dir ../counts-known
cargo run --release -- count --count-known-only --text-dir ../corpus/text \
    --out-dir ../counts-docs-known
cargo run --release -- dictionary-headwords --proper-nouns \
    --lexicon-dir ../sudachidict --out-dir ../counts-known
cargo run --release -- build --counts-dir ../counts-known \
    --docs-counts-dir ../counts-docs-known --min-component-count 1 --out-dir ../variants/base
cargo run --release -- judge --artifacts-dir ../variants/base --concatenation established \
    --eval ../eval/coined.tsv ../eval/established.tsv
```

広げた正解集合は、元の集合と解析辞書にない 1 形態素の集合を連ねて作った。

```
cd ../eval
cat coined.tsv coined-unknown.tsv > coined-all.tsv
cat established.tsv established-unknown.tsv > established-all.tsv
cd ../corpus-tool
cargo run --release -- judge --artifacts-dir ../variants/base --concatenation established \
    --eval ../eval/coined-all.tsv ../eval/established-all.tsv
```

解析辞書にない 1 形態素の行は、2 つの回数を数え直すところから始めた。見出しは基準と同じなので写した。

```
cargo run --release -- count \
    --dump <ダンプ>/jawiki-20251229-cirrussearch-content.json.gz --out-dir ../counts-unknown
cargo run --release -- count --text-dir ../corpus/text --out-dir ../counts-docs-unknown
cp ../counts-known/dictionary-headwords.tsv ../counts-known/dictionary-headwords-stats.json \
    ../counts-unknown/
cargo run --release -- build --counts-dir ../counts-unknown \
    --docs-counts-dir ../counts-docs-unknown --min-component-count 1 \
    --out-dir ../variants/unknown
cargo run --release -- judge --artifacts-dir ../variants/unknown --unknown-morphemes \
    --concatenation established --eval ../eval/coined-all.tsv ../eval/established-all.tsv
```

連結の 3 行は基準の成果物を引くので、`judge` の引数だけを変えた。構成の分析だけは、単位の回数を読むために回数の TSV のパスも渡した。

```
cargo run --release -- judge --artifacts-dir ../variants/base --concatenation gray \
    --eval ../eval/coined.tsv ../eval/established.tsv
cargo run --release -- judge --artifacts-dir ../variants/base --concatenation off \
    --eval ../eval/coined.tsv ../eval/established.tsv
cargo run --release -- judge --artifacts-dir ../variants/base --concatenation composition \
    --counts-dir ../counts-known --docs-counts-dir ../counts-docs-known \
    --eval ../eval/coined.tsv ../eval/established.tsv
```

固有名詞の行は見出しだけを組み直した。回数の TSV と記録は基準のものを指す。

```
cargo run --release -- dictionary-headwords \
    --lexicon-dir ../sudachidict --out-dir ../counts-noproper
for name in compound-counts.tsv component-counts.tsv count-stats.json; do
    ln -s ../counts-known/$name ../counts-noproper/$name
done
cargo run --release -- build --counts-dir ../counts-noproper \
    --docs-counts-dir ../counts-docs-known --min-component-count 1 \
    --out-dir ../variants/noproper
cargo run --release -- judge --artifacts-dir ../variants/noproper --concatenation established \
    --eval ../eval/coined.tsv ../eval/established.tsv
```

回数の下限と部品の頻度の下限の行は、`build` の引数だけを変えた。`judge` は基準と同じ引数で、成果物のディレクトリだけを差し替えた。

```
cargo run --release -- build --counts-dir ../counts-known \
    --docs-counts-dir ../counts-docs-known --min-count 5 --min-component-count 1 \
    --out-dir ../variants/min-count-5
cargo run --release -- build --counts-dir ../counts-known \
    --docs-counts-dir ../counts-docs-known --min-component-count 3 \
    --out-dir ../variants/min-component-3
```

組み合わせは、固有名詞を外した見出しに回数の下限と部品の頻度の下限を重ねた。

```
cargo run --release -- build --counts-dir ../counts-noproper \
    --docs-counts-dir ../counts-docs-known --min-count 5 --min-component-count 3 \
    --out-dir ../variants/combo
cargo run --release -- judge --artifacts-dir ../variants/combo --concatenation established \
    --eval ../eval/coined.tsv ../eval/established.tsv
```

組み合わせに解析辞書にない 1 形態素を足す行では、その形も数えた回数と、固有名詞のない見出しを 1 つのディレクトリに揃えてから組んだ。部品の下限は 5 まで上げても判定が動かないので、頻度表をさらに 1.5 MB 削った。

```
cargo run --release -- dictionary-headwords \
    --lexicon-dir ../sudachidict --out-dir ../counts-unknown-noproper
for name in compound-counts.tsv component-counts.tsv count-stats.json; do
    ln -s ../counts-unknown/$name ../counts-unknown-noproper/$name
done
cargo run --release -- build --counts-dir ../counts-unknown-noproper \
    --docs-counts-dir ../counts-docs-unknown --min-count 5 --min-component-count 5 \
    --out-dir ../variants/combo-unknown
cargo run --release -- judge --artifacts-dir ../variants/combo-unknown --unknown-morphemes \
    --concatenation established --eval ../eval/coined-all.tsv ../eval/established-all.tsv
```

部品の比の閾値の掃引は、Wikipedia だけで組んだ成果物(`../variants/wiki`、組み方は `../input-sources/README.md`)で行った。

```
cargo run --release -- judge --artifacts-dir ../variants/wiki --threshold <閾値> \
    --concatenation established --eval ../eval/coined.tsv ../eval/established.tsv
```

`build` は成果物と同じパスに来歴を書く。その `variant` の節に 4 つの引数が残るので、成果物からどの選択で組んだかを読める。連結の扱いは判定の側の選択なので、`variant` の節には入らない。

## 結果

### 5 つの選択の対応表

対応表は、基準の成果物と、選択を 1 つずつ変えた成果物と、大きさを予算に収める組み合わせを、キー数とファイルの大きさと 4 つの割合で並べる。造語の再現率・言い換えの誤検出率・既存語の誤検出率・グレー率の定義は `../../eval/README.md` の「指標の定義」にある。既存語の誤検出率の分母は、元の正解集合では既存語の例 563 語、広げた正解集合では 832 語である。

| 行 | キー数 | `freq_filter.bin` | `component_freq.fst` | 造語の再現率 | 言い換えの誤検出率 | 既存語の誤検出率 | グレー率 |
|---|---:|---:|---:|---|---|---|---|
| 基準 | 10,391,904 | 23,396,438 | 15,533,241 | 107/129(82.9%) | 32/56(57.1%) | 96/563(17.1%) | 67/767(8.7%) |
| 基準(広げた正解集合) | 10,391,904 | 23,396,438 | 15,533,241 | 107/129(82.9%) | 34/58(58.6%) | 365/832(43.9%) | 67/1039(6.4%) |
| 解析辞書にない 1 形態素を足す | 11,906,559 | 26,804,310 | 15,533,241 | 107/129(82.9%) | 33/58(56.9%) | 130/832(15.6%) | 93/1039(9.0%) |
| 連結をグレーにする | 10,391,904 | 23,396,438 | 15,533,241 | 107/129(82.9%) | 32/56(57.1%) | 96/563(17.1%) | 95/767(12.4%) |
| 連結の検査をやめる | 10,391,904 | 23,396,438 | 15,533,241 | 114/129(88.4%) | 35/56(62.5%) | 114/563(20.2%) | 67/767(8.7%) |
| 連結を構成の分析に置き換える | 10,391,904 | 23,396,438 | 15,533,241 | 108/129(83.7%) | 34/56(60.7%) | 103/563(18.3%) | 85/767(11.1%) |
| 固有名詞の見出しを外す | 9,479,118 | 21,364,822 | 15,533,241 | 107/129(82.9%) | 32/56(57.1%) | 97/563(17.2%) | 70/767(9.1%) |
| 回数の下限を 5 にする | 6,711,853 | 15,138,902 | 15,533,241 | 113/129(87.6%) | 33/56(58.9%) | 105/563(18.7%) | 47/767(6.1%) |
| 部品の頻度の下限を 3 にする | 10,391,904 | 23,396,438 | 5,841,038 | 107/129(82.9%) | 32/56(57.1%) | 96/563(17.1%) | 67/767(8.7%) |
| 組み合わせ | 5,736,425 | 12,910,678 | 5,841,038 | 113/129(87.6%) | 33/56(58.9%) | 106/563(18.8%) | 50/767(6.5%) |
| 組み合わせに解析辞書にない 1 形態素を足す | 6,721,131 | 15,138,902 | 4,315,469 | 113/129(87.6%) | 34/58(58.6%) | 149/832(17.9%) | 67/1039(6.4%) |

造語の再現率の分母は、どの行も区分が `造語` の 129 語である。広げた正解集合を使う行でも、足した 3 語は `言い換え` と `意味の転用` に入るので、この分母は動かない。動くのは `言い換え` の分母で、56 語から 58 語になる。既存語の分母も 563 語から 832 語へ動くので、広げた正解集合の行は基準ではなく「基準(広げた正解集合)」と比べる。

大きさは 2 つのバイト数の和で読む。基準は 38.9 MB、組み合わせは 18.8 MB であり、組み合わせに解析辞書にない 1 形態素を足すと 19.5 MB になる。2 つの組み合わせは、どちらも予算の 15 MB から 20 MB に収まる。

どちらの組み合わせも、造語の再現率は 87.6% で、比べる基準より 4.7 ポイント高い。組み合わせでは、既存語の誤検出率も基準の 17.1% から 18.8% へ 1.7 ポイント上がる。組み合わせに解析辞書にない 1 形態素を足すと、誤検出率は「基準(広げた正解集合)」の 43.9% から 17.9% へ 26.0 ポイント下がる。

解析辞書にない 1 形態素の効果は、広げた正解集合でだけ見える。この形の語をキーにしない基準は、広げた既存語 832 語のうち 365 語を造語の候補に倒す。キーにすると 130 語まで下がり、造語の再現率は 1 語も落ちない。フィルタは 3.4 MB 大きくなる。

連結の扱いは判定の側の選択なので、どの扱いでも成果物の大きさは基準と変わらない。検査をやめると、造語の再現率が 5.5 ポイント、既存語の誤検出率が 3.1 ポイント上がる。グレーにすると、どちらの割合も動かず、グレー率だけが 3.7 ポイント上がる。構成の分析に置き換えたときの上がり幅は、再現率が 0.8 ポイント、誤検出率が 1.2 ポイントである。構成の分析の内訳は `../composition/README.md` にある。

固有名詞の見出しを外すと、フィルタが 2.0 MB 小さくなる。2026-07-23 の SudachiDict の見出し 1,482,281 件のうち 1,398,687 件が固有名詞である。既存語の例では、辞書の理由が 35 語減り、31 語が高頻度へ、3 語が低頻度へ、1 語が未登録へ移る。回数の側が同じ語を高頻度の既存語として持つので、誤検出は 96 語から 97 語へ 1 語増えるだけである。

回数の下限を 5 にすると、フィルタが 8.3 MB 小さくなる。バケット 1 が消えるので、グレーは 67 語から 47 語へ減る。`造語` で減る 6 語はすべて造語の候補になり、既存語の例で減る 13 語は造語の候補 9 語と既存語の連結 4 語に割れる。造語の再現率は 4.7 ポイント上がり、既存語の誤検出率も 1.6 ポイント上がる。

部品の頻度の下限を 3 にすると、頻度表が 9.7 MB 小さくなり、判定は 1 語も動かない。判定が動かないのは、落ちる部品が頻度 2 以下のものだけで、部品の比の閾値 1000 に遠く届かないためである。

### 決めた既定

この測定を根拠に決めた既定は、5 つの選択それぞれについて次のとおり。他の測定の分も含めた既定の一覧は `../../design.md` の「既定の根拠」にある。

| 選択 | 決めた既定 | 理由 |
|---|---|---|
| 解析辞書にない 1 形態素 | 数える | 広げた既存語の誤検出が 365 語から 130 語へ減り、造語の再現率が落ちないため |
| 回数の下限 | 3 のまま(バケット 1 を残す) | 「見たことはあるが稀」という段階を、warn ではなく info に落とす根拠にするため |
| 固有名詞の見出し | 外す | akunuki が固有名詞を含む連なりを候補にせず、登録しても引かれないため |
| 部品の頻度の下限 | 3(2 回以下の部品を捨てる) | 判定を動かさずに頻度表を小さくできるため |
| 隣り合う対の連結の検査 | やめる | 対がすべて高頻度な語を既存語とし、`未認証データ` のような圧縮された造語を既存語に倒すため |

### 部品の比の閾値

閾値は名前を変えず、造語の候補の理由だけを分ける。部品の比の検査と、どれにも当たらない場合がどちらも造語の候補を返すため、4 つの割合はどの閾値でも同じ値になる。

成果物は全記事だけで組み、判定は閾値だけを 7 通りに振った。正解集合は区分が `造語` の 129 語と既存語の例 563 語である。差は、造語の候補に占める理由 `部品` の割合を 2 つの群で引いた値である。

| 閾値 | `造語` | 既存語の例 | 差 |
|---:|---|---|---:|
| 0 | 104/108(96.3%) | 107/134(79.9%) | 16.4 |
| 100 | 102/108(94.4%) | 97/134(72.4%) | 22.1 |
| 500 | 91/108(84.3%) | 77/134(57.5%) | 26.8 |
| 1000 | 82/108(75.9%) | 59/134(44.0%) | 31.9 |
| 2000 | 72/108(66.7%) | 49/134(36.6%) | 30.1 |
| 3000 | 67/108(62.0%) | 36/134(26.9%) | 35.2 |
| 10000 | 31/108(28.7%) | 20/134(14.9%) | 13.8 |

既定値は 1000 である。差は 1000 から 3000 でほぼ変わらず 30 ポイントから 35 ポイントに収まるため、その範囲でいちばん小さい 1000 を採ると造語が最も多く残る。10000 まで上げると差は 14 ポイントに落ち、部品で説明できる造語も 31 語に減る。技術文書を足した成果物で同じ掃引を行っても、既定値は動かない。差がいちばん大きいのは閾値 3000 の 31.4 ポイント(`造語` 67/107、既存語の例 30/96)で、次が 1000 の 30.9 ポイントである。2 つの差は 0.5 ポイントしか離れていないので、造語を 14 語多く残す 1000 を採った。

### 区分を 2 つに分けた場合と 3 つに分けた場合

区分が 2 つのときは、再現率の分母が造語の例 172 語と、広げた造語の例 175 語だった。3 つに分けた分母は `造語` の 129 語で、広げた集合でも同じ 129 語である。分母から抜けた語のうち、`言い換え` の 56 語は判定の結果を分子にも持っていたので、分子も同時に小さくなる。

表は、区分を 2 つに分けたときと 3 つに分けたときの造語の再現率を行ごとに並べる。`表` の列は値の出どころで、「技術文書を足した前後」の行は `../input-sources/README.md` の同名の表から、「対応表」の行は「5 つの選択の対応表」から引いた。

| 表 | 行 | 2 区分 | 3 区分 | 差 |
|---|---|---|---|---:|
| 技術文書を足した前後 | Wikipedia だけ | 141/172(82.0%) | 108/129(83.7%) | 1.7 |
| 技術文書を足した前後 | 技術文書を足す | 140/172(81.4%) | 107/129(82.9%) | 1.5 |
| 対応表 | 基準(広げた正解集合) | 140/175(80.0%) | 107/129(82.9%) | 2.9 |
| 対応表 | 解析辞書にない 1 形態素を足す | 141/175(80.6%) | 107/129(82.9%) | 2.3 |
| 対応表 | 連結をグレーにする | 140/172(81.4%) | 107/129(82.9%) | 1.5 |
| 対応表 | 連結の検査をやめる | 150/172(87.2%) | 114/129(88.4%) | 1.2 |
| 対応表 | 連結を構成の分析に置き換える | 143/172(83.1%) | 108/129(83.7%) | 0.6 |
| 対応表 | 固有名詞の見出しを外す | 140/172(81.4%) | 107/129(82.9%) | 1.5 |
| 対応表 | 回数の下限を 5 にする | 147/172(85.5%) | 113/129(87.6%) | 2.1 |
| 対応表 | 部品の頻度の下限を 3 にする | 140/172(81.4%) | 107/129(82.9%) | 1.5 |
| 対応表 | 組み合わせ | 147/172(85.5%) | 113/129(87.6%) | 2.1 |
| 対応表 | 組み合わせに解析辞書にない 1 形態素を足す | 148/175(84.6%) | 113/129(87.6%) | 3.0 |

差はどの行も 0.6 ポイントから 3.0 ポイントに収まり、行の間の順序は変わらない。`言い換え` の語は既存語と造語の候補の両方へ散るので、分母と一緒に分子も減り、割合はほとんど動かない。区分を分けて得られるのは割合の変化ではなく、`言い換え` の 32 語を誤検出として別に数えられることである。
