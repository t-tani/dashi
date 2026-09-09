//! 平文にした技術文書のディレクトリを読み、複合語と部品を文書単位で数える。
//!
//! Wikipedia の記事は出現回数で数えるが、技術文書は同じ数え方をすると、
//! ナビゲーションと免責とライセンスの定型文が文書ごとに繰り返される分だけ、
//! 定型文の複合語が高い回数を持つ。そこで 1 文書に何度現れても 1 回とし、
//! キーが現れた文書の数を数える。
//!
//! 読むのは `flatten` が `manifest.tsv` に `text` と書いたソースだけである。
//! 平文にしない形式のソースは `-` を持ち、読まない。正解集合の採り先のソースは
//! 平文にしてあるので、評価用の成果物では `--exclude` で外す。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::compound::{Counts, UnknownMorphemes};
use crate::count::{COMPONENT_COUNTS_FILE, COMPOUND_COUNTS_FILE, DOMAIN_COUNTS_FILE};
use crate::domain::{
    self, DOMAIN_COUNT, Domain, DomainCounting, DomainCounts, DomainSet, DomainTally,
};
use crate::tsv;

/// 数えた記録を書くファイル名。`build` がこれを読んで manifest に写す。
pub const DOCUMENT_STATS_FILE: &str = "document-stats.json";

/// 平文の記録のファイル名。`flatten` が平文と同じディレクトリに書く。
const MANIFEST_FILE: &str = "manifest.tsv";

/// 平文にしたソースが、平文の記録のパスの欄に持つ値。平文にしない形式のソースは
/// `-` を持つ。
const TEXT_OUTPUT_DIR: &str = "text";

/// 平文のファイルの拡張子。`flatten` は原文の名前の末尾にこれを足す。
const TEXT_EXTENSION: &str = "txt";

/// 技術文書の分野。入力は翻訳された公式文書と技術書なので、
/// ソースによらず情報技術である。
const DOCUMENT_DOMAIN: Domain = Domain::InformationTechnology;

/// 1 度に並列で解析する文書の数。まとめて解析してから 1 つの表へ畳むので、この
/// 数を大きくすると解析の待ちが減り、途中の表が持つメモリが増える。
const BATCH_DOCUMENTS: usize = 256;

/// 技術文書を数えた記録。
#[derive(Serialize, Deserialize)]
pub struct DocumentStats {
    /// 平文を読んだディレクトリ。
    pub text_dir: String,
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた平文のバイト数。
    pub text_bytes: u64,
    /// 解析に失敗して飛ばした文書の数。この文書の本文も `text_bytes` に入る。
    pub skipped_documents: u64,
    /// `--exclude` で外したソースの名前。
    pub excluded_sources: Vec<String>,
    /// 未知語 1 形態素を複合語のキーとして数えたか。
    pub unknown_morphemes: bool,
    /// 分野ごとに数えた文書の数。分野の組を数えなかった場合は空である。
    #[serde(default)]
    pub domain_documents: Vec<DomainTally>,
    /// ソースごとの内訳。
    pub sources: Vec<SourceStats>,
}

/// 1 ソースの内訳。commit とライセンスは、`flatten` が書いた記録から写す。
#[derive(Serialize, Deserialize)]
pub struct SourceStats {
    /// ソースの名前。
    pub name: String,
    /// 取得した commit。
    pub commit: String,
    /// 原文のライセンス。
    pub license: String,
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた平文のバイト数。
    pub text_bytes: u64,
}

/// 1 文書を数えた結果。
struct Document {
    /// 文書単位に均した回数。
    counts: Counts,
    /// 平文のバイト数。
    text_bytes: u64,
    /// 解析に失敗したか。
    skipped: bool,
}

/// `text_dir` の平文を文書単位で数え、`out_dir` に回数の TSV と記録を書く。
/// `exclude` に名前があるソースは読まない。`limit` を渡すと、パスの昇順で先頭の
/// その数の文書だけを読む。
///
/// `domains` が [`DomainCounting::Count`] なら、語と分野の組の回数も書く。技術
/// 文書はどのソースも情報技術である。
///
/// # Errors
///
/// 平文の記録を読めない場合、ソースの平文が無い場合、文書を読めない場合、
/// 書き出しが失敗する場合に返す。
pub fn run(
    text_dir: &Path,
    out_dir: &Path,
    exclude: &[String],
    limit: Option<u64>,
    unknown: UnknownMorphemes,
    domains: DomainCounting,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let mut sources = read_manifest(&text_dir.join(MANIFEST_FILE))?;
    sources.retain(|source| !exclude.contains(&source.name));
    let mut paths = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let source_dir = text_dir.join(&source.name);
        if !source_dir.is_dir() {
            bail!(
                "ソース '{}' の平文が {} に無い。`flatten` を通したか確かめる",
                source.name,
                source_dir.display()
            );
        }
        for path in text_files(&source_dir)? {
            paths.push((index, path));
        }
    }
    if let Some(limit) = limit {
        paths.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }

    let source_domains: Vec<Domain> = vec![DOCUMENT_DOMAIN; sources.len()];
    let Counted {
        counts,
        domains: domain_keys,
        tallied,
        skipped,
    } = count_paths(
        &paths,
        sources.len(),
        unknown,
        domains.counts().then_some(source_domains.as_slice()),
    )?;
    for (source, tallied) in sources.iter_mut().zip(tallied) {
        source.documents = tallied.documents;
        source.text_bytes = tallied.text_bytes;
    }

    tsv::write(&out_dir.join(COMPOUND_COUNTS_FILE), &counts.compounds)?;
    tsv::write(&out_dir.join(COMPONENT_COUNTS_FILE), &counts.components)?;
    if domains.counts() {
        tsv::write(&out_dir.join(DOMAIN_COUNTS_FILE), &domain_keys.keys)?;
    }
    let stats = DocumentStats {
        text_dir: text_dir.display().to_string(),
        documents: sources.iter().map(|source| source.documents).sum(),
        text_bytes: sources.iter().map(|source| source.text_bytes).sum(),
        skipped_documents: skipped,
        excluded_sources: exclude.to_vec(),
        unknown_morphemes: unknown.counted(),
        domain_documents: domain::tallies(&domain_documents(
            &sources
                .iter()
                .map(|source| source.documents)
                .collect::<Vec<u64>>(),
            &source_domains,
            domains,
        )),
        sources,
    };
    let stats_path = out_dir.join(DOCUMENT_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} ソースの {} 文書({} バイト)を数え、{} 文書を飛ばした。複合語のキーは {} 件、部品は {} 件である",
        stats.sources.len(),
        stats.documents,
        stats.text_bytes,
        stats.skipped_documents,
        counts.compounds.len(),
        counts.components.len()
    );
    Ok(())
}

/// 文書のまとまりを数えた結果。
pub struct Counted {
    /// 複合語と部品の回数。
    pub counts: Counts,
    /// 語と分野の組の回数。分野を数えない場合は空である。
    pub domains: DomainCounts,
    /// ソースごとの内訳。
    pub tallied: Vec<Tallied>,
    /// 解析に失敗して飛ばした文書の数。
    pub skipped: u64,
}

/// `paths` の平文を文書単位で数え、回数とソースごとの内訳と、解析に失敗して飛ばした
/// 文書の数を返す。`paths` の各項は、ソースの番号と平文のディレクトリである。番号は
/// `sources` 未満でなければならない。
///
/// `source_domains` を渡すと、語と分野の組も数える。組の回数は、その分野でキーが
/// 現れた文書の数である。`source_domains` の長さは `sources` に等しいこと。
///
/// # Errors
///
/// 文書を読めない場合に返す。
pub fn count_paths(
    paths: &[(usize, PathBuf)],
    sources: usize,
    unknown: UnknownMorphemes,
    source_domains: Option<&[Domain]>,
) -> Result<Counted> {
    let mut counts = Counts::new(unknown);
    let mut domains = DomainCounts::default();
    let mut tallied: Vec<Tallied> = (0..sources).map(|_| Tallied::default()).collect();
    let mut skipped = 0;
    for batch in paths.chunks(BATCH_DOCUMENTS) {
        let documents: Vec<Document> = batch
            .par_iter()
            .map(|(_, path)| count_document(path, unknown))
            .collect::<Result<_>>()?;
        for ((index, _), document) in batch.iter().zip(documents) {
            if let Some(source_domains) = source_domains {
                domains.add_keys(
                    DomainSet::only(source_domains[*index]),
                    document.counts.compounds.keys().map(|key| &**key),
                );
            }
            counts.merge(document.counts);
            tallied[*index].documents += 1;
            tallied[*index].text_bytes += document.text_bytes;
            skipped += u64::from(document.skipped);
        }
    }
    Ok(Counted {
        counts,
        domains,
        tallied,
        skipped,
    })
}

/// 分野ごとに数えた文書の数。[`Domain::ALL`] と同じ並びである。分野を数えない
/// 場合はすべて 0 になる。
pub fn domain_documents(
    documents: &[u64],
    source_domains: &[Domain],
    domains: DomainCounting,
) -> [u64; DOMAIN_COUNT] {
    let mut tallied = [0_u64; DOMAIN_COUNT];
    if domains.counts() {
        for (documents, domain) in documents.iter().zip(source_domains) {
            tallied[usize::from(domain.code() - 1)] += documents;
        }
    }
    tallied
}

/// 1 ソースで数えた量。
#[derive(Default)]
pub struct Tallied {
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた平文のバイト数。
    pub text_bytes: u64,
}

/// 1 文書を読んで数え、回数を文書単位に均す。解析に失敗した文書は、理由を書き出して
/// そこまでの回数を返す。1 文書の失敗で全体の走行を落とさないためである。
fn count_document(path: &Path, unknown: UnknownMorphemes) -> Result<Document> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    let mut counts = Counts::new(unknown);
    let mut skipped = false;
    if let Err(error) = counts.add_text(&text) {
        eprintln!("{} を飛ばす: {error}", path.display());
        skipped = true;
    }
    counts.count_once_per_key();
    Ok(Document {
        counts,
        text_bytes: text.len() as u64,
        skipped,
    })
}

/// `dir` の下の平文を、パスの昇順で集める。
///
/// # Errors
///
/// ディレクトリを読めない場合に返す。
pub fn text_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    collect_text_files(dir, &mut found)?;
    found.sort();
    Ok(found)
}

/// `dir` を再帰で辿り、平文を `found` へ足す。
fn collect_text_files(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("{} を読めない", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("{} を読めない", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("{} の種類を読めない", path.display()))?;
        if file_type.is_dir() {
            collect_text_files(&path, found)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == TEXT_EXTENSION)
        {
            found.push(path);
        }
    }
    Ok(())
}

/// 平文の記録を読み、頻度に使うソースだけを返す。
fn read_manifest(path: &Path) -> Result<Vec<SourceStats>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    let mut sources = Vec::new();
    for (index, line) in text.lines().enumerate().skip(1) {
        let columns: Vec<&str> = line.split('\t').collect();
        let [
            name,
            _repo,
            _branch,
            commit,
            _commit_date,
            _paths,
            _format,
            license,
            output_dir,
            ..,
        ] = columns[..]
        else {
            bail!("{} の {} 行目の欄が足りない", path.display(), index + 1);
        };
        if output_dir != TEXT_OUTPUT_DIR {
            continue;
        }
        sources.push(SourceStats {
            name: name.to_owned(),
            commit: commit.to_owned(),
            license: license.to_owned(),
            documents: 0,
            text_bytes: 0,
        });
    }
    Ok(sources)
}
