//! メンテナーが集めた日本語コーパスのディレクトリを読み、複合語と部品を文書単位で
//! 数える。
//!
//! コーパスはソースごとに 1 つのディレクトリを持ち、その下の `text/` に 1 文書
//! 1 ファイルの平文が並ぶ。数え方は技術文書と同じで、1 文書に何度現れても 1 回と
//! する。官公庁の文書は同じ注記と様式を文書ごとに繰り返すので、出現回数で数えると
//! その定型文の複合語が高い回数を持つ。
//!
//! ソースの列挙はディレクトリ名で行い、`manifest.jsonl` からは文体とライセンス
//! だけを読む。読まないソースと評価に取り置くソースは `--exclude` で外す。
//!
//! 法令のソースだけは、文書ごとに公布の時期でも外す。文語体の法令は解析辞書に
//! 合わず、意味のないキーを増やすためである。
//!
//! コーパスのディレクトリは記録しない。成果物を組み直す側が引数で渡すためである。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::compound::UnknownMorphemes;
use crate::count::documents::{Counted, Tallied, count_paths, domain_documents, text_files};
use crate::count::{COMPONENT_COUNTS_FILE, COMPOUND_COUNTS_FILE, DOMAIN_COUNTS_FILE};
use crate::domain::{self, Domain, DomainCounting, DomainTally};
use crate::tsv;

/// 数えた記録を書くファイル名。`build` がこれを読んで manifest に写す。
pub const CORPUS_STATS_FILE: &str = "corpus-stats.json";

/// 1 文書 1 行の記録のファイル名。ソースのディレクトリの直下にある。
const MANIFEST_FILE: &str = "manifest.jsonl";

/// 平文のディレクトリ。ソースのディレクトリの下にある。
const TEXT_DIR: &str = "text";

/// 法令の平文を持つソースの名前。このソースの文書だけ、法令 ID の公布の時期で
/// 文語体の法令を落とす。
const LAW_SOURCE: &str = "egov-law-api";

/// 口語体の法令が始まる昭和の年。昭和 22 年は 1947 年である。
const SPOKEN_STYLE_SHOWA_YEAR: u32 = 22;

/// 日本語コーパスを数えた記録。
#[derive(Serialize, Deserialize)]
pub struct CorpusStats {
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた平文のバイト数。
    pub text_bytes: u64,
    /// 解析に失敗して飛ばした文書の数。この文書の本文も `text_bytes` に入る。
    pub skipped_documents: u64,
    /// `--exclude` で外したソースの名前。
    pub excluded_sources: Vec<String>,
    /// 文語体として外した法令の数。
    pub excluded_laws: u64,
    /// 法令 ID の形を読めず、外さずに数えた法令の数。
    pub unreadable_law_ids: u64,
    /// 未知語 1 形態素を複合語のキーとして数えたか。
    pub unknown_morphemes: bool,
    /// 分野ごとに数えた文書の数。分野の組を数えなかった場合は空である。
    #[serde(default)]
    pub domain_documents: Vec<DomainTally>,
    /// ソースごとの内訳。
    pub sources: Vec<CorpusSourceStats>,
}

/// 1 ソースの内訳。文体とライセンスは `manifest.jsonl` から写す。
#[derive(Serialize, Deserialize)]
pub struct CorpusSourceStats {
    /// ソースの名前。ディレクトリ名である。
    pub name: String,
    /// 文書ごとの文体。1 ソースは 1 つの値を持つが、混ざれば全部が並ぶ。
    pub styles: Vec<String>,
    /// 文書ごとのライセンス。1 ソースは 1 つの値を持つが、混ざれば全部が並ぶ。
    pub licenses: Vec<String>,
    /// このソースに割り当てた分野。分野の組を数えなかった場合は `null` である。
    #[serde(default)]
    pub domain: Option<String>,
    /// 数えた文書の数。
    pub documents: u64,
    /// 数えた平文のバイト数。
    pub text_bytes: u64,
}

/// `manifest.jsonl` の 1 行のうち、記録に写す欄。
#[derive(Deserialize)]
struct ManifestRow {
    /// 原文のライセンス。
    license: String,
    /// 文書の文体。
    style: String,
}

/// `corpus_dir` の平文を文書単位で数え、`out_dir` に回数の TSV と記録を書く。
/// `exclude` に名前があるソースは読まない。`limit` を渡すと、パスの昇順で先頭の
/// その数の文書だけを読む。
///
/// `domains` が [`DomainCounting::Count`] なら、語と分野の組の回数も書く。ソース
/// の分野は機関と文体で決める([`Domain::from_corpus_source`])。
///
/// # Errors
///
/// コーパスのディレクトリを読めない場合、ソースに平文か記録が無い場合、文書を
/// 読めない場合、書き出しが失敗する場合に返す。
pub fn run(
    corpus_dir: &Path,
    out_dir: &Path,
    exclude: &[String],
    limit: Option<u64>,
    unknown: UnknownMorphemes,
    domains: DomainCounting,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("{} を作れない", out_dir.display()))?;
    let collected = collect(corpus_dir, exclude, domains)?;
    let Collected {
        mut sources,
        source_domains,
        mut paths,
        excluded_laws,
        unreadable_law_ids,
    } = collected;
    if let Some(limit) = limit {
        paths.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    }

    let Counted {
        counts,
        domains: domain_keys,
        tallied,
        skipped,
    } = count_paths(
        &paths,
        sources.len(),
        unknown,
        domains.counts().then_some(source_domains.as_slice()),
    )?;
    for (
        source,
        Tallied {
            documents,
            text_bytes,
        },
    ) in sources.iter_mut().zip(tallied)
    {
        source.documents = documents;
        source.text_bytes = text_bytes;
    }

    tsv::write(&out_dir.join(COMPOUND_COUNTS_FILE), &counts.compounds)?;
    tsv::write(&out_dir.join(COMPONENT_COUNTS_FILE), &counts.components)?;
    if domains.counts() {
        tsv::write(&out_dir.join(DOMAIN_COUNTS_FILE), &domain_keys.keys)?;
    }
    let stats = CorpusStats {
        documents: sources.iter().map(|source| source.documents).sum(),
        text_bytes: sources.iter().map(|source| source.text_bytes).sum(),
        skipped_documents: skipped,
        excluded_sources: exclude.to_vec(),
        excluded_laws,
        unreadable_law_ids,
        unknown_morphemes: unknown.counted(),
        domain_documents: domain::tallies(&domain_documents(
            &sources
                .iter()
                .map(|source| source.documents)
                .collect::<Vec<u64>>(),
            &source_domains,
            domains,
        )),
        sources,
    };
    let stats_path = out_dir.join(CORPUS_STATS_FILE);
    let json = serde_json::to_string_pretty(&stats).context("記録を JSON にできない")?;
    std::fs::write(&stats_path, json + "\n")
        .with_context(|| format!("{} を書けない", stats_path.display()))?;
    println!(
        "{} ソースの {} 文書({} バイト)を数え、{} 文書を飛ばした。複合語のキーは {} 件、部品は {} 件である",
        stats.sources.len(),
        stats.documents,
        stats.text_bytes,
        stats.skipped_documents,
        counts.compounds.len(),
        counts.components.len()
    );
    println!(
        "文語体として外した法令は {} 件、ID の形を読めず外さなかった法令は {} 件である",
        stats.excluded_laws, stats.unreadable_law_ids
    );
    Ok(())
}

/// 読むソースと平文のディレクトリ。
struct Collected {
    /// ソースごとの内訳。数える前なので、文書の数とバイト数は 0 である。
    sources: Vec<CorpusSourceStats>,
    /// ソースごとに割り当てた分野。`sources` と同じ並びである。
    source_domains: Vec<Domain>,
    /// ソースの番号と平文のディレクトリ。
    paths: Vec<(usize, PathBuf)>,
    /// 文語体として外した法令の数。
    excluded_laws: u64,
    /// 法令 ID の形を読めず、外さずに数えた法令の数。
    unreadable_law_ids: u64,
}

/// 読むソースを名前の昇順に並べ、その平文のディレクトリを集める。文語体の法令は
/// ここで落とす。
///
/// # Errors
///
/// コーパスのディレクトリを読めない場合、ソースに平文か記録が無い場合に返す。
fn collect(corpus_dir: &Path, exclude: &[String], domains: DomainCounting) -> Result<Collected> {
    let mut sources = Vec::new();
    let mut source_domains = Vec::new();
    let mut paths = Vec::new();
    let mut excluded_laws = 0;
    let mut unreadable_law_ids = 0;
    for name in source_names(corpus_dir, exclude)? {
        let source_dir = corpus_dir.join(&name);
        let (styles, licenses) = read_manifest(&source_dir.join(MANIFEST_FILE))?;
        let text_dir = source_dir.join(TEXT_DIR);
        if !text_dir.is_dir() {
            bail!(
                "ソース '{name}' の平文が {} に無い。読まないソースは --exclude で外す",
                text_dir.display()
            );
        }
        let index = sources.len();
        for path in text_files(&text_dir)? {
            if name == LAW_SOURCE {
                match promulgation_of(&path) {
                    Some(promulgation) if promulgation.is_classical_style() => {
                        excluded_laws += 1;
                        continue;
                    }
                    Some(_) => {}
                    None => unreadable_law_ids += 1,
                }
            }
            paths.push((index, path));
        }
        let domain = Domain::from_corpus_source(&name, &styles);
        source_domains.push(domain);
        sources.push(CorpusSourceStats {
            name,
            styles,
            licenses,
            domain: domains.counts().then(|| domain.label().to_owned()),
            documents: 0,
            text_bytes: 0,
        });
    }
    Ok(Collected {
        sources,
        source_domains,
        paths,
        excluded_laws,
        unreadable_law_ids,
    })
}

/// 法令 ID の先頭の桁が表す元号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Era {
    /// 明治。
    Meiji,
    /// 大正。
    Taisho,
    /// 昭和。
    Showa,
    /// 平成。
    Heisei,
    /// 令和。
    Reiwa,
}

impl Era {
    /// 法令 ID の先頭の桁から元号を決める。5 つの桁のどれでもなければ `None` を
    /// 返す。
    fn from_digit(digit: char) -> Option<Self> {
        match digit {
            '1' => Some(Self::Meiji),
            '2' => Some(Self::Taisho),
            '3' => Some(Self::Showa),
            '4' => Some(Self::Heisei),
            '5' => Some(Self::Reiwa),
            _ => None,
        }
    }
}

/// 法令を公布した元号と年。e-Gov の法令 ID は先頭の桁が元号、続く 2 桁がその元号
/// の年である。`105DF0000000337` なら明治 5 年である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Promulgation {
    /// 公布の元号。
    era: Era,
    /// その元号の年。
    year: u32,
}

impl Promulgation {
    /// 法令 ID から公布の時期を読む。先頭の 3 文字が元号の桁と 2 桁の年でなければ
    /// `None` を返す。
    fn from_law_id(id: &str) -> Option<Self> {
        let mut chars = id.chars();
        let era = Era::from_digit(chars.next()?)?;
        let year: u32 = id.get(1..3)?.parse().ok()?;
        Some(Self { era, year })
    }

    /// 文語体で書かれた時期の法令か。明治と大正の全部と、昭和 22 年より前が
    /// これにあたる。
    fn is_classical_style(self) -> bool {
        match self.era {
            Era::Meiji | Era::Taisho => true,
            Era::Showa => self.year < SPOKEN_STYLE_SHOWA_YEAR,
            Era::Heisei | Era::Reiwa => false,
        }
    }
}

/// 法令の平文のディレクトリから、公布の時期を読む。ファイル名の幹が法令 ID である。
fn promulgation_of(path: &Path) -> Option<Promulgation> {
    Promulgation::from_law_id(path.file_stem()?.to_str()?)
}

/// 読むソースの名前を、名前の昇順で返す。`exclude` に挙げた名前と、ディレクトリ
/// でない項目は落とす。
fn source_names(corpus_dir: &Path, exclude: &[String]) -> Result<Vec<String>> {
    let entries = std::fs::read_dir(corpus_dir)
        .with_context(|| format!("{} を読めない", corpus_dir.display()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("{} を読めない", corpus_dir.display()))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if exclude.contains(&name) {
            continue;
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

/// 1 文書 1 行の記録を読み、文書ごとの文体とライセンスを、それぞれ重複を畳んで
/// 返す。1 ソースの値は 1 つずつだが、混ざった場合に片方だけを記録すると、成果物
/// の来歴が実際の入力と食い違う。
fn read_manifest(path: &Path) -> Result<(Vec<String>, Vec<String>)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    let mut styles = BTreeSet::new();
    let mut licenses = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: ManifestRow = serde_json::from_str(line).with_context(|| {
            format!(
                "{} の {} 行目に license と style が無い",
                path.display(),
                index + 1
            )
        })?;
        styles.insert(row.style);
        licenses.insert(row.license);
    }
    if styles.is_empty() {
        bail!("{} に文書の行が無い", path.display());
    }
    Ok((styles.into_iter().collect(), licenses.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 ソース分の平文と記録を書く。
    fn write_source(corpus_dir: &Path, name: &str, style: &str, documents: &[(&str, &str)]) {
        let source_dir = corpus_dir.join(name);
        let text_dir = source_dir.join(TEXT_DIR);
        std::fs::create_dir_all(&text_dir).unwrap();
        let mut rows = Vec::new();
        for (file_name, text) in documents {
            std::fs::write(text_dir.join(format!("{file_name}.txt")), text).unwrap();
            rows.push(format!(
                r#"{{"id": "{file_name}", "license": "政府標準利用規約", "style": "{style}"}}"#
            ));
        }
        std::fs::write(source_dir.join(MANIFEST_FILE), rows.join("\n")).unwrap();
    }

    #[test]
    fn 昭和22年より前の法令を文語体として外す() {
        let classical = |id: &str| Promulgation::from_law_id(id).unwrap().is_classical_style();
        // 口語体の法令は昭和 22 年に始まるので、昭和 21 年と 22 年で扱いが分かれる。
        assert!(classical("321AC0000000001"));
        assert!(!classical("322AC0000000001"));
        // 明治と大正は全部が文語体、平成と令和は全部が口語体である。
        assert!(classical("105DF0000000337"));
        assert!(classical("212AC0000000033"));
        assert!(!classical("404AC0000000001"));
        assert!(!classical("505AC0000000001"));
    }

    #[test]
    fn 形を読めない法令idから公布の時期を読まない() {
        // 元号の桁は 1 から 5 であり、6 は元号を表さない。
        assert_eq!(Promulgation::from_law_id("605AC0000000001"), None);
        // 続く 2 桁が数でない ID と、3 文字に満たない ID も読めない。
        assert_eq!(Promulgation::from_law_id("3X2AC0000000001"), None);
        assert_eq!(Promulgation::from_law_id("32"), None);
    }

    #[test]
    fn 文語体の法令を数えず_idを読めない法令は数える() {
        let dir = std::env::temp_dir().join("corpus-tool-corpus-law-test");
        let corpus_dir = dir.join("corpus");
        write_source(
            &corpus_dir,
            "egov-law-api",
            "法令文",
            &[
                // 昭和 21 年の法令は文語体として外す。
                ("321AC0000000001", "本法ハ完了条件ヲ定ム。"),
                // 昭和 22 年の法令は数える。
                ("322AC0000000001", "この法律は完了条件を定める。"),
                // ID の形を読めない文書は、外さずに数える。
                ("draft-001", "この案は電子署名を定める。"),
            ],
        );

        let out_dir = dir.join("counts");
        run(
            &corpus_dir,
            &out_dir,
            &[],
            None,
            UnknownMorphemes::Skip,
            DomainCounting::Skip,
        )
        .unwrap();
        let stats: CorpusStats = serde_json::from_str(
            &std::fs::read_to_string(out_dir.join(CORPUS_STATS_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(stats.documents, 2);
        assert_eq!(stats.excluded_laws, 1);
        assert_eq!(stats.unreadable_law_ids, 1);
        let compounds: Vec<(String, u64)> = tsv::read(&out_dir.join(COMPOUND_COUNTS_FILE)).unwrap();
        // 外した法令にしかないキーは出ず、残る 2 文書のキーは出る。
        assert!(compounds.contains(&("完了条件".to_owned(), 1)));
        assert!(compounds.contains(&("電子署名".to_owned(), 1)));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn textの文書を数え_excludeで外したソースを数えない() {
        let dir = std::env::temp_dir().join("corpus-tool-corpus-dir-test");
        let corpus_dir = dir.join("corpus");
        write_source(
            &corpus_dir,
            "nco-standards",
            "指示文",
            &[
                ("kijun", "情報システムの完了条件を確かめる。"),
                ("tebiki", "完了条件を満たす。"),
            ],
        );
        write_source(
            &corpus_dir,
            "jawiki",
            "百科",
            &[("article", "電子署名は公開鍵暗号を使う。")],
        );

        // 外さなければ、2 つのソースの文書をすべて数える。
        let all = dir.join("counts-all");
        run(
            &corpus_dir,
            &all,
            &[],
            None,
            UnknownMorphemes::Skip,
            DomainCounting::Count,
        )
        .unwrap();
        let stats: CorpusStats =
            serde_json::from_str(&std::fs::read_to_string(all.join(CORPUS_STATS_FILE)).unwrap())
                .unwrap();
        assert_eq!(stats.documents, 3);
        assert_eq!(stats.sources.len(), 2);
        let compounds: Vec<(String, u64)> = tsv::read(&all.join(COMPOUND_COUNTS_FILE)).unwrap();
        // 2 文書に現れる複合語は 2、外したソースにだけある複合語も入る。
        assert!(compounds.contains(&("完了条件".to_owned(), 2)));
        assert!(compounds.contains(&("電子署名".to_owned(), 1)));
        // 語と分野の組は、ソースの機関で分かれる。nco は情報技術、機関の一覧に
        // 無い jawiki は行政である。
        let by_domain: Vec<(String, u64)> = tsv::read(&all.join(DOMAIN_COUNTS_FILE)).unwrap();
        assert!(by_domain.contains(&("完了条件\t1".to_owned(), 2)));
        assert!(by_domain.contains(&("電子署名\t6".to_owned(), 1)));
        assert_eq!(
            stats.domain_documents,
            vec![
                DomainTally {
                    domain: "情報技術".to_owned(),
                    documents: 2,
                },
                DomainTally {
                    domain: "行政".to_owned(),
                    documents: 1,
                },
            ]
        );

        // 外したソースの文書は数に入らず、そのキーも出ない。
        let kept = dir.join("counts-kept");
        run(
            &corpus_dir,
            &kept,
            &["jawiki".to_owned()],
            None,
            UnknownMorphemes::Skip,
            DomainCounting::Skip,
        )
        .unwrap();
        let stats: CorpusStats =
            serde_json::from_str(&std::fs::read_to_string(kept.join(CORPUS_STATS_FILE)).unwrap())
                .unwrap();
        assert_eq!(stats.documents, 2);
        assert_eq!(stats.excluded_sources, vec!["jawiki".to_owned()]);
        let source = &stats.sources[0];
        assert_eq!(source.name, "nco-standards");
        assert_eq!(source.styles, vec!["指示文".to_owned()]);
        assert_eq!(source.licenses, vec!["政府標準利用規約".to_owned()]);
        let compounds: Vec<(String, u64)> = tsv::read(&kept.join(COMPOUND_COUNTS_FILE)).unwrap();
        assert!(compounds.contains(&("完了条件".to_owned(), 2)));
        assert!(!compounds.iter().any(|(key, _)| key == "電子署名"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
