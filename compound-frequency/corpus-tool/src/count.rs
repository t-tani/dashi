//! ダンプを読んで記事を絞り、複合語と部品の回数を TSV に書く。
//!
//! 平文にした技術文書を文書単位で数える入力は [`documents`] にあり、メンテナーが
//! 集めた日本語コーパスを同じ数え方で読む入力は [`corpus`] にある。
//!
//! `--domains` を渡すと、語と分野の組の回数も書く。記事の分野は話題の予測から
//! 決め、1 記事が複数の分野を持つことを許す。組の回数は、その分野でキーが現れた
//! 記事の数であり、1 記事に何度現れても 1 回である。

pub mod corpus;
pub mod documents;
mod runs;

use std::path::Path;

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::compound::{Counts, UnknownMorphemes};
use crate::count::runs::Runs;
use crate::domain::{
    self, DOMAIN_COUNT, DomainCounting, DomainCounts, DomainSet, DomainTally, domains_from_tags,
};
use crate::dump::{self, Dump, Selection};
use crate::tsv;

/// 複合語の回数を書くファイル名。
pub const COMPOUND_COUNTS_FILE: &str = "compound-counts.tsv";

/// 部品の回数を書くファイル名。
pub const COMPONENT_COUNTS_FILE: &str = "component-counts.tsv";

/// 語と分野の組の回数を書くファイル名。`--domains` を渡した場合だけ書く。
pub const DOMAIN_COUNTS_FILE: &str = "domain-counts.tsv";

/// 数えた記録を書くファイル名。`build` がこれを読んで manifest に写す。
pub const COUNT_STATS_FILE: &str = "count-stats.json";

/// 複合語の run のファイル名の頭。
const COMPOUND_RUN_PREFIX: &str = "compound-run-";

/// 語と分野の組の run のファイル名の頭。
const DOMAIN_RUN_PREFIX: &str = "domain-run-";

/// 1 度に並列で解析する記事の数。まとめて解析してから 1 つの表へ畳むので、この
/// 数を大きくすると解析の待ちが減り、途中の表が持つメモリが増える。
const BATCH_ARTICLES: usize = 64;

/// 途中経過を書き出す間隔(読んだ記事の数)。
const PROGRESS_ARTICLES: u64 = 200_000;

/// 複合語の回数を run へ書き出すキーの数。この数のキーとその文字列でおよそ
/// 500 MB を占めるので、全記事を数えても最大メモリは 1 GB の桁に収まる。分野の
/// 組も数える場合は 2 つの表の合計をこの数と比べ、両方を同時に書き出す。
const SPILL_KEYS: usize = 8_000_000;

/// 数えた記録。入力と絞り込みの結果を持つ。
#[derive(Serialize, Deserialize)]
pub struct CountStats {
    /// ダンプのファイル名。
    pub dump_file_name: String,
    /// ダンプの版。ファイル名から取れなければ `null` になる。
    pub dump_version: Option<String>,
    /// 記事を採った条件。
    pub selection_condition: String,
    /// 読んだ記事の数。
    pub scanned_articles: u64,
    /// 条件に当たった記事の数。
    pub selected_articles: u64,
    /// 条件に当たった記事の本文のバイト数。
    pub text_bytes: u64,
    /// 解析に失敗して飛ばした記事の数。この記事の本文も `text_bytes` に入る。
    pub skipped_articles: u64,
    /// 未知語 1 形態素を複合語のキーとして数えたか。
    pub unknown_morphemes: bool,
    /// 分野ごとに数えた記事の数。分野の組を数えなかった場合は空である。
    #[serde(default)]
    pub domain_articles: Vec<DomainTally>,
}

/// `dump_path` のダンプを `selection` の絞り込みで読み、`out_dir` に回数の TSV と
/// 記録を書く。
///
/// 複合語の回数は、キーが [`SPILL_KEYS`] に達するたびに run へ書き出し、最後に
/// キー順で併合する。部品の回数は複合語よりずっと少ないので、最後まで表で持つ。
/// `domains` が [`DomainCounting::Count`] なら、語と分野の組の回数も同じ形で書く。
///
/// # Errors
///
/// ダンプを読めない場合、書き出しが失敗する場合に返す。
pub fn run(
    dump_path: &Path,
    out_dir: &Path,
    selection: Selection,
    limit: Option<u64>,
    unknown: UnknownMorphemes,
    domains: DomainCounting,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let mut dump = Dump::open(dump_path, selection, limit)?;
    let mut counts = Counts::new(unknown);
    let mut domain_counts = DomainCounts::default();
    let mut domain_articles = [0_u64; DOMAIN_COUNT];
    let mut runs = Runs::new(out_dir, COMPOUND_RUN_PREFIX);
    let mut domain_runs = Runs::new(out_dir, DOMAIN_RUN_PREFIX);
    let mut skipped = 0;
    let mut batch: Vec<(String, DomainSet)> = Vec::with_capacity(BATCH_ARTICLES);
    let mut reported = 0;
    while let Some(article) = dump.next_article()? {
        let article_domains = if domains.counts() {
            domains_from_tags(&article.weighted_tags)
        } else {
            DomainSet::default()
        };
        for domain in article_domains.iter() {
            domain_articles[usize::from(domain.code() - 1)] += 1;
        }
        batch.push((article.text, article_domains));
        if batch.len() == BATCH_ARTICLES {
            let counted = count_batch(&batch, unknown);
            counts.merge(counted.counts);
            domain_counts.merge(counted.domains);
            skipped += counted.skipped;
            batch.clear();
            if counts.compounds.len() + domain_counts.len() >= SPILL_KEYS {
                runs.spill(&mut counts.compounds)?;
                domain_runs.spill(&mut domain_counts.keys)?;
            }
        }
        if dump.scanned - reported >= PROGRESS_ARTICLES {
            reported = dump.scanned;
            eprintln!(
                "{} 記事を読み、{} 記事を採り、メモリの複合語のキーが {} 件、語と分野の組が {} 件",
                dump.scanned,
                dump.selected,
                counts.compounds.len(),
                domain_counts.len()
            );
        }
    }
    let counted = count_batch(&batch, unknown);
    counts.merge(counted.counts);
    domain_counts.merge(counted.domains);
    skipped += counted.skipped;

    runs.spill(&mut counts.compounds)?;
    let compound_keys = runs.merge_into(&out_dir.join(COMPOUND_COUNTS_FILE))?;
    tsv::write(&out_dir.join(COMPONENT_COUNTS_FILE), &counts.components)?;
    let domain_keys = if domains.counts() {
        domain_runs.spill(&mut domain_counts.keys)?;
        Some(domain_runs.merge_into(&out_dir.join(DOMAIN_COUNTS_FILE))?)
    } else {
        None
    };
    let stats = CountStats {
        dump_file_name: dump::file_name(dump_path),
        dump_version: dump::version(dump_path),
        selection_condition: selection.condition().to_owned(),
        scanned_articles: dump.scanned,
        selected_articles: dump.selected,
        text_bytes: dump.text_bytes,
        skipped_articles: skipped,
        unknown_morphemes: unknown.counted(),
        domain_articles: domain::tallies(&domain_articles),
    };
    let stats_path = out_dir.join(COUNT_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} 記事を読み、{} 記事({} バイト)を採り、{} 記事を飛ばした。複合語のキーは {} 件、部品は {} 件である",
        dump.scanned,
        dump.selected,
        dump.text_bytes,
        skipped,
        compound_keys,
        counts.components.len()
    );
    if let Some(domain_keys) = domain_keys {
        println!("語と分野の組は {domain_keys} 件である");
        for tally in &stats.domain_articles {
            println!("  {} は {} 記事である", tally.domain, tally.documents);
        }
    }
    Ok(())
}

/// 記事のまとまりを数えた結果。
struct Counted {
    /// 複合語と部品の回数。
    counts: Counts,
    /// 語と分野の組の回数。
    domains: DomainCounts,
    /// 解析に失敗して飛ばした記事の数。
    skipped: u64,
}

/// 記事のまとまりを並列に数え、1 つの表へ畳む。解析に失敗した記事は、理由を
/// 書き出して飛ばす。1 記事の失敗で数時間の走行を落とさないためである。
///
/// 語と分野の組は記事ごとに数える。1 記事のキーの集合をその記事の分野へ足すので、
/// 同じ記事に何度も現れるキーも 1 回になる。
fn count_batch(batch: &[(String, DomainSet)], unknown: UnknownMorphemes) -> Counted {
    batch
        .par_iter()
        .map(|(text, article_domains)| {
            let mut counts = Counts::new(unknown);
            let mut skipped = 0;
            if let Err(error) = counts.add_text(text) {
                eprintln!("記事を飛ばす: {error}");
                skipped = 1;
            }
            let mut domains = DomainCounts::default();
            if !article_domains.is_empty() {
                domains.add_keys(*article_domains, counts.compounds.keys().map(|key| &**key));
            }
            Counted {
                counts,
                domains,
                skipped,
            }
        })
        .reduce(
            || Counted {
                counts: Counts::new(unknown),
                domains: DomainCounts::default(),
                skipped: 0,
            },
            |mut left, right| {
                left.counts.merge(right.counts);
                left.domains.merge(right.domains);
                left.skipped += right.skipped;
                left
            },
        )
}
