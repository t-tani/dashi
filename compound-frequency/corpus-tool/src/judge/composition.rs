//! 語を左右 2 つの単位に分ける分割を列挙し、単位の頻度から点数を出す。
//!
//! LLM の造語は句を複合語に圧縮して作られることが多く、`未認証のデータ` が
//! `未認証データ` になる形を取る。この形の語は、よく使う部品を新しくつないだもの
//! なので、左右の単位はどちらも高い頻度を持ち、全体だけが未登録になる。点数は
//! その形を数にしたものであり、分割ごとの `min(f(左), f(右))` の最大値である。
//!
//! 分割の単位は 1 形態素か、フィルタに登録のある複合語である。1 形態素の頻度は
//! 部品の頻度表から、複合語の回数は `count` が書いた TSV から取る。

use std::collections::HashMap;
use std::path::Path;

use aku_freq::{ConstituentFrequencies, FrequencyFilter};
use aku_morph::Morpheme;
use anyhow::Result;

use crate::count::COMPOUND_COUNTS_FILE;
use crate::tsv;

/// 分割の単位として扱う複合語の回数の下限。`build` の既定と同じ値であり、
/// これに満たない複合語はフィルタにも入らない。
const MIN_UNIT_COUNT: u64 = 3;

/// 構成の分析の点数を名前に分ける 2 つの閾値。
#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    /// これ以上の点数を造語の候補とする。
    pub upper: u64,
    /// これ以上で `upper` に満たない点数をグレーとする。
    pub lower: u64,
}

/// 語の点数と、点数に使えなかった分割の有無。
#[derive(Debug, Clone, Copy)]
pub struct Score {
    /// 分割ごとの `min(f(左), f(右))` の最大値。単位にならない分割しかなければ
    /// 0 である。
    pub best: u64,
    /// 頻度が不明な単位のために、点数に使えなかった分割があったか。
    pub unknown_unit: bool,
}

/// 分割の単位になる複合語の回数。Wikipedia の出現回数と技術文書の文書数を足した
/// 値が [`MIN_UNIT_COUNT`] 以上のキーだけを持つ。
///
/// 回数の TSV は全記事で 38,021,885 行あるが、フィルタに入るのは 9,453,107 行で
/// ある。残りを落とすことで、この表は判定の間じゅう常駐できる大きさに収まる。
pub struct UnitFrequencies {
    /// キーごとの、足した回数。
    counts: HashMap<Box<str>, u64>,
}

impl UnitFrequencies {
    /// 回数の TSV を持たない空の表を作る。構成の分析を使わない判定で渡す。
    #[must_use]
    pub fn empty() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// `counts_dir` と `docs_counts_dir` の複合語の回数を足して読む。
    ///
    /// 2 つの回数は `build` の既定の重み(どちらも 1.0)で足す。重みを変えて組んだ
    /// 成果物を判定するときは、この表と成果物のバケットが食い違う。
    ///
    /// # Errors
    ///
    /// 回数の TSV を読めない場合に返す。
    pub fn load(counts_dir: &Path, docs_counts_dir: Option<&Path>) -> Result<Self> {
        let mut documents: HashMap<String, u64> = HashMap::new();
        if let Some(dir) = docs_counts_dir {
            documents = tsv::read(&dir.join(COMPOUND_COUNTS_FILE))?
                .into_iter()
                .collect();
        }
        let mut counts: HashMap<Box<str>, u64> = HashMap::new();
        tsv::for_each_row(&counts_dir.join(COMPOUND_COUNTS_FILE), |key, count| {
            let total = count + documents.get(key).copied().unwrap_or(0);
            if total >= MIN_UNIT_COUNT {
                counts.insert(key.into(), total);
            }
        })?;
        // Wikipedia に無く、技術文書にだけあるキー。
        for (key, count) in documents {
            if count >= MIN_UNIT_COUNT {
                counts.entry(key.into_boxed_str()).or_insert(count);
            }
        }
        Ok(Self { counts })
    }

    /// キーと回数の並びから表を作る。
    #[cfg(test)]
    #[must_use]
    pub fn from_rows(rows: &[(&str, u64)]) -> Self {
        Self {
            counts: rows
                .iter()
                .map(|(key, count)| ((*key).into(), *count))
                .collect(),
        }
    }

    /// `key` の回数。[`MIN_UNIT_COUNT`] に満たないキーは持たないので、`None` は
    /// 回数が下限に届かないことも表す。
    fn count(&self, key: &str) -> Option<u64> {
        self.counts.get(key).copied()
    }
}

/// 1 つの分割の単位の頻度。
enum UnitFrequency {
    /// 頻度がわかる単位。
    Known(u64),
    /// フィルタに登録はあるが、回数の TSV に無い単位。解析辞書の見出しがこれに
    /// あたり、頻度がわからない。
    Unknown,
    /// 分割の単位にならない連なり。
    Absent,
}

/// `morphemes` の語を左右 2 つの単位に分けるすべての分割から、点数を出す。
#[must_use]
pub fn score(
    morphemes: &[Morpheme<'_>],
    units: &UnitFrequencies,
    constituents: &ConstituentFrequencies,
    filter: &FrequencyFilter<'_>,
) -> Score {
    let mut best = 0;
    let mut unknown_unit = false;
    for split in 1..morphemes.len() {
        let left = unit_frequency(&morphemes[..split], units, constituents, filter);
        let right = unit_frequency(&morphemes[split..], units, constituents, filter);
        match (left, right) {
            (UnitFrequency::Known(left), UnitFrequency::Known(right)) => {
                best = best.max(left.min(right));
            }
            (UnitFrequency::Absent, _) | (_, UnitFrequency::Absent) => {}
            _ => unknown_unit = true,
        }
    }
    Score { best, unknown_unit }
}

/// 1 つの単位の頻度を引く。
///
/// 1 形態素の単位は部品の頻度表から引く。`count` が部品を表層形で数えているので、
/// 表層形だけで引く。2 形態素以上の単位は回数の TSV から引き、表層形を連ねた
/// キーで当たらなければ正規化形を連ねたキーで引く。
fn unit_frequency(
    unit: &[Morpheme<'_>],
    units: &UnitFrequencies,
    constituents: &ConstituentFrequencies,
    filter: &FrequencyFilter<'_>,
) -> UnitFrequency {
    if let [morpheme] = unit {
        return UnitFrequency::Known(constituents.frequency(morpheme.surface).unwrap_or(0));
    }
    let surface: String = unit.iter().map(|morpheme| morpheme.surface).collect();
    let normalized: String = unit.iter().map(Morpheme::normalized_form).collect();
    if let Some(count) = units.count(&surface).or_else(|| units.count(&normalized)) {
        return UnitFrequency::Known(count);
    }
    if filter
        .bucket(&surface)
        .or_else(|| filter.bucket(&normalized))
        .is_some()
    {
        return UnitFrequency::Unknown;
    }
    UnitFrequency::Absent
}
