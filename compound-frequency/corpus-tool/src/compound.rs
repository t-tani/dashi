//! 本文から複合語の候補と部品を切り出し、回数を数える。
//!
//! 切り出しは分割単位 A の形態素列に対して行う。akunuki が検査でキーを作るのと
//! 同じ解析([`analyze_short`])で割らなければ、数えたキーは検査する側の候補と
//! 当たらない。
//!
//! 区間は原文で隣り合う形態素だけからなるが、1 つだけ例外がある。漢字を含む
//! 名詞とラテン文字だけの名詞が対をなす箇所は、間に空白 1 つ(U+0020)を
//! 挟んでも区間を切らない。akunuki の `unknown-compound` が「読み取り専用 API」
//! の形をこの規則で連ね、空白を除いたキーでフィルタを引くので、数える側が同じ
//! 規則で数えなければそのキーはフィルタに載らない。対の判定は akunuki の
//! [`bridges_kanji_latin_pair`] をそのまま呼び、ここには写しを持たない。
//!
//! ラテン文字だけの名詞を含む区間は、全体と隣り合う対に加えて、ラテン文字の
//! 形態素で区切った日本語だけの部分区間も、それぞれ独立の区間として数える。
//! 検査する側がラテン文字で区切った漢語の部分列を別に判定するので、数える側も
//! 同じキーを持たなければ当たらない。

use std::collections::HashMap;
use std::sync::LazyLock;

use aku_morph::{MorphError, Morpheme, PartOfSpeech, analyze_short, bridges_kanji_latin_pair};
use regex::Regex;

/// 解析器へ 1 回に渡す最大バイト数。解析器自身の上限より十分小さく取り、句点と
/// 改行で切った文がこの長さを超えた場合だけ、読点と固定長でさらに切る。
const MAX_ANALYSIS_BYTES: usize = 8192;

/// 全体を 1 つの複合語として数える区間の、最大の形態素数。これより長い区間は
/// 隣り合う 2 形態素の対だけを数える。
const MAX_SPAN_MORPHEMES: usize = 5;

/// ラテン文字だけからなる表層形。akunuki の `unknown-compound` が、漢語の
/// 部分列を区切るラテン文字の名詞を見る条件と同じパターンである。
static LATIN_SURFACE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^[A-Za-z]+$").expect("固定パターンのコンパイルに失敗しない")
});

/// 解析辞書に無いカタカナと英字の 1 形態素を、複合語のキーとして数えるか。
///
/// 解析辞書に無いカタカナと英字の連なりは、未知語として 1 形態素にまとまる。
/// `kubectl` がこれにあたり、2 形態素以上という区間の条件から落ちる。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UnknownMorphemes {
    /// 数えない。複合語のキーは 2 形態素以上の区間だけから作る。
    #[default]
    Skip,
    /// 未知語 1 形態素も 1 つのキーとして数える。
    Count,
}

impl UnknownMorphemes {
    /// 記録に書く真偽値。
    #[must_use]
    pub fn counted(self) -> bool {
        self == Self::Count
    }

    /// 記録の真偽値から設定を戻す。
    #[must_use]
    pub fn from_counted(counted: bool) -> Self {
        if counted { Self::Count } else { Self::Skip }
    }
}

/// 複合語と部品の回数。
#[derive(Default)]
pub struct Counts {
    /// 複合語のキーごとの回数。キーは表層形を連ねた文字列と、正規化形を連ねた
    /// 文字列である。
    pub compounds: HashMap<Box<str>, u32>,
    /// 部品の表層形ごとの回数。複合語の区間に現れた形態素だけを数える。
    pub components: HashMap<Box<str>, u32>,
    /// 未知語 1 形態素をキーにするか。
    unknown: UnknownMorphemes,
}

impl Counts {
    /// 未知語 1 形態素の扱いを決めて、空の回数を作る。
    #[must_use]
    pub fn new(unknown: UnknownMorphemes) -> Self {
        Self {
            unknown,
            ..Self::default()
        }
    }

    /// `text` を文に切って解析し、複合語と部品の回数を足す。
    ///
    /// # Errors
    ///
    /// 解析辞書を読み込めない場合、解析器が入力を分けられない場合に返す。
    pub fn add_text(&mut self, text: &str) -> Result<(), MorphError> {
        for chunk in analysis_chunks(text) {
            let morphemes = analyze_short(chunk)?;
            self.add_morphemes(chunk, &morphemes);
        }
        Ok(())
    }

    /// `other` の回数を足し込む。
    pub fn merge(&mut self, other: Self) {
        for (table, source) in [
            (&mut self.compounds, other.compounds),
            (&mut self.components, other.components),
        ] {
            for (key, count) in source {
                table
                    .entry(key)
                    .and_modify(|total| *total = total.saturating_add(count))
                    .or_insert(count);
            }
        }
    }

    /// キーごとの回数を 1 にする。1 文書分の回数にこれを当ててから足し込めば、
    /// 足した先はキーが現れた文書の数を数える。
    ///
    /// 技術文書はナビゲーションやライセンスの定型文を文書ごとに繰り返すので、
    /// 出現回数で数えると定型文の複合語が高い回数を持つ。
    pub fn count_once_per_key(&mut self) {
        for table in [&mut self.compounds, &mut self.components] {
            for count in table.values_mut() {
                *count = 1;
            }
        }
    }

    /// 形態素列から、複合語を成す品詞が原文で続く区間を切り出して数える。
    ///
    /// 区間は、複合語を成さない品詞に当たったところと、形態素が原文で隣り合わなく
    /// なったところで切れる。解析は空白を形態素にしないので、空白での切れ目は
    /// 形態素のバイト範囲が続くかどうかに現れる。ただし、漢字を含む名詞と
    /// ラテン文字名詞の対([`bridges_kanji_latin_pair`])だけは、間の空白
    /// 1 つを跨いでも区間を続ける。区間のキーは表層形と正規化形を連ねた文字列
    /// なので、跨いだ空白はキーに残らない。`text` は `morphemes` を解析した
    /// 原文で、跨ぐ空白を確かめるのに使う。
    fn add_morphemes(&mut self, text: &str, morphemes: &[Morpheme<'_>]) {
        let mut start: Option<usize> = None;
        for (index, morpheme) in morphemes.iter().enumerate() {
            if !is_compound_part(&morpheme.part_of_speech) {
                if let Some(begin) = start.take() {
                    self.add_span(&morphemes[begin..index]);
                }
                continue;
            }
            let joins = start.is_some() && {
                let previous = &morphemes[index - 1];
                previous.range.end == morpheme.range.start
                    || bridges_kanji_latin_pair(text, previous, morpheme)
            };
            if !joins {
                if let Some(begin) = start.take() {
                    self.add_span(&morphemes[begin..index]);
                }
                start = Some(index);
            }
        }
        if let Some(begin) = start {
            self.add_span(&morphemes[begin..]);
        }
    }

    /// 1 つの区間を数える。2 形態素に満たない区間は複合語にならないので数えない。
    /// ただし [`UnknownMorphemes::Count`] では、未知語 1 形態素をキーにする。
    ///
    /// 区間にラテン文字だけの名詞([`is_latin_only_noun`])があれば、その形態素で
    /// 区切った各部分区間も、独立の区間として同じ規則で数える。
    /// 「Django認証バックエンド」は全体と対のほかに、部分区間「認証バックエンド」
    /// のキーを持つ。同じキーが全体の対と部分区間の両方から出ても、1 つの区間の
    /// 出現は 1 回なので、1 回だけ数える。部品は区間全体の形態素で数える。
    fn add_span(&mut self, span: &[Morpheme<'_>]) {
        let mut keys: Vec<String> = Vec::new();
        self.push_span_keys(&mut keys, span);
        if span.iter().any(is_latin_only_noun) {
            for part in span.split(is_latin_only_noun) {
                self.push_span_keys(&mut keys, part);
            }
        }
        keys.sort_unstable();
        keys.dedup();
        for key in &keys {
            increment(&mut self.compounds, key);
        }
        if span.len() >= 2 {
            for morpheme in span {
                increment(&mut self.components, morpheme.surface);
            }
        }
    }

    /// 1 つの区間から作るキーを `keys` に足す。区間全体(最大
    /// [`MAX_SPAN_MORPHEMES`] 形態素)と、3 形態素以上なら隣り合う対である。
    fn push_span_keys(&self, keys: &mut Vec<String>, span: &[Morpheme<'_>]) {
        if span.len() < 2 {
            if self.unknown == UnknownMorphemes::Count
                && let [morpheme] = span
                && is_unknown_run(morpheme)
            {
                push_compound(keys, span);
            }
            return;
        }
        if span.len() <= MAX_SPAN_MORPHEMES {
            push_compound(keys, span);
        }
        if span.len() >= 3 {
            for pair in span.windows(2) {
                push_compound(keys, pair);
            }
        }
    }
}

/// 区間の 2 種類のキーを `keys` に足す。表層形のキーと正規化形のキーが同じなら
/// 1 つにまとめ、違えばどちらも足す。
fn push_compound(keys: &mut Vec<String>, span: &[Morpheme<'_>]) {
    let surface: String = span.iter().map(|morpheme| morpheme.surface).collect();
    let normalized: String = span.iter().map(Morpheme::normalized_form).collect();
    if normalized != surface {
        keys.push(normalized);
    }
    keys.push(surface);
}

/// ラテン文字だけからなる名詞か。「Django」「API」がこれにあたる。
fn is_latin_only_noun(morpheme: &Morpheme<'_>) -> bool {
    morpheme.part_of_speech.category() == "名詞" && LATIN_SURFACE.is_match(morpheme.surface)
}

/// `key` の回数を 1 増やす。既にあるキーでは文字列を確保しない。
fn increment(table: &mut HashMap<Box<str>, u32>, key: &str) {
    if let Some(count) = table.get_mut(key) {
        *count = count.saturating_add(1);
    } else {
        table.insert(key.into(), 1);
    }
}

/// 複合語を成す品詞か。名詞のうち普通名詞と固有名詞、接頭辞、接尾辞を採る。
/// 数詞と記号は採らない。
///
/// 記事名から見出しを採る [`crate::titles`] も、名前の形態素列がこの品詞だけで
/// できているかをこの関数で見る。数える側と登録する側で区間の条件を揃えるため
/// である。
pub fn is_compound_part(part_of_speech: &PartOfSpeech) -> bool {
    match part_of_speech.category() {
        "名詞" => matches!(part_of_speech.subdivision(1), "普通名詞" | "固有名詞"),
        "接頭辞" | "接尾辞" => true,
        _ => false,
    }
}

/// 解析辞書に見出しが無いカタカナか英字の連なりか。この形の 1 形態素だけが、
/// [`UnknownMorphemes::Count`] で複合語のキーになる。
///
/// 数える側と検査する側がこの 1 つの関数を通るので、キーにする形態素の条件は
/// 両者で必ず揃う。キーそのものは、区間の長さによらず表層形と正規化形の 2 つで
/// ある。解析器は英字の正規化形を小文字にし、`sha256` の `256` のような末尾の
/// 数字を別の形態素へ割るので、英字の未知語の正規化はこの 2 つのキーで足りる。
pub fn is_unknown_run(morpheme: &Morpheme<'_>) -> bool {
    !morpheme.known && (is_latin(morpheme.surface) || is_katakana(morpheme.surface))
}

/// 英字だけの表層形か。英字と数字を採り、少なくとも 1 文字は英字であること。
fn is_latin(surface: &str) -> bool {
    surface.chars().all(|c| c.is_ascii_alphanumeric())
        && surface.contains(|c: char| c.is_ascii_alphabetic())
}

/// カタカナだけの表層形か。長音符と中黒も採る。
fn is_katakana(surface: &str) -> bool {
    surface
        .chars()
        .all(|c| matches!(c, 'ァ'..='ヶ' | 'ー' | '・'))
}

/// 解析器へ渡す塊に切る。句点と改行で切り、[`MAX_ANALYSIS_BYTES`] を超える塊は
/// 読点で、それでも超える塊は文字の境目で切る。
fn analysis_chunks(text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    for sentence in text.split_inclusive(['\n', '。']) {
        if sentence.len() <= MAX_ANALYSIS_BYTES {
            push_chunk(&mut chunks, sentence);
            continue;
        }
        for clause in sentence.split_inclusive('、') {
            if clause.len() <= MAX_ANALYSIS_BYTES {
                push_chunk(&mut chunks, clause);
            } else {
                push_fixed_length_chunks(&mut chunks, clause);
            }
        }
    }
    chunks
}

/// 空でない塊を足す。
fn push_chunk<'t>(chunks: &mut Vec<&'t str>, chunk: &'t str) {
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
}

/// 句点も改行も読点も無い長い塊を、[`MAX_ANALYSIS_BYTES`] 以下の文字の境目で切る。
/// 語の途中で切れることがあるが、この長さの塊は表や羅列であり、文ではない。
fn push_fixed_length_chunks<'t>(chunks: &mut Vec<&'t str>, chunk: &'t str) {
    let mut rest = chunk;
    while rest.len() > MAX_ANALYSIS_BYTES {
        let mut end = MAX_ANALYSIS_BYTES;
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&rest[..end]);
        rest = &rest[end..];
    }
    push_chunk(chunks, rest);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` を数え、複合語のキーと回数を並べる。
    fn compounds(text: &str) -> Vec<(String, u32)> {
        compounds_with(text, UnknownMorphemes::Skip)
    }

    /// 未知語 1 形態素の扱いを決めて `text` を数える。
    fn compounds_with(text: &str, unknown: UnknownMorphemes) -> Vec<(String, u32)> {
        let mut counts = Counts::new(unknown);
        counts.add_text(text).unwrap();
        let mut keys: Vec<(String, u32)> = counts
            .compounds
            .into_iter()
            .map(|(key, count)| (key.into_string(), count))
            .collect();
        keys.sort_unstable();
        keys
    }

    /// `text` を数えたキーに `key` があるか。
    fn has_key(text: &str, key: &str) -> bool {
        compounds(text).iter().any(|(found, _)| found == key)
    }

    /// `documents` を 1 つずつ文書単位で数え、足し込んだ回数を返す。
    fn document_counts(documents: &[&str]) -> Counts {
        let mut totals = Counts::default();
        for document in documents {
            let mut counts = Counts::default();
            counts.add_text(document).unwrap();
            counts.count_once_per_key();
            totals.merge(counts);
        }
        totals
    }

    #[test]
    fn 同じ文書に2回現れる複合語を1回と数える() {
        let counts = document_counts(&["公開鍵暗号を作る。公開鍵暗号を配る。"]);
        assert_eq!(counts.compounds.get("公開鍵暗号").copied(), Some(1));
        // 部品も同じく、1 文書に何度現れても 1 回である。
        assert_eq!(counts.components.get("公開").copied(), Some(1));
    }

    #[test]
    fn 別々の文書に現れる複合語を文書の数だけ数える() {
        let counts = document_counts(&["公開鍵暗号を作る。", "公開鍵暗号を配る。"]);
        assert_eq!(counts.compounds.get("公開鍵暗号").copied(), Some(2));
        assert_eq!(counts.components.get("公開").copied(), Some(2));
    }

    #[test]
    fn 名詞が続く区間を複合語として数える() {
        // 3 形態素の区間は、全体と隣り合う 2 形態素の対を数える。
        let keys = compounds("公開鍵暗号を使う。");
        assert_eq!(
            keys,
            vec![
                ("公開鍵".to_owned(), 1),
                ("公開鍵暗号".to_owned(), 1),
                ("鍵暗号".to_owned(), 1),
            ]
        );
    }

    #[test]
    fn 助詞をまたぐ区間は続かない() {
        // 「認証の基盤」は助詞で切れるので、複合語のキーにならない。
        assert!(compounds("認証の基盤。").is_empty());
    }

    #[test]
    fn 空白をまたぐ区間は続かない() {
        // 解析は空白を形態素にしないので、切れ目は形態素のバイト範囲に現れる。
        assert!(compounds("認証 基盤。").is_empty());
    }

    #[test]
    fn 漢字を含む名詞とラテン文字名詞の対は空白1つを跨ぐ() {
        // 「読み取り」「専用」は漢字を含む名詞、「API」はラテン文字だけの名詞
        // なので、間の空白 1 つを跨いで 1 つの区間になる。キーは表層形を連ねる
        // ので、空白は残らない。
        assert!(has_key("読み取り専用 API を呼ぶ。", "読み取り専用API"));
        // 送りがなの無い漢語でも跨ぐ。「演算子」は「演算」と「子」に割れる。
        assert!(has_key("AND 演算子を使う。", "AND演算子"));
        // 対の並びは逆でもよい。
        assert!(has_key("API 呼び出しを数える。", "API呼び出し"));
    }

    #[test]
    fn ラテン文字で区切った部分区間も数える() {
        // 全体と隣り合う対のほかに、ラテン文字の Django で区切った日本語だけの
        // 部分区間 認証バックエンド もキーになる。
        let keys = compounds("Django 認証バックエンドを使う。");
        for key in ["Django認証バックエンド", "認証バックエンド", "Django認証"] {
            assert!(
                keys.iter().any(|(found, _)| found == key),
                "{key}: {keys:?}"
            );
        }
        // 空白なしで隣接した型にも同じ規則を当てる。
        let adjacent = compounds("Django認証バックエンドを使う。");
        assert_eq!(adjacent, keys);
        // 部分区間のキーが全体の対と重なっても、1 つの区間では 1 回である。
        assert!(
            keys.contains(&("認証バックエンド".to_owned(), 1)),
            "{keys:?}"
        );
        // ラテン文字を含まない区間は、これまでどおり全体と対だけである。
        assert_eq!(
            compounds("公開鍵暗号を使う。"),
            vec![
                ("公開鍵".to_owned(), 1),
                ("公開鍵暗号".to_owned(), 1),
                ("鍵暗号".to_owned(), 1),
            ]
        );
    }

    #[test]
    fn 対の型でない箇所は空白で切れたままである() {
        // 漢語どうしは対の型でないので、空白での切れ目が残る。
        assert!(compounds("侵害 成功。").is_empty());
        // カタカナの名詞は空白を跨がない。空白なしなら今までどおり続く。
        assert!(!has_key("Azure サブドメインを作る。", "Azureサブドメイン"));
        assert!(has_key("Azureサブドメインを作る。", "Azureサブドメイン"));
        // 跨ぐのは半角空白 1 つだけで、全角空白と 2 つ以上の空白は跨がない。
        assert!(!has_key(
            "読み取り専用\u{3000}API を呼ぶ。",
            "読み取り専用API"
        ));
        assert!(!has_key("読み取り専用  API を呼ぶ。", "読み取り専用API"));
        // 「済み」は「gzip 済み」のように接辞として続く形が正当なので、対の
        // 一方にならない。
        assert!(!has_key("gzip 済みの辞書。", "gzip済み"));
    }

    #[test]
    fn 数詞に当たった区間はそこで切れる() {
        // 「3」は数詞なので、その前後は別々の区間になり、どちらも 1 形態素で
        // 複合語にならない。
        assert!(compounds("暗号3方式。").is_empty());
    }

    #[test]
    fn 部品は区間に現れた形態素だけを数える() {
        let mut counts = Counts::default();
        counts.add_text("公開鍵を作る。鍵は長い。").unwrap();
        // 「鍵」は区間の中に 1 回、区間の外に 1 回現れる。数えるのは中の 1 回だけ。
        assert_eq!(counts.components.get("鍵").copied(), Some(1));
        assert_eq!(counts.components.get("公開").copied(), Some(1));
    }

    #[test]
    fn 辞書に無いカタカナの1形態素をキーにする() {
        // 「レジリエンスバジェット」は解析辞書に無く、1 形態素にまとまる。
        // 既定では 2 形態素以上という区間の条件から落ちる。
        let text = "レジリエンスバジェットを配る。";
        let key = "レジリエンスバジェット".to_owned();
        let skipped = compounds(text);
        assert!(
            !skipped.iter().any(|(found, _)| *found == key),
            "{skipped:?}"
        );
        let counted = compounds_with(text, UnknownMorphemes::Count);
        assert!(counted.contains(&(key, 1)), "{counted:?}");
    }

    #[test]
    fn 辞書にあるカタカナ語はキーにならない() {
        // 「コンピュータ」は見出しがあるので未知語ではなく、1 形態素の区間は
        // どちらの設定でもキーにならない。
        let counted = compounds_with("コンピュータを配る。", UnknownMorphemes::Count);
        assert!(counted.is_empty(), "{counted:?}");
    }

    #[test]
    fn 英字の未知語は小文字のキーでも数える() {
        // 解析器が正規化形を小文字にするので、表層形と正規化形の 2 つのキーが
        // 立つ。
        let keys = compounds_with("Kubectlを配る。", UnknownMorphemes::Count);
        assert_eq!(
            keys,
            vec![("Kubectl".to_owned(), 1), ("kubectl".to_owned(), 1)]
        );
    }

    #[test]
    fn キーにする文字種を分ける() {
        assert!(is_latin("api2"));
        // 数字だけの連なりは英字の語ではない。
        assert!(!is_latin("2026"));
        assert!(is_katakana("フルユニバース"));
        // 漢字が混じる連なりはどちらでもない。
        assert!(!is_katakana("フル宇宙"));
        assert!(!is_latin("フル宇宙"));
    }

    #[test]
    fn 長い塊は句点と読点と固定長で切れる() {
        // 句点で切った塊が上限に収まる。「あ」は UTF-8 で 3 バイトである。
        let sentence = format!("{}。{}。", "あ".repeat(2000), "い".repeat(10));
        let chunks = analysis_chunks(&sentence);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|chunk| chunk.len() <= MAX_ANALYSIS_BYTES));
        // 句点も読点も無い塊は固定長で切れる。
        let run = "う".repeat(10_000);
        let chunks = analysis_chunks(&run);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.len() <= MAX_ANALYSIS_BYTES));
        assert_eq!(chunks.concat(), run);
    }
}
