//! 分野の区分と、分野ごとに数えたキーの回数。
//!
//! 区分は 7 つの分野と、分野をまたいで現れる語の「一般」からなる。Wikipedia の
//! 記事は話題の予測から分野を決め、1 記事が複数の分野を持つことを許す。技術文書
//! は情報技術とし、メンテナーが集めた日本語コーパスはソースの機関と文体で決める。
//!
//! 番号は `domain_filter.bin` のキーに書く値であり、成果物の manifest が名前との
//! 対応を持つ。番号を変えると、それまでに作ったフィルタは別の分野を指す。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::topic::{self, Category};

/// 分野の区分。値の順は [`Domain::ALL`] と番号に合わせる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// `STEM.Computing` と `STEM.Technology`、技術文書、情報技術の機関の文書。
    InformationTechnology,
    /// 情報技術を除く `STEM`。
    OtherStem,
    /// `Culture`。
    Culture,
    /// `Geography`。
    Geography,
    /// `History_and_Society`。
    Society,
    /// 官公庁の説明文と指示文。
    Administration,
    /// 法令文。
    Law,
    /// 登録される分野の数が閾値以上の語。分野ごとの登録の代わりに置く。
    General,
}

/// 区分の数。[`Domain::ALL`] の長さである。
pub const DOMAIN_COUNT: usize = 8;

/// 分野の番号と語を分ける文字。複合語のキーは名詞と接辞の表層形からできており、
/// タブを含まない。
const KEY_SEPARATOR: char = '\t';

/// 情報技術の文書を出す機関。日本語コーパスのソース名の、最初の `-` までが
/// この一覧にあれば情報技術とする。
const TECHNOLOGY_AGENCIES: [&str; 5] = ["ipa", "jpccert", "jvn", "nco", "nicter"];

/// 法令の文体。日本語コーパスの記録が文書ごとに持つ値である。
const LAW_STYLE: &str = "法令文";

impl Domain {
    /// 番号の順に並べた全区分。
    pub const ALL: [Self; DOMAIN_COUNT] = [
        Self::InformationTechnology,
        Self::OtherStem,
        Self::Culture,
        Self::Geography,
        Self::Society,
        Self::Administration,
        Self::Law,
        Self::General,
    ];

    /// フィルタのキーに書く番号。1 から始まり、[`Domain::ALL`] の並びに合わせる。
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::InformationTechnology => 1,
            Self::OtherStem => 2,
            Self::Culture => 3,
            Self::Geography => 4,
            Self::Society => 5,
            Self::Administration => 6,
            Self::Law => 7,
            Self::General => 8,
        }
    }

    /// TSV と manifest と README に書く区分の名前。
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::InformationTechnology => "情報技術",
            Self::OtherStem => "その他の理工",
            Self::Culture => "文化",
            Self::Geography => "地理",
            Self::Society => "社会",
            Self::Administration => "行政",
            Self::Law => "法令",
            Self::General => "一般",
        }
    }

    /// 名前から区分を戻す。名前のどれでもなければ `None` を返す。
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|domain| domain.label() == label)
    }

    /// 話題の予測を畳んだ区分から分野を決める。どの分野にも当たらない予測は
    /// `None` を返す。
    fn from_category(category: Category) -> Option<Self> {
        match category {
            Category::InformationTechnology => Some(Self::InformationTechnology),
            Category::OtherStem => Some(Self::OtherStem),
            Category::Culture => Some(Self::Culture),
            Category::Geography => Some(Self::Geography),
            Category::Society => Some(Self::Society),
            Category::Other => None,
        }
    }

    /// 日本語コーパスのソースの分野。法令文のソースは法令、情報技術の機関の
    /// ソースは情報技術、残る官公庁のソースは行政である。
    #[must_use]
    pub fn from_corpus_source(name: &str, styles: &[String]) -> Self {
        if styles.iter().any(|style| style == LAW_STYLE) {
            Self::Law
        } else if TECHNOLOGY_AGENCIES.contains(&agency(name)) {
            Self::InformationTechnology
        } else {
            Self::Administration
        }
    }
}

/// ソース名が指す機関。名前の最初の `-` までである。
fn agency(name: &str) -> &str {
    name.split('-').next().unwrap_or(name)
}

/// 記事の `weighted_tags` から、その記事が持つ分野を集める。話題の予測は
/// [`crate::topic`] が畳み、点数の下限もそちらが当てる。
#[must_use]
pub fn domains_from_tags(tags: &[String]) -> DomainSet {
    let present = topic::categories_from_tags(tags);
    let mut domains = DomainSet::default();
    for category in Category::ALL {
        if present[category.index()]
            && let Some(domain) = Domain::from_category(category)
        {
            domains.insert(domain);
        }
    }
    domains
}

/// 分野の集合。区分ごとの 1 bit を持つ。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DomainSet(u8);

impl DomainSet {
    /// 1 つの分野だけを持つ集合。
    #[must_use]
    pub fn only(domain: Domain) -> Self {
        let mut set = Self::default();
        set.insert(domain);
        set
    }

    /// `domain` を足す。
    pub fn insert(&mut self, domain: Domain) {
        self.0 |= 1 << (domain.code() - 1);
    }

    /// `domain` を持つか。
    #[must_use]
    pub fn contains(self, domain: Domain) -> bool {
        self.0 & (1 << (domain.code() - 1)) != 0
    }

    /// 1 つも持たないか。
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 持っている分野の数。
    #[must_use]
    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// 持っている分野を番号の順に並べる。
    pub fn iter(self) -> impl Iterator<Item = Domain> {
        Domain::ALL
            .into_iter()
            .filter(move |domain| self.contains(*domain))
    }

    /// 持っている分野の名前を `・` で連ねたもの。判定の理由に書く。
    #[must_use]
    pub fn labels(self) -> String {
        self.iter()
            .map(Domain::label)
            .collect::<Vec<_>>()
            .join("・")
    }
}

/// 分野ごとにキーを数えるか。数える場合、`count` は語と分野の組の回数を
/// `domain-counts.tsv` へ書く。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DomainCounting {
    /// 数えない。
    #[default]
    Skip,
    /// 数える。
    Count,
}

impl DomainCounting {
    /// 数えるか。
    #[must_use]
    pub fn counts(self) -> bool {
        self == Self::Count
    }

    /// 引数の真偽値から設定を作る。
    #[must_use]
    pub fn from_counted(counted: bool) -> Self {
        if counted { Self::Count } else { Self::Skip }
    }
}

/// 語と分野の組ごとの回数。キーは [`count_key`] の `<語><タブ><分野の番号>` で
/// ある。
///
/// 数えるのは、その分野でキーが現れた記事か文書の数である。1 記事に何度現れても
/// 1 回とするので、呼び出し側は 1 記事分のキーの集合を渡す。
#[derive(Default)]
pub struct DomainCounts {
    /// キーごとの回数。
    pub keys: HashMap<Box<str>, u32>,
    /// キーを組み立てる緩衝。キーごとに確保し直さないために使い回す。
    buffer: String,
}

impl DomainCounts {
    /// `keys` の各キーを、`domains` のそれぞれについて 1 回数える。
    pub fn add_keys<'k>(&mut self, domains: DomainSet, keys: impl Iterator<Item = &'k str>) {
        for key in keys {
            for domain in domains.iter() {
                self.buffer.clear();
                push_count_key(&mut self.buffer, key, domain);
                if let Some(count) = self.keys.get_mut(self.buffer.as_str()) {
                    *count = count.saturating_add(1);
                } else {
                    self.keys.insert(self.buffer.as_str().into(), 1);
                }
            }
        }
    }

    /// `other` の回数を足し込む。
    pub fn merge(&mut self, other: Self) {
        for (key, count) in other.keys {
            self.keys
                .entry(key)
                .and_modify(|total| *total = total.saturating_add(count))
                .or_insert(count);
        }
    }

    /// 持っているキーの数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

/// 分野ごとに数えた記事か文書の数。`count` の記録に書き、成果物の manifest へ
/// 写す。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainTally {
    /// 分野の名前。
    pub domain: String,
    /// その分野で数えた記事か文書の数。
    pub documents: u64,
}

/// 分野ごとに数えた記事か文書の数を、番号の順に並べる。`tallies` は
/// [`Domain::ALL`] と同じ並びの数である。
#[must_use]
pub fn tallies(counts: &[u64; DOMAIN_COUNT]) -> Vec<DomainTally> {
    Domain::ALL
        .into_iter()
        .zip(counts)
        .filter(|(_, documents)| **documents > 0)
        .map(|(domain, documents)| DomainTally {
            domain: domain.label().to_owned(),
            documents: *documents,
        })
        .collect()
}

/// 語と分野の組のキー。`domain_filter.bin` の照合と構築が使う。
#[must_use]
pub fn key(domain: Domain, word: &str) -> String {
    format!("{}{KEY_SEPARATOR}{word}", domain.code())
}

/// 回数の TSV が持つ、語と分野の組のキーを `buffer` の後ろへ書く。
///
/// 分野の番号を語の後ろに置くのは、キーの昇順に並べると 1 つの語の行が隣り合う
/// ためである。`build` はその並びを読み進めて語ごとの総数を出せるので、数千万件
/// の組を表に載せずに分野の割合を求められる。
///
/// キーごとに `String` を確保し直さないために、緩衝を受け取る形にしてある。
/// 数える側は 1 記事につきキーの数だけこれを呼ぶ。
pub fn push_count_key(buffer: &mut String, word: &str, domain: Domain) {
    buffer.push_str(word);
    buffer.push(KEY_SEPARATOR);
    buffer.push(char::from(b'0' + domain.code()));
}

/// 回数の TSV のキーから語と分野を戻す。分野の番号でない末尾を持つキーでは
/// `None` を返す。
///
/// 語が空のキーも返す。正規化形が空になる形態素があり、それだけを連ねた区間は
/// 空のキーとして数えられているためである。
#[must_use]
pub fn split_count_key(key: &str) -> Option<(&str, Domain)> {
    let (word, code) = key.rsplit_once(KEY_SEPARATOR)?;
    let code: u8 = code.parse().ok()?;
    let domain = *Domain::ALL.get(usize::from(code.checked_sub(1)?))?;
    Some((word, domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 分野の番号は1から始まる() {
        assert_eq!(Domain::InformationTechnology.code(), 1);
        assert_eq!(Domain::Law.code(), 7);
        assert_eq!(Domain::General.code(), 8);
        // 番号は ALL の並びの位置に合わせる。片方だけを変えると、キーの番号と
        // 名前の対応がずれる。
        for (index, domain) in Domain::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(domain.code()), index + 1);
        }
        // 名前と番号は往復する。
        for domain in Domain::ALL {
            assert_eq!(Domain::from_label(domain.label()), Some(domain));
        }
        assert_eq!(Domain::from_label("情報"), None);
    }

    #[test]
    fn 記事の話題の予測を分野に畳む() {
        let tags = vec![
            "classification.prediction.articletopic/STEM.Computing|803".to_owned(),
            "classification.prediction.articletopic/Culture.Linguistics*|538".to_owned(),
            // 点数が下限に届かない予測は分野にならない。
            "classification.prediction.articletopic/Geography.Regions|499".to_owned(),
        ];
        let domains = domains_from_tags(&tags);
        assert!(domains.contains(Domain::InformationTechnology));
        assert!(domains.contains(Domain::Culture));
        assert!(!domains.contains(Domain::Geography));
        assert_eq!(domains.len(), 2);
        // 予測を持たない記事はどの分野にも入らない。
        assert!(domains_from_tags(&[]).is_empty());
    }

    #[test]
    fn 日本語コーパスのソースを機関と文体で分ける() {
        let explanation = vec!["説明文".to_owned()];
        let law = vec!["法令文".to_owned()];
        // 情報技術の機関の文書は、文体によらず情報技術である。
        assert_eq!(
            Domain::from_corpus_source("jpccert-eyes", &explanation),
            Domain::InformationTechnology
        );
        assert_eq!(
            Domain::from_corpus_source("nco-standards", &["指示文".to_owned()]),
            Domain::InformationTechnology
        );
        // 法令文のソースは法令、残る官公庁のソースは行政である。
        assert_eq!(
            Domain::from_corpus_source("egov-law-api", &law),
            Domain::Law
        );
        assert_eq!(
            Domain::from_corpus_source("board-of-audit", &explanation),
            Domain::Administration
        );
    }

    #[test]
    fn 分野の集合は番号の順に並ぶ() {
        let mut set = DomainSet::default();
        set.insert(Domain::Culture);
        set.insert(Domain::InformationTechnology);
        assert_eq!(set.labels(), "情報技術・文化");
        assert_eq!(set.len(), 2);
        // 同じ分野を 2 度足しても増えない。
        set.insert(Domain::Culture);
        assert_eq!(set.len(), 2);
        assert!(DomainSet::only(Domain::Law).contains(Domain::Law));
    }

    #[test]
    fn 分野ごとに記事の数だけキーを数える() {
        let mut counts = DomainCounts::default();
        let mut both = DomainSet::only(Domain::InformationTechnology);
        both.insert(Domain::Culture);
        counts.add_keys(both, ["公開鍵暗号", "電子署名"].into_iter());
        // 2 つの分野を持つ記事は、両方の分野で 1 回数える。
        assert_eq!(counts.keys.get("公開鍵暗号\t1").copied(), Some(1));
        assert_eq!(counts.keys.get("公開鍵暗号\t3").copied(), Some(1));
        // 別の記事の分は足し込む。
        let mut next = DomainCounts::default();
        next.add_keys(
            DomainSet::only(Domain::InformationTechnology),
            ["公開鍵暗号"].into_iter(),
        );
        counts.merge(next);
        assert_eq!(counts.keys.get("公開鍵暗号\t1").copied(), Some(2));
        assert_eq!(counts.keys.get("電子署名\t3").copied(), Some(1));
    }

    #[test]
    fn キーは分野と語に割れる() {
        assert_eq!(key(Domain::Law, "完了条件"), "7\t完了条件");
        let mut count_key = String::new();
        push_count_key(&mut count_key, "完了条件", Domain::Law);
        assert_eq!(count_key, "完了条件\t7");
        assert_eq!(split_count_key(&count_key), Some(("完了条件", Domain::Law)));
        // 分野の番号でない末尾のキーは読めない。
        assert_eq!(split_count_key("完了条件\t0"), None);
        assert_eq!(split_count_key("完了条件\t9"), None);
        assert_eq!(split_count_key("完了条件"), None);
        // 語が空のキーは読める。正規化形が空になる形態素だけの区間が、この形で
        // 数えられている。
        assert_eq!(split_count_key("\t7"), Some(("", Domain::Law)));
    }

    #[test]
    fn 回数のキーは語ごとに隣り合う() {
        // タブは語を成す文字より小さいので、キーの昇順では 1 つの語の行が続く。
        let mut keys: Vec<String> = [
            ("公開鍵", Domain::Culture),
            ("公開鍵暗号", Domain::InformationTechnology),
            ("公開鍵", Domain::InformationTechnology),
        ]
        .into_iter()
        .map(|(word, domain)| {
            let mut key = String::new();
            push_count_key(&mut key, word, domain);
            key
        })
        .collect();
        keys.sort();
        assert_eq!(keys, ["公開鍵\t1", "公開鍵\t3", "公開鍵暗号\t1"]);
    }
}
