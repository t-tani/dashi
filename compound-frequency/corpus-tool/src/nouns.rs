//! 名詞の出現回数を数える。外来語の置き換え表の見出し語の一覧が引く頻度である。
//!
//! 複合語の回数([`crate::count`])が分割単位 A で連なりを数えるのに対し、ここでは
//! 分割単位 C で 1 語ずつ数える。数えるのは名詞と形状詞で、数詞は数えない。キーは
//! 正規化形である。回数は 2 つ持つ。全出現数と、前後に名詞・形状詞・接辞が付かない
//! 単独の出現数である。`門` や `網` のように単独で立つ語が問題の形なので、単独の
//! 出現を分けて数える。
//!
//! 入力はダンプの記事、平文にした技術文書、日本語コーパスの 3 つで、どれも
//! [`crate::count`] と同じ読み方をする。技術文書と日本語コーパスも出現回数で
//! 数える。文書数で数える複合語の回数とは目的が違い、語の頻度の比較には出現の
//! 密度が要るためである。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use aku_morph::{MorphError, Morpheme, PartOfSpeech, analyze};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

use crate::compound::analysis_chunks;
use crate::count::{corpus, documents};
use crate::dump::{self, Dump, Selection};

/// 名詞の回数を書くファイル名。行は `<正規化形>\t<全出現数>\t<単独の出現数>` である。
pub const NOUN_COUNTS_FILE: &str = "noun-counts.tsv";

/// 数えた記録を書くファイル名。
pub const NOUN_STATS_FILE: &str = "noun-stats.json";

/// 1 度に並列で解析する文書の数。
const BATCH_DOCUMENTS: usize = 64;

/// 途中経過を書き出す間隔(読んだ文書の数)。
const PROGRESS_DOCUMENTS: u64 = 200_000;

/// 名詞の回数。
#[derive(Default)]
pub struct NounCounts {
    /// 正規化形ごとの全出現数。
    total: HashMap<Box<str>, u64>,
    /// 正規化形ごとの単独の出現数。
    alone: HashMap<Box<str>, u64>,
    /// 数えた名詞のトークン数。頻度を 100 万語あたりに直す分母である。
    tokens: u64,
}

impl NounCounts {
    /// `text` を解析し、名詞の回数を足す。
    ///
    /// # Errors
    ///
    /// 解析辞書を読み込めない場合、解析器が入力を分けられない場合に返す。
    pub fn add_text(&mut self, text: &str) -> Result<(), MorphError> {
        for chunk in analysis_chunks(text) {
            let morphemes = analyze(chunk)?;
            self.add_morphemes(&morphemes);
        }
        Ok(())
    }

    /// 1 塊の形態素列から名詞の回数を足す。
    fn add_morphemes(&mut self, morphemes: &[Morpheme<'_>]) {
        for (index, morpheme) in morphemes.iter().enumerate() {
            if !is_counted_noun(&morpheme.part_of_speech) {
                continue;
            }
            let key = morpheme.normalized_form();
            if is_placeholder(key) {
                continue;
            }
            self.tokens += 1;
            increment(&mut self.total, key);
            if !joined_before(morphemes, index) && !joined_after(morphemes, index) {
                increment(&mut self.alone, key);
            }
        }
    }

    /// 別の回数を足し込む。
    pub fn merge(&mut self, other: Self) {
        for (key, count) in other.total {
            *self.total.entry(key).or_insert(0) += count;
        }
        for (key, count) in other.alone {
            *self.alone.entry(key).or_insert(0) += count;
        }
        self.tokens += other.tokens;
    }

    /// 回数の表を `path` へ書く。行は全出現数の多い順、同数なら正規化形の順である。
    fn write(&self, path: &Path) -> Result<()> {
        let mut rows: Vec<(&str, u64, u64)> = self
            .total
            .iter()
            .map(|(key, count)| {
                (
                    key.as_ref(),
                    *count,
                    self.alone.get(key).copied().unwrap_or(0),
                )
            })
            .collect();
        rows.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let file = File::create(path).with_context(|| format!("{} を作れない", path.display()))?;
        let mut writer = BufWriter::new(file);
        for (key, total, alone) in rows {
            writeln!(writer, "{key}\t{total}\t{alone}")
                .with_context(|| format!("{} を書けない", path.display()))?;
        }
        writer
            .flush()
            .with_context(|| format!("{} を書けない", path.display()))
    }
}

/// 数える名詞か。名詞と形状詞を採り、数詞は採らない。
fn is_counted_noun(part_of_speech: &PartOfSpeech) -> bool {
    match part_of_speech.category() {
        "名詞" => part_of_speech.subdivision(1) != "数詞",
        "形状詞" => true,
        _ => false,
    }
}

/// 検査の対象から外した範囲の跡に置いた名詞([`crate::flatten::PLACEHOLDER`])を
/// 含む語か。日本語の語ではないので、回数にも名詞トークンの総数にも数えない。
/// 跡の名詞は隣の名詞と連なって複合語にもなるので、含むかどうかで見る。
fn is_placeholder(normalized: &str) -> bool {
    normalized.contains(&crate::flatten::PLACEHOLDER.to_lowercase())
        || normalized.contains(crate::flatten::PLACEHOLDER)
}

/// 空白の形態素か。`Windows 版` のように空白を挟む複合を、空白で切れた別の語と見ないために
/// 読み飛ばす対象である。
pub(crate) fn is_space(part_of_speech: &PartOfSpeech) -> bool {
    part_of_speech.category() == "空白" || part_of_speech.subdivision(1) == "空白"
}

/// `index` の語の前に、空白を挟んで名詞・形状詞・接頭辞が付くか。付けば語の一部である。
pub(crate) fn joined_before(morphemes: &[Morpheme<'_>], index: usize) -> bool {
    morphemes[..index]
        .iter()
        .rev()
        .find(|morpheme| !is_space(&morpheme.part_of_speech))
        .is_some_and(|morpheme| {
            matches!(
                morpheme.part_of_speech.category(),
                "名詞" | "形状詞" | "接頭辞"
            )
        })
}

/// `index` の語の後に、空白を挟んで名詞・形状詞・接尾辞が付くか。
pub(crate) fn joined_after(morphemes: &[Morpheme<'_>], index: usize) -> bool {
    morphemes[index + 1..]
        .iter()
        .find(|morpheme| !is_space(&morpheme.part_of_speech))
        .is_some_and(|morpheme| {
            matches!(
                morpheme.part_of_speech.category(),
                "名詞" | "形状詞" | "接尾辞"
            )
        })
}

/// 回数を 1 足す。
fn increment(table: &mut HashMap<Box<str>, u64>, key: &str) {
    if let Some(count) = table.get_mut(key) {
        *count += 1;
    } else {
        table.insert(Box::from(key), 1);
    }
}

/// 数えた記録。
#[derive(Serialize)]
pub struct NounStats {
    /// 読んだ入力。ダンプのファイル名か、平文のディレクトリである。
    pub input: String,
    /// 入力の絞り込みの条件。ダンプでは記事を採る条件、平文では外したソースである。
    pub selection: String,
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた本文のバイト数。
    pub text_bytes: u64,
    /// 解析に失敗して飛ばした文書の数。
    pub skipped_documents: u64,
    /// 数えた名詞のトークン数。
    pub noun_tokens: u64,
    /// 正規化形の種類数。
    pub distinct: u64,
}

/// 文書のまとまりを並列に数え、1 つの表へ畳む。解析に失敗した文書は理由を書き出して
/// 飛ばし、その数を返す。
fn count_batch(texts: &[String]) -> (NounCounts, u64) {
    let counted: Vec<Result<NounCounts, MorphError>> = texts
        .par_iter()
        .map(|text| {
            let mut counts = NounCounts::default();
            counts.add_text(text)?;
            Ok(counts)
        })
        .collect();
    let mut merged = NounCounts::default();
    let mut skipped = 0;
    for result in counted {
        match result {
            Ok(counts) => merged.merge(counts),
            Err(error) => {
                eprintln!("解析に失敗した文書を飛ばす: {error}");
                skipped += 1;
            }
        }
    }
    (merged, skipped)
}

/// ダンプの記事を数え、`out_dir` に回数と記録を書く。
///
/// # Errors
///
/// ダンプを読めない場合、書き出しが失敗する場合に返す。
pub fn run_dump(
    dump_path: &Path,
    out_dir: &Path,
    selection: Selection,
    limit: Option<u64>,
) -> Result<()> {
    let mut dump = Dump::open(dump_path, selection, limit)?;
    let mut counts = NounCounts::default();
    let mut skipped = 0;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_DOCUMENTS);
    let mut reported = 0;
    while let Some(article) = dump.next_article()? {
        batch.push(article.text);
        if batch.len() == BATCH_DOCUMENTS {
            let (counted, failed) = count_batch(&batch);
            counts.merge(counted);
            skipped += failed;
            batch.clear();
        }
        if dump.scanned - reported >= PROGRESS_DOCUMENTS {
            reported = dump.scanned;
            eprintln!(
                "{} 記事を読み、{} 記事を採り、名詞の種類が {} 件",
                dump.scanned,
                dump.selected,
                counts.total.len()
            );
        }
    }
    let (counted, failed) = count_batch(&batch);
    counts.merge(counted);
    skipped += failed;
    let stats = NounStats {
        input: dump::file_name(dump_path),
        selection: selection.condition().to_owned(),
        documents: dump.selected,
        text_bytes: dump.text_bytes,
        skipped_documents: skipped,
        noun_tokens: counts.tokens,
        distinct: counts.total.len() as u64,
    };
    finish(out_dir, &counts, &stats)
}

/// 平文のファイルを数え、`out_dir` に回数と記録を書く。`paths` は
/// [`documents::text_paths`] か [`corpus::text_paths`] が返す。
///
/// # Errors
///
/// 文書を読めない場合、書き出しが失敗する場合に返す。
pub fn run_paths(paths: &[PathBuf], input: &Path, selection: String, out_dir: &Path) -> Result<()> {
    let mut counts = NounCounts::default();
    let mut skipped = 0;
    let mut text_bytes = 0;
    for chunk in paths.chunks(BATCH_DOCUMENTS) {
        let mut texts = Vec::with_capacity(chunk.len());
        for path in chunk {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("{} を読めない", path.display()))?;
            text_bytes += text.len() as u64;
            texts.push(text);
        }
        let (counted, failed) = count_batch(&texts);
        counts.merge(counted);
        skipped += failed;
    }
    let stats = NounStats {
        input: input.display().to_string(),
        selection,
        documents: paths.len() as u64,
        text_bytes,
        skipped_documents: skipped,
        noun_tokens: counts.tokens,
        distinct: counts.total.len() as u64,
    };
    finish(out_dir, &counts, &stats)
}

/// 回数の表と記録を書き、要約を標準出力に出す。
fn finish(out_dir: &Path, counts: &NounCounts, stats: &NounStats) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    counts.write(&out_dir.join(NOUN_COUNTS_FILE))?;
    let stats_path = out_dir.join(NOUN_STATS_FILE);
    let json = serde_json::to_string_pretty(stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} 文書({} バイト)を数え、{} 文書を飛ばした。名詞のトークンは {} 件、種類は {} 件である",
        stats.documents,
        stats.text_bytes,
        stats.skipped_documents,
        stats.noun_tokens,
        stats.distinct
    );
    Ok(())
}

/// 技術文書の平文のパスを、`documents::text_paths` の形で集める。
///
/// # Errors
///
/// 平文の記録を読めない場合に返す。
pub fn document_paths(
    text_dir: &Path,
    exclude: &[String],
    limit: Option<u64>,
) -> Result<Vec<PathBuf>> {
    documents::text_paths(text_dir, exclude, limit)
}

/// 日本語コーパスの平文のパスを、`corpus::text_paths` の形で集める。
///
/// # Errors
///
/// コーパスのディレクトリを読めない場合に返す。
pub fn corpus_paths(
    corpus_dir: &Path,
    exclude: &[String],
    limit: Option<u64>,
) -> Result<Vec<PathBuf>> {
    corpus::text_paths(corpus_dir, exclude, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 単独の名詞と複合の部品を分けて数える() {
        let mut counts = NounCounts::default();
        counts.add_text("門を通過させる。論理ゲートの門。").unwrap();
        assert_eq!(counts.total.get("門").copied(), Some(2));
        // 「論理ゲートの門」の「門」は「の」を挟むので単独である。「論理ゲート」の
        // 「ゲート」は「論理」に続くので単独ではない。
        assert_eq!(counts.alone.get("門").copied(), Some(2));
        assert_eq!(counts.total.get("ゲート").copied(), Some(1));
        assert_eq!(counts.alone.get("ゲート"), None);
    }

    #[test]
    fn 空白を挟む複合は単独に数えない() {
        let mut counts = NounCounts::default();
        counts
            .add_text("Windows 版のファイル。版を上げる。")
            .unwrap();
        assert_eq!(counts.total.get("版").copied(), Some(2));
        // 「Windows 版」の「版」は空白を挟んでも語の一部である。
        assert_eq!(counts.alone.get("版").copied(), Some(1));
    }

    #[test]
    fn 数詞は数えない() {
        let mut counts = NounCounts::default();
        counts.add_text("3 個のファイル").unwrap();
        assert!(!counts.total.contains_key("3"));
        assert_eq!(counts.total.get("ファイル").copied(), Some(1));
    }
}
