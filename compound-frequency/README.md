# 複合語の頻度フィルタ

akunuki は、複合名詞の候補を既存語とグレーと造語の候補へ振り分けるときに、複合語の頻度フィルタを引きます。フィルタの入力は、Wikipedia 日本語版の全記事と日本語の技術文書と日本語コーパス、それに解析辞書 full の見出しと記事名です。何をどう数え、どう判定するかの仕組みと既定の根拠は `design.md` に、正解つきの語の集合と配布中の成果物の当たり方は `eval/README.md` にあります。

## 成果物

`build` が書く成果物は 3 つあります。

| ファイル | 中身 |
|---|---|
| `freq_filter.bin` | 複合語と見出しと記事名のキーと、その頻度のバケットを持つフィルタ |
| `component_freq.fst` | 複合語の部品ごとの出現数と、前後に付く相手の種類数の表 |
| `domain_filter.bin` | 語と分野の組を登録したフィルタ |

3 つとも GitHub リリースのアセットとして配布します。どこでどう使われるかと、ファイルの形式は `artifacts.md` にあります。組んだときの来歴は `manifest.json` が持ちます。入力とその絞り込みの条件、合算に使った重み、バージョンのほか、キーと見出しと記事名と文書の数、成果物のバイト数と SHA-256、バケットの境界が載ります。

バージョンは `dashi-<年>.<月>.<連番>` の形で、`build --version` に渡します。月は 0 埋めし、同じ月に組み直すたびに連番を上げます。入力にしたダンプの日付は `manifest.json` の `dump` が持つので、名前には入れません。

`manifest.json` は、2 つのフィルタのキー数を別々に持ちます。`freq_filter.bin` のキー数は、回数から作ったキーに解析辞書の見出しと記事名を足した数です。`domain_filter.bin` のキー数は、語と分野の組の数です。

成果物は入力の違いで 2 種類あります。配布用は入力をすべて入れて組みます。評価用は、正解つきの語の集合の採り先を入力から外して組み、`variants/evaluation/` に置いて、来歴を `manifest-evaluation.json` に書きます。外すソースは `design.md` の「正解集合の採り先の除外」にあります。

## 構成

`compound-frequency/` の直下に置くファイルとディレクトリは次のとおりです。

| パス | 中身 |
|---|---|
| `README.md` | 成果物と、それを組む手順 |
| `design.md` | 入力から判定までの仕組みと、既定の根拠 |
| `artifacts.md` | 成果物がどこでどう使われるかと、ファイルの形式と引ける情報 |
| `manifest.json` | 配布用の成果物の来歴 |
| `corpus-tool/` | 入力から成果物までを通す Rust の crate |
| `eval/` | 造語と既存語の正解つきの語の集合と、配布中の成果物の当たり方 |
| `archive/` | 終わった実験の記録 |

追跡しない中間物は `.gitignore` に列挙してあります。`counts*/` の回数と見出しと記事名の TSV、`variants/` の成果物の変形、`sudachidict/` の解析辞書の配布物、`corpus/` の取得した技術文書とその平文がこれにあたり、どれも手順を通せば取り直せます。大きさは、複合語の回数だけで 918 MB、語と分野の組の回数で 1.7 GB あります。

## サブコマンド

`corpus-tool` のサブコマンドは 8 つあります。

| サブコマンド | 役割 |
|---|---|
| `count` | 入力を読み、複合語と部品と、語と分野の組の回数を TSV に書く |
| `dictionary-headwords` | 解析辞書の見出しのうち、2 形態素以上に割れる名詞を TSV に書く |
| `article-titles` | ダンプの記事名とリダイレクト名のうち、複合語の区間になる名前を TSV に書く |
| `flatten` | 取得した技術文書を、1 文書 1 ファイルの平文にする |
| `build` | 回数と見出しの TSV から、3 つの成果物と manifest を書く |
| `query` | 成果物を引き、語のバケットと各部品の出現数と相手の種類数を表示する |
| `judge` | 語を振り分け、正解つきの語の集合を渡すと混同行列を出す |
| `topic-distribution` | 正解つきの語の集合が Wikipedia のどの分野に偏るかを測る |

回数と成果物の間に TSV を挟むのは、数十分かかる数え直しと数秒で済むバケットの切り直しを別々にやり直せるようにするためです。引数の一覧は `cargo run --release -- <サブコマンド> --help` で読めます。

## 準備

### Wikipedia のダンプ

Wikimedia が配る cirrussearch のダンプを取ります。大きさは 12 GB です。Wikimedia は古いバージョンを消すので、別の日付で取る場合はファイル名の日付も読み替えてください。

```
curl -O https://dumps.wikimedia.org/other/cirrussearch/20251229/jawiki-20251229-cirrussearch-content.json.gz
```

### 解析辞書の配布物

SudachiDict の `small_lex.zip` と `core_lex.zip` と `notcore_lex.zip` を取って展開すると、 3 つで 71 MB の zip から CSV が 1 つずつ出ます。別のバージョンで取る場合は、URL の日付を akunuki の `aku_morph::DICTIONARY_VERSION` が持つ日付に合わせてください。食い違っていれば `dictionary-headwords` がその場で止まるので、気づけます。

```
mkdir -p compound-frequency/sudachidict
cd compound-frequency/sudachidict
for name in small_lex core_lex notcore_lex; do
    curl -O https://sudachi.s3.ap-northeast-1.amazonaws.com/sudachidict-raw/20260723/$name.zip
    unzip -o $name.zip
done
```

### 技術文書

翻訳された公式文書と技術書を `corpus/raw/<ソース名>/` へ置きます。ソースごとのリポジトリとブランチと commit、日付とパスと形式とライセンスを `corpus/raw/manifest.tsv` に書きます。ソースの一覧は `design.md` の「日本語の技術文書」にあります。

`flatten` が取得物を 1 文書 1 ファイルの平文にし、`corpus/text/<ソース名>/` へ書きます。

```
cd compound-frequency/corpus-tool
cargo run --release -- flatten --raw-dir ../corpus/raw --out-dir ../corpus
```

`corpus/` は 547 MB を使います。内訳は取得物が 460 MB、平文が 88 MB です。いちばん大きいソースは MDN の `files/ja` で、取得物だけで 112 MB あります。

### 日本語コーパス

官公庁と公的機関が公開する書き言葉を集めたコーパスは、リポジトリに入っていません。パスを `count --corpus-dir` に渡してください。資料の形は `design.md` の「日本語コーパス」にあります。

## 再現手順

`corpus-tool` の中で、入力から成果物までを 7 段階 9 コマンドで通します。技術文書と日本語コーパスは配布用と評価用で入力が違うので、この 2 つを数える段階だけコマンドが 2 本ずつ並びます。`<ダンプ>` は取ったダンプを置いたディレクトリ、`<日本語コーパス>` は「日本語コーパス」で用意したディレクトリ、`<バージョン>` は付ける版の名前に読み替えてください。版のほかは引数を渡さなければ既定の値が効くので、そのまま配布用の成果物が組めます。

```
cd compound-frequency/corpus-tool

# 1. 全記事を読み、複合語と部品と、語と分野の組の回数を TSV に書く
cargo run --release -- count --domains \
    --dump <ダンプ>/jawiki-20251229-cirrussearch-content.json.gz --out-dir ../counts

# 2. 記事名とリダイレクト名から、見出しの TSV を書く
cargo run --release -- article-titles \
    --dump <ダンプ>/jawiki-20251229-cirrussearch-content.json.gz --out-dir ../counts-titles

# 3. 解析辞書から、2 形態素以上に割れる名詞の見出しを TSV に書く
cargo run --release -- dictionary-headwords \
    --lexicon-dir ../sudachidict --out-dir ../counts

# 4. 技術文書の平文を読み、複合語と部品の文書数を TSV に書く
cargo run --release -- count --domains --text-dir ../corpus/text --out-dir ../counts-docs
cargo run --release -- count --domains --text-dir ../corpus/text \
    --exclude owasp-top10,ts-survival --out-dir ../counts-docs-evaluation

# 5. 日本語コーパスの平文を読み、複合語と部品の文書数を TSV に書く
cargo run --release -- count --domains --corpus-dir <日本語コーパス> \
    --exclude jawiki,raw --out-dir ../counts-corpus
cargo run --release -- count --domains --corpus-dir <日本語コーパス> \
    --exclude jawiki,raw,jpccert-eyes,ipa-websec --out-dir ../counts-corpus-evaluation

# 6. 配布用の成果物と manifest.json を書く
cargo run --release -- build --counts-dir ../counts --docs-counts-dir ../counts-docs \
    --corpus-counts-dir ../counts-corpus --titles-dir ../counts-titles \
    --version <バージョン> --out-dir ..

# 7. 評価用の成果物と manifest-evaluation.json を書く
cargo run --release -- build --counts-dir ../counts \
    --docs-counts-dir ../counts-docs-evaluation \
    --corpus-counts-dir ../counts-corpus-evaluation --titles-dir ../counts-titles \
    --artifact evaluation --version <バージョン> --out-dir ../variants/evaluation
```

`count` と `article-titles` に `--limit <記事数>` を渡すと、先頭のその数の記事だけを読みます。全体を実行する前の動きの確認に使えます。

組んだ成果物は `query` と `judge` で引きます。`judge` が出す混同行列は、行に正解の区分、列に判定を取り、当てはまる語数を並べた表です。読み方と、配布中の成果物の結果は `eval/README.md` にあります。

```
# 語のバケットと各部品の出現数と相手の種類数を引く
cargo run --release -- query --artifacts-dir .. 公開鍵暗号 深層学習

# 語を振り分ける
cargo run --release -- judge --artifacts-dir ../variants/evaluation 公開鍵暗号 幽霊参照

# 正解つきの語の集合の混同行列を出す
cargo run --release -- judge --artifacts-dir ../variants/evaluation --eval ../eval/coined.tsv \
    ../eval/established.tsv ../eval/established-security.tsv

# 文書の分野を情報技術として、転用の候補も出す
cargo run --release -- judge --artifacts-dir ../variants/evaluation \
    --document-domain 情報技術 --eval ../eval/coined.tsv \
    ../eval/established.tsv ../eval/established-security.tsv
```

## 実行時間とメモリ

時間と最大 RSS は、`/usr/bin/time -v` の経過時間と maximum resident set size です。

| コマンド | 入力 | 時間 | 最大 RSS |
|---|---|---:|---:|
| `count --domains --dump` | 全記事 1,484,267 件 | 30 分 48 秒 | 1,263,568 kB |
| `article-titles` | 全記事 1,484,267 件 | 3 分 31 秒 | 262,048 kB |
| `dictionary-headwords` | 解析辞書 full | 3.7 秒 | 458,688 kB |
| `count --domains --text-dir` | 技術文書 13,846 文書 | 3.4 秒 | 220,528 kB |
| `count --domains --corpus-dir` | 日本語コーパス 14,540 文書 | 26.2 秒 | 389,776 kB |
| `build` | 3 つの回数と見出しと記事名の TSV | 37.1 秒 | 3,286,912 kB |
| `topic-distribution` | 全記事 1,484,267 件 | 4 分 5 秒 | 14,816 kB |

語と分野の組を数えても `count` の最大 RSS が上がらないのは、複合語の表と組の表の合計を見て両方を途中で書き出すためです。技術文書の `count` は、数える本文が記事の 0.6% しかないので、途中の表を書き出さずに最後まで持てます。時間の側では、本文を解析しない `article-titles` と `topic-distribution` が、同じダンプでも `count` より 1 桁短く終わります。`dictionary-headwords` の 3.7 秒は、解析を CPU の数だけ並べた結果です。評価用の 2 つの `count` は、どちらも配布用より短い時間で終わります。

## 数えた結果

入力ごとの件数のうち、配布用と評価用で違うのは技術文書と日本語コーパスだけです。

読んだ全記事は 1,484,267 件、数えた本文は 9,285,571,411 バイトです。解析に失敗して飛ばした記事はありません。複合語のキーは 45,888,704 件、部品は 1,617,547 件です。語と分野の組は 79,277,802 件で、分野ごとの記事の数は `design.md` の「分野の区分」にあります。

技術文書は、配布用が 16 ソースの 13,846 文書、平文 54,803,373 バイトで、キーは 168,598 件、部品は 13,848 件です。評価用は 14 ソースの 13,562 文書、53,656,706 バイトで、キーは 165,320 件、部品は 13,614 件です。どちらも解析に失敗して飛ばした文書はありません。文書単位で数えるので、1 つのキーの回数は文書の数を超えません。

日本語コーパスは、配布用が 20 ソースの 14,540 文書、平文 535,840,821 バイトで、キーは 675,136 件、部品は 48,129 件です。評価用は 18 ソースの 14,400 文書、533,972,365 バイトで、キーは 662,992 件、部品は 47,383 件です。どちらも文語体の法令 284 件を外してあります。

解析辞書の見出しは、2,274,679 件の名詞のうち 83,594 件が 2 形態素以上に割れます。`dictionary-headwords` は固有名詞の見出しを書かないので、内訳は普通名詞 83,548 件と、数詞と助動詞語幹の 46 件です。記事名の側では、`article-titles` が 1,484,267 件の記事名と 928,100 件のリダイレクト名を読み、464,833 語を登録します。

配布用と評価用の成果物が持つキー数と部品数、書き出したファイルのバイト数は次のとおりです。

| 成果物 | キー数 | `freq_filter.bin` のバイト数 | 部品数 | `component_freq.fst` のバイト数 | 組のキー数 | `domain_filter.bin` のバイト数 |
|---|---:|---:|---:|---:|---:|---:|
| 配布用 | 11,538,100 | 26,017,878 | 701,675 | 10,070,883 | 8,087,384 | 18,219,094 |
| 評価用 | 11,536,810 | 26,017,878 | 701,631 | 10,070,592 | 8,087,207 | 18,219,094 |

2 つの差は、キー 1,290 件と部品 44 件、それに語と分野の組 177 件だけです。外した 4 ソースが持つ語の大半が、Wikipedia の本文か記事名か解析辞書の見出しにもあるためです。`freq_filter.bin` のバイト数が同じなのは、フィルタの配列がキーの数を段階で丸めた大きさを取るためです。
