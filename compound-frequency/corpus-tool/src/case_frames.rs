//! 対象の語ごとに、付いた格助詞と係り先の述語の組の回数を数える。語の意味が
//! どの述語のどの格で分かれるかを見るための材料である。
//!
//! 周辺の内容語を数える共起([`crate::cooccurrence`])が話題を測るのに対し、
//! こちらは名詞の役割を測る。検査する文書は分野が決まっているので、話題は
//! 分けたい意味の対を跨いで一定になる。`段を組む` と `段を実行する` のように、
//! 格と述語は跨がない。
//!
//! 数えるのは、単独で立つ出現だけである。単独とは、前に名詞・形状詞・接頭辞が
//! なく、後ろに名詞・形状詞・接尾辞がない出現で、[`crate::nouns`] の数え方と
//! 同じである。係り先は akunuki の係り受けの木から読む。受動態の述語に係る
//! 「が」は「を」に直す。
//!
//! 対象の語は一覧のファイルで渡す。1 行 1 語で、分割単位 C の正規化形で書く。
//! キーは対象の語と格助詞と述語の辞書形をタブで繋いだものである。
//!
//! 入力は [`crate::nouns`] と同じ 3 つで、ダンプの記事、平文にした技術文書、
//! 日本語コーパスである。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aku_core::{ScanConfig, scan};
use aku_morph::{Analysis, MorphError, RoleDependencyTables, case_frames, noun_modifiers};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

use crate::compound::analysis_chunks;
use crate::cooccurrence::Targets;
use crate::count::runs::Runs;
use crate::dump::{self, Dump, Selection};
use crate::tsv;

/// 組の回数を書くファイル名。行は `<語>\t<格助詞>\t<述語>\t<回数>` である。
pub const CASE_FRAME_COUNTS_FILE: &str = "case-frame-counts.tsv";

/// 対象の語ごとの、述語に係った単独の出現数を書くファイル名。組の回数を確率に
/// 直す分母である。
pub const CASE_FRAME_TARGETS_FILE: &str = "case-frame-targets.tsv";

/// 数えた記録を書くファイル名。
pub const CASE_FRAME_STATS_FILE: &str = "case-frame-stats.json";

/// 1 度に並列で解析する文書の数。
const BATCH_DOCUMENTS: usize = 64;

/// 途中経過を書き出す間隔(読んだ文書の数)。
const PROGRESS_DOCUMENTS: u64 = 50_000;

/// 組の回数を run へ書き出すキーの数。
const SPILL_KEYS: usize = 8_000_000;

/// run のファイル名の頭。
const RUN_PREFIX: &str = "case-frame-run-";

/// 語と格助詞と述語を繋ぐ文字。正規化形も辞書形もタブを含まない。
const KEY_SEPARATOR: char = '\t';

/// 格と述語の組の回数。
#[derive(Default)]
pub struct CaseFrameCounts {
    /// 語と格助詞と述語の組の回数。
    frames: HashMap<Box<str>, u32>,
    /// 対象の語が述語に係った、単独の出現数。
    targets: HashMap<Box<str>, u32>,
    /// 複合語の一部として述語に係った出現数。単独の出現がどれだけ少ないかを
    /// 記録に残すために数える。
    joined: u64,
    /// 対象の語の単独の出現ごとの、修飾の種類と語の回数。キーは
    /// `<語>\t<種類>\t<修飾の語>` である。
    modifiers: HashMap<Box<str>, u32>,
}

impl CaseFrameCounts {
    /// `text` を解析し、組の回数を足す。
    ///
    /// # Errors
    ///
    /// 解析辞書を読み込めない場合、解析器が入力を分けられない場合に返す。
    pub fn add_text(
        &mut self,
        text: &str,
        targets: &Targets,
        tables: &RoleDependencyTables,
    ) -> Result<(), MorphError> {
        for chunk in analysis_chunks(text) {
            let scanned = scan(chunk, ScanConfig::default());
            let analysis = Analysis::of(chunk, &scanned.fragments)?;
            for found in noun_modifiers(&scanned.fragments, &analysis, tables) {
                if found.standalone && targets.contains(&found.word) {
                    increment_owned(
                        &mut self.modifiers,
                        format!(
                            "{}\t{}\t{}",
                            found.word,
                            kind_name(found.kind),
                            found.modifier
                        ),
                    );
                }
            }
            for frame in case_frames(&scanned.fragments, &analysis, tables) {
                if !targets.contains(&frame.word) {
                    continue;
                }
                if !frame.standalone {
                    self.joined += 1;
                    continue;
                }
                increment(&mut self.targets, &frame.word);
                let mut key = String::with_capacity(
                    frame.word.len() + frame.case.len() + frame.predicate.len() + 2,
                );
                key.push_str(&frame.word);
                key.push(KEY_SEPARATOR);
                key.push_str(&frame.case);
                key.push(KEY_SEPARATOR);
                key.push_str(&frame.predicate);
                increment_owned(&mut self.frames, key);
            }
        }
        Ok(())
    }

    /// 別の回数を足し込む。
    pub fn merge(&mut self, other: Self) {
        for (key, count) in other.frames {
            *self.frames.entry(key).or_insert(0) += count;
        }
        for (key, count) in other.targets {
            *self.targets.entry(key).or_insert(0) += count;
        }
        self.joined += other.joined;
        for (key, count) in other.modifiers {
            *self.modifiers.entry(key).or_insert(0) += count;
        }
    }
}

/// 回数を 1 足す。
fn increment(table: &mut HashMap<Box<str>, u32>, key: &str) {
    if let Some(count) = table.get_mut(key) {
        *count += 1;
    } else {
        table.insert(Box::from(key), 1);
    }
}

/// 組み立てた鍵で回数を 1 足す。
fn increment_owned(table: &mut HashMap<Box<str>, u32>, key: String) {
    if let Some(count) = table.get_mut(key.as_str()) {
        *count += 1;
    } else {
        table.insert(key.into_boxed_str(), 1);
    }
}

/// 数えた記録。
#[derive(Serialize)]
pub struct CaseFrameStats {
    /// 読んだ入力。
    pub input: String,
    /// 入力の絞り込みの条件。
    pub selection: String,
    /// 対象の語の一覧の語数。
    pub targets: u64,
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた本文のバイト数。
    pub text_bytes: u64,
    /// 解析に失敗して飛ばした文書の数。
    pub skipped_documents: u64,
    /// 述語に係った、対象の語の単独の出現の延べ数。
    pub standalone_occurrences: u64,
    /// 述語に係ったが複合語の一部だった出現の延べ数。
    pub joined_occurrences: u64,
    /// 語と格助詞と述語の組の数。
    pub frames: u64,
}

/// 文書のまとまりを並列に数え、1 つの表へ畳む。
fn count_batch(
    texts: &[String],
    targets: &Targets,
    tables: &RoleDependencyTables,
) -> (CaseFrameCounts, u64) {
    let counted: Vec<Result<CaseFrameCounts, MorphError>> = texts
        .par_iter()
        .map(|text| {
            let mut counts = CaseFrameCounts::default();
            counts.add_text(text, targets, tables)?;
            Ok(counts)
        })
        .collect();
    let mut merged = CaseFrameCounts::default();
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

/// ダンプの記事の組を数え、`out_dir` に回数と記録を書く。
///
/// # Errors
///
/// ダンプを読めない場合、係り受けの表を読めない場合、書き出しが失敗する場合に返す。
pub fn run_dump(
    dump_path: &Path,
    targets: &Targets,
    out_dir: &Path,
    selection: Selection,
    limit: Option<u64>,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let tables = tables()?;
    let mut dump = Dump::open(dump_path, selection, limit)?;
    let mut counts = CaseFrameCounts::default();
    let mut runs = Runs::new(out_dir, RUN_PREFIX);
    let mut skipped = 0;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_DOCUMENTS);
    let mut reported = 0;
    while let Some(article) = dump.next_article()? {
        batch.push(article.text);
        if batch.len() == BATCH_DOCUMENTS {
            let (counted, failed) = count_batch(&batch, targets, &tables);
            counts.merge(counted);
            skipped += failed;
            batch.clear();
            if counts.frames.len() >= SPILL_KEYS {
                runs.spill(&mut counts.frames)?;
            }
        }
        if dump.scanned - reported >= PROGRESS_DOCUMENTS {
            reported = dump.scanned;
            eprintln!(
                "{} 記事を読み、{} 記事を採り、メモリの組が {} 件",
                dump.scanned,
                dump.selected,
                counts.frames.len()
            );
        }
    }
    let (counted, failed) = count_batch(&batch, targets, &tables);
    counts.merge(counted);
    skipped += failed;
    let stats = CaseFrameStats {
        input: dump::file_name(dump_path),
        selection: selection.condition().to_owned(),
        targets: targets.len() as u64,
        documents: dump.selected,
        text_bytes: dump.text_bytes,
        skipped_documents: skipped,
        standalone_occurrences: 0,
        joined_occurrences: 0,
        frames: 0,
    };
    finish(out_dir, counts, runs, stats)
}

/// 平文のファイルの組を数え、`out_dir` に回数と記録を書く。
///
/// # Errors
///
/// 文書を読めない場合、係り受けの表を読めない場合、書き出しが失敗する場合に返す。
pub fn run_paths(
    paths: &[PathBuf],
    targets: &Targets,
    input: &Path,
    selection: String,
    out_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let tables = tables()?;
    let mut counts = CaseFrameCounts::default();
    let mut runs = Runs::new(out_dir, RUN_PREFIX);
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
        let (counted, failed) = count_batch(&texts, targets, &tables);
        counts.merge(counted);
        skipped += failed;
        if counts.frames.len() >= SPILL_KEYS {
            runs.spill(&mut counts.frames)?;
        }
    }
    let stats = CaseFrameStats {
        input: input.display().to_string(),
        selection,
        targets: targets.len() as u64,
        documents: paths.len() as u64,
        text_bytes,
        skipped_documents: skipped,
        standalone_occurrences: 0,
        joined_occurrences: 0,
        frames: 0,
    };
    finish(out_dir, counts, runs, stats)
}

/// 係り受けの木を組み立てる参照表を、埋め込みから読む。
pub(crate) fn tables() -> Result<RoleDependencyTables> {
    aku_morph::embedded_role_dependency_tables().context("係り受け用の参照表を読めない")
}

/// 回数の表と記録を書き、要約を標準出力に出す。
fn finish(
    out_dir: &Path,
    mut counts: CaseFrameCounts,
    mut runs: Runs,
    mut stats: CaseFrameStats,
) -> Result<()> {
    stats.standalone_occurrences = counts.targets.values().map(|count| u64::from(*count)).sum();
    stats.joined_occurrences = counts.joined;
    runs.spill(&mut counts.frames)?;
    let frames = runs.merge_into(&out_dir.join(CASE_FRAME_COUNTS_FILE))?;
    tsv::write(&out_dir.join(CASE_FRAME_TARGETS_FILE), &counts.targets)?;
    tsv::write(&out_dir.join("noun-modifier-counts.tsv"), &counts.modifiers)?;
    stats.frames = frames;
    let stats_path = out_dir.join(CASE_FRAME_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} 文書({} バイト)を数え、{} 文書を飛ばした。単独の出現は {} 件、複合の一部は {} 件、組は {} 件である",
        stats.documents,
        stats.text_bytes,
        stats.skipped_documents,
        stats.standalone_occurrences,
        stats.joined_occurrences,
        frames
    );
    Ok(())
}

/// 修飾の種類の、回数の表と出現の記録での名前。
pub(crate) fn kind_name(kind: aku_morph::ModifierKind) -> &'static str {
    match kind {
        aku_morph::ModifierKind::No => "の",
        aku_morph::ModifierKind::Adnominal => "連体",
        aku_morph::ModifierKind::Continuative => "連用",
        aku_morph::ModifierKind::Other => "その他",
        aku_morph::ModifierKind::None => "なし",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counted(text: &str, words: &[&str]) -> CaseFrameCounts {
        let targets = Targets::from_lines(&words.join("\n"));
        let tables = tables().expect("係り受け用の参照表を読めること");
        let mut counts = CaseFrameCounts::default();
        counts.add_text(text, &targets, &tables).unwrap();
        counts
    }

    #[test]
    fn 格助詞と係り先の述語の組を数える() {
        let counts = counted("段を組む。", &["段"]);
        assert_eq!(counts.frames.get("段\tを\t組む").copied(), Some(1));
        assert_eq!(counts.targets.get("段").copied(), Some(1));
    }

    #[test]
    fn 複合語の一部は数えない() {
        let counts = counted("プロトコル層を実装する。", &["層"]);
        assert_eq!(counts.targets.get("層"), None);
        assert_eq!(counts.joined, 1);
    }

    #[test]
    fn 一覧にない語は数えない() {
        let counts = counted("段を組む。", &["層"]);
        assert!(counts.frames.is_empty());
    }
}
