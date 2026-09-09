//! 回数と見出しの TSV から、フィルタと部品の頻度表と manifest を書く。
//!
//! 回数の入力は 3 つある。Wikipedia の記事ごとの出現回数と、技術文書の文書数と、
//! メンテナーが集めた日本語コーパスの文書数である。同じキーの回数に重みを掛けて
//! 足し、その和をバケットに丸める。バケット 7 の見出しは解析辞書と記事名の
//! 2 つの TSV から読み、回数のバケットに重ねる。
//!
//! 語と分野の組の回数がある入力では、分野のフィルタ [`domain_filter`] も書く。
//! 登録の条件は頻度のフィルタと別であり、`--domain-min-count` と
//! `--general-threshold` が決める。
//!
//! 成果物は入力の違いで 2 つに分かれる。[`Artifact::Release`] は akunuki に
//! 埋め込む成果物であり、[`Artifact::Evaluation`] は正解集合の採り先を除いて
//! 組んだ、混同行列を出すための成果物である。どちらで組んだかは manifest の
//! ファイル名と項目に残る。

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use aku_freq::{Bucket, ConstituentFrequencies, FrequencyFilter};
use aku_morph::DICTIONARY_VERSION;
use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::compound::UnknownMorphemes;
use crate::count::corpus::{CORPUS_STATS_FILE, CorpusSourceStats, CorpusStats};
use crate::count::documents::{DOCUMENT_STATS_FILE, DocumentStats, SourceStats};
use crate::count::{
    COMPONENT_COUNTS_FILE, COMPOUND_COUNTS_FILE, COUNT_STATS_FILE, CountStats, DOMAIN_COUNTS_FILE,
};
use crate::domain::{Domain, DomainTally};
use crate::domain_filter::{self, DOMAIN_FILTER_FILE, DomainKeys, Registration};
use crate::headwords::{HEADWORD_STATS_FILE, HEADWORDS_FILE, HeadwordStats};
use crate::titles::{TITLE_STATS_FILE, TITLES_FILE, TitleStats};
use crate::tsv;

/// 複合語の頻度のフィルタを書くファイル名。
pub const FILTER_FILE: &str = "freq_filter.bin";

/// 部品の頻度表を書くファイル名。
pub const CONSTITUENT_FILE: &str = "component_freq.fst";

/// フィルタに入れる複合語の回数の下限の既定値。バケット 1 の下端である。
pub const DEFAULT_MIN_COUNT: u64 = 3;

/// 部品の頻度表に入れる頻度の下限の既定値。2 回以下の部品は捨てる。この剪定は
/// 判定を変えず、頻度表を 9.7 MB 小さくする。
pub const DEFAULT_MIN_COMPONENT_COUNT: u64 = 3;

/// 分野のフィルタに登録する、分野の中の回数の下限の既定値。語がその分野の
/// 5 記事か 5 文書に現れることを求める。
pub const DEFAULT_DOMAIN_MIN_COUNT: u64 = 5;

/// 一般に畳む、登録される分野の数の下限の既定値。3 と 4 は転用の候補の再現率が
/// 同じで、4 は大きさと誤検出だけが増える。5 は再現率を倍にする。3 通りを測った
/// 結果は `compound-frequency/README.md` にある。
pub const DEFAULT_GENERAL_THRESHOLD: usize = 5;

/// 組む成果物の種類。入力の違いだけで分かれる。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Artifact {
    /// akunuki に埋め込む成果物。入力をすべて入れて組む。
    #[default]
    Release,
    /// 混同行列を出す成果物。正解集合の採り先の 4 ソースを除いて組む。統計に
    /// 入れた文書から採った既存語は必ず登録済みになり、誤検出率が測れない。
    Evaluation,
}

impl Artifact {
    /// 来歴を書くファイル名。2 つの成果物を同じディレクトリへ組んでも、来歴は
    /// 上書きし合わない。
    fn manifest_file(self) -> &'static str {
        match self {
            Self::Release => "manifest.json",
            Self::Evaluation => "manifest-evaluation.json",
        }
    }
}

/// バケットの下限。回数がこの値以上のうち、最大のバケットを採る。回数が 2 以下の
/// 複合語はフィルタに入れない。6 はこの成果物では使わない。7 は解析辞書の見出しに
/// 与えるので、回数からは作らない。
const BUCKET_MINIMUMS: [(u8, u64); 5] = [(1, 3), (2, 10), (3, 100), (4, 1_000), (5, 10_000)];

/// `build` が読む回数のディレクトリと、合算の重み。
pub struct Inputs {
    /// Wikipedia を数えた `count` と `dictionary-headwords` の出力。
    pub counts_dir: PathBuf,
    /// 技術文書を数えた `count` の出力。
    pub docs_counts_dir: Option<PathBuf>,
    /// 日本語コーパスを数えた `count` の出力。
    pub corpus_counts_dir: Option<PathBuf>,
    /// 記事名を読んだ `article-titles` の出力。渡さなければ記事名を登録しない。
    pub titles_dir: Option<PathBuf>,
    /// 合算の重み。
    pub weights: Weights,
    /// 剪定の下限。
    pub pruning: Pruning,
    /// 分野のフィルタの登録の条件。
    pub domain: Registration,
    /// 組む成果物の種類。
    pub artifact: Artifact,
    /// 成果物の版の名前。`dashi-<年>.<月>.<連番>` の形で渡す。入力にした
    /// ダンプの日付は manifest の `dump` が持つので、名前には入れない。
    pub version: String,
}

/// 剪定の下限。回数がこれに満たない複合語のキーと部品は、成果物に入れない。
#[derive(Clone, Copy)]
pub struct Pruning {
    /// フィルタに入れる複合語の回数の下限。
    pub min_count: u64,
    /// 部品の頻度表に入れる頻度の下限。
    pub min_component_count: u64,
}

/// 合算の重み。同じキーの回数に掛けてから足す。
#[derive(Clone, Copy)]
pub struct Weights {
    /// Wikipedia の出現回数に掛ける重み。
    pub counts: f64,
    /// 技術文書の文書数に掛ける重み。
    pub docs: f64,
    /// 日本語コーパスの文書数に掛ける重み。
    pub corpus: f64,
}

/// 1 つのキーを、3 つの入力がそれぞれ数えた回数。
#[derive(Clone, Copy, Default)]
struct Counted {
    /// Wikipedia の出現回数。
    wikipedia: u64,
    /// 技術文書の文書数。
    documents: u64,
    /// 日本語コーパスの文書数。
    corpus: u64,
}

/// 成果物の来歴。
#[derive(Serialize)]
struct Manifest {
    /// 組んだ成果物の種類。
    artifact: Artifact,
    /// 成果物の版の名前。`dashi-<年>.<月>.<連番>` の形であり、作り直すたびに
    /// 上げる。
    version: String,
    /// 回数の入力と合算の重み。
    inputs: InputsSection,
    /// 入力にしたダンプ。
    dump: DumpSection,
    /// 記事の絞り込み。
    selection: SelectionSection,
    /// 統計を作った akunuki の版。
    akunuki: AkunukiSection,
    /// 見出しを採った解析辞書。
    dictionary: DictionarySection,
    /// 見出しを採った記事名。記事名を登録しなかった場合は `null` である。
    titles: Option<TitlesSection>,
    /// フィルタの作り方の選択。
    variant: VariantSection,
    /// 分野のフィルタ。分野の回数が無い入力で組んだ場合は `null` である。
    domain: Option<DomainSection>,
    /// フィルタに登録したキーの数。回数から作ったキーと見出しのキーの和である。
    compound_keys: u32,
    /// 成果物のバイト数。
    artifact_bytes: ArtifactBytesSection,
    /// バケットの境界。
    buckets: Vec<BucketRange>,
}

/// フィルタの作り方の選択。既定はどれも `count` と `dictionary-headwords` と
/// `build` の引数の既定であり、この節を読めば成果物を組み直せる。
#[derive(Serialize)]
struct VariantSection {
    /// 未知語 1 形態素を複合語のキーとして数えたか。
    unknown_morphemes: bool,
    /// 解析辞書の固有名詞の見出しを登録したか。
    proper_noun_headwords: bool,
    /// フィルタに入れた複合語の回数の下限。
    min_count: u64,
    /// 部品の頻度表に入れた頻度の下限。
    min_component_count: u64,
}

/// 分野のフィルタ。番号と名前の対応、登録の条件、大きさを持つ。登録の条件の
/// 項目は、件数で登録したか割合で登録したかで分かれる。
#[derive(Serialize)]
struct DomainSection {
    /// 分野の番号と名前の対応。番号は `domain_filter.bin` のキーが持つ値である。
    domains: Vec<DomainName>,
    /// 件数で登録した場合の、語がその分野で現れる記事か文書の数の下限。
    #[serde(skip_serializing_if = "Option::is_none")]
    min_count: Option<u64>,
    /// 件数で登録した場合の、一般に畳む分野の数の下限。
    #[serde(skip_serializing_if = "Option::is_none")]
    general_threshold: Option<usize>,
    /// 割合で登録した場合の、語ごとの総数に対する分野の割合の下限。
    #[serde(skip_serializing_if = "Option::is_none")]
    share_threshold: Option<f64>,
    /// 割合で登録した場合に、割合によらず一般で登録した語の総数の下限。
    #[serde(skip_serializing_if = "Option::is_none")]
    general_min_total: Option<u64>,
    /// 登録したキーの数。
    keys: u32,
    /// フィルタのバイト数と SHA-256。
    domain_filter_bin: FileDigest,
    /// 分野ごとに登録した語の数。
    registered: Vec<DomainKeys>,
}

/// 分野の番号と名前。
#[derive(Serialize)]
struct DomainName {
    /// フィルタのキーが持つ番号。
    code: u8,
    /// 分野の名前。
    name: String,
}

/// 回数の入力と合算の重み。
#[derive(Serialize)]
struct InputsSection {
    /// Wikipedia の回数。
    wikipedia: CountsInput,
    /// 技術文書の文書数。合算しなかった場合は `null` である。
    documents: Option<DocumentsInput>,
    /// 日本語コーパスの文書数。合算しなかった場合は `null` である。
    corpus: Option<CorpusInput>,
}

/// Wikipedia の回数の入力。
#[derive(Serialize)]
struct CountsInput {
    /// 回数の TSV を読んだディレクトリ。
    counts_dir: String,
    /// 回数に掛けた重み。
    weight: f64,
}

/// 技術文書の文書数の入力。
#[derive(Serialize)]
struct DocumentsInput {
    /// 文書数の TSV を読んだディレクトリ。
    counts_dir: String,
    /// 文書数に掛けた重み。
    weight: f64,
    /// 平文を読んだディレクトリ。
    text_dir: String,
    /// 数えた文書の数。
    documents: u64,
    /// 数えた平文のバイト数。
    text_bytes: u64,
    /// 数えなかったソースの名前。
    excluded_sources: Vec<String>,
    /// ソースごとの内訳。
    sources: Vec<SourceStats>,
}

/// 日本語コーパスの文書数の入力。コーパスのディレクトリは書かない。成果物を組み
/// 直す側が `count` に引数で渡すためである。
#[derive(Serialize)]
struct CorpusInput {
    /// 文書数の TSV を読んだディレクトリ。
    counts_dir: String,
    /// 文書数に掛けた重み。
    weight: f64,
    /// 数えた文書の数。
    documents: u64,
    /// 数えた平文のバイト数。
    text_bytes: u64,
    /// 数えなかったソースの名前。
    excluded_sources: Vec<String>,
    /// 文語体として外した法令の数。
    excluded_laws: u64,
    /// 法令 ID の形を読めず、外さずに数えた法令の数。
    unreadable_law_ids: u64,
    /// ソースごとの内訳。
    sources: Vec<CorpusSourceStats>,
}

/// 入力にしたダンプ。
#[derive(Serialize)]
struct DumpSection {
    /// ダンプのファイル名。
    file_name: String,
    /// ダンプの版。
    version: Option<String>,
}

/// 記事の絞り込み。
#[derive(Serialize)]
struct SelectionSection {
    /// 記事を採った条件。
    condition: String,
    /// 読んだ記事の数。
    scanned_articles: u64,
    /// 条件に当たった記事の数。
    selected_articles: u64,
    /// 条件に当たった記事の本文のバイト数。
    text_bytes: u64,
    /// 解析に失敗して飛ばした記事の数。
    skipped_articles: u64,
    /// 分野ごとに数えた記事の数。分野の回数を数えなかった場合は空である。
    domain_articles: Vec<DomainTally>,
}

/// 統計を作った akunuki の版。
#[derive(Serialize)]
struct AkunukiSection {
    /// 依存に固定した commit。
    rev: String,
    /// 複合語を割った解析辞書の版。
    dictionary_version: String,
}

/// 見出しを採った解析辞書。full は small と core と notcore の和であり、akunuki が
/// 埋め込む core は small と core の和である。
#[derive(Serialize)]
struct DictionarySection {
    /// 解析辞書の版。
    version: String,
    /// 見出しを読んだ配布物の名前。
    lexicon_files: Vec<String>,
    /// 読んだ名詞の見出しの数。
    read_headwords: u64,
    /// バケット 7 で登録した見出しの数。
    registered_headwords: u64,
    /// 登録した見出しのうち、普通名詞の数。
    registered_common_nouns: u64,
    /// 登録した見出しのうち、固有名詞の数。
    registered_proper_nouns: u64,
    /// 登録した見出しのうち、普通名詞でも固有名詞でもない名詞の数。
    registered_other_nouns: u64,
}

/// 見出しを採った記事名。
#[derive(Serialize)]
struct TitlesSection {
    /// 記事名の TSV を読んだディレクトリ。
    titles_dir: String,
    /// 名前を読んだ記事の数。
    scanned_articles: u64,
    /// 読んだ記事名の数。
    read_titles: u64,
    /// 読んだリダイレクト名の数。
    read_redirect_titles: u64,
    /// バケット 7 で登録した語の数。
    registered_titles: u64,
}

/// 成果物のバイト数。
#[derive(Serialize)]
struct ArtifactBytesSection {
    /// フィルタのバイト数と SHA-256。
    freq_filter_bin: FileDigest,
    /// 部品の頻度表のバイト数と SHA-256。
    component_freq_fst: FileDigest,
}

/// 1 つの成果物ファイルのバイト数と検証用のハッシュ。3 つの成果物ファイルは
/// 追跡しないので、取得した配布物がこの値と一致するかで壊れた転送やすり替えを
/// 検出できる。
#[derive(Serialize)]
struct FileDigest {
    /// ファイルのバイト数。
    bytes: usize,
    /// ファイルの SHA-256(16 進)。
    sha256: String,
}

/// バケット 1 つが受け持つ回数の範囲。`max_count` が `null` のバケットに上限は
/// ない。
#[derive(Serialize)]
struct BucketRange {
    /// バケットの値。
    bucket: u8,
    /// 回数の下限。
    min_count: u64,
    /// 回数の上限。
    max_count: Option<u64>,
}

/// `inputs` の回数と見出しから、`out_dir` に成果物を書く。
///
/// # Errors
///
/// 重みが 0 以上の有限の数でない場合、回数か見出しを読めない場合、フィルタか
/// 頻度表を組めない場合、書き出しが失敗する場合に返す。
pub fn run(inputs: &Inputs, out_dir: &Path) -> Result<()> {
    check_weights(inputs.weights)?;
    check_min_count(inputs.pruning.min_count)?;
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;

    let loaded = load_stats(inputs)?;
    let version = inputs.version.clone();
    let written = write_artifacts(inputs, out_dir)?;
    let domain = write_domain_filter(inputs, out_dir)?;
    let manifest = build_manifest(inputs, loaded, version, &written, domain);

    let manifest_path = out_dir.join(inputs.artifact.manifest_file());
    let json = serde_json::to_string_pretty(&manifest).context("来歴を JSON にできない")?;
    std::fs::write(&manifest_path, json + "\n")
        .with_context(|| format!("{} を書けない", manifest_path.display()))?;
    report(&written, manifest.domain.as_ref());
    Ok(())
}

/// `build` が読む回数と見出しの記録一式。
struct LoadedStats {
    /// Wikipedia の回数の記録。
    stats: CountStats,
    /// 解析辞書の見出しの記録。
    headword_stats: HeadwordStats,
    /// 技術文書の文書数の記録。合算しなかった場合は `None` である。
    document_stats: Option<DocumentStats>,
    /// 日本語コーパスの文書数の記録。合算しなかった場合は `None` である。
    corpus_stats: Option<CorpusStats>,
    /// 記事名の記録。登録しなかった場合は `None` である。
    title_stats: Option<TitleStats>,
    /// すべての回数が揃えた、未知語 1 形態素を数える設定。
    unknown_morphemes: UnknownMorphemes,
}

/// `inputs` が指すディレクトリから、回数と見出しの記録一式を読む。3 つの入力の
/// 未知語 1 形態素の設定が食い違えば断る。
fn load_stats(inputs: &Inputs) -> Result<LoadedStats> {
    let counts_dir = inputs.counts_dir.as_path();
    let stats: CountStats = read_stats(&counts_dir.join(COUNT_STATS_FILE))?;
    let headword_stats: HeadwordStats = read_stats(&counts_dir.join(HEADWORD_STATS_FILE))?;
    let document_stats: Option<DocumentStats> = inputs
        .docs_counts_dir
        .as_deref()
        .map(|dir| read_stats(&dir.join(DOCUMENT_STATS_FILE)))
        .transpose()?;
    let corpus_stats: Option<CorpusStats> = inputs
        .corpus_counts_dir
        .as_deref()
        .map(|dir| read_stats(&dir.join(CORPUS_STATS_FILE)))
        .transpose()?;
    let title_stats: Option<TitleStats> = inputs
        .titles_dir
        .as_deref()
        .map(|dir| read_stats(&dir.join(TITLE_STATS_FILE)))
        .transpose()?;

    let unknown_morphemes = check_unknown_morphemes(
        UnknownMorphemes::from_counted(stats.unknown_morphemes),
        &[
            (
                "技術文書",
                document_stats
                    .as_ref()
                    .map(|stats| UnknownMorphemes::from_counted(stats.unknown_morphemes)),
            ),
            (
                "日本語コーパス",
                corpus_stats
                    .as_ref()
                    .map(|stats| UnknownMorphemes::from_counted(stats.unknown_morphemes)),
            ),
        ],
    )?;

    Ok(LoadedStats {
        stats,
        headword_stats,
        document_stats,
        corpus_stats,
        title_stats,
        unknown_morphemes,
    })
}

/// 読んだ記録と組んだ成果物から、manifest を組み立てる。
fn build_manifest(
    inputs: &Inputs,
    loaded: LoadedStats,
    version: String,
    written: &Written,
    domain: Option<DomainSection>,
) -> Manifest {
    let LoadedStats {
        stats,
        headword_stats,
        document_stats,
        corpus_stats,
        title_stats,
        unknown_morphemes,
    } = loaded;
    Manifest {
        artifact: inputs.artifact,
        version,
        inputs: inputs_section(inputs, document_stats, corpus_stats),
        dump: DumpSection {
            file_name: stats.dump_file_name,
            version: stats.dump_version,
        },
        selection: SelectionSection {
            condition: stats.selection_condition,
            scanned_articles: stats.scanned_articles,
            selected_articles: stats.selected_articles,
            text_bytes: stats.text_bytes,
            skipped_articles: stats.skipped_articles,
            domain_articles: stats.domain_articles,
        },
        akunuki: AkunukiSection {
            rev: env!("CORPUS_TOOL_AKUNUKI_REV").to_owned(),
            dictionary_version: DICTIONARY_VERSION.to_owned(),
        },
        dictionary: DictionarySection {
            version: headword_stats.dictionary_version,
            lexicon_files: headword_stats.lexicon_files,
            read_headwords: headword_stats.read_headwords,
            registered_headwords: headword_stats.registered_headwords,
            registered_common_nouns: headword_stats.registered_common_nouns,
            registered_proper_nouns: headword_stats.registered_proper_nouns,
            registered_other_nouns: headword_stats.registered_other_nouns,
        },
        titles: inputs
            .titles_dir
            .as_deref()
            .zip(title_stats)
            .map(|(dir, stats)| TitlesSection {
                titles_dir: dir.display().to_string(),
                scanned_articles: stats.scanned_articles,
                read_titles: stats.read_titles,
                read_redirect_titles: stats.read_redirect_titles,
                registered_titles: stats.registered_titles,
            }),
        variant: VariantSection {
            unknown_morphemes: unknown_morphemes.counted(),
            proper_noun_headwords: headword_stats.include_proper_nouns,
            min_count: inputs.pruning.min_count,
            min_component_count: inputs.pruning.min_component_count,
        },
        domain,
        compound_keys: written.compound_keys,
        artifact_bytes: ArtifactBytesSection {
            freq_filter_bin: FileDigest {
                bytes: written.filter_bytes,
                sha256: written.filter_sha256.clone(),
            },
            component_freq_fst: FileDigest {
                bytes: written.constituent_bytes,
                sha256: written.constituent_sha256.clone(),
            },
        },
        buckets: bucket_ranges(),
    }
}

/// 組んだ成果物の大きさを書き出す。
fn report(written: &Written, domain: Option<&DomainSection>) {
    println!(
        "キー {} 件のフィルタが {} バイト、部品 {} 件の頻度表が {} バイトである",
        written.compound_keys, written.filter_bytes, written.components, written.constituent_bytes
    );
    let Some(domain) = domain else { return };
    println!(
        "語と分野の組 {} 件の {DOMAIN_FILTER_FILE} が {} バイトである",
        domain.keys, domain.domain_filter_bin.bytes
    );
    for registered in &domain.registered {
        println!("  {} は {} 語である", registered.domain, registered.keys);
    }
}

/// 分野の回数がある入力から、分野のフィルタを組んで書く。Wikipedia の回数に
/// 分野の回数が無ければ、組まずに `None` を返す。
///
/// 足す入力の側にだけ分野の回数が無い場合は断る。その入力が持つ語は分野の中の
/// 回数に入らず、登録の下限に届かないまま落ちるためである。
fn write_domain_filter(inputs: &Inputs, out_dir: &Path) -> Result<Option<DomainSection>> {
    if !inputs.counts_dir.join(DOMAIN_COUNTS_FILE).is_file() {
        return Ok(None);
    }
    let mut dirs = vec![inputs.counts_dir.clone()];
    for (option, dir) in [
        ("--docs-counts-dir", inputs.docs_counts_dir.as_ref()),
        ("--corpus-counts-dir", inputs.corpus_counts_dir.as_ref()),
    ] {
        let Some(dir) = dir else { continue };
        if !dir.join(DOMAIN_COUNTS_FILE).is_file() {
            bail!(
                "{option} の {} に {DOMAIN_COUNTS_FILE} が無い。`count --domains` で数え直す",
                dir.display()
            );
        }
        dirs.push(dir.clone());
    }
    let written = domain_filter::write(&dirs, inputs.domain, DICTIONARY_VERSION, out_dir)?;
    let (min_count, general_threshold, share_threshold, general_min_total) = match inputs.domain {
        Registration::Count {
            min_count,
            general_threshold,
        } => (Some(min_count), Some(general_threshold), None, None),
        Registration::Share {
            threshold,
            general_min_total,
        } => (None, None, Some(threshold), general_min_total),
    };
    Ok(Some(DomainSection {
        domains: Domain::ALL
            .into_iter()
            .map(|domain| DomainName {
                code: domain.code(),
                name: domain.label().to_owned(),
            })
            .collect(),
        min_count,
        general_threshold,
        share_threshold,
        general_min_total,
        keys: written.keys,
        domain_filter_bin: FileDigest {
            bytes: written.bytes,
            sha256: written.sha256,
        },
        registered: written.registered,
    }))
}

/// 組んで書き出した成果物の大きさ。manifest に写す。
struct Written {
    /// フィルタに登録したキーの数。
    compound_keys: u32,
    /// フィルタのバイト数。
    filter_bytes: usize,
    /// フィルタの SHA-256(16 進)。
    filter_sha256: String,
    /// 部品の頻度表に入れた部品の数。
    components: usize,
    /// 部品の頻度表のバイト数。
    constituent_bytes: usize,
    /// 部品の頻度表の SHA-256(16 進)。
    constituent_sha256: String,
}

/// フィルタと部品の頻度表を組み、`out_dir` へ書く。
fn write_artifacts(inputs: &Inputs, out_dir: &Path) -> Result<Written> {
    let entries = bucket_table(inputs)?;
    let filter =
        FrequencyFilter::build(&entries, DICTIONARY_VERSION).context("フィルタを組めない")?;
    let filter_bytes = filter.to_bytes();
    let filter_sha256 = crate::checksum::sha256_hex(&filter_bytes);
    let filter_path = out_dir.join(FILTER_FILE);
    std::fs::write(&filter_path, &filter_bytes)
        .with_context(|| format!("{} を書けない", filter_path.display()))?;

    let components = component_frequencies(inputs)?;
    let constituent_bytes =
        ConstituentFrequencies::build(&components).context("部品の頻度表を組めない")?;
    let constituent_sha256 = crate::checksum::sha256_hex(&constituent_bytes);
    let constituent_path = out_dir.join(CONSTITUENT_FILE);
    std::fs::write(&constituent_path, &constituent_bytes)
        .with_context(|| format!("{} を書けない", constituent_path.display()))?;

    Ok(Written {
        compound_keys: filter.key_count(),
        filter_bytes: filter_bytes.len(),
        filter_sha256,
        components: components.len(),
        constituent_bytes: constituent_bytes.len(),
        constituent_sha256,
    })
}

/// 回数の入力と合算の重みの節。読んだ記録から、入力ごとの文書数とソースの一覧を
/// 写す。
fn inputs_section(
    inputs: &Inputs,
    document_stats: Option<DocumentStats>,
    corpus_stats: Option<CorpusStats>,
) -> InputsSection {
    let documents = inputs
        .docs_counts_dir
        .as_deref()
        .zip(document_stats)
        .map(|(dir, stats)| DocumentsInput {
            counts_dir: dir.display().to_string(),
            weight: inputs.weights.docs,
            text_dir: stats.text_dir,
            documents: stats.documents,
            text_bytes: stats.text_bytes,
            excluded_sources: stats.excluded_sources,
            sources: stats.sources,
        });
    let corpus = inputs
        .corpus_counts_dir
        .as_deref()
        .zip(corpus_stats)
        .map(|(dir, stats)| CorpusInput {
            counts_dir: dir.display().to_string(),
            weight: inputs.weights.corpus,
            documents: stats.documents,
            text_bytes: stats.text_bytes,
            excluded_sources: stats.excluded_sources,
            excluded_laws: stats.excluded_laws,
            unreadable_law_ids: stats.unreadable_law_ids,
            sources: stats.sources,
        });
    InputsSection {
        wikipedia: CountsInput {
            counts_dir: inputs.counts_dir.display().to_string(),
            weight: inputs.weights.counts,
        },
        documents,
        corpus,
    }
}

/// `path` の記録を読む。
fn read_stats<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let json =
        std::fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    serde_json::from_str(&json).with_context(|| format!("{} が JSON でない", path.display()))
}

/// 重みが合算に使える値か検査する。負の重みはバケットの順序を反転させ、有限でない
/// 重みはすべてのキーを最大のバケットへ送る。
fn check_weights(weights: Weights) -> Result<()> {
    for (name, weight) in [
        ("--counts-weight", weights.counts),
        ("--docs-weight", weights.docs),
        ("--corpus-weight", weights.corpus),
    ] {
        if !weight.is_finite() || weight < 0.0 {
            bail!("{name} の {weight} は 0 以上の有限の数でない");
        }
    }
    Ok(())
}

/// 剪定の下限が、バケットの下端より下でないか検査する。バケット 1 の下限より
/// 小さい値を渡してもキーは増えないので、効かない設定を黙って受けない。
fn check_min_count(min_count: u64) -> Result<()> {
    let lowest = BUCKET_MINIMUMS[0].1;
    if min_count < lowest {
        bail!(
            "--min-count の {min_count} は、バケット {} の下限 {lowest} より小さい",
            BUCKET_MINIMUMS[0].0
        );
    }
    Ok(())
}

/// すべての回数を、同じ未知語 1 形態素の設定で数えたか検査する。設定が食い違う
/// 回数を足すと、一部の入力にだけ未知語のキーがある成果物ができる。返すのは
/// その設定である。
fn check_unknown_morphemes(
    counts: UnknownMorphemes,
    others: &[(&str, Option<UnknownMorphemes>)],
) -> Result<UnknownMorphemes> {
    for (name, other) in others {
        if let Some(other) = *other
            && other != counts
        {
            bail!(
                "Wikipedia の回数は未知語 1 形態素を{}、{name}の回数は{}。同じ設定で数え直す",
                wording(counts),
                wording(other)
            );
        }
    }
    Ok(counts)
}

/// 未知語 1 形態素の設定を、エラーに書く文言にする。
fn wording(unknown: UnknownMorphemes) -> &'static str {
    match unknown {
        UnknownMorphemes::Count => "数えている",
        UnknownMorphemes::Skip => "数えていない",
    }
}

/// 重みを掛けた回数の和。四捨五入して整数の回数に戻す。
///
/// 回数も重みも 0 以上であり、和は f64 が正確に表せる整数の範囲に収まる。
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
fn weighted_count(counted: Counted, weights: Weights) -> u64 {
    (counted.wikipedia as f64 * weights.counts
        + counted.documents as f64 * weights.docs
        + counted.corpus as f64 * weights.corpus)
        .round() as u64
}

/// `dir` の `file` の回数を読む。その入力を合算しない場合は空の表を返す。
fn optional_counts(dir: Option<&Path>, file: &str) -> Result<HashMap<String, u64>> {
    let Some(dir) = dir else {
        return Ok(HashMap::new());
    };
    Ok(tsv::read(&dir.join(file))?.into_iter().collect())
}

/// フィルタに登録するキーとバケットの表。3 つの入力の回数を重み付きで足してから
/// バケットに丸め、そこへ見出しのバケットを重ねる。同じキーには大きい方のバケットを
/// 残す。両方を登録しても照合は最大のバケットを返すが、1 つにまとめてキーの数を
/// 増やさない。
fn bucket_table(inputs: &Inputs) -> Result<Vec<(String, Bucket)>> {
    let counts_dir = inputs.counts_dir.as_path();
    let mut documents = optional_counts(inputs.docs_counts_dir.as_deref(), COMPOUND_COUNTS_FILE)?;
    let mut corpus = optional_counts(inputs.corpus_counts_dir.as_deref(), COMPOUND_COUNTS_FILE)?;
    let mut table: BTreeMap<String, Bucket> = BTreeMap::new();
    let insert = |table: &mut BTreeMap<String, Bucket>, key: String, counted: Counted| {
        if let Some(bucket) = bucket_of(
            weighted_count(counted, inputs.weights),
            inputs.pruning.min_count,
        ) {
            table.insert(key, bucket);
        }
    };
    for (key, count) in tsv::read(&counts_dir.join(COMPOUND_COUNTS_FILE))? {
        let counted = Counted {
            wikipedia: count,
            documents: documents.remove(&key).unwrap_or(0),
            corpus: corpus.remove(&key).unwrap_or(0),
        };
        insert(&mut table, key, counted);
    }
    // Wikipedia に無く、技術文書にあるキー。
    for (key, count) in documents {
        let counted = Counted {
            documents: count,
            corpus: corpus.remove(&key).unwrap_or(0),
            ..Counted::default()
        };
        insert(&mut table, key, counted);
    }
    // Wikipedia にも技術文書にも無く、日本語コーパスにだけあるキー。
    for (key, count) in corpus {
        let counted = Counted {
            corpus: count,
            ..Counted::default()
        };
        insert(&mut table, key, counted);
    }
    for path in headword_files(inputs) {
        for (key, value) in tsv::read(&path)? {
            let bucket = u8::try_from(value)
                .ok()
                .and_then(Bucket::new)
                .with_context(|| {
                    format!(
                        "{} のバケット {value} は 1 から 7 の外である",
                        path.display()
                    )
                })?;
            table
                .entry(key)
                .and_modify(|current| *current = (*current).max(bucket))
                .or_insert(bucket);
        }
    }
    Ok(table.into_iter().collect())
}

/// バケット 7 の見出しを読む TSV。解析辞書の見出しは必ず読み、記事名は
/// `--titles-dir` を渡した場合だけ読む。
fn headword_files(inputs: &Inputs) -> Vec<PathBuf> {
    let mut paths = vec![inputs.counts_dir.join(HEADWORDS_FILE)];
    paths.extend(inputs.titles_dir.as_ref().map(|dir| dir.join(TITLES_FILE)));
    paths
}

/// 部品の頻度表に入れる、部品ごとの重み付きの頻度。
fn component_frequencies(inputs: &Inputs) -> Result<Vec<(String, u64)>> {
    let mut documents = optional_counts(inputs.docs_counts_dir.as_deref(), COMPONENT_COUNTS_FILE)?;
    let mut corpus = optional_counts(inputs.corpus_counts_dir.as_deref(), COMPONENT_COUNTS_FILE)?;
    let mut rows: Vec<(String, u64)> = tsv::read(&inputs.counts_dir.join(COMPONENT_COUNTS_FILE))?
        .into_iter()
        .map(|(key, count)| {
            let counted = Counted {
                wikipedia: count,
                documents: documents.remove(&key).unwrap_or(0),
                corpus: corpus.remove(&key).unwrap_or(0),
            };
            (key, weighted_count(counted, inputs.weights))
        })
        .collect();
    // Wikipedia に無く、技術文書にある部品。
    rows.extend(documents.into_iter().map(|(key, count)| {
        let counted = Counted {
            documents: count,
            corpus: corpus.remove(&key).unwrap_or(0),
            ..Counted::default()
        };
        (key, weighted_count(counted, inputs.weights))
    }));
    // Wikipedia にも技術文書にも無く、日本語コーパスにだけある部品。
    rows.extend(corpus.into_iter().map(|(key, count)| {
        let counted = Counted {
            corpus: count,
            ..Counted::default()
        };
        (key, weighted_count(counted, inputs.weights))
    }));
    rows.retain(|(_, frequency)| *frequency >= inputs.pruning.min_component_count);
    Ok(rows)
}

/// `count` 回現れた複合語のバケット。`min_count` に満たなければ `None` を返す。
fn bucket_of(count: u64, min_count: u64) -> Option<Bucket> {
    if count < min_count {
        return None;
    }
    BUCKET_MINIMUMS
        .iter()
        .rev()
        .find(|(_, minimum)| count >= *minimum)
        .and_then(|(bucket, _)| Bucket::new(*bucket))
}

/// バケットの境界。manifest に書く。
fn bucket_ranges() -> Vec<BucketRange> {
    BUCKET_MINIMUMS
        .iter()
        .enumerate()
        .map(|(index, (bucket, minimum))| BucketRange {
            bucket: *bucket,
            min_count: *minimum,
            max_count: BUCKET_MINIMUMS
                .get(index + 1)
                .map(|(_, next)| next.saturating_sub(1)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 既定の重み。
    const ONES: Weights = Weights {
        counts: 1.0,
        docs: 1.0,
        corpus: 1.0,
    };

    /// 剪定で何も落とさない下限。バケット 1 の下端と、頻度 1 の部品まで残す。
    const KEEP_ALL: Pruning = Pruning {
        min_count: 3,
        min_component_count: 1,
    };

    /// 分野のフィルタの登録の条件。分野の回数を持たない入力では効かない。
    const DOMAIN: Registration = Registration::Count {
        min_count: 5,
        general_threshold: 3,
    };

    /// `dir` に回数の TSV を書く。
    fn write_counts(dir: &Path, file: &str, rows: &[(&str, u32)]) {
        std::fs::create_dir_all(dir).unwrap();
        let table: std::collections::HashMap<Box<str>, u32> = rows
            .iter()
            .map(|(key, count)| ((*key).into(), *count))
            .collect();
        tsv::write(&dir.join(file), &table).unwrap();
    }

    /// 3 つの入力が数えた回数。
    fn counted(wikipedia: u64, documents: u64, corpus: u64) -> Counted {
        Counted {
            wikipedia,
            documents,
            corpus,
        }
    }

    #[test]
    fn 重みを掛けた回数を足す() {
        // 既定の重みでは単純な和である。
        assert_eq!(weighted_count(counted(2, 1, 4), ONES), 7);
        // 重みを掛けた和は四捨五入して回数に戻る。2 + 5 * 0.3 は 3.5 である。
        let light = Weights {
            counts: 1.0,
            docs: 0.3,
            corpus: 0.0,
        };
        assert_eq!(weighted_count(counted(2, 5, 100), light), 4);
        // 重み 0 の入力は和に効かない。
        let only_documents = Weights {
            counts: 0.0,
            docs: 1.0,
            corpus: 0.0,
        };
        assert_eq!(weighted_count(counted(1_000, 4, 20), only_documents), 4);
    }

    #[test]
    fn 合算に使えない重みを断る() {
        assert!(check_weights(ONES).is_ok());
        // 負の重みはバケットの順序を反転させる。
        assert!(
            check_weights(Weights {
                counts: 1.0,
                docs: -0.1,
                corpus: 1.0,
            })
            .is_err()
        );
        // 有限でない重みはすべてのキーを最大のバケットへ送る。
        assert!(
            check_weights(Weights {
                counts: f64::INFINITY,
                docs: 1.0,
                corpus: 1.0,
            })
            .is_err()
        );
    }

    #[test]
    fn 三つの入力の回数を足してからバケットに丸める() {
        let dir = std::env::temp_dir().join("corpus-tool-bucket-table-test");
        let counts_dir = dir.join("counts");
        let docs_dir = dir.join("docs");
        let corpus_dir = dir.join("corpus");
        write_counts(
            &counts_dir,
            COMPOUND_COUNTS_FILE,
            &[("公開鍵", 2), ("暗号方式", 100)],
        );
        write_counts(&counts_dir, HEADWORDS_FILE, &[("電子署名", 7)]);
        let titles_dir = dir.join("titles");
        write_counts(&titles_dir, TITLES_FILE, &[("検証手順", 7)]);
        write_counts(
            &docs_dir,
            COMPOUND_COUNTS_FILE,
            &[("公開鍵", 1), ("設定項目", 3)],
        );
        write_counts(
            &corpus_dir,
            COMPOUND_COUNTS_FILE,
            &[("設定項目", 2), ("完了条件", 5)],
        );
        let inputs = Inputs {
            counts_dir,
            docs_counts_dir: Some(docs_dir),
            corpus_counts_dir: Some(corpus_dir),
            titles_dir: Some(titles_dir),
            weights: ONES,
            pruning: KEEP_ALL,
            domain: DOMAIN,
            artifact: Artifact::Release,
            version: "test-version".to_owned(),
        };

        let table: BTreeMap<String, u8> = bucket_table(&inputs)
            .unwrap()
            .into_iter()
            .map(|(key, bucket)| (key, bucket.get()))
            .collect();
        // 片方だけでは 2 回で捨てるキーが、足すと 3 回になってバケット 1 を持つ。
        assert_eq!(table.get("公開鍵").copied(), Some(1));
        // Wikipedia にだけあるキーは、そのまま丸まる。
        assert_eq!(table.get("暗号方式").copied(), Some(3));
        // 技術文書と日本語コーパスの回数も足す。3 + 2 は 5 回である。
        assert_eq!(table.get("設定項目").copied(), Some(1));
        // 日本語コーパスにだけあるキーも、そのまま丸まる。
        assert_eq!(table.get("完了条件").copied(), Some(1));
        // 見出しのバケットは回数のバケットと同じ表に載る。
        assert_eq!(table.get("電子署名").copied(), Some(7));
        // 記事名の見出しも、解析辞書の見出しと同じ表に載る。
        assert_eq!(table.get("検証手順").copied(), Some(7));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 回数をバケットに丸める() {
        // 2 回以下は捨てる。
        assert_eq!(bucket_of(0, 3), None);
        assert_eq!(bucket_of(2, 3), None);
        // 境目の回数が下のバケットに落ちない。
        assert_eq!(bucket_of(3, 3).map(Bucket::get), Some(1));
        assert_eq!(bucket_of(9, 3).map(Bucket::get), Some(1));
        assert_eq!(bucket_of(10, 3).map(Bucket::get), Some(2));
        assert_eq!(bucket_of(99, 3).map(Bucket::get), Some(2));
        assert_eq!(bucket_of(100, 3).map(Bucket::get), Some(3));
        assert_eq!(bucket_of(1_000, 3).map(Bucket::get), Some(4));
        assert_eq!(bucket_of(10_000, 3).map(Bucket::get), Some(5));
        assert_eq!(bucket_of(1_000_000, 3).map(Bucket::get), Some(5));
    }

    #[test]
    fn 剪定の下限に満たないキーを捨てる() {
        // 下限 3 では残る回数が、下限 5 では落ちる。
        assert_eq!(bucket_of(3, 3).map(Bucket::get), Some(1));
        assert_eq!(bucket_of(3, 5), None);
        // 下限まで届いた回数は、これまでと同じバケットに丸まる。
        assert_eq!(bucket_of(5, 5).map(Bucket::get), Some(1));
        // 下限 10 ではバケット 1 の回数がすべて落ちる。
        assert_eq!(bucket_of(9, 10), None);
        assert_eq!(bucket_of(10, 10).map(Bucket::get), Some(2));
    }

    #[test]
    fn 分野の回数が揃わない入力を断る() {
        let dir = std::env::temp_dir().join("corpus-tool-domain-inputs-test");
        let counts_dir = dir.join("counts");
        let docs_dir = dir.join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        write_counts(&counts_dir, DOMAIN_COUNTS_FILE, &[("公開鍵暗号\t1", 5)]);
        let mut inputs = Inputs {
            counts_dir,
            docs_counts_dir: Some(docs_dir),
            corpus_counts_dir: None,
            titles_dir: None,
            weights: ONES,
            pruning: KEEP_ALL,
            domain: DOMAIN,
            artifact: Artifact::Release,
            version: "test-version".to_owned(),
        };
        // 足す入力の側に分野の回数が無ければ、その入力の語が下限に届かないまま
        // 落ちるので断る。
        assert!(write_domain_filter(&inputs, &dir).is_err());
        // Wikipedia の側に分野の回数が無ければ、分野のフィルタを組まない。
        inputs.counts_dir = dir.join("counts-without-domains");
        inputs.docs_counts_dir = None;
        write_counts(&inputs.counts_dir, COMPOUND_COUNTS_FILE, &[("公開鍵", 5)]);
        assert!(write_domain_filter(&inputs, &dir).unwrap().is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 効かない下限を断る() {
        assert!(check_min_count(3).is_ok());
        // バケット 1 の下限より小さい値は、渡してもキーを増やさない。
        assert!(check_min_count(2).is_err());
    }

    #[test]
    fn 部品の頻度表の下限に満たない部品を捨てる() {
        let dir = std::env::temp_dir().join("corpus-tool-component-pruning-test");
        let counts_dir = dir.join("counts");
        write_counts(
            &counts_dir,
            COMPONENT_COUNTS_FILE,
            &[("公開", 2), ("暗号", 3)],
        );
        let mut inputs = Inputs {
            counts_dir,
            docs_counts_dir: None,
            corpus_counts_dir: None,
            titles_dir: None,
            weights: ONES,
            pruning: KEEP_ALL,
            domain: DOMAIN,
            artifact: Artifact::Release,
            version: "test-version".to_owned(),
        };
        let all: Vec<(String, u64)> = component_frequencies(&inputs).unwrap();
        assert_eq!(all.len(), 2);
        // 下限を 3 にすると、2 回の部品が落ちる。
        inputs.pruning.min_component_count = 3;
        assert_eq!(
            component_frequencies(&inputs).unwrap(),
            vec![("暗号".to_owned(), 3_u64)]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 未知語の設定が食い違う回数を断る() {
        let count = UnknownMorphemes::Count;
        let skip = UnknownMorphemes::Skip;
        // 足す入力が無い場合と、設定が揃う場合は通る。
        assert_eq!(
            check_unknown_morphemes(count, &[("技術文書", None)]).unwrap(),
            count
        );
        assert_eq!(
            check_unknown_morphemes(skip, &[("技術文書", Some(skip))]).unwrap(),
            skip
        );
        // 1 つでも設定の違う回数があれば足さない。
        assert!(check_unknown_morphemes(skip, &[("技術文書", Some(count))]).is_err());
        assert!(
            check_unknown_morphemes(
                skip,
                &[("技術文書", Some(skip)), ("日本語コーパス", Some(count))]
            )
            .is_err()
        );
    }

    #[test]
    fn バケットの境界は回数の範囲を隙間なく覆う() {
        let ranges = bucket_ranges();
        assert_eq!(ranges.len(), 5);
        assert_eq!(ranges[0].min_count, 3);
        assert_eq!(ranges[0].max_count, Some(9));
        // 最大のバケットに上限は無い。
        assert_eq!(ranges[4].max_count, None);
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].max_count, Some(pair[1].min_count - 1));
        }
    }
}
