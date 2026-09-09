//! 記事の `weighted_tags` にある articletopic の予測を、6 つの区分に畳む。
//!
//! Wikipedia の話題の予測は `STEM.Computing` や `Culture.Linguistics` のように
//! 階層名を持つ。`topic-distribution` サブコマンドはこの階層名を、情報技術と
//! その他の理工と文化と地理と社会とそれ以外の 6 区分に畳んでから、語ごとの
//! 記事数を数える。

use crate::dump::{self, MIN_SCORE};

/// 分野の区分。値の順は [`Category::ALL`] と表の列の並びに合わせる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// `STEM.Computing` と `STEM.Technology`。
    InformationTechnology,
    /// 情報技術を除く `STEM`。
    OtherStem,
    /// `Culture`。
    Culture,
    /// `Geography`。
    Geography,
    /// `History_and_Society`。
    Society,
    /// 上のどれにも当たらない予測の名前。
    Other,
}

/// 区分の数。[`Category::ALL`] の長さと [`categories_from_tags`] が返す配列の
/// 大きさに合わせる。
pub const CATEGORY_COUNT: usize = 6;

impl Category {
    /// 表の列の並びに揃えた全区分。
    pub const ALL: [Self; CATEGORY_COUNT] = [
        Self::InformationTechnology,
        Self::OtherStem,
        Self::Culture,
        Self::Geography,
        Self::Society,
        Self::Other,
    ];

    /// TSV と README の見出しに書く区分の名前。
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::InformationTechnology => "情報技術",
            Self::OtherStem => "その他の理工",
            Self::Culture => "文化",
            Self::Geography => "地理",
            Self::Society => "社会",
            Self::Other => "それ以外",
        }
    }

    /// [`Category::ALL`] の中でのこの区分の位置。
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|category| *category == self)
            .unwrap_or_else(|| unreachable!("Category::ALL がすべての区分を持つ"))
    }

    /// 話題の予測の名前(点数を除いた部分)を区分に畳む。
    fn from_topic_name(name: &str) -> Self {
        if name == "STEM.Computing" || name == "STEM.Technology" {
            Self::InformationTechnology
        } else if name.starts_with("STEM") {
            Self::OtherStem
        } else if name.starts_with("Culture") {
            Self::Culture
        } else if name.starts_with("Geography") {
            Self::Geography
        } else if name.starts_with("History_and_Society") {
            Self::Society
        } else {
            Self::Other
        }
    }
}

/// 記事の `weighted_tags` から、点数が [`MIN_SCORE`] 以上の予測が持つ区分を
/// 立てる。戻り値は [`Category::ALL`] と同じ並びの真偽の配列であり、記事がその
/// 区分の予測を 1 つ以上持てば真になる。
#[must_use]
pub fn categories_from_tags(tags: &[String]) -> [bool; CATEGORY_COUNT] {
    let mut present = [false; CATEGORY_COUNT];
    for tag in tags {
        if let Some((name, score)) = dump::topic_score(tag)
            && score >= MIN_SCORE
        {
            present[Category::from_topic_name(name).index()] = true;
        }
    }
    present
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 情報技術とその他の理工を分ける() {
        assert_eq!(
            Category::from_topic_name("STEM.Computing"),
            Category::InformationTechnology
        );
        assert_eq!(
            Category::from_topic_name("STEM.Technology"),
            Category::InformationTechnology
        );
        // 情報技術以外の STEM はその他の理工である。
        assert_eq!(
            Category::from_topic_name("STEM.Physics"),
            Category::OtherStem
        );
    }

    #[test]
    fn 文化と地理と社会とそれ以外を分ける() {
        assert_eq!(
            Category::from_topic_name("Culture.Linguistics"),
            Category::Culture
        );
        assert_eq!(
            Category::from_topic_name("Geography.Regions.Asia.East_Asia"),
            Category::Geography
        );
        assert_eq!(
            Category::from_topic_name("History_and_Society.Society"),
            Category::Society
        );
        // 4 つの階層のどれでもない名前はそれ以外に畳む。
        assert_eq!(
            Category::from_topic_name("Unknown.Category"),
            Category::Other
        );
    }

    #[test]
    fn 点数が500未満の予測は無視する() {
        let tags = vec!["classification.prediction.articletopic/STEM.Computing|499".to_owned()];
        assert_eq!(categories_from_tags(&tags), [false; CATEGORY_COUNT]);
    }

    #[test]
    fn 点数が500以上の予測を区分に立てる() {
        let tags = vec![
            "classification.prediction.articletopic/STEM.Computing|500".to_owned(),
            "classification.prediction.articletopic/Culture.Linguistics*|991".to_owned(),
        ];
        let mut expected = [false; CATEGORY_COUNT];
        expected[Category::InformationTechnology.index()] = true;
        expected[Category::Culture.index()] = true;
        assert_eq!(categories_from_tags(&tags), expected);
    }
}
