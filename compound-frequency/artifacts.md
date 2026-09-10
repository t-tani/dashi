# 成果物の形式と使われ方

複合語の頻度フィルタを組む `corpus-tool` の `build` は、3 つのファイルと来歴の `manifest.json` を書きます。形式の原本は akunuki の `crates/aku_freq/` です。構築と照合とバイナリの読み書きは、この crate が持っています。数える側の corpus-tool と検査する側の akunuki が同じ crate を使わなければ、数えたキーは検査する側の候補と当たりません。

## ファイル一覧

`build` が書くファイルと、それぞれを読む側は次のとおりです。

| ファイル | 形式 | 読む側 |
|---|---|---|
| `freq_filter.bin` | `aku_freq` の binary fuse filter | akunuki の `unknown-compound`・`field-transfer`、corpus-tool の `query`・`judge` |
| `domain_filter.bin` | `aku_freq` の binary fuse filter(キーの形が違う) | akunuki の `field-transfer`、corpus-tool の `judge --document-domain` |
| `component_freq.fst` | fst の Map | corpus-tool の `query`・`judge` だけ |
| `manifest.json` | JSON | 成果物を組み直す人 |

akunuki が埋め込むのは `freq_filter.bin` と `domain_filter.bin` の 2 つです。 `component_freq.fst` はリリースに載せますが、akunuki は読みません。部品の比で造語の候補を分ける判定は、corpus-tool の `judge` だけが行います。

## 利用箇所

成果物が dashi から akunuki と corpus-tool へ渡る道筋は次のとおりです。

```
dashi: corpus-tool build
  │  freq_filter.bin, domain_filter.bin, component_freq.fst, manifest.json
  │
  ├─▶ GitHub リリース <バージョン> のアセット
  │     │
  │     ├─▶ cargo build 時: crates/aku_morph/build.rs
  │     │     取得 → SHA-256 の検証 → include_bytes! でバイナリへ埋め込み(2 つのフィルタだけ)
  │     │     │
  │     │     └─▶ aku lint 実行時: プロセスに 1 回だけ読み、文書ごとの検査で使い回す
  │     │           unknown-compound ── freq_filter.bin
  │     │           field-transfer ──── freq_filter.bin + domain_filter.bin
  │     │
  │     └─▶ wasm: 埋め込まず、呼び出し側が Linter.supply_frequency_filter /
  │                supply_domain_filter でバイト列を渡す
  │
  └─▶ corpus-tool query / judge(3 ファイルすべて。評価と確認に使う)
```

### ビルド時の埋め込み

akunuki の `crates/aku_morph/build.rs` が、`morph` feature のビルドで 2 つのフィルタを取得してバイナリへ埋め込みます。

```
for file in [freq_filter.bin, domain_filter.bin]:
    bytes = $AKUNUKI_FREQ_FILTER_DIR/<file>            // 環境変数があればそこから
          | <キャッシュ>/<バージョン>-<file>              // 前に取得していればそこから
          | https://github.com/t-tani/dashi/releases/download/<バージョン>/<file>
    assert sha256(bytes) == build.rs の定数              // バージョンごとに固定
    include_bytes!(bytes)                                // static な &[u8] になる
```

バージョンと 2 つの SHA-256 は、build.rs の定数が持ちます。値は `manifest.json` と同じで、バージョンの形は `dashi-<年>.<月>.<連番>` です。成果物を組み直してバージョンを上げたら、この定数も上げてください。

フィルタを差し替えられるのは構築時だけで、実行時に別のフィルタを読む口はありません。ただし wasm のビルドでは埋め込まないため、呼び出し側が `Linter` の `supply_frequency_filter` と `supply_domain_filter` に配布物のバイト列をそのまま渡します。

### 実行時の読み込み

埋め込んだバイト列は、最初に必要になったときに 1 回だけ `FrequencyFilter::from_bytes` で組み、プロセスの間は使い回します(`OnceLock`)。読み込むときは、ヘッダーの解析辞書のバージョンを `aku_morph` の `DICTIONARY_VERSION` と突き合わせ、食い違えば失敗します。

フィルタが無くても検査は続きます。読み込みに失敗した場合は、標準エラー出力に理由を出したうえで、`unknown-compound` は頻度の照合をせず、`field-transfer` は指摘を出しません。wasm で呼び出し側がバイト列を渡すまでの間も、同じ挙動です。

読むかどうかは設定で決まります。`[detectors.unknown-compound]` が有効なら頻度フィルタを、 `[detectors.field-transfer]` が有効なら 2 つのフィルタを読みます。どちらも無効なら読みません。

### unknown-compound での使い方

文書の断片ごとに分割単位 A で形態素解析し、名詞と接頭辞が原文で隣り合う連なりを切り出します。連なりは数詞と空白で切れます。候補になるのは、固有名詞を含まず、名詞を 1 つ以上含む連なりだけです。そのうえで、次のどちらかを満たす連なりが候補になります。

- 漢字を含む形態素が 2 つ以上あり、ラテン文字を含まない
- 送りがな付きの漢字名詞と、ラテン文字かカタカナの名詞とが対で現れる

候補になった連なりは、バケットと既知語の照合で重大度を決めます。

```
for run in compound_runs(fragment):                  // 名詞と接頭辞の連なり
    if !is_candidate(run): continue
    surface    = fragment.text[run.range]
    normalized = concat(m.normalized_form for m in run)
    bucket     = max(freq.bucket(surface), freq.bucket(normalized))   // None なら登録なし

    if bucket >= 2:                                  // コーパスで 10 回以上、または見出し
        continue                                     // 既知語。報告しない
    if surface in allow_list or surface in jmdict_headwords:
        continue
    if bucket == 1:                                  // 3 回から 9 回
        report(info)
    elif splits_into_known_pieces(surface):          // 2 字以上の既知語と 1 字の接辞で敷き詰められる
        report(info)
    else:
        report(warn)
```

`splits_into_known_pieces` の片の判定にも同じフィルタを使い、片が 2 字以上でバケット 2 以上なら既知語の片と見なします。接辞は、同梱の 60 字と `affixes` で追加した字です。同梱の側は `builtin_affixes` で切れます。バケットの境界(2 が 10 回以上)は、`manifest.json` の `buckets` と `aku_morph` の `ESTABLISHED_BUCKET` が持っています。

### field-transfer での使い方

同じ連なりを、同じ 2 つのキーで引きます。対象は頻度フィルタでバケット 2 以上と決まった連なりだけで、造語の候補は見ません。

```
document = { domain_of_field(tag) for tag in config.project.fields }   // comp → 情報技術、law → 法令
if document is empty: return                         // 比べる先がないので何も報告しない

for run in compound_runs(fragment):
    if !is_candidate(run): continue
    if max(freq.bucket(surface), freq.bucket(normalized)) < 2: continue
    found = domains.domains(surface) ∪ domains.domains(normalized)     // 分野の集合
    if found is empty: continue                      // どの分野でも下限に届かない語
    if 一般 in found or (found ∩ document) != ∅: continue
    report(info, word = surface, domains = names(found))               // 重大度は info に固定
```

文書の分野は、設定の `[project].fields` の分野タグから引きます。タグは JMdict の略称で、フィルタの分野と指すものが同じ `comp` と `law` だけが対応を持ちます。

### corpus-tool での使い方

`query` は語を分割単位 A で割り、表層形と正規化形で `freq_filter.bin` を引いてバケットを出し、各部品の頻度を `component_freq.fst` から並べます。`judge` は完全一致、部品の比、分野の 3 つの検査で語を 4 つの名前に振り分け、正解つきの語の集合を渡すと混同行列を出します。名前と検査の順と閾値は、`design.md` の「判定の名前」と「判定」にあります。

## フィルタのバイナリ形式

`freq_filter.bin` と `domain_filter.bin` は同じ形式です。値を持たない集合のフィルタ (xorf 0.13.0 の `BinaryFuse16`)に、キーの文字列とバケットから作った 64 bit のハッシュを登録します。整数はすべて little endian です。

```
FilterFile {
    magic:                u8[8]    = "AKUFREQ\0"
    format_version:       u16      = 2
    key_hash_seed:        u64      = 0x616b755f66726571   // "aku_freq" の ASCII
    key_count:            u32                             // 重複を除いた登録数
    crate_version:        str16                           // aku_freq の CARGO_PKG_VERSION
    dictionary_version:   str16                           // "sudachi-dictionary-20260723"
    seed:                 u64                             // ここから 4 つは BinaryFuse16 の descriptor
    segment_length:       u32                             // 2 のべき乗
    segment_length_mask:  u32                             // segment_length - 1
    segment_count_length: u32
    array_length:         u64                             // segment_count_length + 2 * segment_length
    fingerprints:         u16[array_length]               // BinaryFuse16 の配列
}

str16 {
    length: u16
    bytes:  u8[length]                                    // UTF-8
}
```

キーの登録と照合では、バケットを値として持たず、キーのハッシュにバケットごとの定数を XOR して畳み込みます。定数はバケットに奇数の乗数を掛けた値で、64 bit の全体に散ります。シードにバケットを畳み込む形にはしません。xxh3 は 8 バイト以下の入力ではシードを特定のバイトにしか掛けないため、短いキーの対が別のバケットで同じハッシュになるからです。

```
key_hash(key, bucket) = xxh3_64(key.as_bytes(), seed = key_hash_seed) ^ (bucket * 0x9e3779b97f4a7c15)

register(key, bucket):  filter.insert(key_hash(key, bucket))
bucket(key):            max { b in 7..=1 | filter.contains(key_hash(key, b)) }
                        // 1 つも当たらなければ None(登録なし)
```

照合はバケット 7 から 1 へ順に試し、最初に当たったバケットを返します。同じキーを複数のバケットで登録しても、照合が返すのは最大の 1 つだけです。

読み込むときは、マジックナンバーから解析辞書のバージョンまでのヘッダーの 5 項目を読み込む側の期待と突き合わせ、1 つでも食い違えば失敗します。`aku_freq` のバージョンは完全一致を求めるため、crate のバージョンを上げると、それ以前に組んだ成果物は読めなくなります。解析辞書のバージョンは、akunuki の `aku_morph::DICTIONARY_VERSION` と一致していなければなりません。キーは解析辞書で割った形態素から作るため、辞書のバージョンが違えば候補はキーに当たりません。

登録のないキーが誤って当たる率は、7 通りの照合の和で 7 ÷ 65536、およそ 0.011% です。登録したキーは必ず当たります。ただし、登録したバケットより上のバケットでフィンガープリントが衝突すると、照合はそちらを返します。この率も同じ桁です。

この形式は集合のフィルタなので、次の 3 つは取り出せません。

- キーの一覧
- キーの回数
- あるキーが「回数から来た」か「見出しから来た」かの区別

中身はハッシュのフィンガープリントだけなので、キーの文字列は残りません。回数も、残るのはバケットだけです。バケット 7 は見出しと記事名の印なので、その語が回数の側にもあったかはわかりません。

## freq_filter.bin

複合語のキーとそのバケットを持ちます。キーは 3 種類あります。

| キー | 作り方 | バケット |
|---|---|---|
| 複合語 | 複合語を成す品詞が原文で続く 2 から 5 形態素の区間について、表層形を連ねた文字列と正規化形を連ねた文字列 | 3 つの入力の回数を重みつきで足した和を丸めた 1 から 5 |
| 解析辞書にない 1 形態素 | 解析辞書に見出しがないカタカナと英字の連なりの表層形 | 3 つの入力の回数を重みつきで足した和を丸めた 1 から 5 |
| 見出しと記事名 | 分割単位 A で 2 形態素以上に割れ、固有名詞を含まない SudachiDict full の名詞の見出しと、Wikipedia の記事名とリダイレクト名 | 7 |

複合語を成す品詞は、普通名詞と固有名詞と接頭辞と接尾辞です。区間の切り出しの条件は `design.md` の「数え方」にあり、3 形態素以上の区間では隣り合う 2 形態素の対もキーになります。一方、解析辞書にない 1 形態素は `kubectl` や `フルユニバース` の形で、1 形態素が 1 つのキーになります。また、記事名は全体が複合語の 1 つの区間として成り立つものだけを採ります。

バケットの境界は `manifest.json` の `buckets` が持ちます。

| バケット | 回数 |
|---:|---|
| 1 | 3 から 9 |
| 2 | 10 から 99 |
| 3 | 100 から 999 |
| 4 | 1,000 から 9,999 |
| 5 | 10,000 以上 |
| 7 | 解析辞書の見出しと記事名 |

引けるのは「この文字列は登録されているか。されているならバケットはいくつか」だけです。バケットの読み方は「unknown-compound での使い方」にあります。

corpus-tool の `query` で引くと、語ごとにバケットと部品の頻度が並びます。

```
cargo run --release -- query --artifacts-dir .. 公開鍵暗号 幽霊参照
公開鍵暗号	7	公開:265016 鍵:7237 暗号:17388
幽霊参照	未登録	幽霊:8224 参照:43056
```

## domain_filter.bin

語がどの分野に現れるかを引きます。形式は `freq_filter.bin` と同じで、違うのはキーの形とバケットの使い方です。

```
domain_key(code, word) = format("{code}\t{word}")       // code は 1..=8 の 10 進、word は複合語のキー

register(code, word):   filter.insert(key_hash(domain_key(code, word), bucket = 7))
domains(word):          { code in 1..=8 | bucket(domain_key(code, word)) != None }
```

語は `freq_filter.bin` の複合語のキーと同じ文字列(表層形と正規化形)です。登録はすべてバケット 7 で行い、バケットは何も表しません。分野をキーに置くのは、照合が当たったバケットのうち最大の 1 つしか返さず、分野の集合をバケットでは取れないためです。

分野の番号と名前、その由来は次のとおりです。

| 番号 | 分野 | 由来 |
|---:|---|---|
| 1 | 情報技術 | Wikipedia の `STEM.Computing`・`STEM.Technology`、技術文書、情報技術の機関の文書 |
| 2 | その他の理工 | 残りの `STEM` |
| 3 | 文化 | `Culture` |
| 4 | 地理 | `Geography` |
| 5 | 社会 | `History_and_Society` |
| 6 | 行政 | 官公庁の説明文と指示文 |
| 7 | 法令 | 法令文 |
| 8 | 一般 | 登録される分野の数が閾値以上の語 |

番号と名前の対応は、`manifest.json` の `domain.domains` にもあります。番号を変えると、それまでに作ったフィルタは別の分野を指すようになります。akunuki 側も同じ 8 つを `aku_morph` の `compound_domain_filter.rs` に持っています。

照合は、8 つの番号でキーを組んで順に引き、当たった番号を集めます。1 つの組を引くたびにバケット 7 から 1 まで 7 通りを試すため、1 語あたりの照合は 56 回です。登録のない組が誤って当たる率は、語ごとにおよそ 0.09%(56 ÷ 65536)になります。引けるのは「この語はどの分野に登録されているか」の集合だけで、分野ごとの回数や割合は残りません。

登録の条件(件数の下限 5、5 分野以上は一般に畳む)は `design.md` の「登録の条件」に、条件の値は `manifest.json` の `domain` にあります。集合が空の語は、どの分野でも下限に届かなかった語です。集合の読み方は「field-transfer での使い方」にあります。corpus-tool の `judge --document-domain <分野>` も同じように判定します。

## component_freq.fst

複合語の部品ごとの頻度表です。形式は fst 0.4.7 の `Map` で、キーが部品の文字列、値が `u64` の頻度です。フィルタと違って、キーの一覧の取り出し、接頭辞での検索、頻度そのものの取り出しができます。

部品は、複合語の区間に現れた形態素の表層形です。正規化形は持たないため、引くときも表層形で引きます。区間の外に現れた名詞は数えません。

頻度は、3 つの入力の回数を重み 1.0 で足した和です。Wikipedia では出現回数を、技術文書と日本語コーパスでは文書数を数えます。和が 3 に満たない部品は載せません。

読み込むのは corpus-tool だけです。`query` が各部品の頻度を並べ、`judge` が部品の頻度の比 `min(f(X), f(Y)) / (f(XY) + 1)` で造語の候補の理由を「部品」と「未登録」に分けます。構成の分析(`judge --concatenation composition`)も、1 形態素の単位の頻度をこの表から取ります。

## manifest.json

`build` が成果物と同じパスに書く来歴です。評価用の成果物では `manifest-evaluation.json` という名前になります。書き出す側の定義は、corpus-tool の `src/build.rs` の `Manifest` にあります。

```
Manifest {
    artifact:       "release" | "evaluation"
    version:        string                  // "dashi-<年>.<月>.<連番>"
    inputs: {
        wikipedia:  { counts_dir: string, weight: f64 }
        documents:  {                       // 技術文書。合算しなければ null
            counts_dir: string, weight: f64, text_dir: string,
            documents: u64, text_bytes: u64,
            excluded_sources: string[],
            sources: [{ name, commit, license: string, documents, text_bytes: u64 }]
        } | null
        corpus:     {                       // 日本語コーパス。合算しなければ null。パスは書かない
            counts_dir: string, weight: f64,
            documents: u64, text_bytes: u64,
            excluded_sources: string[], excluded_laws: u64, unreadable_law_ids: u64,
            sources: [{ name: string, styles: string[], licenses: string[], domain: string,
                        documents, text_bytes: u64 }]
        } | null
    }
    dump:           { file_name: string, version: string | null }
    selection:      { condition: string, scanned_articles, selected_articles, text_bytes,
                      skipped_articles: u64, domain_articles: [{ domain: string, documents: u64 }] }
    akunuki:        { rev: string, dictionary_version: string }
    dictionary:     { version: string, lexicon_files: string[], read_headwords,
                      registered_headwords, registered_common_nouns, registered_proper_nouns,
                      registered_other_nouns: u64 }
    titles:         { titles_dir: string, scanned_articles, read_titles, read_redirect_titles,
                      registered_titles: u64 } | null
    variant:        { unknown_morphemes: bool, proper_noun_headwords: bool,
                      min_count: u64, min_component_count: u64 }
    domain:         {                       // 分野の回数がなければ null
        domains:   [{ code: u8, name: string }]
        min_count: u64, general_threshold: usize          // 件数で登録したとき
        share_threshold: f64, general_min_total: u64      // 割合で登録したとき(件数のときは無い)
        keys: u32
        domain_filter_bin: { bytes: u64, sha256: string }
        registered: [{ domain: string, keys: u64 }]
    } | null
    compound_keys:  u32                     // freq_filter.bin に登録したキーの数
    artifact_bytes: { freq_filter_bin:    { bytes: u64, sha256: string },
                      component_freq_fst: { bytes: u64, sha256: string } }
    buckets:        [{ bucket: u8, min_count: u64, max_count: u64 | null }]
}
```

`inputs.corpus` にコーパスのディレクトリを書かないのは、組み直す側が `count` に引数で渡すためです。一方、`variant` と `domain` の条件を読めば、同じ回数の TSV から同じ成果物を組み直せます。 `akunuki.rev` と `dictionary.version` は、成果物を読める akunuki のバージョンを決めます。

## 中間の TSV

`count` と `dictionary-headwords` と `article-titles` が書く TSV は、どれも 1 行が 1 キーで、キーの昇順に並びます。`build` はこの TSV だけを読むため、数え直しとバケットの切り直しを別々にやり直せます。git では追跡していません。

行の形とキーの作り方は次のとおりです。

```
row               = key "\t" count "\n"          // count は 10 進の整数
key (compound)    = surface | normalized          // 形態素の表層形か正規化形を連ねた文字列
key (component)   = surface                       // 1 形態素の表層形
key (domain)      = word "\t" code                // code は 1..=7 の 10 進。一般(8)は build が決める
```

TSV ごとの書く側とキーと回数は次のとおりです。

| ファイル | 書く側 | キー | 回数 |
|---|---|---|---|
| `compound-counts.tsv` | `count` | 複合語のキー | Wikipedia では出現回数、技術文書と日本語コーパスでは文書数 |
| `component-counts.tsv` | `count` | 部品の表層形 | Wikipedia では出現回数、技術文書と日本語コーパスでは文書数 |
| `domain-counts.tsv` | `count --domains` | `<語><タブ><分野の番号>` | その分野で語が現れた記事か文書の数 |
| `dictionary-headwords.tsv` | `dictionary-headwords` | 見出し | 1 |
| `article-titles.tsv` | `article-titles` | 記事名とリダイレクト名 | 1 |

`domain-counts.tsv` は番号を語の後ろに置きます。キーの昇順で 1 つの語の行が隣り合うため、 `build` は数千万件の組を表に載せずに語ごとの総数を出せます。フィルタに登録するときに `<番号><タブ><語>` へ組み替えます。

TSV を書くコマンドは、それぞれの TSV と並べて、数えた記録を JSON で書きます。`build` がこれを読んで、`manifest.json` の各節に写します。

| ファイル | 書く側 | 記録 |
|---|---|---|
| `count-stats.json` | `count --dump` | ダンプの名前とバージョン、記事を採った条件と数、分野ごとの記事の数 |
| `document-stats.json` | `count --text-dir` | 技術文書のソースごとの文書数とバイト数 |
| `corpus-stats.json` | `count --corpus-dir` | 日本語コーパスのソースごとの文体とライセンスと文書数、外した法令の数 |
| `dictionary-headwords-stats.json` | `dictionary-headwords` | 解析辞書のバージョンと、読んだ見出しと登録した見出しの数 |
| `article-titles-stats.json` | `article-titles` | 読んだ記事名とリダイレクト名の数と、登録した数 |
