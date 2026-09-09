//! 解析辞書の見出しを読み、分割単位 A で 2 形態素以上に割れる名詞を TSV に書く。
//!
//! akunuki が埋め込む解析辞書は core なので、full にだけある見出しは分割単位 A で
//! 2 形態素以上に割れ、コーパスに現れなければ造語の候補に見える。full の見出しは
//! 人手で採録された既存語なので、頻度によらず既存語として扱う。1 形態素に割れる
//! 見出しは core でも full でも 1 語であり、造語の候補にならないので書かない。

use std::collections::HashMap;
use std::path::Path;

use aku_morph::{DICTIONARY_VERSION, MorphError, analyze_short};
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::tsv;

/// 見出しの TSV を書くファイル名。
pub const HEADWORDS_FILE: &str = "dictionary-headwords.tsv";

/// 見出しを読んだ記録を書くファイル名。`build` がこれを読んで manifest に写す。
pub const HEADWORD_STATS_FILE: &str = "dictionary-headwords-stats.json";

/// 見出しに与えるバケット。頻度のバケットの最大である 5 より大きいので、同じキーが
/// 回数の TSV にもあれば、照合はこちらを返す。
pub const HEADWORD_BUCKET: u32 = 7;

/// 見出しを読む辞書の版。配布元の zip はこの日付の下に置かれている。
const LEXICON_VERSION: &str = "20260723";

/// 読む CSV の名前の幹。配布元の zip は `<幹>.zip`、展開後の CSV は `<幹>.csv`
/// である。full は 3 つの和であり、akunuki が埋め込む core は `small` と `core` の
/// 和である。core にある見出しも、分割単位 A で割れるなら登録の対象になるので、
/// 3 つとも読む。
const LEXICON_STEMS: [&str; 3] = ["small_lex", "core_lex", "notcore_lex"];

/// 解析辞書の固有名詞の見出しをフィルタに入れるか。固有名詞は登録する見出しの
/// 大半を占めるので、外すとフィルタが小さくなる。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProperNouns {
    /// 固有名詞の見出しも登録する。
    #[default]
    Include,
    /// 固有名詞の見出しを登録しない。
    Exclude,
}

/// 見出しの列。
const HEADWORD_COLUMN: usize = 0;

/// 品詞の 1 段目の列。
const CATEGORY_COLUMN: usize = 5;

/// 品詞の 2 段目の列。
const SUBDIVISION_COLUMN: usize = 6;

/// 見出しを読んだ記録。
#[derive(Serialize, Deserialize)]
pub struct HeadwordStats {
    /// 見出しを読んだ解析辞書の版。
    pub dictionary_version: String,
    /// 読んだ配布物の名前。
    pub lexicon_files: Vec<String>,
    /// 読んだ名詞の見出しの数。同じ見出しを持つ行は 1 件に畳んだ後の数である。
    pub read_headwords: u64,
    /// 2 形態素以上に割れて登録した見出しの数。
    pub registered_headwords: u64,
    /// 登録した見出しのうち、普通名詞の数。
    pub registered_common_nouns: u64,
    /// 登録した見出しのうち、固有名詞の数。
    pub registered_proper_nouns: u64,
    /// 登録した見出しのうち、普通名詞でも固有名詞でもない名詞の数。数詞と
    /// 助動詞語幹がこれにあたる。
    pub registered_other_nouns: u64,
    /// 固有名詞の見出しを登録したか。
    pub include_proper_nouns: bool,
}

/// 名詞の下位分類。同じ見出しが複数の行に現れることがあるので、普通名詞の行が
/// あれば普通名詞、無くて固有名詞の行があれば固有名詞と決める。行を読む順に
/// 依らずに分類が決まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NounKind {
    /// 普通名詞。
    Common,
    /// 固有名詞。
    Proper,
    /// 数詞と助動詞語幹。
    Other,
}

impl NounKind {
    /// 品詞の 2 段目から分類を決める。
    fn from_subdivision(subdivision: &str) -> Self {
        match subdivision {
            "普通名詞" => Self::Common,
            "固有名詞" => Self::Proper,
            _ => Self::Other,
        }
    }
}

/// `lexicon_dir` の CSV から名詞の見出しを読み、`out_dir` に見出しの TSV と記録を
/// 書く。
///
/// # Errors
///
/// 辞書の版が [`LEXICON_VERSION`] と食い違う場合、CSV を読めない場合、見出しを
/// 解析できない場合、書き出しが失敗する場合に返す。
pub fn run(lexicon_dir: &Path, out_dir: &Path, proper_nouns: ProperNouns) -> Result<()> {
    if !DICTIONARY_VERSION.ends_with(LEXICON_VERSION) {
        bail!(
            "解析辞書の版 '{DICTIONARY_VERSION}' が、見出しを取る配布物の日付 {LEXICON_VERSION} と食い違う。akunuki の依存を上げたなら LEXICON_STEMS の取得元も上げる"
        );
    }
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;

    let mut headwords: HashMap<Box<str>, NounKind> = HashMap::new();
    for stem in LEXICON_STEMS {
        let path = lexicon_dir.join(format!("{stem}.csv"));
        read_lexicon(&path, &mut headwords)?;
    }

    let mut registered = split_into_two_or_more(&headwords)?;
    if proper_nouns == ProperNouns::Exclude {
        registered.retain(|(_, kind)| *kind != NounKind::Proper);
    }
    let stats = HeadwordStats {
        dictionary_version: DICTIONARY_VERSION.to_owned(),
        lexicon_files: LEXICON_STEMS
            .iter()
            .map(|stem| format!("{stem}.zip"))
            .collect(),
        read_headwords: headwords.len() as u64,
        registered_headwords: registered.len() as u64,
        registered_common_nouns: count_kind(&registered, NounKind::Common),
        registered_proper_nouns: count_kind(&registered, NounKind::Proper),
        registered_other_nouns: count_kind(&registered, NounKind::Other),
        include_proper_nouns: proper_nouns == ProperNouns::Include,
    };

    let table: HashMap<Box<str>, u32> = registered
        .into_iter()
        .map(|(headword, _)| (headword.into(), HEADWORD_BUCKET))
        .collect();
    tsv::write(&out_dir.join(HEADWORDS_FILE), &table)?;

    let stats_path = out_dir.join(HEADWORD_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "名詞の見出し {} 件のうち {} 件が 2 形態素以上に割れた。普通名詞が {} 件、固有名詞が {} 件、その他の名詞が {} 件である",
        stats.read_headwords,
        stats.registered_headwords,
        stats.registered_common_nouns,
        stats.registered_proper_nouns,
        stats.registered_other_nouns
    );
    Ok(())
}

/// `path` の CSV から品詞の 1 段目が名詞の見出しを読み、`headwords` に足す。
fn read_lexicon(path: &Path, headwords: &mut HashMap<Box<str>, NounKind>) -> Result<()> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        // 見出しの行の列の数は一定ではない。分割の情報を持つ行だけ列が増える。
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("{} を開けない", path.display()))?;
    for record in reader.records() {
        let record = record.with_context(|| format!("{} を読めない", path.display()))?;
        if record.get(CATEGORY_COLUMN) != Some("名詞") {
            continue;
        }
        let (Some(headword), Some(subdivision)) =
            (record.get(HEADWORD_COLUMN), record.get(SUBDIVISION_COLUMN))
        else {
            continue;
        };
        let kind = NounKind::from_subdivision(subdivision);
        headwords
            .entry(headword.into())
            .and_modify(|current| *current = (*current).min(kind))
            .or_insert(kind);
    }
    Ok(())
}

/// 分割単位 A で 2 形態素以上に割れる見出しだけを残す。
fn split_into_two_or_more(
    headwords: &HashMap<Box<str>, NounKind>,
) -> Result<Vec<(&str, NounKind)>, MorphError> {
    headwords
        .par_iter()
        .map(|(headword, kind)| {
            let morphemes = analyze_short(headword)?;
            Ok((morphemes.len() >= 2).then_some((headword.as_ref(), *kind)))
        })
        .filter_map(Result::transpose)
        .collect()
}

/// `kind` の見出しの数。
fn count_kind(registered: &[(&str, NounKind)], kind: NounKind) -> u64 {
    registered
        .iter()
        .filter(|(_, current)| *current == kind)
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 品詞の2段目から分類を決める() {
        assert_eq!(NounKind::from_subdivision("普通名詞"), NounKind::Common);
        assert_eq!(NounKind::from_subdivision("固有名詞"), NounKind::Proper);
        // 数詞と助動詞語幹は、普通名詞でも固有名詞でもない名詞として数える。
        assert_eq!(NounKind::from_subdivision("数詞"), NounKind::Other);
    }

    #[test]
    fn 同じ見出しの複数の分類は普通名詞に寄せる() {
        // 分類の順は普通名詞・固有名詞・その他であり、小さい方を採る。
        assert!(NounKind::Common < NounKind::Proper);
        assert!(NounKind::Proper < NounKind::Other);
    }

    #[test]
    fn 名詞の行だけを読む() {
        let dir = std::env::temp_dir().join("corpus-tool-headwords-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lex.csv");
        std::fs::write(
            &path,
            // 1 行目は固有名詞、2 行目は同じ見出しの普通名詞、3 行目は名詞でない。
            "東京駅,1,1,0,東京駅,名詞,固有名詞,一般,*,*,*\n\
             東京駅,1,1,0,東京駅,名詞,普通名詞,一般,*,*,*\n\
             走る,1,1,0,走る,動詞,一般,*,*,五段-ラ行,終止形-一般\n",
        )
        .unwrap();
        let mut headwords = HashMap::new();
        read_lexicon(&path, &mut headwords).unwrap();
        assert_eq!(headwords.len(), 1);
        assert_eq!(headwords.get("東京駅").copied(), Some(NounKind::Common));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// 見出しの TSV に載ったキーを並べる。
    fn registered_keys(dir: &Path, proper_nouns: ProperNouns) -> Vec<String> {
        let lexicon_dir = dir.join("lexicon");
        std::fs::create_dir_all(&lexicon_dir).unwrap();
        for stem in LEXICON_STEMS {
            std::fs::write(
                lexicon_dir.join(format!("{stem}.csv")),
                // 1 行目は固有名詞、2 行目は普通名詞。どちらも分割単位 A で
                // 2 形態素に割れる。
                "東京駅,1,1,0,東京駅,名詞,固有名詞,一般,*,*,*\n\
                 公開鍵,1,1,0,公開鍵,名詞,普通名詞,一般,*,*,*\n",
            )
            .unwrap();
        }
        let out_dir = dir.join("out");
        run(&lexicon_dir, &out_dir, proper_nouns).unwrap();
        let mut keys: Vec<String> = tsv::read(&out_dir.join(HEADWORDS_FILE))
            .unwrap()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn 固有名詞の見出しを外せる() {
        let dir = std::env::temp_dir().join("corpus-tool-headwords-proper-test");
        assert_eq!(
            registered_keys(&dir, ProperNouns::Include),
            vec!["公開鍵".to_owned(), "東京駅".to_owned()]
        );
        // 外すと固有名詞の見出しだけが落ち、普通名詞は残る。
        assert_eq!(
            registered_keys(&dir, ProperNouns::Exclude),
            vec!["公開鍵".to_owned()]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 見出しは引用符と読点を含んでも1列目として読める() {
        let dir = std::env::temp_dir().join("corpus-tool-headwords-quote-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lex.csv");
        std::fs::write(
            &path,
            "\"あれ,これ\",1,1,0,\"あれ,これ\",名詞,普通名詞,一般,*,*,*\n",
        )
        .unwrap();
        let mut headwords = HashMap::new();
        read_lexicon(&path, &mut headwords).unwrap();
        assert!(headwords.contains_key("あれ,これ"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
