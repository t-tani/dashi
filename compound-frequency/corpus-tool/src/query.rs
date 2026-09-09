//! 成果物を読み、語のバケットと各部品の頻度を表示する。

use std::path::Path;

use aku_freq::{ConstituentFrequencies, FrequencyFilter};
use aku_morph::{DICTIONARY_VERSION, Morpheme, analyze_short};
use anyhow::{Context, Result};

use crate::build::{CONSTITUENT_FILE, FILTER_FILE};

/// 登録が無い複合語に書く値。
const UNREGISTERED: &str = "未登録";

/// `artifacts_dir` の成果物で `words` を引き、1 語 1 行で書き出す。
///
/// 行は `<語>\t<バケット>\t<部品>:<頻度> ...` である。部品は入力の語を分割単位 A
/// で割ったもので、頻度が表に無ければ 0 を書く。
///
/// # Errors
///
/// 成果物を読めない場合、語を解析できない場合に返す。
pub fn run(artifacts_dir: &Path, words: &[String]) -> Result<()> {
    let filter_path = artifacts_dir.join(FILTER_FILE);
    let filter_bytes = std::fs::read(&filter_path)
        .with_context(|| format!("{} を読めない", filter_path.display()))?;
    let filter = FrequencyFilter::from_bytes(&filter_bytes, DICTIONARY_VERSION)
        .with_context(|| format!("{} を読めない", filter_path.display()))?;
    let constituent_path = artifacts_dir.join(CONSTITUENT_FILE);
    let constituent_bytes = std::fs::read(&constituent_path)
        .with_context(|| format!("{} を読めない", constituent_path.display()))?;
    let components = ConstituentFrequencies::read(constituent_bytes)
        .with_context(|| format!("{} を読めない", constituent_path.display()))?;

    for word in words {
        let morphemes = analyze_short(word).with_context(|| format!("'{word}' を解析できない"))?;
        let surface: String = morphemes.iter().map(|morpheme| morpheme.surface).collect();
        let normalized: String = morphemes.iter().map(Morpheme::normalized_form).collect();
        let bucket = filter
            .bucket(&surface)
            .or_else(|| filter.bucket(&normalized));
        let frequencies: Vec<String> = morphemes
            .iter()
            .map(|morpheme| {
                let frequency = components.frequency(morpheme.surface).unwrap_or(0);
                format!("{}:{frequency}", morpheme.surface)
            })
            .collect();
        let bucket = bucket.map_or_else(
            || UNREGISTERED.to_owned(),
            |bucket| bucket.get().to_string(),
        );
        println!("{word}\t{bucket}\t{}", frequencies.join(" "));
    }
    Ok(())
}
