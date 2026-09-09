//! `topic-distribution` サブコマンド。
//!
//! いまの頻度フィルタは語の有無だけで既存語と造語の候補を分けるので、`樹形図` の
//! ように実在する語を別の意味で当てた語は、既存語として通してしまう。この仕組みに
//! 見込みがあるかを測るには、その語が Wikipedia のどの分野に現れるかを知る必要が
//! ある。区分が意味の転用の語の多くが情報技術の外の分野に偏るなら、語と分野の組を
//! キーにした登録で、文書の分野に含まれない語を候補として拾える見通しが立つ。
//!
//! 語の一覧は正解つきの語の集合の TSV から採る。`eval/coined.tsv` の区分の列から
//! 意味の転用と造語の語を採り、`eval/established-security.tsv` は先頭から決めた数
//! だけ採る。区分が言い換えの語は、プロジェクトの表記の選択であって分野の偏りを
//! 測る対象ではないので採らない。語ごとに、本文にその語を含む記事を
//! [`aho_corasick`] で 1 回の走査にまとめて数え、記事が持つ話題の予測を
//! [`crate::topic::categories_from_tags`] で 6 区分に畳んで、区分ごとの記事数を
//! 集計する。

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use aho_corasick::AhoCorasick;
use anyhow::{Context, Result, bail};
use clap::Args;

use crate::dump::{Dump, Selection};
use crate::topic::{CATEGORY_COUNT, Category, categories_from_tags};

/// セキュリティの既存語の例から採る語数の既定値。
pub const DEFAULT_ESTABLISHED_SECURITY_LIMIT: usize = 50;

/// 出力の TSV のファイル名。
pub const OUTPUT_FILE: &str = "topic-distribution.tsv";

/// 途中経過を書き出す間隔(読んだ記事の数)。
const PROGRESS_ARTICLES: u64 = 200_000;

/// `eval/coined.tsv` の、区分の列。
const COINED_CATEGORY_COLUMN: usize = 3;

/// `run` の入力。`main` の `Command::TopicDistribution` がそのまま `#[command(flatten)]`
/// する。
#[derive(Args)]
pub struct Input {
    /// cirrussearch のダンプ。
    #[arg(long)]
    pub dump: PathBuf,
    /// 造語の例の TSV。区分の列から意味の転用と造語の語を採る。
    #[arg(long)]
    pub coined: PathBuf,
    /// セキュリティの既存語の例の TSV。
    #[arg(long)]
    pub established_security: PathBuf,
    /// セキュリティの既存語の例から採る語数。
    #[arg(long, default_value_t = DEFAULT_ESTABLISHED_SECURITY_LIMIT)]
    pub established_security_limit: usize,
    /// 先頭のこの数の記事だけを読む。動きの確認に使う。
    #[arg(long)]
    pub limit: Option<u64>,
}

/// 語が属する群。README の表を組むための分類であり、記事数の数え方には関わらない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    /// `eval/coined.tsv` で区分が意味の転用の語。
    Repurposed,
    /// `eval/coined.tsv` で区分が造語の語。
    Coined,
    /// `eval/established-security.tsv` の先頭から採った語。
    EstablishedSecurity,
}

impl Group {
    /// TSV に書く群の名前。
    fn label(self) -> &'static str {
        match self {
            Self::Repurposed => "意味の転用",
            Self::Coined => "造語",
            Self::EstablishedSecurity => "セキュリティの既存語",
        }
    }
}

/// 語 1 件の入力。
struct Word {
    /// 語の表層形。本文への部分文字列の一致だけで見るので、形態素解析は行わない。
    text: String,
    /// 属する群。
    group: Group,
}

/// 語 1 件の集計。
struct Tally {
    /// 語を本文に含む記事の総数。話題の予測を持たない記事も数える。
    total_articles: u64,
    /// 区分ごとの記事数。[`Category::ALL`] と同じ並びである。1 記事が複数の
    /// 区分を持てば、その全部を数える。
    category_articles: [u64; CATEGORY_COUNT],
}

impl Tally {
    fn new() -> Self {
        Self {
            total_articles: 0,
            category_articles: [0; CATEGORY_COUNT],
        }
    }
}

/// `input` の語の一覧でダンプを 1 回走査し、`out_dir` に区分ごとの記事数の TSV を
/// 書く。
///
/// # Errors
///
/// TSV を読めない場合、区分の列が 3 区分のどれでもない場合、ダンプを読めない場合、
/// 書き出しが失敗する場合に返す。
pub fn run(input: &Input, out_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let mut words = read_coined(&input.coined)?;
    words.extend(read_established_security(
        &input.established_security,
        input.established_security_limit,
    )?);

    let patterns: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
    let automaton = AhoCorasick::new(&patterns).context("語の一覧から automaton を作れない")?;
    let mut tallies: Vec<Tally> = (0..words.len()).map(|_| Tally::new()).collect();

    let mut dump = Dump::open(&input.dump, Selection::AllArticles, input.limit)?;
    let mut matched = HashSet::new();
    let mut reported = 0;
    while let Some(article) = dump.next_article()? {
        matched.clear();
        for pattern_match in automaton.find_overlapping_iter(&article.text) {
            matched.insert(pattern_match.pattern().as_usize());
        }
        if !matched.is_empty() {
            let categories = categories_from_tags(&article.weighted_tags);
            for &index in &matched {
                tallies[index].total_articles += 1;
                for (category_index, present) in categories.iter().enumerate() {
                    if *present {
                        tallies[index].category_articles[category_index] += 1;
                    }
                }
            }
        }
        if dump.scanned - reported >= PROGRESS_ARTICLES {
            reported = dump.scanned;
            eprintln!("{} 記事を読んだ", dump.scanned);
        }
    }

    write_output(&out_dir.join(OUTPUT_FILE), &words, &tallies)?;
    println!("{} 記事を読み、{} 語を数えた", dump.scanned, words.len());
    Ok(())
}

/// `path` の造語の例を読み、区分が意味の転用と造語の語を採る。区分が言い換えの語は
/// 落とす。
///
/// # Errors
///
/// ファイルを読めない場合、行の欄が足りない場合、区分の列が 3 区分のどれでもない
/// 場合に返す。
fn read_coined(path: &Path) -> Result<Vec<Word>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    let mut words = Vec::new();
    for (index, line) in text.lines().enumerate().skip(1) {
        let columns: Vec<&str> = line.split('\t').collect();
        let word = columns
            .first()
            .with_context(|| format!("{} の {} 行目に語が無い", path.display(), index + 1))?;
        let category = columns
            .get(COINED_CATEGORY_COLUMN)
            .with_context(|| format!("{} の {} 行目に区分の列が無い", path.display(), index + 1))?;
        let group = match *category {
            "意味の転用" => Group::Repurposed,
            "造語" => Group::Coined,
            "言い換え" => continue,
            other => bail!(
                "{} の {} 行目の区分 '{other}' が造語でも言い換えでも意味の転用でもない",
                path.display(),
                index + 1
            ),
        };
        words.push(Word {
            text: (*word).to_owned(),
            group,
        });
    }
    Ok(words)
}

/// `path` のセキュリティの既存語の例から、先頭の `limit` 語を読む。
///
/// # Errors
///
/// ファイルを読めない場合、語が `limit` に届かない場合に返す。
fn read_established_security(path: &Path, limit: usize) -> Result<Vec<Word>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    let mut lines = text.lines();
    lines.next(); // 見出し行を読み飛ばす。
    let words: Vec<Word> = lines
        .take(limit)
        .map(|line| Word {
            text: line.split('\t').next().unwrap_or_default().to_owned(),
            group: Group::EstablishedSecurity,
        })
        .collect();
    if words.len() < limit {
        bail!(
            "{} が {limit} 語に届かない({} 語しか無い)",
            path.display(),
            words.len()
        );
    }
    Ok(words)
}

/// 語ごとの群と集計を `path` へ TSV で書く。
///
/// # Errors
///
/// ファイルを作れない場合、書き出しが失敗する場合に返す。
fn write_output(path: &Path, words: &[Word], tallies: &[Tally]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("{} を作れない", path.display()))?;
    let mut writer = BufWriter::new(file);
    let mut header = vec!["語".to_owned(), "群".to_owned(), "記事の総数".to_owned()];
    header.extend(
        Category::ALL
            .iter()
            .map(|category| category.label().to_owned()),
    );
    header.push("情報技術の割合".to_owned());
    writeln!(writer, "{}", header.join("\t"))
        .with_context(|| format!("{} を書けない", path.display()))?;
    for (word, tally) in words.iter().zip(tallies) {
        let mut columns = vec![
            word.text.clone(),
            word.group.label().to_owned(),
            tally.total_articles.to_string(),
        ];
        columns.extend(tally.category_articles.iter().map(u64::to_string));
        columns.push(information_technology_ratio(tally));
        writeln!(writer, "{}", columns.join("\t"))
            .with_context(|| format!("{} を書けない", path.display()))?;
    }
    writer
        .flush()
        .with_context(|| format!("{} を書けない", path.display()))
}

/// 語を含む記事のうち、情報技術の区分を持つ割合。記事が 1 件も無ければ `-` を
/// 書く。
fn information_technology_ratio(tally: &Tally) -> String {
    if tally.total_articles == 0 {
        return "-".to_owned();
    }
    let it_articles = tally.category_articles[Category::InformationTechnology.index()];
    #[allow(
        clippy::cast_precision_loss,
        reason = "記事数は高々 150 万件であり、f64 の精度で割合を出すのに支障が無い"
    )]
    let ratio = it_articles as f64 / tally.total_articles as f64;
    format!("{ratio:.4}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// テスト用の TSV を書き、パスを返す。
    fn write_tsv(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn 造語の例から意味の転用と造語の語を採る() {
        let dir = std::env::temp_dir().join("corpus-tool-topic-distribution-test-coined");
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_tsv(
            &dir,
            "coined.tsv",
            "語\t判定者\t出典\t区分\t区分の判定者\n\
             樹形図\tmaintainer\t出典\t意味の転用\tmaintainer\n\
             生成癖\tmaintainer\t出典\t造語\tmaintainer\n\
             社会工学\tmaintainer\t出典\t言い換え\tmaintainer\n",
        );
        let words = read_coined(&path).unwrap();
        // 区分が言い換えの `社会工学` は落ちるので、2 語が残る。
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "樹形図");
        assert_eq!(words[0].group, Group::Repurposed);
        assert_eq!(words[1].text, "生成癖");
        assert_eq!(words[1].group, Group::Coined);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn セキュリティの既存語を先頭から数だけ採る() {
        let dir = std::env::temp_dir().join("corpus-tool-topic-distribution-test-security");
        std::fs::create_dir_all(&dir).unwrap();
        let path = write_tsv(
            &dir,
            "established-security.tsv",
            "語\t判定者\t出典\n攻撃者\tmodel\t出典\n攻撃活動\tmodel\t出典\n攻撃ツール\tmodel\t出典\n",
        );
        let words = read_established_security(&path, 2).unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "攻撃者");
        assert_eq!(words[1].text, "攻撃活動");
        assert!(
            words
                .iter()
                .all(|word| word.group == Group::EstablishedSecurity)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 情報技術の割合を記事数から出す() {
        let mut tally = Tally::new();
        tally.total_articles = 4;
        tally.category_articles[Category::InformationTechnology.index()] = 1;
        assert_eq!(information_technology_ratio(&tally), "0.2500");
        // 記事が無ければ割合を出さない。
        assert_eq!(information_technology_ratio(&Tally::new()), "-");
    }
}
