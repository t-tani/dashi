//! 語と分野の組を登録したフィルタ。構築と照合とファイル名を持つ。
//!
//! キーは `<分野の番号><タブ><語>` の文字列であり、値は持たない。照合は分野の数
//! だけキーを組んで引き、当たった分野を集めて返す。頻度のフィルタが持つバケット
//! を分野の番号に使えないのは、[`FrequencyFilter::bucket`] が当たったバケットの
//! うち最大の 1 つだけを返し、分野の集合を取れないためである。
//!
//! [`FrequencyFilter::bucket`] は 1 回の照合で 7 通りのバケットを試すので、登録の
//! 無い組が誤って当たる率は 7 かける 65536 分の 1 である。1 語につき分野の数だけ
//! 引くので、語ごとの率はその 8 倍のおよそ 0.09% になる。
//!
//! 登録の条件は 2 通りある。[`Registration::Count`] は、語がその分野の中で下限
//! 以上の記事か文書に現れる分野で登録する。[`Registration::Share`] は、語ごとの
//! 総数に対する分野の割合が閾値以上の分野で登録する。どちらも、分野をまたぐ語は
//! 分野ごとの登録の代わりに [`Domain::General`] の 1 つで登録する。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aku_freq::{Bucket, FrequencyFilter};
use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::count::DOMAIN_COUNTS_FILE;
use crate::domain::{self, DOMAIN_COUNT, Domain, DomainSet};
use crate::tsv;

/// 分野のフィルタを書くファイル名。
pub const DOMAIN_FILTER_FILE: &str = "domain_filter.bin";

/// 登録に使うバケット。[`FrequencyFilter::bucket`] は 7 から 1 へ順に試すので、
/// 7 で登録すると当たるキーが 1 回目の照合で返る。分野は値ではなくキーが持つので、
/// このバケットは何も表さない。
const REGISTERED_BUCKET: u8 = 7;

/// 割合で登録するときに要る、語ごとの総数の下限。総数がこれに満たない語を登録
/// しないのは、数記事しか無い語の割合が 1 記事で大きく動くためである。
const MIN_TOTAL: u64 = 5;

/// 割合で登録するときに一般へ畳む、閾値以上の分野の数の下限。この数の分野で
/// 閾値に届く語は、分野をまたいで使われる語である。
const GENERAL_DOMAINS: usize = 3;

/// 登録の条件。分野の中の件数で決める形と、語ごとの総数に対する分野の割合で
/// 決める形がある。
#[derive(Debug, Clone, Copy)]
pub enum Registration {
    /// 分野の中の件数で登録する。
    Count {
        /// 語がその分野で現れる記事か文書の数の下限。
        min_count: u64,
        /// 一般に畳む、登録される分野の数の下限。
        general_threshold: usize,
    },
    /// 語ごとの総数に対する分野の割合で登録する。
    Share {
        /// 登録する分野の割合の下限。
        threshold: f64,
        /// 割合によらず一般で登録する、語の総数の下限。渡さなければ総数では
        /// 畳まない。
        general_min_total: Option<u64>,
    },
}

impl Registration {
    /// `totals` の回数を持つ語を登録する分野。どの分野にも登録しない語では空の
    /// 集合を返す。`totals` は [`Domain::ALL`] と同じ並びの、分野ごとの記事か
    /// 文書の数である。
    ///
    /// 割合の分母は分野ごとの数の和である。1 記事が複数の分野を持つ場合、その
    /// 記事は持つ分野の数だけ和に入るので、分野をまたぐ記事に現れる語ほど 1 つの
    /// 分野の割合が下がる。
    ///
    /// [`Registration::Share`] の `general_min_total` は、割合を見る前に当たる。
    /// 総数がその下限に届く語は、どの分野にどれだけ偏っていても一般で登録する。
    #[expect(
        clippy::cast_precision_loss,
        reason = "回数は記事と文書の数であり、f64 が正確に表せる範囲に収まる"
    )]
    fn domains(self, totals: &[u64; DOMAIN_COUNT]) -> DomainSet {
        match self {
            Self::Count {
                min_count,
                general_threshold,
            } => {
                let domains = select(totals, |count| count >= min_count);
                fold_general(domains, general_threshold)
            }
            Self::Share {
                threshold,
                general_min_total,
            } => {
                let total: u64 = totals.iter().sum();
                if total < MIN_TOTAL {
                    return DomainSet::default();
                }
                if general_min_total.is_some_and(|minimum| total >= minimum) {
                    return DomainSet::only(Domain::General);
                }
                let domains = select(totals, |count| count as f64 >= threshold * total as f64);
                if domains.is_empty() || domains.len() >= GENERAL_DOMAINS {
                    DomainSet::only(Domain::General)
                } else {
                    domains
                }
            }
        }
    }
}

/// `keep` が真を返した分野の集合。`totals` は [`Domain::ALL`] と同じ並びである。
fn select(totals: &[u64; DOMAIN_COUNT], keep: impl Fn(u64) -> bool) -> DomainSet {
    let mut domains = DomainSet::default();
    for (domain, count) in Domain::ALL.into_iter().zip(totals) {
        if keep(*count) {
            domains.insert(domain);
        }
    }
    domains
}

/// 組んで書き出したフィルタの大きさ。manifest に写す。
pub struct Written {
    /// 登録したキーの数。
    pub keys: u32,
    /// フィルタのバイト数。
    pub bytes: usize,
    /// フィルタの SHA-256(16 進)。
    pub sha256: String,
    /// 分野ごとに登録した語の数。
    pub registered: Vec<DomainKeys>,
}

/// 1 つの分野に登録した語の数。
#[derive(Serialize)]
pub struct DomainKeys {
    /// 分野の名前。
    pub domain: String,
    /// その分野で登録した語の数。
    pub keys: u64,
}

/// `counts_dirs` の分野の回数からフィルタを組み、`out_dir` へ書く。先頭の
/// ディレクトリは行ごとに読み、残りは表に載せてから足す。Wikipedia の分野の回数
/// は数千万行あるので、全体を表に載せない。
///
/// # Errors
///
/// 回数を読めない場合、回数のキーが `<語><タブ><分野の番号>` の形でない場合、
/// フィルタを組めない場合、書き出しが失敗する場合に返す。
pub fn write(
    counts_dirs: &[PathBuf],
    registration: Registration,
    dictionary_version: &str,
    out_dir: &Path,
) -> Result<Written> {
    let [primary, rest @ ..] = counts_dirs else {
        bail!("分野の回数を読むディレクトリが渡されていない");
    };
    let mut registering = Registering::new(registration);
    for dir in rest {
        let path = dir.join(DOMAIN_COUNTS_FILE);
        for (key, count) in tsv::read(&path)? {
            registering.add_other(&path, &key, count);
        }
    }
    let primary_path = primary.join(DOMAIN_COUNTS_FILE);
    tsv::for_each_row(&primary_path, |key, count| {
        registering.add_row(&primary_path, key, count);
    })?;
    let registered = registering.finish();
    if let Some((path, key)) = registered.unreadable {
        bail!(
            "{} の分野の回数のキー '{key}' が <語><タブ><分野の番号> の形でない。`count --domains` で数え直す",
            path.display()
        );
    }

    let filter = FrequencyFilter::build(&registered.entries, dictionary_version)
        .context("分野のフィルタを組めない")?;
    let bytes = filter.to_bytes();
    let sha256 = crate::checksum::sha256_hex(&bytes);
    let path = out_dir.join(DOMAIN_FILTER_FILE);
    std::fs::write(&path, &bytes).with_context(|| format!("{} を書けない", path.display()))?;
    Ok(Written {
        keys: filter.key_count(),
        bytes: bytes.len(),
        sha256,
        registered: Domain::ALL
            .into_iter()
            .zip(registered.counted)
            .map(|(domain, keys)| DomainKeys {
                domain: domain.label().to_owned(),
                keys,
            })
            .collect(),
    })
}

/// 回数の行を語ごとにまとめ、登録する組を決める。
///
/// 先頭の入力の回数はキーの昇順に並ぶので、1 つの語の行は隣り合う。語が変わった
/// ところで、それまでに積んだ分野ごとの回数を登録に掛ける。この読み方なら、
/// 数千万件の組を表に載せずに語ごとの総数と割合を出せる。
struct Registering {
    /// 登録の条件。
    registration: Registration,
    /// 登録に使うバケット。
    bucket: Bucket,
    /// 先頭より後ろの入力が数えた回数。語ごとに持ち、先頭の入力を読みながら引く。
    added: HashMap<Box<str>, [u64; DOMAIN_COUNT]>,
    /// 読み進めている語。
    word: String,
    /// 読み進めている語があるか。語が空のキーもあるので、`word` の中身では
    /// 語の切れ目を判断できない。
    started: bool,
    /// 読み進めている語の、分野ごとの回数。
    totals: [u64; DOMAIN_COUNT],
    /// 登録した組。
    entries: Vec<(String, Bucket)>,
    /// 分野ごとに登録した語の数。
    counted: [u64; DOMAIN_COUNT],
    /// 形を読めなかった最初のキーと、そのパス。
    unreadable: Option<(PathBuf, String)>,
}

/// 登録し終えた組。
struct Registered {
    /// 登録した組。
    entries: Vec<(String, Bucket)>,
    /// 分野ごとに登録した語の数。
    counted: [u64; DOMAIN_COUNT],
    /// 形を読めなかった最初のキーと、そのパス。
    unreadable: Option<(PathBuf, String)>,
}

impl Registering {
    /// `registration` の条件で登録する。
    fn new(registration: Registration) -> Self {
        Self {
            registration,
            bucket: Bucket::new(REGISTERED_BUCKET)
                .unwrap_or_else(|| unreachable!("7 はバケットの範囲に入る")),
            added: HashMap::new(),
            word: String::new(),
            started: false,
            totals: [0; DOMAIN_COUNT],
            entries: Vec::new(),
            counted: [0; DOMAIN_COUNT],
            unreadable: None,
        }
    }

    /// 先頭より後ろの入力の 1 行を、語ごとの回数へ足す。
    fn add_other(&mut self, path: &Path, key: &str, count: u64) {
        let Some((word, domain)) = domain::split_count_key(key) else {
            self.mark_unreadable(path, key);
            return;
        };
        let totals = self.added.entry(word.into()).or_insert([0; DOMAIN_COUNT]);
        let total = &mut totals[usize::from(domain.code() - 1)];
        *total = total.saturating_add(count);
    }

    /// 先頭の入力の 1 行を積む。語が変わっていれば、前の語を登録する。
    fn add_row(&mut self, path: &Path, key: &str, count: u64) {
        let Some((word, domain)) = domain::split_count_key(key) else {
            self.mark_unreadable(path, key);
            return;
        };
        if !self.started || self.word != word {
            self.flush();
            self.word.push_str(word);
            self.started = true;
        }
        let total = &mut self.totals[usize::from(domain.code() - 1)];
        *total = total.saturating_add(count);
    }

    /// 読み進めていた語を登録し、回数を空にする。
    fn flush(&mut self) {
        if !self.started {
            return;
        }
        let mut totals = std::mem::replace(&mut self.totals, [0; DOMAIN_COUNT]);
        if let Some(other) = self.added.remove(self.word.as_str()) {
            for (total, count) in totals.iter_mut().zip(other) {
                *total = total.saturating_add(count);
            }
        }
        register(
            &self.word,
            &totals,
            self.registration,
            self.bucket,
            &mut self.entries,
            &mut self.counted,
        );
        self.word.clear();
        self.started = false;
    }

    /// 読めなかった最初のキーを残す。
    fn mark_unreadable(&mut self, path: &Path, key: &str) {
        self.unreadable
            .get_or_insert_with(|| (path.to_path_buf(), key.to_owned()));
    }

    /// 残っている語を登録し、結果を返す。
    fn finish(mut self) -> Registered {
        self.flush();
        // 先頭の入力に無い語は、後ろの入力の回数だけで登録を決める。
        for (word, totals) in std::mem::take(&mut self.added) {
            register(
                &word,
                &totals,
                self.registration,
                self.bucket,
                &mut self.entries,
                &mut self.counted,
            );
        }
        Registered {
            entries: self.entries,
            counted: self.counted,
            unreadable: self.unreadable,
        }
    }
}

/// `word` を登録する分野の組を `entries` へ足し、分野ごとの語数を数える。
fn register(
    word: &str,
    totals: &[u64; DOMAIN_COUNT],
    registration: Registration,
    bucket: Bucket,
    entries: &mut Vec<(String, Bucket)>,
    counted: &mut [u64; DOMAIN_COUNT],
) {
    for domain in registration.domains(totals).iter() {
        entries.push((domain::key(domain, word), bucket));
        counted[usize::from(domain.code() - 1)] += 1;
    }
}

/// 登録する分野。分野の数が `threshold` 以上なら、分野ごとの登録の代わりに一般の
/// 1 つを返す。
fn fold_general(domains: DomainSet, threshold: usize) -> DomainSet {
    if domains.len() >= threshold {
        DomainSet::only(Domain::General)
    } else {
        domains
    }
}

/// 語が現れる分野の集合を引くフィルタ。
pub struct DomainFilter {
    filter: FrequencyFilter,
}

impl DomainFilter {
    /// 読み込んだフィルタから作る。
    #[must_use]
    pub fn new(filter: FrequencyFilter) -> Self {
        Self { filter }
    }

    /// `artifacts_dir` のフィルタを読む。`dictionary_version` は読み手が期待する
    /// 解析辞書の版である。
    ///
    /// # Errors
    ///
    /// フィルタを読めない場合に返す。
    pub fn open(artifacts_dir: &Path, dictionary_version: &str) -> Result<Self> {
        let path = artifacts_dir.join(DOMAIN_FILTER_FILE);
        let bytes = std::fs::read(&path).with_context(|| {
            format!(
                "{} を読めない。`build` に分野の回数を渡して組む",
                path.display()
            )
        })?;
        let filter = FrequencyFilter::from_bytes(&bytes, dictionary_version)
            .with_context(|| format!("{} を読めない", path.display()))?;
        Ok(Self::new(filter))
    }

    /// `word` が現れる分野の集合。登録が無ければ空の集合を返す。照合は分野の数
    /// だけ行う。
    #[must_use]
    pub fn domains(&self, word: &str) -> DomainSet {
        let mut found = DomainSet::default();
        for domain in Domain::ALL {
            if self.filter.bucket(&domain::key(domain, word)).is_some() {
                found.insert(domain);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解析辞書の版の見本。読み込みは文字列を照合するだけなので、実物の版でなくて
    /// よい。
    const DICTIONARY_VERSION: &str = "test-dictionary-20260101";

    /// 語と分野の組からフィルタを組む。
    fn filter_of(pairs: &[(Domain, &str)]) -> DomainFilter {
        let bucket = Bucket::new(REGISTERED_BUCKET).unwrap();
        let entries: Vec<(String, Bucket)> = pairs
            .iter()
            .map(|(domain, word)| (domain::key(*domain, word), bucket))
            .collect();
        DomainFilter::new(FrequencyFilter::build(&entries, DICTIONARY_VERSION).unwrap())
    }

    #[test]
    fn 特定の分野だけの語は登録した分野を返す() {
        let filter = filter_of(&[
            (Domain::Culture, "樹形図"),
            (Domain::OtherStem, "樹形図"),
            (Domain::InformationTechnology, "公開鍵暗号"),
        ]);
        let found = filter.domains("樹形図");
        assert_eq!(found.labels(), "その他の理工・文化");
        assert!(!found.contains(Domain::InformationTechnology));
        // 別の語の登録は混ざらない。
        assert_eq!(
            filter.domains("公開鍵暗号").labels(),
            Domain::InformationTechnology.label()
        );
        // 登録の無い語は空の集合を返す。
        assert!(filter.domains("幽霊参照").is_empty());
    }

    #[test]
    fn 一般の語は一般だけを返す() {
        let filter = filter_of(&[(Domain::General, "完了条件")]);
        let found = filter.domains("完了条件");
        assert!(found.contains(Domain::General));
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn 分野の数が閾値以上の語を一般に畳む() {
        let mut three = DomainSet::only(Domain::InformationTechnology);
        three.insert(Domain::Culture);
        three.insert(Domain::Geography);
        // 閾値 3 では 3 分野の語が一般の 1 つになる。
        assert_eq!(fold_general(three, 3), DomainSet::only(Domain::General));
        // 閾値 4 では、同じ語が分野ごとに登録される。
        assert_eq!(fold_general(three, 4), three);
        // 1 分野の語はどの閾値でも畳まれない。
        let one = DomainSet::only(Domain::Law);
        assert_eq!(fold_general(one, 3), one);
    }

    /// `totals` に分野ごとの回数を入れた配列。並びは [`Domain::ALL`] である。
    fn totals(counts: &[(Domain, u64)]) -> [u64; DOMAIN_COUNT] {
        let mut totals = [0; DOMAIN_COUNT];
        for (domain, count) in counts {
            totals[usize::from(domain.code() - 1)] = *count;
        }
        totals
    }

    /// 回数の TSV を `dir` へ書く。行はキーの昇順に並べる。
    fn write_counts(dir: &Path, rows: &[(&str, Domain, u64)]) {
        std::fs::create_dir_all(dir).unwrap();
        let mut lines: Vec<String> = rows
            .iter()
            .map(|(word, domain, count)| {
                let mut key = String::new();
                domain::push_count_key(&mut key, word, *domain);
                format!("{key}\t{count}\n")
            })
            .collect();
        lines.sort();
        std::fs::write(dir.join(DOMAIN_COUNTS_FILE), lines.concat()).unwrap();
    }

    #[test]
    fn 下限に届いた組だけを登録する() {
        let dir = std::env::temp_dir().join("corpus-tool-domain-filter-test");
        let counts_dir = dir.join("counts");
        let added_dir = dir.join("counts-docs");
        // 文化の 5 記事は下限に届き、地理の 2 記事は届かない。
        write_counts(
            &counts_dir,
            &[
                ("樹形図", Domain::Culture, 5),
                ("樹形図", Domain::Geography, 2),
                ("公開鍵暗号", Domain::InformationTechnology, 3),
            ],
        );
        // 別の入力の回数は、同じ組で足してから下限に当てる。
        write_counts(
            &added_dir,
            &[
                ("公開鍵暗号", Domain::InformationTechnology, 2),
                ("完了条件", Domain::Law, 9),
            ],
        );

        let written = write(
            &[counts_dir, added_dir],
            Registration::Count {
                min_count: 5,
                general_threshold: 3,
            },
            DICTIONARY_VERSION,
            &dir,
        )
        .unwrap();
        assert_eq!(written.keys, 3);

        let filter = DomainFilter::open(&dir, DICTIONARY_VERSION).unwrap();
        assert_eq!(filter.domains("樹形図").labels(), "文化");
        assert_eq!(filter.domains("公開鍵暗号").labels(), "情報技術");
        // 先頭のディレクトリに無い組も、残りのディレクトリから登録する。
        assert_eq!(filter.domains("完了条件").labels(), "法令");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn 割合が閾値に届いた分野で登録する() {
        // 10 記事のうち情報技術が 9 記事、その他の理工が 1 記事の語である。
        let counts = totals(&[(Domain::InformationTechnology, 9), (Domain::OtherStem, 1)]);
        // 閾値 0.20 では、割合 0.1 のその他の理工が落ちる。
        let share = Registration::Share {
            threshold: 0.20,
            general_min_total: None,
        };
        assert_eq!(share.domains(&counts).labels(), "情報技術");
        // 閾値 0.05 では同じ語の 2 分野とも届く。
        let low = Registration::Share {
            threshold: 0.05,
            general_min_total: None,
        };
        assert_eq!(low.domains(&counts).labels(), "情報技術・その他の理工");
        // 総数が 5 に満たない語は、割合がいくら高くても登録しない。
        let rare = totals(&[(Domain::InformationTechnology, 4)]);
        assert!(share.domains(&rare).is_empty());
        assert!(low.domains(&rare).is_empty());
        // 総数が 5 に届けば登録する。
        let least = totals(&[(Domain::InformationTechnology, 5)]);
        assert_eq!(share.domains(&least).labels(), "情報技術");
    }

    #[test]
    fn 割合で分野をまたぐ語を一般に畳む() {
        let share = Registration::Share {
            threshold: 0.20,
            general_min_total: None,
        };
        // 閾値に届く分野が 3 つある語は、分野ごとの登録の代わりに一般になる。
        let three = totals(&[
            (Domain::InformationTechnology, 4),
            (Domain::Culture, 3),
            (Domain::Geography, 3),
        ]);
        assert_eq!(share.domains(&three).labels(), "一般");
        // 2 つなら分野ごとに登録する。
        let two = totals(&[(Domain::InformationTechnology, 7), (Domain::Culture, 3)]);
        assert_eq!(share.domains(&two).labels(), "情報技術・文化");
        // どの分野も閾値に届かない語も一般になる。6 分野に均等な語の割合は
        // どれも 0.167 である。
        let even = totals(&[
            (Domain::InformationTechnology, 5),
            (Domain::OtherStem, 5),
            (Domain::Culture, 5),
            (Domain::Geography, 5),
            (Domain::Society, 5),
            (Domain::Administration, 5),
        ]);
        assert_eq!(share.domains(&even).labels(), "一般");
    }

    #[test]
    fn 総数が下限に届く語を一般に畳む() {
        let counts = totals(&[(Domain::InformationTechnology, 90), (Domain::Culture, 10)]);
        // 総数 100 が下限に届く語は、情報技術に 9 割偏っていても一般になる。
        let folded = Registration::Share {
            threshold: 0.20,
            general_min_total: Some(100),
        };
        assert_eq!(folded.domains(&counts).labels(), "一般");
        // 下限に届かない語は、割合の届いた分野で登録する。
        let kept = Registration::Share {
            threshold: 0.20,
            general_min_total: Some(101),
        };
        assert_eq!(kept.domains(&counts).labels(), "情報技術");
        // 総数の下限を渡さなければ、総数の大きさは登録を変えない。
        let none = Registration::Share {
            threshold: 0.20,
            general_min_total: None,
        };
        assert_eq!(none.domains(&counts).labels(), "情報技術");
        // 総数 5 未満の語は、総数の下限より先に落ちる。
        let rare = totals(&[(Domain::InformationTechnology, 4)]);
        assert!(folded.domains(&rare).is_empty());
    }

    #[test]
    fn 語ごとの総数を数えて割合で登録する() {
        let dir = std::env::temp_dir().join("corpus-tool-domain-share-test");
        let counts_dir = dir.join("counts");
        let added_dir = dir.join("counts-docs");
        write_counts(
            &counts_dir,
            &[
                // 正規化形が空になる区間のキー。語ごとの行の先頭に来る。
                ("", Domain::Culture, 100),
                ("公開鍵暗号", Domain::InformationTechnology, 9),
                ("公開鍵暗号", Domain::Culture, 1),
                ("樹形図", Domain::OtherStem, 4),
                ("樹形図", Domain::Culture, 5),
                ("樹形図", Domain::Geography, 1),
                ("監視対象", Domain::InformationTechnology, 2),
                ("監視対象", Domain::Culture, 2),
            ],
        );
        write_counts(
            &added_dir,
            &[
                ("公開鍵暗号", Domain::InformationTechnology, 1),
                ("完了条件", Domain::Law, 9),
            ],
        );

        let written = write(
            &[counts_dir, added_dir],
            Registration::Share {
                threshold: 0.20,
                general_min_total: None,
            },
            DICTIONARY_VERSION,
            &dir,
        )
        .unwrap();
        assert_eq!(written.keys, 5);

        let filter = DomainFilter::open(&dir, DICTIONARY_VERSION).unwrap();
        // 総数 10 のうち 4 と 5 を持つ 2 分野が届き、1 の地理は落ちる。空のキーの
        // 回数は、次の語の総数に混ざらない。
        assert_eq!(filter.domains("樹形図").labels(), "その他の理工・文化");
        assert_eq!(filter.domains("").labels(), "文化");
        // 別の入力の回数も同じ語の総数に入る。11 のうち情報技術が 10 である。
        assert_eq!(filter.domains("公開鍵暗号").labels(), "情報技術");
        assert_eq!(filter.domains("完了条件").labels(), "法令");
        // 総数 4 の語は、割合を出さずに落とす。
        assert!(filter.domains("監視対象").is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
