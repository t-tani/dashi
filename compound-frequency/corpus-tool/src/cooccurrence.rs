//! 対象の語ごとに、前後に現れた内容語の回数を数える。外来語の置き換え表の判定が、
//! 書かれた語とカタカナ語のどちらの文脈に近いかを比べるための材料である。
//!
//! 数えるのは、対象の語の出現ごとに、その前後 [`WINDOW`] 形態素の内容語である。
//! 内容語は名詞・形状詞・動詞・形容詞で、数詞と非自立の語は数えない。キーは対象の
//! 語と文脈の語をタブで繋いだもので、どちらも正規化形である。対象の語自身は文脈に
//! 数えない。
//!
//! 対象の語は一覧のファイルで渡す。1 行 1 語で、分割単位 C の正規化形で書く。
//! 一覧にない語は、対象としても文脈としても数えない。表が語彙の 2 乗に膨らむのを
//! 避けるためである。
//!
//! 入力は [`crate::nouns`] と同じ 3 つで、ダンプの記事、平文にした技術文書、
//! 日本語コーパスである。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use aku_morph::{MorphError, Morpheme, PartOfSpeech, analyze};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;

use crate::compound::analysis_chunks;
use crate::count::runs::Runs;
use crate::dump::{self, Dump, Selection};
use crate::nouns::{joined_after, joined_before};
use crate::tsv;

/// 共起の回数を書くファイル名。行は `<対象の語>\t<文脈の語>\t<回数>` である。
pub const COOCCURRENCE_COUNTS_FILE: &str = "cooccurrence-counts.tsv";

/// 対象の語ごとの出現数を書くファイル名。共起の回数を確率に直す分母である。
pub const TARGET_COUNTS_FILE: &str = "cooccurrence-targets.tsv";

/// 数えた記録を書くファイル名。
pub const COOCCURRENCE_STATS_FILE: &str = "cooccurrence-stats.json";

/// 文脈に数える前後の形態素の数。
pub const WINDOW: usize = 5;

/// 1 度に並列で解析する文書の数。
const BATCH_DOCUMENTS: usize = 64;

/// 途中経過を書き出す間隔(読んだ文書の数)。
const PROGRESS_DOCUMENTS: u64 = 200_000;

/// 共起の回数を run へ書き出すキーの数。
const SPILL_KEYS: usize = 8_000_000;

/// run のファイル名の頭。
const RUN_PREFIX: &str = "cooccurrence-run-";

/// 対象の語と文脈の語を繋ぐ文字。正規化形はタブを含まない。
const KEY_SEPARATOR: char = '\t';

/// 対象の語の一覧。
pub struct Targets(HashSet<Box<str>>);

impl Targets {
    /// 1 行 1 語の一覧を読む。空行と前後の空白は落とす。
    ///
    /// # Errors
    ///
    /// ファイルを読めない場合に返す。
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("{} を読めない", path.display()))?;
        Ok(Self::from_lines(&text))
    }

    /// 1 行 1 語の並びを読む。空行と前後の空白は落とす。
    #[must_use]
    pub fn from_lines(text: &str) -> Self {
        Self(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(Box::from)
                .collect(),
        )
    }

    /// 対象の語か。
    #[must_use]
    pub fn contains(&self, word: &str) -> bool {
        self.0.contains(word)
    }

    /// 語の数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// 語を持たないか。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// 共起の回数。
#[derive(Default)]
pub struct Cooccurrences {
    /// 対象の語と文脈の語の組の回数。
    pairs: HashMap<Box<str>, u32>,
    /// 対象の語の出現数。
    targets: HashMap<Box<str>, u32>,
}

impl Cooccurrences {
    /// `text` を解析し、共起の回数を足す。
    ///
    /// # Errors
    ///
    /// 解析辞書を読み込めない場合、解析器が入力を分けられない場合に返す。
    pub fn add_text(&mut self, text: &str, targets: &Targets) -> Result<(), MorphError> {
        for chunk in analysis_chunks(text) {
            let morphemes = analyze(chunk)?;
            self.add_morphemes(&morphemes, targets);
        }
        Ok(())
    }

    /// 1 塊の形態素列から共起の回数を足す。
    fn add_morphemes(&mut self, morphemes: &[Morpheme<'_>], targets: &Targets) {
        for (index, morpheme) in morphemes.iter().enumerate() {
            let form = morpheme.normalized_form();
            if !is_content_word(&morpheme.part_of_speech) || !targets.contains(form) {
                continue;
            }
            if joined_before(morphemes, index) || joined_after(morphemes, index) {
                continue;
            }
            increment(&mut self.targets, form);
            for context in context_words(morphemes, index, targets) {
                let mut key = String::with_capacity(form.len() + context.len() + 1);
                key.push_str(form);
                key.push(KEY_SEPARATOR);
                key.push_str(&context);
                increment_owned(&mut self.pairs, key);
            }
        }
    }

    /// 別の回数を足し込む。
    pub fn merge(&mut self, other: Self) {
        for (key, count) in other.pairs {
            *self.pairs.entry(key).or_insert(0) += count;
        }
        for (key, count) in other.targets {
            *self.targets.entry(key).or_insert(0) += count;
        }
    }
}

/// `index` の語の前後 [`WINDOW`] 形態素から、一覧にある内容語を返す。語自身は入れない。
///
/// 数える側([`Cooccurrences`])と検査する側([`crate::occurrences`])が同じ切り出しを通る。
#[must_use]
pub fn context_words(morphemes: &[Morpheme<'_>], index: usize, targets: &Targets) -> Vec<String> {
    let start = index.saturating_sub(WINDOW);
    let end = (index + WINDOW + 1).min(morphemes.len());
    morphemes[start..end]
        .iter()
        .enumerate()
        .filter(|(offset, morpheme)| {
            start + offset != index
                && is_content_word(&morpheme.part_of_speech)
                && targets.contains(morpheme.normalized_form())
        })
        .map(|(_, morpheme)| morpheme.normalized_form().to_owned())
        .collect()
}

/// 文脈に数える品詞か。名詞・形状詞・動詞・形容詞のうち、数詞と非自立の語を外す。
pub fn is_content_word(part_of_speech: &PartOfSpeech) -> bool {
    let subdivision = part_of_speech.subdivision(1);
    match part_of_speech.category() {
        "名詞" => !matches!(subdivision, "数詞"),
        "形状詞" | "動詞" | "形容詞" => subdivision != "非自立可能",
        _ => false,
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
pub struct CooccurrenceStats {
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
    /// 文脈に現れた対象の語の延べ数。
    pub target_occurrences: u64,
    /// 対象の語と文脈の語の組の数。
    pub pairs: u64,
    /// 前後に数えた形態素の数。
    pub window: u64,
}

/// 文書のまとまりを並列に数え、1 つの表へ畳む。
fn count_batch(texts: &[String], targets: &Targets) -> (Cooccurrences, u64) {
    let counted: Vec<Result<Cooccurrences, MorphError>> = texts
        .par_iter()
        .map(|text| {
            let mut counts = Cooccurrences::default();
            counts.add_text(text, targets)?;
            Ok(counts)
        })
        .collect();
    let mut merged = Cooccurrences::default();
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

/// ダンプの記事の共起を数え、`out_dir` に回数と記録を書く。
///
/// # Errors
///
/// ダンプを読めない場合、書き出しが失敗する場合に返す。
pub fn run_dump(
    dump_path: &Path,
    targets: &Targets,
    out_dir: &Path,
    selection: Selection,
    limit: Option<u64>,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let mut dump = Dump::open(dump_path, selection, limit)?;
    let mut counts = Cooccurrences::default();
    let mut runs = Runs::new(out_dir, RUN_PREFIX);
    let mut skipped = 0;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_DOCUMENTS);
    let mut reported = 0;
    while let Some(article) = dump.next_article()? {
        batch.push(article.text);
        if batch.len() == BATCH_DOCUMENTS {
            let (counted, failed) = count_batch(&batch, targets);
            counts.merge(counted);
            skipped += failed;
            batch.clear();
            if counts.pairs.len() >= SPILL_KEYS {
                runs.spill(&mut counts.pairs)?;
            }
        }
        if dump.scanned - reported >= PROGRESS_DOCUMENTS {
            reported = dump.scanned;
            eprintln!(
                "{} 記事を読み、{} 記事を採り、メモリの組が {} 件",
                dump.scanned,
                dump.selected,
                counts.pairs.len()
            );
        }
    }
    let (counted, failed) = count_batch(&batch, targets);
    counts.merge(counted);
    skipped += failed;
    let stats = CooccurrenceStats {
        input: dump::file_name(dump_path),
        selection: selection.condition().to_owned(),
        targets: targets.len() as u64,
        documents: dump.selected,
        text_bytes: dump.text_bytes,
        skipped_documents: skipped,
        target_occurrences: counts.targets.values().map(|count| u64::from(*count)).sum(),
        pairs: 0,
        window: WINDOW as u64,
    };
    finish(out_dir, counts, runs, stats)
}

/// 平文のファイルの共起を数え、`out_dir` に回数と記録を書く。
///
/// # Errors
///
/// 文書を読めない場合、書き出しが失敗する場合に返す。
pub fn run_paths(
    paths: &[PathBuf],
    targets: &Targets,
    input: &Path,
    selection: String,
    out_dir: &Path,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let mut counts = Cooccurrences::default();
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
        let (counted, failed) = count_batch(&texts, targets);
        counts.merge(counted);
        skipped += failed;
        if counts.pairs.len() >= SPILL_KEYS {
            runs.spill(&mut counts.pairs)?;
        }
    }
    let stats = CooccurrenceStats {
        input: input.display().to_string(),
        selection,
        targets: targets.len() as u64,
        documents: paths.len() as u64,
        text_bytes,
        skipped_documents: skipped,
        target_occurrences: counts.targets.values().map(|count| u64::from(*count)).sum(),
        pairs: 0,
        window: WINDOW as u64,
    };
    finish(out_dir, counts, runs, stats)
}

/// 回数の表と記録を書き、要約を標準出力に出す。
fn finish(
    out_dir: &Path,
    mut counts: Cooccurrences,
    mut runs: Runs,
    mut stats: CooccurrenceStats,
) -> Result<()> {
    runs.spill(&mut counts.pairs)?;
    let pairs = runs.merge_into(&out_dir.join(COOCCURRENCE_COUNTS_FILE))?;
    tsv::write(&out_dir.join(TARGET_COUNTS_FILE), &counts.targets)?;
    stats.pairs = pairs;
    let stats_path = out_dir.join(COOCCURRENCE_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} 文書({} バイト)を数え、{} 文書を飛ばした。対象の語の延べ数は {} 件、組は {} 件である",
        stats.documents, stats.text_bytes, stats.skipped_documents, stats.target_occurrences, pairs
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(words: &[&str]) -> Targets {
        Targets(words.iter().map(|word| Box::from(*word)).collect())
    }

    #[test]
    fn 対象の語の前後の内容語を数える() {
        let mut counts = Cooccurrences::default();
        let targets = targets(&["門", "通過", "論理", "ゲート"]);
        counts
            .add_text("門を通過させる。論理ゲートの設計。", &targets)
            .unwrap();
        assert_eq!(counts.targets.get("門").copied(), Some(1));
        assert_eq!(counts.pairs.get("門\t通過").copied(), Some(1));
        // 一覧にない語(設計)は文脈に数えない。
        assert_eq!(counts.pairs.get("ゲート\t設計"), None);
        // 対象の語自身は文脈に数えない。
        assert_eq!(counts.pairs.get("門\t門"), None);
        // 「論理ゲート」の「ゲート」は語の一部なので、対象として数えない。
        assert_eq!(counts.targets.get("ゲート"), None);
    }

    #[test]
    fn 空白を挟む複合は対象に数えない() {
        let mut counts = Cooccurrences::default();
        let targets = targets(&["版", "更新"]);
        counts.add_text("Windows 版を更新する。", &targets).unwrap();
        assert_eq!(counts.targets.get("版"), None);
    }

    #[test]
    fn 助詞と記号は文脈に数えない() {
        let mut counts = Cooccurrences::default();
        let targets = targets(&["門", "を"]);
        counts.add_text("門を。", &targets).unwrap();
        assert_eq!(counts.pairs.get("門\tを"), None);
    }
}
