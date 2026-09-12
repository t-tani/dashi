//! 成果物を読み、語のバケットと各部品の頻度と相手の種類数を表示する。

use std::path::Path;

use aku_freq::{AlignedBytes, ConstituentEntry, ConstituentFrequencies, FrequencyFilter};
use aku_morph::{DICTIONARY_VERSION, Morpheme, analyze, analyze_short};
use anyhow::{Context, Result};

use crate::build::partners::spans_short_units;
use crate::build::{CONSTITUENT_FILE, FILTER_FILE};

/// 登録が無い複合語に書く値。
const UNREGISTERED: &str = "未登録";

/// `artifacts_dir` の成果物で `words` を引き、1 語 1 行で書き出す。
///
/// 行は `<語>\t<バケット>\t<部品>:<出現数>:<前・外来語>:<前・漢語>:<後ろ・外来語>:<後ろ・漢語> ...`
/// である。部品は入力の語を分割単位 A で割ったもので、5 つの欄は部品の頻度表の
/// 値を復号した順に並ぶ。部品が表に無ければ、5 つとも 0 を書く。A の部品の
/// 後ろには、語を分割単位 C で割った形態素のうち A で 2 形態素以上に割れる語を、
/// 同じ形で並べる。
///
/// # Errors
///
/// 成果物を読めない場合、語を解析できない場合に返す。
pub fn run(artifacts_dir: &Path, words: &[String]) -> Result<()> {
    let filter_path = artifacts_dir.join(FILTER_FILE);
    let filter_bytes = std::fs::read(&filter_path)
        .with_context(|| format!("{} を読めない", filter_path.display()))?;
    // `from_bytes` はフィンガープリントの配列をバイト列から借りるので、2 バイト境界に
    // 揃えた写しを通して渡す。
    let aligned = AlignedBytes::new(&filter_bytes);
    let filter = FrequencyFilter::from_bytes(aligned.as_bytes(), DICTIONARY_VERSION)
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
        let long = analyze(word).with_context(|| format!("'{word}' を解析できない"))?;
        let frequencies: Vec<String> = morphemes
            .iter()
            .chain(
                long.iter()
                    .filter(|morpheme| spans_short_units(morpheme, &morphemes)),
            )
            .map(|morpheme| {
                let ConstituentEntry {
                    frequency,
                    partners,
                } = components.entry(morpheme.surface).unwrap_or_default();
                format!(
                    "{}:{frequency}:{}:{}:{}:{}",
                    morpheme.surface,
                    partners.prefix_foreign,
                    partners.prefix_kango,
                    partners.suffix_foreign,
                    partners.suffix_kango
                )
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
