//! Wikipedia 日本語版の記事と日本語の技術文書から複合語の頻度統計を作り、akunuki
//! が引くフィルタと部品の頻度表を書く。
//!
//! 手順は 5 つに分かれる。`count` が入力を読んで回数の TSV を書き、
//! `dictionary-headwords` が解析辞書から、`article-titles` がダンプの記事名から
//! 見出しの TSV を書き、`build` がその 3 つから成果物を書き、`query` が成果物を
//! 引く。TSV を挟むのは、数え直し(数十分)とバケットの切り直し(数秒)を別々に
//! やり直せるようにするためである。
//!
//! `count` の入力は 2 通りある。Wikipedia のダンプは記事ごとの出現回数で数え、
//! `flatten` が書いた平文の技術文書は文書単位で数える。`build` は 2 つの回数の
//! TSV を重み付きで足してからバケットに丸めるので、どちらの入力も同じ `build` へ
//! 渡る。
//!
//! `count --domains` は、語と分野の組の回数も書く。`build` はその回数から
//! `domain_filter.bin` を組み、`judge --document-domain` がそれを引いて、文書の
//! 分野に現れない既存語を転用の候補として出す。
//!
//! `flatten` はこの 5 つの外にあり、GitHub から取った日本語
//! の技術文書を平文にする。`judge` も 5 つの外にあり、成果物を引いて語を既存語と
//! グレーと転用の候補と造語の候補へ振り分ける。正解つきの語の集合を渡すと混同
//! 行列を出す。
//! `topic-distribution` も 5 つの外にあり、正解つきの語の集合が Wikipedia の記事の
//! 分野の分布でどれだけ情報技術に偏るかを測る。

mod build;
mod checksum;
mod compound;
mod count;
mod domain;
mod domain_filter;
mod dump;
mod flatten;
mod headwords;
mod judge;
mod query;
mod titles;
mod topic;
mod topic_distribution;
mod tsv;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use crate::build::Artifact;
use crate::compound::UnknownMorphemes;
use crate::domain::{Domain, DomainCounting};
use crate::domain_filter::Registration;
use crate::dump::Selection;
use crate::headwords::ProperNouns;
use crate::judge::ConcatenationRule;
use crate::judge::composition::Thresholds;

/// コマンドライン。
#[derive(Parser)]
#[command(about = "Wikipedia の記事と日本語の技術文書から複合語の頻度統計を作る")]
struct Cli {
    /// 実行するサブコマンド。
    #[command(subcommand)]
    command: Command,
}

/// サブコマンド。
#[derive(Subcommand)]
enum Command {
    /// 入力を読んで複合語と部品の回数を TSV に書く。
    Count {
        /// 数える入力。
        #[command(flatten)]
        input: CountInput,
        /// 回数の TSV と数えた記録を書くディレクトリ。
        #[arg(long)]
        out_dir: PathBuf,
        /// 先頭のこの数の記事か文書だけを読む。動きの確認に使う。
        #[arg(long)]
        limit: Option<u64>,
        /// 解析辞書に無いカタカナと英字の 1 形態素を、複合語のキーとして数えない。
        /// 既定はこの形の 1 形態素も数える。
        #[arg(long)]
        count_known_only: bool,
        /// 語と分野の組の回数も書く。記事の分野は話題の予測から決め、技術文書は
        /// 情報技術、日本語コーパスはソースの機関と文体で決める。
        #[arg(long)]
        domains: bool,
    },
    /// 解析辞書の見出しのうち、分割単位 A で 2 形態素以上に割れる名詞を TSV に
    /// 書く。
    DictionaryHeadwords {
        /// 配布物を展開した CSV のディレクトリ。
        #[arg(long)]
        lexicon_dir: PathBuf,
        /// 見出しの TSV と読んだ記録を書くディレクトリ。
        #[arg(long)]
        out_dir: PathBuf,
        /// 固有名詞の見出しも書く。既定は固有名詞の見出しを書かない。
        #[arg(long)]
        proper_nouns: bool,
    },
    /// ダンプの記事名とリダイレクト名のうち、分割単位 A で 2 形態素以上に割れ、
    /// 固有名詞を含まない名前を TSV に書く。
    ArticleTitles {
        /// cirrussearch のダンプ。
        #[arg(long)]
        dump: PathBuf,
        /// 記事名の TSV と読んだ記録を書くディレクトリ。
        #[arg(long)]
        out_dir: PathBuf,
        /// 先頭のこの数の記事だけを読む。動きの確認に使う。
        #[arg(long)]
        limit: Option<u64>,
    },
    /// 取得した日本語の技術文書を、1 文書 1 ファイルの平文にする。
    Flatten {
        /// 取得物と `manifest.tsv` を置いたディレクトリ。
        #[arg(long)]
        raw_dir: PathBuf,
        /// 平文を書くディレクトリ。この下に `text/` と `held-out/` ができる。
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// 回数と見出しの TSV からフィルタと部品の頻度表と manifest を書く。
    Build {
        /// Wikipedia を数えた `count` と `dictionary-headwords` の出力。
        #[arg(long)]
        counts_dir: PathBuf,
        /// 技術文書を数えた `count` の出力。渡さなければ Wikipedia だけで組む。
        #[arg(long)]
        docs_counts_dir: Option<PathBuf>,
        /// 日本語コーパスを数えた `count` の出力。渡さなければ足さない。
        #[arg(long)]
        corpus_counts_dir: Option<PathBuf>,
        /// 記事名を読んだ `article-titles` の出力。渡さなければ記事名を登録しない。
        #[arg(long)]
        titles_dir: Option<PathBuf>,
        /// Wikipedia の出現回数に掛ける重み。
        #[arg(long, default_value_t = 1.0)]
        counts_weight: f64,
        /// 技術文書の文書数に掛ける重み。
        #[arg(long, default_value_t = 1.0)]
        docs_weight: f64,
        /// 日本語コーパスの文書数に掛ける重み。
        #[arg(long, default_value_t = 1.0)]
        corpus_weight: f64,
        /// フィルタに入れる複合語の回数の下限。
        #[arg(long, default_value_t = build::DEFAULT_MIN_COUNT)]
        min_count: u64,
        /// 部品の頻度表に入れる頻度の下限。
        #[arg(long, default_value_t = build::DEFAULT_MIN_COMPONENT_COUNT)]
        min_component_count: u64,
        /// 分野のフィルタに登録する、語がその分野で現れる記事か文書の数の下限。
        #[arg(long, default_value_t = build::DEFAULT_DOMAIN_MIN_COUNT)]
        domain_min_count: u64,
        /// 分野のフィルタで一般に畳む、登録される分野の数の下限。
        #[arg(long, default_value_t = build::DEFAULT_GENERAL_THRESHOLD)]
        general_threshold: usize,
        /// 分野のフィルタに登録する分野を、語ごとの総数に対する割合で決める。
        /// 渡すと --domain-min-count と --general-threshold は使わない。
        #[arg(long, conflicts_with_all = ["domain_min_count", "general_threshold"])]
        domain_share_threshold: Option<f64>,
        /// 割合で登録するとき、総数がこの数以上の語を割合によらず一般で登録する。
        #[arg(long, requires = "domain_share_threshold")]
        general_min_total: Option<u64>,
        /// 組む成果物の種類。来歴のファイル名と項目に残る。
        #[arg(long, value_enum, default_value_t = Artifact::default())]
        artifact: Artifact,
        /// 成果物の版の名前。`dashi-<年>.<月>.<連番>` の形で渡す。月は 0 埋め
        /// する。作り直すたびに連番を上げる。
        #[arg(long)]
        version: String,
        /// 成果物を書くディレクトリ。
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// 語を振り分け、正解つきの語の集合を渡すと混同行列を出す。
    Judge {
        /// `build` が書いたディレクトリ。
        #[arg(long)]
        artifacts_dir: PathBuf,
        /// 部品の頻度の比の閾値。これを超えた未登録の語は、理由が部品になる。
        #[arg(long, default_value_t = judge::DEFAULT_THRESHOLD)]
        threshold: f64,
        /// 3 形態素以上の語に当てる 2 つ目の検査。
        #[arg(long, value_enum, default_value_t = ConcatenationRule::default())]
        concatenation: ConcatenationRule,
        /// 構成の分析で造語の候補と決める点数の下限。
        #[arg(long, default_value_t = judge::DEFAULT_UPPER_THRESHOLD)]
        upper_threshold: u64,
        /// 構成の分析でグレーと決める点数の下限。
        #[arg(long, default_value_t = judge::DEFAULT_LOWER_THRESHOLD)]
        lower_threshold: u64,
        /// 構成の分析が単位の回数を読む、Wikipedia を数えた `count` の出力。
        #[arg(long, required_if_eq("concatenation", "composition"))]
        counts_dir: Option<PathBuf>,
        /// 構成の分析が単位の回数を読む、技術文書を数えた `count` の出力。
        #[arg(long)]
        docs_counts_dir: Option<PathBuf>,
        /// 解析辞書に無いカタカナと英字の 1 形態素も候補にする。
        #[arg(long)]
        unknown_morphemes: bool,
        /// 判定する文書の分野。渡すと、完全一致で既存語と決まった語のうち、
        /// この分野にも一般にも現れない語を転用の候補として出す。
        #[arg(long, value_parser = parse_document_domain)]
        document_domain: Option<Domain>,
        /// 造語の例と既存語の例の TSV。渡すと混同行列を出す。造語の例は 4 列目に
        /// 3 区分のどれかを持ち、造語 の行だけが再現率の母数になる。既存語の例は
        /// 複数渡せて、集合ごとに別の行で振り分けと誤検出率が出る。
        #[arg(long, num_args = 2.., value_names = ["COINED", "ESTABLISHED"])]
        eval: Option<Vec<PathBuf>>,
        /// 判定する語。
        words: Vec<String>,
    },
    /// 成果物を読み、語のバケットと各部品の頻度と相手の種類数を表示する。
    Query {
        /// `build` が書いたディレクトリ。
        #[arg(long)]
        artifacts_dir: PathBuf,
        /// 引く語。
        words: Vec<String>,
    },
    /// 正解つきの語の集合を Wikipedia のダンプで走査し、語ごとに分野の分布の
    /// TSV を書く。
    TopicDistribution {
        /// 走査する語の一覧とダンプ。
        #[command(flatten)]
        input: topic_distribution::Input,
        /// 分野の分布の TSV を書くディレクトリ。
        #[arg(long)]
        out_dir: PathBuf,
    },
}

/// `count` が数える入力。3 つは排他であり、どれか 1 つを渡す。
#[derive(clap::Args)]
struct CountInput {
    /// cirrussearch のダンプ。記事ごとの出現回数で数える。
    #[arg(
        long,
        conflicts_with_all = ["text_dir", "corpus_dir"],
        required_unless_present_any = ["text_dir", "corpus_dir"]
    )]
    dump: Option<PathBuf>,
    /// 情報技術の記事だけを数える。既定は名前空間 0 の全記事である。
    #[arg(long, conflicts_with_all = ["text_dir", "corpus_dir"])]
    topics: bool,
    /// `flatten` が書いた平文のディレクトリ。1 ファイルを 1 文書として数える。
    #[arg(long, conflicts_with = "corpus_dir")]
    text_dir: Option<PathBuf>,
    /// メンテナーが集めた日本語コーパスのディレクトリ。ソースごとのディレクトリの
    /// `text/` を、技術文書と同じ文書単位で数える。
    #[arg(long)]
    corpus_dir: Option<PathBuf>,
    /// 数えないソースの名前。読まないソースと、正解集合の採り先のソースを外す。
    /// 平文の技術文書と日本語コーパスに効き、ダンプには効かない。
    #[arg(long, value_delimiter = ',', conflicts_with = "dump")]
    exclude: Vec<String>,
}

/// 渡された入力を数える。3 つの入力は排他であり、どれかは要る。clap がその両方を
/// 強制する。
fn count(
    input: CountInput,
    out_dir: &Path,
    limit: Option<u64>,
    unknown: UnknownMorphemes,
    domains: DomainCounting,
) -> Result<()> {
    if let Some(corpus_dir) = input.corpus_dir {
        count::corpus::run(
            &corpus_dir,
            out_dir,
            &input.exclude,
            limit,
            unknown,
            domains,
        )
    } else if let Some(text_dir) = input.text_dir {
        count::documents::run(&text_dir, out_dir, &input.exclude, limit, unknown, domains)
    } else if let Some(dump) = input.dump {
        let selection = Selection::from_topics(input.topics);
        count::run(&dump, out_dir, selection, limit, unknown, domains)
    } else {
        bail!("--dump か --text-dir か --corpus-dir のどれかを渡す")
    }
}

/// 文書の分野を名前から読む。一般は語の側の区分なので、文書の分野にならない。
fn parse_document_domain(label: &str) -> Result<Domain, String> {
    match Domain::from_label(label) {
        Some(Domain::General) | None => Err(format!(
            "'{label}' は文書の分野でない。次のどれかを渡す: {}",
            document_domain_labels()
        )),
        Some(domain) => Ok(domain),
    }
}

/// 文書の分野になる区分の名前を並べたもの。
fn document_domain_labels() -> String {
    Domain::ALL
        .into_iter()
        .filter(|domain| *domain != Domain::General)
        .map(Domain::label)
        .collect::<Vec<_>>()
        .join("、")
}

#[expect(
    clippy::too_many_lines,
    reason = "サブコマンドごとに 1 つの分岐を持つ振り分けであり、分けても読みやすくならない"
)]
fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Count {
            input,
            out_dir,
            limit,
            count_known_only,
            domains,
        } => count(
            input,
            &out_dir,
            limit,
            UnknownMorphemes::from_counted(!count_known_only),
            DomainCounting::from_counted(domains),
        ),
        Command::DictionaryHeadwords {
            lexicon_dir,
            out_dir,
            proper_nouns,
        } => {
            let proper_nouns = if proper_nouns {
                ProperNouns::Include
            } else {
                ProperNouns::Exclude
            };
            headwords::run(&lexicon_dir, &out_dir, proper_nouns)
        }
        Command::ArticleTitles {
            dump,
            out_dir,
            limit,
        } => titles::run(&dump, &out_dir, limit),
        Command::Flatten { raw_dir, out_dir } => flatten::run(&raw_dir, &out_dir),
        Command::Build {
            counts_dir,
            docs_counts_dir,
            corpus_counts_dir,
            titles_dir,
            counts_weight,
            docs_weight,
            corpus_weight,
            min_count,
            min_component_count,
            domain_min_count,
            general_threshold,
            domain_share_threshold,
            general_min_total,
            artifact,
            version,
            out_dir,
        } => build::run(
            &build::Inputs {
                counts_dir,
                docs_counts_dir,
                corpus_counts_dir,
                titles_dir,
                weights: build::Weights {
                    counts: counts_weight,
                    docs: docs_weight,
                    corpus: corpus_weight,
                },
                pruning: build::Pruning {
                    min_count,
                    min_component_count,
                },
                domain: match domain_share_threshold {
                    Some(threshold) => Registration::Share {
                        threshold,
                        general_min_total,
                    },
                    None => Registration::Count {
                        min_count: domain_min_count,
                        general_threshold,
                    },
                },
                artifact,
                version,
            },
            &out_dir,
        ),
        Command::Judge {
            artifacts_dir,
            threshold,
            concatenation,
            upper_threshold,
            lower_threshold,
            counts_dir,
            docs_counts_dir,
            unknown_morphemes,
            document_domain,
            eval,
            words,
        } => judge::run(
            &artifacts_dir,
            judge::Options {
                threshold,
                concatenation,
                thresholds: Thresholds {
                    upper: upper_threshold,
                    lower: lower_threshold,
                },
                unknown: UnknownMorphemes::from_counted(unknown_morphemes),
                document_domain,
            },
            judge::CountsDirs {
                counts_dir: counts_dir.as_deref(),
                docs_counts_dir: docs_counts_dir.as_deref(),
            },
            eval.as_deref(),
            &words,
        ),
        Command::Query {
            artifacts_dir,
            words,
        } => query::run(&artifacts_dir, &words),
        Command::TopicDistribution { input, out_dir } => topic_distribution::run(&input, &out_dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 引数を渡さずに `command` を組み立てる。
    fn parse(arguments: &[&str]) -> Command {
        Cli::parse_from(std::iter::once("corpus-tool").chain(arguments.iter().copied())).command
    }

    #[test]
    fn countの既定は未知語1形態素を数える() {
        let Command::Count {
            count_known_only, ..
        } = parse(&["count", "--dump", "dump.gz", "--out-dir", "counts"])
        else {
            panic!("count のはず");
        };
        assert!(!count_known_only);
        // 古い形は引数で選べる。
        let Command::Count {
            count_known_only, ..
        } = parse(&[
            "count",
            "--dump",
            "dump.gz",
            "--out-dir",
            "counts",
            "--count-known-only",
        ])
        else {
            panic!("count のはず");
        };
        assert!(count_known_only);
    }

    #[test]
    fn 見出しの既定は固有名詞を書かない() {
        let Command::DictionaryHeadwords { proper_nouns, .. } = parse(&[
            "dictionary-headwords",
            "--lexicon-dir",
            "lex",
            "--out-dir",
            "counts",
        ]) else {
            panic!("dictionary-headwords のはず");
        };
        assert!(!proper_nouns);
        // 古い形は引数で選べる。
        let Command::DictionaryHeadwords { proper_nouns, .. } = parse(&[
            "dictionary-headwords",
            "--lexicon-dir",
            "lex",
            "--out-dir",
            "counts",
            "--proper-nouns",
        ]) else {
            panic!("dictionary-headwords のはず");
        };
        assert!(proper_nouns);
    }

    #[test]
    fn buildの既定は部品を3回で切り配布用を組む() {
        let Command::Build {
            min_count,
            min_component_count,
            domain_min_count,
            general_threshold,
            domain_share_threshold,
            artifact,
            ..
        } = parse(&[
            "build",
            "--counts-dir",
            "counts",
            "--out-dir",
            ".",
            "--version",
            "test-version",
        ])
        else {
            panic!("build のはず");
        };
        assert_eq!(min_count, 3);
        assert_eq!(min_component_count, 3);
        // 分野のフィルタの既定は、5 記事か 5 文書の下限と、5 分野で一般に畳む
        // 閾値である。
        assert_eq!(domain_min_count, 5);
        assert_eq!(general_threshold, 5);
        assert_eq!(artifact, Artifact::Release);
        // 割合の登録は引数で選ぶ。既定では渡らない。
        assert_eq!(domain_share_threshold, None);
        // 評価用は引数で選ぶ。
        let Command::Build { artifact, .. } = parse(&[
            "build",
            "--counts-dir",
            "counts",
            "--out-dir",
            ".",
            "--version",
            "test-version",
            "--artifact",
            "evaluation",
        ]) else {
            panic!("build のはず");
        };
        assert_eq!(artifact, Artifact::Evaluation);
    }

    #[test]
    fn 割合の登録は件数の引数と併せて渡せない() {
        let Command::Build {
            domain_share_threshold,
            ..
        } = parse(&[
            "build",
            "--counts-dir",
            "counts",
            "--out-dir",
            ".",
            "--version",
            "test-version",
            "--domain-share-threshold",
            "0.10",
        ])
        else {
            panic!("build のはず");
        };
        assert_eq!(domain_share_threshold, Some(0.10));
        // 総数で一般に畳む下限は、割合の閾値と併せてしか渡せない。
        assert!(
            Cli::try_parse_from([
                "corpus-tool",
                "build",
                "--counts-dir",
                "counts",
                "--out-dir",
                ".",
                "--version",
                "test-version",
                "--general-min-total",
                "1000",
            ])
            .is_err()
        );
        // 件数の下限と割合の閾値は、どちらで登録するかが決まらないので併せて
        // 渡せない。
        assert!(
            Cli::try_parse_from([
                "corpus-tool",
                "build",
                "--counts-dir",
                "counts",
                "--out-dir",
                ".",
                "--version",
                "test-version",
                "--domain-share-threshold",
                "0.10",
                "--domain-min-count",
                "5",
            ])
            .is_err()
        );
    }

    #[test]
    fn judgeの既定は連結の検査をやめる() {
        let Command::Judge { concatenation, .. } = parse(&["judge", "--artifacts-dir", "."]) else {
            panic!("judge のはず");
        };
        assert_eq!(concatenation, ConcatenationRule::Off);
        // 古い形は引数で選べる。
        let Command::Judge { concatenation, .. } = parse(&[
            "judge",
            "--artifacts-dir",
            ".",
            "--concatenation",
            "established",
        ]) else {
            panic!("judge のはず");
        };
        assert_eq!(concatenation, ConcatenationRule::Established);
    }
}
