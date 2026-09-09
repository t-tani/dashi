//! ダンプの記事名とリダイレクト名から語を採り、解析辞書の見出しと同じ形の TSV を
//! 書く。
//!
//! 記事の本文は `count` が数えているが、リダイレクト名は本文に現れないことがある。
//! Wikipedia の記事名は人手で付けられた既存語なので、頻度によらず既存語として
//! 扱い、[`crate::headwords::HEADWORD_BUCKET`] で登録する。
//!
//! 採るのは、名前の全体が複合語の 1 つの区間として成り立ち、固有名詞の形態素を
//! 含まないものである。区間の条件は `count` が本文から複合語を切り出すのと同じで
//! あり、[`crate::compound::is_compound_part`] の品詞だけででき、形態素が原文で
//! 隣り合うことを指す。記号や空白や助詞を含む名前は区間にならないので、`.jpg` と
//! `恋の掟 (漫画)` はここで落ちる。固有名詞を含む名前は akunuki が候補にしないので、
//! 登録しても引かれない。
//!
//! 1 形態素の名前は、解析辞書に無いカタカナと英字の連なりだけを採る。`count` が
//! この形をキーとして数える既定に合わせる。
//!
//! 本文を解析しないので、走査はダンプを 1 回流す時間で終わる。

use std::collections::HashMap;
use std::path::Path;

use aku_morph::{MorphError, Morpheme, analyze_short};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::compound::{is_compound_part, is_unknown_run};
use crate::dump::{self, Dump, Selection};
use crate::headwords::HEADWORD_BUCKET;
use crate::tsv;

/// 記事名の TSV を書くファイル名。
pub const TITLES_FILE: &str = "article-titles.tsv";

/// 読んだ記録を書くファイル名。`build` がこれを読んで manifest に写す。
pub const TITLE_STATS_FILE: &str = "article-titles-stats.json";

/// 1 度に並列で解析する名前の数。
const BATCH_NAMES: usize = 4096;

/// 途中経過を書き出す間隔(読んだ記事の数)。
const PROGRESS_ARTICLES: u64 = 200_000;

/// 記事名を読んだ記録。
#[derive(Serialize, Deserialize)]
pub struct TitleStats {
    /// ダンプのファイル名。
    pub dump_file_name: String,
    /// ダンプの版。ファイル名から取れなければ `null` になる。
    pub dump_version: Option<String>,
    /// 読んだ記事の数。
    pub scanned_articles: u64,
    /// 読んだ記事名の数。
    pub read_titles: u64,
    /// 読んだリダイレクト名の数。
    pub read_redirect_titles: u64,
    /// 登録した語の数。同じ語を持つ名前は 1 件に畳んだ後の数である。
    pub registered_titles: u64,
}

/// `dump_path` のダンプから記事名とリダイレクト名を読み、`out_dir` に記事名の TSV
/// と記録を書く。`limit` を渡すと、先頭のその数の記事で読み終わる。
///
/// # Errors
///
/// ダンプを読めない場合、名前を解析できない場合、書き出しが失敗する場合に返す。
pub fn run(dump_path: &Path, out_dir: &Path, limit: Option<u64>) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let mut dump = Dump::open(dump_path, Selection::AllArticles, limit)?;
    let mut registered: HashMap<Box<str>, u32> = HashMap::new();
    let mut read_titles = 0;
    let mut read_redirect_titles = 0;
    let mut batch: Vec<String> = Vec::with_capacity(BATCH_NAMES);
    let mut reported = 0;
    while let Some(article) = dump.next_article()? {
        read_titles += 1;
        read_redirect_titles += article.redirect_titles.len() as u64;
        batch.push(article.title);
        batch.extend(article.redirect_titles);
        if batch.len() >= BATCH_NAMES {
            register_batch(&batch, &mut registered)?;
            batch.clear();
        }
        if dump.scanned - reported >= PROGRESS_ARTICLES {
            reported = dump.scanned;
            eprintln!(
                "{} 記事を読み、{} 語を登録した",
                dump.scanned,
                registered.len()
            );
        }
    }
    register_batch(&batch, &mut registered)?;

    tsv::write(&out_dir.join(TITLES_FILE), &registered)?;
    let stats = TitleStats {
        dump_file_name: dump::file_name(dump_path),
        dump_version: dump::version(dump_path),
        scanned_articles: dump.scanned,
        read_titles,
        read_redirect_titles,
        registered_titles: registered.len() as u64,
    };
    let stats_path = out_dir.join(TITLE_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} 記事の名前 {} 件とリダイレクト名 {} 件から、{} 語を登録した",
        stats.scanned_articles,
        stats.read_titles,
        stats.read_redirect_titles,
        stats.registered_titles
    );
    Ok(())
}

/// 名前のまとまりを並列に解析し、登録する語を `registered` へ足す。
fn register_batch(batch: &[String], registered: &mut HashMap<Box<str>, u32>) -> Result<()> {
    let accepted: Vec<&str> = batch
        .par_iter()
        .map(|name| Ok(is_registrable(name)?.then_some(name.as_str())))
        .filter_map(Result::transpose)
        .collect::<Result<_, MorphError>>()
        .context("記事名を解析できない")?;
    for name in accepted {
        registered.insert(name.into(), HEADWORD_BUCKET);
    }
    Ok(())
}

/// 記事名を語として登録するか。名前の全体が複合語の 1 つの区間として成り立ち、
/// 固有名詞を含まない場合に採る。1 形態素の名前は、解析辞書に無いカタカナと英字の
/// 連なりだけを採る。
///
/// # Errors
///
/// 名前を解析できない場合に返す。
fn is_registrable(name: &str) -> Result<bool, MorphError> {
    let morphemes = analyze_short(name)?;
    let Some((first, rest)) = morphemes.split_first() else {
        return Ok(false);
    };
    if rest.is_empty() {
        return Ok(is_unknown_run(first));
    }
    Ok(is_single_span(&morphemes))
}

/// 形態素列の全体が、複合語の 1 つの区間として成り立つか。すべての形態素が複合語を
/// 成す品詞で、固有名詞でなく、原文で隣り合うことを求める。
///
/// 解析は空白を形態素にしないので、空白での切れ目は形態素のバイト範囲が続くか
/// どうかに現れる。
fn is_single_span(morphemes: &[Morpheme<'_>]) -> bool {
    morphemes
        .iter()
        .all(|morpheme| is_compound_part(&morpheme.part_of_speech) && !is_proper_noun(morpheme))
        && morphemes
            .windows(2)
            .all(|pair| pair[0].range.end == pair[1].range.start)
}

/// 固有名詞の形態素か。
fn is_proper_noun(morpheme: &Morpheme<'_>) -> bool {
    morpheme.part_of_speech.category() == "名詞"
        && morpheme.part_of_speech.subdivision(1) == "固有名詞"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 二形態素以上の普通名詞の記事名を採る() {
        // 「公開鍵暗号」は 3 形態素の普通名詞に割れる。
        assert!(is_registrable("公開鍵暗号").unwrap());
        assert!(is_registrable("完了条件").unwrap());
    }

    #[test]
    fn 固有名詞を含む記事名を採らない() {
        // 「東京駅」は固有名詞 1 形態素、「日本国憲法」は固有名詞を含む連なりで
        // ある。どちらも akunuki が候補にしないので登録しない。
        assert!(!is_registrable("東京駅").unwrap());
        assert!(!is_registrable("日本国憲法").unwrap());
    }

    #[test]
    fn 記号と空白と助詞を含む記事名を採らない() {
        // `.` と `(` と `)` は複合語を成す品詞でないので、区間が切れる。
        assert!(!is_registrable(".jpg").unwrap());
        assert!(!is_registrable("恋の掟 (漫画)").unwrap());
        // 助詞をまたぐ名前と、空白で離れた名前も 1 つの区間にならない。
        assert!(!is_registrable("認証の基盤").unwrap());
        assert!(!is_registrable("Internet Protocol").unwrap());
        // 数詞に当たる名前も区間が切れる。
        assert!(!is_registrable("9月22日").unwrap());
    }

    #[test]
    fn 解析辞書にない1形態素の記事名だけを採る() {
        // 解析辞書に無いカタカナの連なりは、`count` が 1 つのキーとして数える。
        assert!(is_registrable("レジリエンスバジェット").unwrap());
        // 解析辞書にある 1 形態素は core でも full でも 1 語であり、造語の候補に
        // ならない。
        assert!(!is_registrable("暗号").unwrap());
    }
}
