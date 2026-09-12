//! 複合語の部品ごとに、前後に付く相手の種類数を数える。
//!
//! 数えるのはフィルタに入れる複合語のキーだけで、回数は見ない。種類数は部品の
//! 頻度表の値に載り、検査する側が、複合語そのものがフィルタに無いときに、その
//! 部品がその位置でどれだけ多くの相手と結び付いているかの根拠に使う。
//!
//! 見出しは 2 種類ある。分割単位 A の部品と、分割単位 C で 1 語になり分割単位 A
//! で 2 形態素以上に割れる語([`spans_short_units`])である。検査する側は
//! 分割単位 C で解析するので、「母集団レビュー」の境界の形態素は「母集団」で
//! ある。A の「集団」で引くと、後ろに外来語が付く種類数は「集団」の分だけ多く、
//! 既知語に倒れる。「母集団」を見出しに持てば、その語の後ろに付く相手の種類数で
//! 判定できる。

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use aku_freq::PartnerCounts;
use aku_morph::{Morpheme, analyze, analyze_short};
use anyhow::{Context, Result};
use rayon::prelude::*;
use regex::Regex;

/// 漢字(Han スクリプト)の文字。akunuki の `unknown-compound` が漢字を含む
/// 形態素を数える判定と同じ文字集合である。
static KANJI: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"\p{Han}").expect("固定パターンのコンパイルに失敗しない")
});

/// ラテン文字とカタカナだけからなる表層形。長音符はカタカナの語の一部だが
/// Script が共通なので、明示して足す。akunuki の `unknown-compound` と同じ
/// パターンである。
static FOREIGN_SURFACE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^[A-Za-z\p{Katakana}ー]+$").expect("固定パターンのコンパイルに失敗しない")
});

/// 相手の種類。相手の表層形の字種で決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PartnerKind {
    /// ラテン文字かカタカナだけの表層形。
    Foreign,
    /// 漢字を含む表層形。
    Kango,
}

/// 部品からみた相手の位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Position {
    /// 相手が部品の前に付く。
    Prefix,
    /// 相手が部品の後ろに付く。
    Suffix,
}

/// 見出しごとの相手の種類数と、見出しのうち分割単位 C の語の集合。
#[derive(Default)]
pub struct Partners {
    /// 見出しごとの相手の種類数。見出しは分割単位 A の部品と、[`Self::long_units`]
    /// の語である。
    pub counts: HashMap<String, PartnerCounts>,
    /// 分割単位 C で 1 語になり、分割単位 A で 2 形態素以上に割れる語。部品の
    /// 回数の TSV には無いことがあるので、頻度表に載せるときに部品と分けて扱う。
    pub long_units: HashSet<String>,
}

impl Partners {
    /// `other` の種類数と語を足し込む。
    fn merge(mut self, other: Self) -> Self {
        for (part, counts) in other.counts {
            let total = self.counts.entry(part).or_default();
            total.prefix_foreign += counts.prefix_foreign;
            total.prefix_kango += counts.prefix_kango;
            total.suffix_foreign += counts.suffix_foreign;
            total.suffix_kango += counts.suffix_kango;
        }
        self.long_units.extend(other.long_units);
        self
    }
}

/// `compounds` の各複合語を分割単位 A で形態素に割り、部品ごとの相手の種類数を
/// 数える。返す表のキーは部品の表層形である。
///
/// 隣り合う形態素の対ごとに、左の部品には後ろに相手が付いたと、右の部品には前に
/// 相手が付いたと数える。相手の種類は相手の表層形で決め、ラテン文字かカタカナ
/// だけなら外来語、漢字を含めば漢語とし、どちらでもない相手は数えない。同じ
/// 複合語は 1 種類 1 と数え、3 形態素以上の複合語は隣り合う対をすべて数える。
///
/// 同じ複合語を分割単位 C でも割り、分割単位 A で 2 形態素以上に割れる語
/// ([`spans_short_units`])があれば、その語を見出しにして、C の形態素列の
/// 隣り合う対から同じ規則で相手の種類数を数える。C の形態素のうち A の 1 形態素
/// と同じ語は、A の側で数えているので数えない。語全体が C の 1 形態素になる
/// 複合語(`公開鍵暗号`)には相手が無いので、見出しを足さない。
///
/// # Errors
///
/// 複合語を解析できない場合に返す。
pub fn count(compounds: &[&str]) -> Result<Partners> {
    compounds
        .par_iter()
        .try_fold(Partners::default, |mut tally, compound| {
            let short =
                analyze_short(compound).with_context(|| format!("'{compound}' を解析できない"))?;
            let surfaces: Vec<&str> = short.iter().map(|morpheme| morpheme.surface).collect();
            add_surfaces(&mut tally.counts, &surfaces, |_| true);
            let long = analyze(compound).with_context(|| format!("'{compound}' を解析できない"))?;
            let spans: Vec<bool> = long
                .iter()
                .map(|morpheme| spans_short_units(morpheme, &short))
                .collect();
            // 語全体が C の 1 形態素なら相手が無いので、見出しにしない。
            if long.len() >= 2 && spans.contains(&true) {
                let surfaces: Vec<&str> = long.iter().map(|morpheme| morpheme.surface).collect();
                add_surfaces(&mut tally.counts, &surfaces, |index| spans[index]);
                for (surface, spans) in surfaces.iter().zip(&spans) {
                    if *spans {
                        tally.long_units.insert((*surface).to_owned());
                    }
                }
            }
            Ok(tally)
        })
        .try_reduce(Partners::default, |left, right| Ok(left.merge(right)))
}

/// 分割単位 C の形態素 `long` が、同じ文字列を分割単位 A で割った `short` の
/// 2 形態素以上にまたがるか。C の形態素は A の形態素を連ねた範囲を持つので、
/// A のどの 1 形態素とも範囲が一致しなければ 2 形態素以上にまたがる。
pub fn spans_short_units(long: &Morpheme<'_>, short: &[Morpheme<'_>]) -> bool {
    !short.iter().any(|morpheme| morpheme.range == long.range)
}

/// 1 つの複合語の形態素の表層形 `surfaces` から、隣り合う対を `tally` に数える。
/// 同じ部品に同じ位置・種類の相手が 2 度付いても、1 つの複合語では 1 と数える。
/// 数えるのは `counted` が真を返す位置の部品だけで、相手はどの位置でもよい。
fn add_surfaces(
    tally: &mut HashMap<String, PartnerCounts>,
    surfaces: &[&str],
    counted: impl Fn(usize) -> bool,
) {
    let mut found: Vec<(&str, Position, PartnerKind)> = Vec::new();
    for (index, pair) in surfaces.windows(2).enumerate() {
        if counted(index)
            && let Some(kind) = partner_kind(pair[1])
        {
            found.push((pair[0], Position::Suffix, kind));
        }
        if counted(index + 1)
            && let Some(kind) = partner_kind(pair[0])
        {
            found.push((pair[1], Position::Prefix, kind));
        }
    }
    found.sort_unstable();
    found.dedup();
    for (part, position, kind) in found {
        let counts = if let Some(counts) = tally.get_mut(part) {
            counts
        } else {
            tally.entry(part.to_owned()).or_default()
        };
        *field(counts, position, kind) += 1;
    }
}

/// `surface` の字種で決まる相手の種類。ラテン文字かカタカナだけなら外来語、
/// 漢字を含めば漢語、どちらでもなければ `None` を返す。
fn partner_kind(surface: &str) -> Option<PartnerKind> {
    if FOREIGN_SURFACE.is_match(surface) {
        Some(PartnerKind::Foreign)
    } else if KANJI.is_match(surface) {
        Some(PartnerKind::Kango)
    } else {
        None
    }
}

/// 相手の位置と種類が指す欄。
fn field(counts: &mut PartnerCounts, position: Position, kind: PartnerKind) -> &mut u32 {
    match (position, kind) {
        (Position::Prefix, PartnerKind::Foreign) => &mut counts.prefix_foreign,
        (Position::Prefix, PartnerKind::Kango) => &mut counts.prefix_kango,
        (Position::Suffix, PartnerKind::Foreign) => &mut counts.suffix_foreign,
        (Position::Suffix, PartnerKind::Kango) => &mut counts.suffix_kango,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 つの欄を並べた組。
    fn counts(
        prefix_foreign: u32,
        prefix_kango: u32,
        suffix_foreign: u32,
        suffix_kango: u32,
    ) -> PartnerCounts {
        PartnerCounts {
            prefix_foreign,
            prefix_kango,
            suffix_foreign,
            suffix_kango,
        }
    }

    #[test]
    fn 相手の種類は表層形の字種で決まる() {
        assert_eq!(partner_kind("ビット"), Some(PartnerKind::Foreign));
        assert_eq!(partner_kind("CNAME"), Some(PartnerKind::Foreign));
        // 長音符はカタカナの語の一部である。
        assert_eq!(partner_kind("レコード"), Some(PartnerKind::Foreign));
        assert_eq!(partner_kind("演算子"), Some(PartnerKind::Kango));
        // 送りがなが付いても、漢字を含めば漢語である。
        assert_eq!(partner_kind("宙吊り"), Some(PartnerKind::Kango));
        // ひらがなだけの相手は、どちらでもない。
        assert_eq!(partner_kind("ゆとり"), None);
    }

    #[test]
    fn 相手の種類数を位置と種類で数える() {
        // 分割単位 A では 演算子 が 演算 と 子 に割れる。
        let table = count(&["ビット演算子", "シフト演算子", "論理演算子"])
            .unwrap()
            .counts;
        // 演算 の前に外来語が 2 種類、漢語が 1 種類付く。後ろの 子 は同じ相手
        // だが、複合語ごとに数えるので 3 種類である。
        assert_eq!(table.get("演算"), Some(&counts(2, 1, 0, 3)));
        assert_eq!(table.get("子"), Some(&counts(0, 3, 0, 0)));
        // ビット の後ろには漢語の 演算 が付く。
        assert_eq!(table.get("ビット"), Some(&counts(0, 0, 0, 1)));
        assert_eq!(table.get("論理"), Some(&counts(0, 0, 0, 1)));
    }

    #[test]
    fn 長単位で1語になる語を見出しに足す() {
        // 分割単位 A では 母集団 が 母 と 集団 に割れ、分割単位 C では 1 語になる。
        let partners = count(&["母集団レビュー", "母集団分布"]).unwrap();
        assert_eq!(partners.long_units, HashSet::from(["母集団".to_owned()]));
        // C の見出し 母集団 の後ろには、外来語と漢語が 1 種類ずつ付く。
        assert_eq!(partners.counts.get("母集団"), Some(&counts(0, 0, 1, 1)));
        // A の部品はこれまでどおり数える。集団 の前には 2 つの複合語で漢語の
        // 母 が付くので、種類数は 2 である。
        assert_eq!(partners.counts.get("集団"), Some(&counts(0, 2, 1, 1)));
        // C でも A の 1 形態素と同じ レビュー は、A の側でだけ数える。
        assert_eq!(partners.counts.get("レビュー"), Some(&counts(0, 1, 0, 0)));
    }

    #[test]
    fn 長単位の見出しが要らない語には足さない() {
        // ゆとり教育 は分割単位 A でも C でも ゆとり と 教育 に割れる。
        let partners = count(&["ゆとり教育"]).unwrap();
        assert!(partners.long_units.is_empty());
        // 公開鍵暗号 は分割単位 C で 1 形態素になり、相手が無い。
        let partners = count(&["公開鍵暗号"]).unwrap();
        assert!(partners.long_units.is_empty());
        assert_eq!(partners.counts.get("公開鍵暗号"), None);
    }

    #[test]
    fn 三形態素以上は隣り合う対をすべて数える() {
        let table = count(&["公開鍵暗号"]).unwrap().counts;
        assert_eq!(table.get("公開"), Some(&counts(0, 0, 0, 1)));
        // 真ん中の 鍵 は、前にも後ろにも漢語が付く。
        assert_eq!(table.get("鍵"), Some(&counts(0, 1, 0, 1)));
        assert_eq!(table.get("暗号"), Some(&counts(0, 1, 0, 0)));
    }

    #[test]
    fn どちらでもない相手は数えない() {
        let table = count(&["ゆとり教育"]).unwrap().counts;
        // ゆとり の後ろの 教育 は漢語なので数える。
        assert_eq!(table.get("ゆとり"), Some(&counts(0, 0, 0, 1)));
        // 教育 の前の ゆとり はひらがなだけなので、教育 に欄が立たない。
        assert_eq!(table.get("教育"), None);
    }

    #[test]
    fn 同じ複合語の中の同じ相手は1と数える() {
        let mut tally = HashMap::new();
        add_surfaces(&mut tally, &["鍵", "鍵", "鍵"], |_| true);
        // 対は 2 つあるが、鍵 の前の漢語も後ろの漢語も 1 つの複合語では 1 である。
        assert_eq!(tally.get("鍵"), Some(&counts(0, 1, 0, 1)));
        // 1 形態素の複合語には対が無い。
        add_surfaces(&mut tally, &["kubectl"], |_| true);
        assert_eq!(tally.len(), 1);
    }

    #[test]
    fn 数えない位置の部品には欄が立たない() {
        let mut tally = HashMap::new();
        add_surfaces(&mut tally, &["母集団", "レビュー"], |index| {
            index == 0
        });
        assert_eq!(tally.get("母集団"), Some(&counts(0, 0, 1, 0)));
        assert_eq!(tally.get("レビュー"), None);
    }
}
