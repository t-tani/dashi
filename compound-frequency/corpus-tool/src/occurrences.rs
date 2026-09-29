//! 検査する文書から、対象の語が単独で立つ出現とその文脈を JSONL に書き出す。
//!
//! 単独の判定と文脈の取り方は [`crate::cooccurrence`] と同じ関数を通る。数える側と検査する側が
//! 同じ規則を通らなければ、集めた文脈と引く文脈が食い違う。
//!
//! 1 行が 1 つの出現で、語の正規化形、文脈の語、ファイル、前後を切り出した本文を持つ。

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use aku_morph::{Morpheme, analyze};
use anyhow::{Context, Result};
use serde::Serialize;

use crate::compound::analysis_chunks;
use crate::cooccurrence::{Targets, context_words, is_content_word};
use crate::nouns::{joined_after, joined_before};

/// 本文を切り出す前後の文字数。
const EXCERPT_CHARS: usize = 30;

/// 出現 1 件。
#[derive(Serialize)]
struct Occurrence {
    /// 語の正規化形。
    word: String,
    /// 前後 [`WINDOW`] 形態素の内容語の正規化形。
    context: Vec<String>,
    /// 読んだファイル。
    file: String,
    /// 前後を切り出した本文。
    excerpt: String,
}

/// `paths` の文書から出現を抜き出し、`out` へ JSONL で書く。返すのは書いた件数である。
///
/// # Errors
///
/// 文書を読めない場合、解析に失敗した場合、書き出しが失敗する場合に返す。
pub fn run(paths: &[PathBuf], targets: &Targets, out: &Path) -> Result<u64> {
    let file =
        std::fs::File::create(out).with_context(|| format!("{} を作れない", out.display()))?;
    let mut writer = BufWriter::new(file);
    let mut written = 0;
    for path in paths {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("{} を読めない", path.display()))?;
        for chunk in analysis_chunks(&text) {
            let Ok(morphemes) = analyze(chunk) else {
                // 1 つの塊の失敗で走行を落とさない。数える側([`crate::cooccurrence`])も
                // 失敗した文書を飛ばす。
                continue;
            };
            for (index, morpheme) in morphemes.iter().enumerate() {
                if !is_content_word(&morpheme.part_of_speech)
                    || !targets.contains(morpheme.normalized_form())
                    || joined_before(&morphemes, index)
                    || joined_after(&morphemes, index)
                {
                    continue;
                }
                let occurrence = Occurrence {
                    word: morpheme.normalized_form().to_owned(),
                    context: context_words(&morphemes, index, targets),
                    file: path.display().to_string(),
                    excerpt: excerpt(chunk, morpheme),
                };
                let line = serde_json::to_string(&occurrence).context("JSON にできない")?;
                writeln!(writer, "{line}").context("書き出しに失敗した")?;
                written += 1;
            }
        }
    }
    writer.flush().context("書き出しに失敗した")?;
    Ok(written)
}

/// 語の前後を切り出す。改行は空白へ直す。
fn excerpt(chunk: &str, morpheme: &Morpheme<'_>) -> String {
    let start = chunk[..morpheme.range.start]
        .char_indices()
        .rev()
        .nth(EXCERPT_CHARS)
        .map_or(0, |(index, _)| index);
    let end = chunk[morpheme.range.end..]
        .char_indices()
        .nth(EXCERPT_CHARS)
        .map_or(chunk.len(), |(index, _)| morpheme.range.end + index);
    chunk[start..end].replace('\n', " ")
}
