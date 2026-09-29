# 外来語の置き換え表の生成スクリプト

見出し語の一覧を作る工程と、LLM に置き換えの候補を挙げさせる工程を、`pipeline/` のスクリプトで行います。コマンドは `loanword-substitution/` の中で `uv run python pipeline/<スクリプト>` の形で実行します。

## 準備

名詞の回数は `../compound-frequency/corpus-tool/` の Rust の `count-nouns` で数えます。ビルドは `cargo build --release` です。見出し語の一覧の生成と、LLM に候補を挙げさせる工程は Python で、依存は uv で管理しています。定義はリポジトリ直下の `pyproject.toml` と `uv.lock` にあります。

```
uv sync
```

入力は 4 つで、いずれも別途ダウンロードが必要です。

| 入力 | 用途 | 入手 |
|---|---|---|
| Wikipedia のダンプ | 話題別の記事と、全記事の頻度 | `compound-frequency/README.md` の「Wikipedia のダンプ」と同じ cirrussearch のダンプ |
| 技術文書の平文 | 分野の頻度 | `compound-frequency/README.md` の「技術文書」の手順で `../compound-frequency/corpus/text/` に作る |
| 日本語コーパス | 分野の頻度(情報技術の公的機関の文書) | `compound-frequency/design.md` の「日本語コーパス」の形のディレクトリ |
| JMdict の XML | 語源の英語 | `http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz` を取得して展開する |

候補を挙げさせるときは LLM のゲートウェイを呼ぶので、`semantic-class/pipeline/README.md` と同じ環境変数(`LLM_BASE_URL`・`LLM_API_KEY`・`ANTHROPIC_MODEL`)が `~/.env` に必要です。

## 生成の流れ

`<ダンプ>` はダンプのファイル、`<日本語コーパス>` は日本語コーパスのディレクトリ、`<JMdict_e.xml>` は JMdict の XML に読み替えてください。段階 1 は `compound-frequency/corpus-tool/` の中で、段階 2 以降は `loanword-substitution/` の中で実行します。

```
cd compound-frequency/corpus-tool

# 1. 名詞の回数を数える。技術文書、情報技術の機関、Wikipedia の STEM.Computing の記事、Wikipedia の全記事
cargo run --release -- count-nouns --text-dir ../corpus/text --out-dir ../../loanword-substitution/counts/it-docs
cargo run --release -- count-nouns --corpus-dir <日本語コーパス> \
    --exclude <情報技術の機関でないソース> \
    --out-dir ../../loanword-substitution/counts/it-agency
cargo run --release -- count-nouns --dump <ダンプ> --computing --out-dir ../../loanword-substitution/counts/wiki-computing
cargo run --release -- count-nouns --dump <ダンプ> --out-dir ../../loanword-substitution/counts/wiki-all

cd ../../loanword-substitution

# 2. JMdict を引く索引を作る。カタカナの読みと、漢字とかなの見出し
uv run python pipeline/jmdict_readings.py --jmdict <JMdict_e.xml> --out counts/jmdict-readings.json
uv run python pipeline/jmdict_glosses.py --jmdict <JMdict_e.xml> --out counts/jmdict-glosses.json

# 3. 見出し語の一覧を書く。カタカナ語の軸と英語の軸
uv run python pipeline/build_headwords.py --counts-dir counts \
    --readings counts/jmdict-readings.json --out headwords/katakana.tsv
uv run python pipeline/build_english_headwords.py --counts-dir counts \
    --readings counts/jmdict-readings.json --glosses counts/jmdict-glosses.json --out headwords/english.tsv

# 4. LLM に渡す分だけに絞った一覧を書く
uv run python pipeline/build_headwords.py --counts-dir counts \
    --readings counts/jmdict-readings.json --min-rate 10 --english-only \
    --out headwords/katakana-selected.tsv
uv run python pipeline/build_english_headwords.py --counts-dir counts \
    --readings counts/jmdict-readings.json --glosses counts/jmdict-glosses.json \
    --min-rate 50 --single-word --min-fan-in 2 --out headwords/english-selected.tsv

# 5. LLM に候補を挙げさせる(ゲートウェイを呼ぶ)。--dry-run はプロンプトを組み立てるだけで呼ばない
uv run python pipeline/collect_candidates.py --headwords headwords/katakana-selected.tsv --axis loanword --dry-run
uv run python pipeline/collect_candidates.py --headwords headwords/english-selected.tsv --axis english --dry-run

# 6. 集めた候補から表を書く
uv run python pipeline/build_table.py --candidates-dir candidates --counts-dir counts \
    --glosses counts/jmdict-glosses.json --readings counts/jmdict-readings.json \
    --out substitution-table.tsv
```

段階 1 の `--exclude` は、日本語コーパスのソースのうち情報技術の機関(IPA・JPCERT・JVN・NCO・NICTER の 9 ソース)以外を外します。`count-nouns` は分割単位 C で名詞と形状詞を数え、数詞は数えません。回数は全出現数と、前後に名詞・形状詞・接辞が付かない単独の出現数の 2 つで、出力は `noun-counts.tsv` と `noun-stats.json` です。

## モジュール

| モジュール | 役割 |
|---|---|
| `jmdict_readings.py` | JMdict のカタカナの読みから英訳と分野タグと語源の言語を引く索引 |
| `jmdict_glosses.py` | JMdict の漢字とかなの見出しから英訳と分野タグを引く索引 |
| `build_headwords.py` | 頻度の閾値と語源の英語の付与による、カタカナ語の一覧の生成 |
| `build_english_headwords.py` | 情報技術で頻出の語の英訳ごとに、その英語を訳に持つ語を並べた、英語の一覧の生成 |
| `collect_candidates.py` | DSPy で LLM に候補を挙げさせ、JSONL に書く。カタカナ語の軸と英語の軸を `--axis` で選ぶ。プロンプトのバージョンと temperature とモデル名をレコードに残す |
| `build_table.py` | 集めた候補を絞り込んで置き換え表を書く |
| `lm.py` | ゲートウェイを呼ぶ LM の組み立て |

カタカナ語の軸のプロンプトは、1 つの語義を 1 回で渡し、その語義の英訳が複数あれば `;` でつないで並べます。候補がどの語義から出たかは `sense` の列に残ります。英訳を 1 つずつ渡した場合、モデルが返すのは、渡した英訳ではなくカタカナ語の別の語義に沿った候補です。`ティア` に `tear` だけを添えると `層`・`階層` が返り、`tier` の語義の答えが `tear` の行に載ります。
