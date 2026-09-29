//! 対象の語が単独で立つ出現ごとに、係り受けの部分木と、名詞へ係る修飾と、格と述語の
//! 組を 1 行の JSON で書き出す。
//!
//! 3 つは akunuki の同じ解析から取り、名詞の範囲で結び付ける。単独の漢字 1 字の名詞の
//! 判定モデルは、この記録から特徴を作って学ぶ。検査する側の akunuki も同じ関数で同じ
//! 3 つを取るので、学んだ特徴と検査で作る特徴が揃う。
//!
//! 1 行は、語の正規化形・ファイル・部分木の節点・解析の誤りが混じりにくいか・前後を
//! 切り出した本文・修飾の種類と語・格と述語を持つ。修飾か組が取れない出現では、その欄が
//! `null` になる。

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use aku_core::{ScanConfig, scan};
use aku_morph::{Analysis, case_frames, noun_modifiers, noun_subtrees};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde_json::json;

use crate::case_frames::{kind_name, tables};
use crate::compound::analysis_chunks;
use crate::cooccurrence::Targets;

/// 本文を切り出す、語の前後のバイト数。
const EXCERPT_BYTES: usize = 120;

/// `paths` の文書から出現を抜き出し、`out` へ JSONL で書く。返すのは書いた件数である。
///
/// 行の並びは、文書は `paths` の順、文書の中は出現の順である。
///
/// # Errors
///
/// 係り受けの表を読めない場合、書き出しが失敗する場合に返す。読めない文書と、解析に
/// 失敗した塊は飛ばす。数える側([`crate::case_frames`])も同じ扱いである。
pub fn run(paths: &[PathBuf], targets: &Targets, out: &Path) -> Result<u64> {
    let tables = tables()?;
    let lines: Vec<Vec<String>> = paths
        .par_iter()
        .map(|path| {
            let Ok(text) = std::fs::read_to_string(path) else {
                return Vec::new();
            };
            let mut lines = Vec::new();
            for chunk in analysis_chunks(&text) {
                let scanned = scan(chunk, ScanConfig::default());
                let Ok(analysis) = Analysis::of(chunk, &scanned.fragments) else {
                    continue;
                };
                let frames = case_frames(&scanned.fragments, &analysis, &tables);
                let modifiers = noun_modifiers(&scanned.fragments, &analysis, &tables);
                for found in noun_subtrees(&scanned.fragments, &analysis, &tables) {
                    if !found.standalone || !targets.contains(&found.word) {
                        continue;
                    }
                    let modifier = modifiers.iter().find(|m| m.range == found.range);
                    let frame = frames.iter().find(|f| f.range == found.range);
                    // 並び順の隣というだけで含めた節点の距離は、実験の記録と同じく usize::MAX で書く。
                    let nodes: Vec<serde_json::Value> = found
                        .nodes
                        .iter()
                        .map(|node| {
                            json!({
                                "id": node.id, "parent": node.parent,
                                "distance": node.distance.unwrap_or(usize::MAX),
                                "position": node.position, "lemma": node.lemma, "pos": node.pos,
                                "function": node.function, "relation": node.relation,
                                "surface": node.surface,
                            })
                        })
                        .collect();
                    let record = json!({
                        "word": found.word, "file": path.display().to_string(),
                        "clean": found.clean, "nodes": nodes,
                        "excerpt": excerpt(chunk, &found.range),
                        "kind": modifier.map(|m| kind_name(m.kind)),
                        "modifier": modifier.map(|m| m.modifier.clone()),
                        "case": frame.map(|f| f.case.clone()),
                        "predicate": frame.map(|f| f.predicate.clone()),
                    });
                    lines.push(record.to_string());
                }
            }
            lines
        })
        .collect();
    let file =
        std::fs::File::create(out).with_context(|| format!("{} を作れない", out.display()))?;
    let mut writer = BufWriter::new(file);
    let mut written = 0;
    for line in lines.iter().flatten() {
        writeln!(writer, "{line}").context("書き出しに失敗した")?;
        written += 1;
    }
    writer.flush().context("書き出しに失敗した")?;
    Ok(written)
}

/// 語の前後 [`EXCERPT_BYTES`] バイトを切り出し、改行を空白にする。範囲が文字の境界に
/// 無ければ内側へ縮める。
fn excerpt(chunk: &str, range: &std::ops::Range<usize>) -> String {
    let mut start = range.start.saturating_sub(EXCERPT_BYTES).min(chunk.len());
    while !chunk.is_char_boundary(start) {
        start += 1;
    }
    let mut end = (range.end + EXCERPT_BYTES).min(chunk.len());
    while !chunk.is_char_boundary(end) {
        end -= 1;
    }
    chunk[start..end].replace('\n', " ")
}
