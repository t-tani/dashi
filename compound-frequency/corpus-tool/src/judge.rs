//! 語を既存語とグレーと転用の候補と造語の候補に振り分け、正解つきの語の集合で
//! 混同行列を出す。
//!
//! 判定は 3 つの検査からなる。完全一致で当たれば、そのバケットが名前を決める。
//! 当たらなければ、部品の頻度と閾値で理由を分け、造語の候補とする。
//!
//! `--document-domain` を渡すと、完全一致で既存語と決まった語について分野の集合を
//! 引く。集合が一般か文書の分野を含めば既存語のままとし、特定の分野だけで文書の
//! 分野を含まなければ転用の候補とする。
//!
//! `--concatenation` は、完全一致と部品の検査の間に、3 形態素以上の語への検査を
//! 挟む。`established` と `gray` は隣り合う 2 形態素の対を引き、対がすべて高頻度なら
//! 既存語の連結として名前を決める。`composition` はこの検査を [`composition`] の
//! 構成の分析に置き換える。既定の `off` はどちらも当てない。対の検査は
//! `未認証データ` のような圧縮された造語を既存語に倒すためである。

pub mod composition;

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use aku_freq::{ConstituentFrequencies, FrequencyFilter};
use aku_morph::{DICTIONARY_VERSION, MorphError, Morpheme, analyze_short};
use anyhow::{Context, Result, bail};

use crate::build::{CONSTITUENT_FILE, FILTER_FILE};
use crate::compound::{UnknownMorphemes, is_unknown_run};
use crate::domain::{Domain, DomainSet};
use crate::domain_filter::DomainFilter;
use crate::judge::composition::{Score, Thresholds, UnitFrequencies};

/// 部品の頻度の比の閾値の既定値。造語の例と既存語の例で閾値を振り、造語の候補に占める
/// 理由 部品 の割合の差がいちばん広がる範囲のうち、造語の例を最も多く残す値である。
/// 振った結果は `compound-frequency/README.md` にある。
pub const DEFAULT_THRESHOLD: f64 = 1000.0;

/// 構成の分析で造語の候補と決める点数の下限の既定値。
pub const DEFAULT_UPPER_THRESHOLD: u64 = 300;

/// 構成の分析でグレーと決める点数の下限の既定値。
///
/// 2 つの既定値は、造語の例と既存語の例で閾値を振り、いまの判定より造語の再現率が
/// 高い組のうち、既存語の誤検出率がいちばん低い組である。振った結果は
/// `compound-frequency/README.md` にある。
pub const DEFAULT_LOWER_THRESHOLD: u64 = 100;

/// 既存語と決めるバケットの下限。これに満たない登録はグレーになる。
const ESTABLISHED_BUCKET: u8 = 2;

/// 解析辞書の見出しに与えたバケット。
const DICTIONARY_BUCKET: u8 = 7;

/// 完全一致の次に当てる 2 つ目の検査の、形態素数の下限。2 形態素の語では、
/// 隣り合う対も左右の分割も 1 通りしかなく、3 つ目の検査と同じ組を見ることに
/// なる。
const SECOND_CHECK_MORPHEMES: usize = 3;

/// 正解つきの語の集合の TSV の見出し行。読み飛ばす。
const TSV_HEADER: &str = "語";

/// 正解つきの語の集合の TSV の、語の列。
const WORD_COLUMN: usize = 0;

/// 造語の例の TSV の、区分の列。
const CATEGORY_COLUMN: usize = 3;

/// 造語の例の区分。`eval/coined.tsv` の区分の列が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    /// 語そのものが存在しない語。この仕組みが拾う対象であり、再現率の母数は
    /// この区分だけからなる。
    Coined,
    /// 実在する用語だが、プロジェクトが別の表記を使う語。拾ってはいけない語
    /// なので、既存語の例と同じ向きで数える。
    Reworded,
    /// 実在する用語を別の意味で使った語。この仕組みは語の有無で判定するので、
    /// この区分は原理的に拾えない。
    Repurposed,
}

impl Category {
    /// 区分の列の値を区分にする。3 つの値のどれでもなければ `None` を返す。
    fn parse(field: &str) -> Option<Self> {
        match field {
            "造語" => Some(Self::Coined),
            "言い換え" => Some(Self::Reworded),
            "意味の転用" => Some(Self::Repurposed),
            _ => None,
        }
    }

    /// 表の行に書く区分の名前。
    fn label(self) -> &'static str {
        match self {
            Self::Coined => "造語",
            Self::Reworded => "言い換え",
            Self::Repurposed => "意味の転用",
        }
    }
}

/// 3 形態素以上の語に当てる 2 つ目の検査。LLM の造語は既存の部品をつないで
/// 作られるので、対がすべて高頻度なら既存語とする形は、`未認証データ` のような
/// 造語を既存語に倒す。既定はこの検査をやめる。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum ConcatenationRule {
    /// 対がすべて高頻度なら既存語とする。
    Established,
    /// 対がすべて高頻度ならグレーとする。
    Gray,
    /// 対を引く検査そのものをやめる。
    #[default]
    Off,
    /// 対を引く検査を構成の分析に置き換える。
    Composition,
}

/// 構成の分析が単位の回数を読む TSV のパス。
#[derive(Debug, Clone, Copy)]
pub struct CountsDirs<'a> {
    /// Wikipedia を数えた `count` の出力。
    pub counts_dir: Option<&'a Path>,
    /// 技術文書を数えた `count` の出力。
    pub docs_counts_dir: Option<&'a Path>,
}

/// 判定の切り替え。
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 部品の頻度の比の閾値。
    pub threshold: f64,
    /// 3 形態素以上の語に当てる 2 つ目の検査。
    pub concatenation: ConcatenationRule,
    /// 構成の分析の点数を名前に分ける 2 つの閾値。
    pub thresholds: Thresholds,
    /// 未知語 1 形態素を候補にするか。
    pub unknown: UnknownMorphemes,
    /// 判定する文書の分野。渡すと、完全一致で既存語と決まった語に分野の検査を
    /// 当てる。
    pub document_domain: Option<Domain>,
}

/// 判定の名前。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Verdict {
    /// コーパスか解析辞書にある語。
    Established,
    /// 登録はあるが回数が少ない語。
    Gray,
    /// 実在するが、文書の分野に現れない語。別の分野の語を転用した疑いがある。
    RepurposeCandidate,
    /// 登録が無く、造語の疑いがある語。
    Coined,
}

impl Verdict {
    /// 出力に書く名前。
    fn name(self) -> &'static str {
        match self {
            Self::Established => "既存語",
            Self::Gray => "グレー",
            Self::RepurposeCandidate => "転用の候補",
            Self::Coined => "造語の候補",
        }
    }
}

/// 判定がその名前になった理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reason {
    /// 解析辞書の見出しに完全一致した。
    Dictionary,
    /// コーパスで 10 回以上現れる語に完全一致した。
    HighFrequency,
    /// コーパスで 3 回から 9 回だけ現れる語に完全一致した。
    LowFrequency,
    /// 隣り合う 2 形態素の対がすべて高頻度だった。
    Concatenation,
    /// 構成の分析の点数が閾値の側で名前を決めた。
    Composition,
    /// 語が現れる分野に、文書の分野も一般も無かった。
    Domain,
    /// 部品の頻度の比が閾値を超えた。
    Constituent,
    /// どの検査でも当たらなかった。
    Unregistered,
}

impl Reason {
    /// 出力に書く名前。
    fn name(self) -> &'static str {
        match self {
            Self::Dictionary => "辞書",
            Self::HighFrequency => "高頻度",
            Self::LowFrequency => "低頻度",
            Self::Concatenation => "連結",
            Self::Composition => "構成",
            Self::Domain => "分野",
            Self::Constituent => "部品",
            Self::Unregistered => "未登録",
        }
    }
}

/// 1 語の判定。
#[derive(Debug, Clone, Copy)]
pub struct Judgment {
    /// 判定の名前。
    pub verdict: Verdict,
    /// その名前になった理由。
    pub reason: Reason,
    /// 構成の分析で、頻度が不明な単位のために点数に使えなかった分割があったか。
    pub unknown_unit: bool,
    /// 語が現れる分野。分野の検査を当てなかった語では空である。
    pub domains: DomainSet,
}

impl fmt::Display for Judgment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}\t{}", self.verdict.name(), self.reason.name())?;
        if !self.domains.is_empty() {
            write!(formatter, "({})", self.domains.labels())?;
        }
        Ok(())
    }
}

/// 成果物を引いて語を判定する。
pub struct Judge {
    /// 複合語のキーとバケットのフィルタ。
    filter: FrequencyFilter,
    /// 複合語の部品ごとの頻度表。
    constituents: ConstituentFrequencies,
    /// 構成の分析が引く、分割の単位の回数。
    units: UnitFrequencies,
    /// 語と分野の組のフィルタ。文書の分野を渡された場合だけ読む。
    domains: Option<DomainFilter>,
    /// 判定の切り替え。
    options: Options,
}

impl Judge {
    /// 成果物と切り替えから判定器を作る。
    #[must_use]
    pub fn new(
        filter: FrequencyFilter,
        constituents: ConstituentFrequencies,
        units: UnitFrequencies,
        domains: Option<DomainFilter>,
        options: Options,
    ) -> Self {
        Self {
            filter,
            constituents,
            units,
            domains,
            options,
        }
    }

    /// `artifacts_dir` の成果物を読んで判定器を作る。
    ///
    /// # Errors
    ///
    /// 成果物を読めない場合に返す。
    pub fn open(artifacts_dir: &Path, units: UnitFrequencies, options: Options) -> Result<Self> {
        let filter_path = artifacts_dir.join(FILTER_FILE);
        let filter_bytes = std::fs::read(&filter_path)
            .with_context(|| format!("{} を読めない", filter_path.display()))?;
        let filter = FrequencyFilter::from_bytes(&filter_bytes, DICTIONARY_VERSION)
            .with_context(|| format!("{} を読めない", filter_path.display()))?;
        let constituent_path = artifacts_dir.join(CONSTITUENT_FILE);
        let constituent_bytes = std::fs::read(&constituent_path)
            .with_context(|| format!("{} を読めない", constituent_path.display()))?;
        let constituents = ConstituentFrequencies::read(constituent_bytes)
            .with_context(|| format!("{} を読めない", constituent_path.display()))?;
        let domains = options
            .document_domain
            .map(|_| DomainFilter::open(artifacts_dir, DICTIONARY_VERSION))
            .transpose()?;
        Ok(Self::new(filter, constituents, units, domains, options))
    }

    /// `word` を判定する。
    ///
    /// # Errors
    ///
    /// 語を解析できない場合に返す。
    pub fn judge(&self, word: &str) -> Result<Judgment, MorphError> {
        let morphemes = analyze_short(word)?;
        if let Some(bucket) = self.bucket(&morphemes) {
            let (verdict, reason) = match bucket {
                DICTIONARY_BUCKET => (Verdict::Established, Reason::Dictionary),
                ESTABLISHED_BUCKET.. => (Verdict::Established, Reason::HighFrequency),
                _ => (Verdict::Gray, Reason::LowFrequency),
            };
            if verdict == Verdict::Established
                && let Some(judgment) = self.domain_check(&morphemes)
            {
                return Ok(judgment);
            }
            return Ok(Judgment {
                verdict,
                reason,
                unknown_unit: false,
                domains: DomainSet::default(),
            });
        }
        if morphemes.len() >= SECOND_CHECK_MORPHEMES
            && let Some(judgment) = self.second_check(&morphemes)
        {
            return Ok(judgment);
        }
        let reason = if self.constituent_ratio(&morphemes) > self.options.threshold {
            Reason::Constituent
        } else {
            Reason::Unregistered
        };
        Ok(Judgment {
            verdict: Verdict::Coined,
            reason,
            unknown_unit: false,
            domains: DomainSet::default(),
        })
    }

    /// 完全一致で既存語と決まった語に、分野の検査を当てる。転用の候補にならない
    /// 語では `None` を返し、判定は既存語のままになる。
    ///
    /// 文書の分野を渡されていない場合と、語が現れる分野の集合が空の場合は当てない。
    /// 集合が空なのは、どの分野でも登録の下限に届かなかったということであり、語が
    /// 別の分野に偏る証拠にならないためである。
    fn domain_check(&self, morphemes: &[Morpheme<'_>]) -> Option<Judgment> {
        let document = self.options.document_domain?;
        let found = self.domain_set(morphemes)?;
        if found.is_empty() || found.contains(Domain::General) || found.contains(document) {
            return None;
        }
        Some(Judgment {
            verdict: Verdict::RepurposeCandidate,
            reason: Reason::Domain,
            unknown_unit: false,
            domains: found,
        })
    }

    /// 語が現れる分野の集合。表層形を連ねたキーで引き、当たらなければ正規化形を
    /// 連ねたキーで引く。`count` が両方のキーを数えているので、照合も両方を試す。
    fn domain_set(&self, morphemes: &[Morpheme<'_>]) -> Option<DomainSet> {
        let filter = self.domains.as_ref()?;
        let surface: String = morphemes.iter().map(|morpheme| morpheme.surface).collect();
        let found = filter.domains(&surface);
        if !found.is_empty() {
            return Some(found);
        }
        let normalized: String = morphemes.iter().map(Morpheme::normalized_form).collect();
        Some(filter.domains(&normalized))
    }

    /// 3 形態素以上の語に当てる 2 つ目の検査。名前が決まらなければ `None` を返し、
    /// 判定は 3 つ目の検査へ落ちる。
    fn second_check(&self, morphemes: &[Morpheme<'_>]) -> Option<Judgment> {
        match self.options.concatenation {
            ConcatenationRule::Off => None,
            ConcatenationRule::Composition => Some(self.compose(morphemes)),
            ConcatenationRule::Established | ConcatenationRule::Gray => {
                if !self.pairs_are_established(morphemes) {
                    return None;
                }
                let verdict = if self.options.concatenation == ConcatenationRule::Gray {
                    Verdict::Gray
                } else {
                    Verdict::Established
                };
                Some(Judgment {
                    verdict,
                    reason: Reason::Concatenation,
                    unknown_unit: false,
                    domains: DomainSet::default(),
                })
            }
        }
    }

    /// 構成の分析の点数を名前に写す。上の閾値以上は圧縮された造語の候補、下の閾値
    /// 以上は疎さで落ちた既存語かもしれないのでグレー、それ未満は部品の側にも
    /// 手掛かりが無いので理由が未登録の造語の候補である。
    fn compose(&self, morphemes: &[Morpheme<'_>]) -> Judgment {
        let Score { best, unknown_unit } =
            composition::score(morphemes, &self.units, &self.constituents, &self.filter);
        let (verdict, reason) = if best >= self.options.thresholds.upper {
            (Verdict::Coined, Reason::Composition)
        } else if best >= self.options.thresholds.lower {
            (Verdict::Gray, Reason::Composition)
        } else {
            (Verdict::Coined, Reason::Unregistered)
        };
        Judgment {
            verdict,
            reason,
            unknown_unit,
            domains: DomainSet::default(),
        }
    }

    /// 形態素の並びのバケット。表層形を連ねたキーで引き、当たらなければ正規化形を
    /// 連ねたキーで引く。`count` が両方のキーを数えているので、照合も両方を試す。
    ///
    /// 未知語 1 形態素を候補にしない設定では、その形の語をどのキーでも引かない。
    /// `count` の側も同じ条件でキーを数えないので、引かないことで数える側と検査
    /// する側の条件が揃う。
    fn bucket(&self, morphemes: &[Morpheme<'_>]) -> Option<u8> {
        if self.options.unknown == UnknownMorphemes::Skip
            && let [morpheme] = morphemes
            && is_unknown_run(morpheme)
        {
            return None;
        }
        let surface: String = morphemes.iter().map(|morpheme| morpheme.surface).collect();
        let normalized: String = morphemes.iter().map(Morpheme::normalized_form).collect();
        self.filter
            .bucket(&surface)
            .or_else(|| self.filter.bucket(&normalized))
            .map(aku_freq::Bucket::get)
    }

    /// 隣り合う 2 形態素の対が、すべて既存語のバケットを持つか。
    fn pairs_are_established(&self, morphemes: &[Morpheme<'_>]) -> bool {
        morphemes.windows(2).all(|pair| {
            self.bucket(pair)
                .is_some_and(|bucket| bucket >= ESTABLISHED_BUCKET)
        })
    }

    /// 部品の頻度の比 `min(f(X), f(Y)) / (f(XY) + 1)`。
    ///
    /// この検査に来る語は、完全一致の検査を外れている。すなわち複合語としての登録が
    /// ないので、`f(XY)` は 0 とし、分母は 1 になる。バケットの代表値を分母に使う
    /// 余地はない。登録のある語はここへ来ないためである。
    ///
    /// 部品の頻度は表層形で引く。`count` が部品を表層形で数えているので、正規化形で
    /// 引くと別の語の頻度を拾う。表にない部品は 0 とする。
    #[expect(
        clippy::cast_precision_loss,
        reason = "頻度は記事 1,484,267 件を数えた回数であり、f64 が正確に表せる範囲に収まる"
    )]
    fn constituent_ratio(&self, morphemes: &[Morpheme<'_>]) -> f64 {
        let minimum = morphemes
            .iter()
            .map(|morpheme| self.constituents.frequency(morpheme.surface).unwrap_or(0))
            .min()
            .unwrap_or(0);
        minimum as f64
    }
}

/// 1 群の判定を数えた表。
#[derive(Default)]
struct Tally {
    /// 群の名前。
    label: String,
    /// 名前ごとの語数。
    verdicts: HashMap<Verdict, u64>,
    /// 理由ごとの語数。
    reasons: HashMap<Reason, u64>,
    /// 頻度が不明な単位のために、点数に使えなかった分割を持つ語数。
    unknown_unit: u64,
    /// 判定した語数。
    total: u64,
}

impl Tally {
    /// 空の表を作る。
    fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            ..Self::default()
        }
    }

    /// 1 語の判定を足す。
    fn add(&mut self, judgment: Judgment) {
        *self.verdicts.entry(judgment.verdict).or_default() += 1;
        *self.reasons.entry(judgment.reason).or_default() += 1;
        if judgment.unknown_unit {
            self.unknown_unit += 1;
        }
        self.total += 1;
    }

    /// 名前ごとの語数。
    fn count(&self, verdict: Verdict) -> u64 {
        self.verdicts.get(&verdict).copied().unwrap_or(0)
    }

    /// 名前と理由の内訳を書き出す。
    fn print(&self) {
        println!(
            "{} {} 語: 既存語 {}、グレー {}、転用の候補 {}、造語の候補 {}",
            self.label,
            self.total,
            self.count(Verdict::Established),
            self.count(Verdict::Gray),
            self.count(Verdict::RepurposeCandidate),
            self.count(Verdict::Coined)
        );
        let reasons: Vec<String> = [
            Reason::Dictionary,
            Reason::HighFrequency,
            Reason::LowFrequency,
            Reason::Concatenation,
            Reason::Composition,
            Reason::Domain,
            Reason::Constituent,
            Reason::Unregistered,
        ]
        .into_iter()
        .map(|reason| {
            let count = self.reasons.get(&reason).copied().unwrap_or(0);
            format!("{} {count}", reason.name())
        })
        .collect();
        println!("  理由の内訳: {}", reasons.join("、"));
        if self.unknown_unit > 0 {
            println!(
                "  頻度が不明な単位のために点数に使えなかった分割を持つ語: {} 語",
                self.unknown_unit
            );
        }
    }
}

/// `artifacts_dir` の成果物で `words` を判定し、`eval` を渡された場合は混同行列を
/// 出す。
///
/// # Errors
///
/// 成果物か TSV を読めない場合、語を解析できない場合に返す。
pub fn run(
    artifacts_dir: &Path,
    options: Options,
    counts: CountsDirs<'_>,
    eval: Option<&[std::path::PathBuf]>,
    words: &[String],
) -> Result<()> {
    let units = if options.concatenation == ConcatenationRule::Composition {
        let Some(counts_dir) = counts.counts_dir else {
            bail!("--concatenation composition には --counts-dir が要る");
        };
        if options.thresholds.lower > options.thresholds.upper {
            bail!(
                "--lower-threshold の {} が --upper-threshold の {} を超えるので、グレーの帯が空になる",
                options.thresholds.lower,
                options.thresholds.upper
            );
        }
        UnitFrequencies::load(counts_dir, counts.docs_counts_dir)?
    } else {
        UnitFrequencies::empty()
    };
    let judge = Judge::open(artifacts_dir, units, options)?;
    for word in words {
        let judgment = judge
            .judge(word)
            .with_context(|| format!("'{word}' を解析できない"))?;
        println!("{word}\t{judgment}");
    }
    if let Some(paths) = eval {
        let [coined, established @ ..] = paths else {
            bail!("--eval は造語の TSV と、1 つ以上の既存語の TSV を受け取る");
        };
        if established.is_empty() {
            bail!("--eval は造語の TSV と、1 つ以上の既存語の TSV を受け取る");
        }
        let (categories, established) = tally_eval(&judge, coined, established)?;
        print_matrix(&categories, &established, options.document_domain);
    }
    Ok(())
}

/// 正解つきの語の集合の TSV を読み、行番号とタブで割った列を返す。見出し行と
/// 語の列が空の行は落とす。
fn read_rows(path: &Path) -> Result<Vec<(usize, Vec<String>)>> {
    let file = File::open(path).with_context(|| format!("{} を開けない", path.display()))?;
    let mut rows = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("{} を読めない", path.display()))?;
        let fields: Vec<String> = line
            .split('\t')
            .map(|field| field.trim().to_owned())
            .collect();
        let word = fields
            .get(WORD_COLUMN)
            .map_or("", |word| word.as_str())
            .to_owned();
        if word.is_empty() || word == TSV_HEADER {
            continue;
        }
        rows.push((index + 1, fields));
    }
    Ok(rows)
}

/// `path` の語を judge に掛けて数える。
fn tally(judge: &Judge, path: &Path, label: &str) -> Result<Tally> {
    let mut tally = Tally::new(label);
    for (line_number, fields) in read_rows(path)? {
        tally.add(judge_row(judge, path, line_number, &fields[WORD_COLUMN])?);
    }
    Ok(tally)
}

/// 区分ごとの表に付ける名前。
fn category_label(category: Category) -> String {
    format!("区分が{}", category.label())
}

/// 造語の例を区分ごとに数えた 3 つの表。
struct CategoryTallies {
    /// 区分が造語の語。再現率の母数はこの表だけからなる。
    coined: Tally,
    /// 区分が言い換えの語。拾ってはいけない語なので、造語の候補と判定した割合を
    /// 既存語の誤検出率と同じ向きで読む。
    reworded: Tally,
    /// 区分が意味の転用の語。原理的に拾えない語の振り分けを示す。
    repurposed: Tally,
}

/// `path` の造語の例を、区分の列で 3 つの表に分けて数える。
fn tally_by_category(judge: &Judge, path: &Path) -> Result<CategoryTallies> {
    let mut tallies = CategoryTallies {
        coined: Tally::new(&category_label(Category::Coined)),
        reworded: Tally::new(&category_label(Category::Reworded)),
        repurposed: Tally::new(&category_label(Category::Repurposed)),
    };
    for (line_number, fields) in read_rows(path)? {
        let word = &fields[WORD_COLUMN];
        let field = fields
            .get(CATEGORY_COLUMN)
            .map_or("", |field| field.as_str());
        let Some(category) = Category::parse(field) else {
            bail!(
                "{} の {line_number} 行目の '{word}' に区分が無い。{} 列目に 造語 か 言い換え か 意味の転用 と書く",
                path.display(),
                CATEGORY_COLUMN + 1
            );
        };
        let judgment = judge_row(judge, path, line_number, word)?;
        match category {
            Category::Coined => tallies.coined.add(judgment),
            Category::Reworded => tallies.reworded.add(judgment),
            Category::Repurposed => tallies.repurposed.add(judgment),
        }
    }
    Ok(tallies)
}

/// 正解つきの語の集合を読んで数える。既存語の例は集合ごとに別の表にする。集合を
/// 混ぜると、採り先の違う語の誤検出率が 1 つの割合に均される。
fn tally_eval(
    judge: &Judge,
    coined: &Path,
    established: &[std::path::PathBuf],
) -> Result<(CategoryTallies, Vec<Tally>)> {
    let categories = tally_by_category(judge, coined)?;
    let established = established
        .iter()
        .map(|path| {
            let label = format!("既存語の例({})", file_name(path));
            tally(judge, path, &label)
        })
        .collect::<Result<Vec<Tally>>>()?;
    Ok((categories, established))
}

/// `path` のファイル名。集合の名前として表の行に書く。
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// 1 行の語を判定する。エラーには TSV のパスと行番号を書く。
fn judge_row(judge: &Judge, path: &Path, line_number: usize, word: &str) -> Result<Judgment> {
    judge.judge(word).with_context(|| {
        format!(
            "{} の {line_number} 行目の '{word}' を解析できない",
            path.display()
        )
    })
}

/// 混同行列と割合を書き出す。誤検出率は既存語の例の集合ごとに 1 行ずつ出す。
///
/// 再現率の母数は区分が造語の語だけである。区分が言い換えの語は拾ってはいけない語
/// なので、造語の候補と判定した割合を誤検出率と同じ向きで出す。グレー率の母数は、
/// 判定したすべての語である。
///
/// `document_domain` を渡した場合は、意味の転用の再現率と、既存語の例の集合ごとの
/// 転用の誤検出率も出す。分野の検査を当てなければ、区分が意味の転用の語は語の有無
/// で判定するこの仕組みでは拾えないので、振り分けだけが出る。
fn print_matrix(
    categories: &CategoryTallies,
    established: &[Tally],
    document_domain: Option<Domain>,
) {
    let CategoryTallies {
        coined,
        reworded,
        repurposed,
    } = categories;
    for tally in [coined, reworded, repurposed] {
        tally.print();
    }
    for tally in established {
        tally.print();
    }
    let counted = [coined, reworded, repurposed]
        .into_iter()
        .chain(established);
    let total: u64 = counted.clone().map(|tally| tally.total).sum();
    let gray: u64 = counted.map(|tally| tally.count(Verdict::Gray)).sum();
    println!(
        "造語の再現率 {}",
        ratio(coined.count(Verdict::Coined), coined.total)
    );
    println!(
        "言い換えの誤検出率 {}",
        ratio(reworded.count(Verdict::Coined), reworded.total)
    );
    for tally in established {
        println!(
            "{}の誤検出率 {}",
            tally.label,
            ratio(tally.count(Verdict::Coined), tally.total)
        );
    }
    println!("グレー率 {}", ratio(gray, total));
    if let Some(document) = document_domain {
        println!("文書の分野は {} である", document.label());
        println!(
            "意味の転用の再現率 {}",
            ratio(
                repurposed.count(Verdict::RepurposeCandidate),
                repurposed.total
            )
        );
        for tally in established {
            println!(
                "{}の転用の誤検出率 {}",
                tally.label,
                ratio(tally.count(Verdict::RepurposeCandidate), tally.total)
            );
        }
    }
}

/// `numerator / denominator` を、母数を添えた文字列にする。
#[expect(
    clippy::cast_precision_loss,
    reason = "語数は正解つきの語の集合の行数であり、f64 が正確に表せる範囲に収まる"
)]
fn ratio(numerator: u64, denominator: u64) -> String {
    if denominator == 0 {
        return "0/0".to_owned();
    }
    let percent = numerator as f64 * 100.0 / denominator as f64;
    format!("{numerator}/{denominator} ({percent:.1}%)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aku_freq::Bucket;

    /// 構成の分析を使わない検査のための閾値。
    const UNUSED_THRESHOLDS: Thresholds = Thresholds { upper: 0, lower: 0 };

    /// キーとバケットの表と部品の頻度から、既定の切り替えで判定器を作る。
    fn judge_with(keys: &[(&str, u8)], constituents: &[(&str, u64)], threshold: f64) -> Judge {
        judge_with_options(
            keys,
            constituents,
            &[],
            Options {
                threshold,
                concatenation: ConcatenationRule::Established,
                thresholds: UNUSED_THRESHOLDS,
                unknown: UnknownMorphemes::Skip,
                document_domain: None,
            },
        )
    }

    /// キーとバケットの表と部品の頻度と単位の回数から判定器を作る。
    fn judge_with_options(
        keys: &[(&str, u8)],
        constituents: &[(&str, u64)],
        units: &[(&str, u64)],
        options: Options,
    ) -> Judge {
        let entries: Vec<(&str, Bucket)> = keys
            .iter()
            .map(|(key, bucket)| (*key, Bucket::new(*bucket).unwrap()))
            .collect();
        let filter = FrequencyFilter::build(&entries, DICTIONARY_VERSION).unwrap();
        let bytes = ConstituentFrequencies::build(constituents).unwrap();
        Judge::new(
            filter,
            ConstituentFrequencies::read(bytes).unwrap(),
            UnitFrequencies::from_rows(units),
            None,
            options,
        )
    }

    /// 語と分野の組から、文書の分野を決めた判定器を作る。完全一致で既存語に
    /// なるように、組の語と `undomained` の語を頻度のフィルタへ登録する。
    /// `undomained` の語は分野のフィルタに登録しないので、分野の集合が空になる。
    fn judge_with_domains(
        pairs: &[(Domain, &str)],
        undomained: &[&str],
        document_domain: Domain,
    ) -> Judge {
        let established = Bucket::new(3).unwrap();
        let words: Vec<(String, Bucket)> = pairs
            .iter()
            .map(|(_, word)| *word)
            .chain(undomained.iter().copied())
            .map(|word| (word.to_owned(), established))
            .collect();
        let registered = Bucket::new(7).unwrap();
        let domain_entries: Vec<(String, Bucket)> = pairs
            .iter()
            .map(|(domain, word)| (crate::domain::key(*domain, word), registered))
            .collect();
        let no_constituents: [(&str, u64); 0] = [];
        let bytes = ConstituentFrequencies::build(&no_constituents).unwrap();
        Judge::new(
            FrequencyFilter::build(&words, DICTIONARY_VERSION).unwrap(),
            ConstituentFrequencies::read(bytes).unwrap(),
            UnitFrequencies::from_rows(&[]),
            Some(DomainFilter::new(
                FrequencyFilter::build(&domain_entries, DICTIONARY_VERSION).unwrap(),
            )),
            Options {
                threshold: 0.0,
                concatenation: ConcatenationRule::Off,
                thresholds: UNUSED_THRESHOLDS,
                unknown: UnknownMorphemes::Skip,
                document_domain: Some(document_domain),
            },
        )
    }

    /// 連結の扱いだけを変えた切り替え。
    fn concatenation_options(concatenation: ConcatenationRule) -> Options {
        Options {
            threshold: 0.0,
            concatenation,
            thresholds: UNUSED_THRESHOLDS,
            unknown: UnknownMorphemes::Skip,
            document_domain: None,
        }
    }

    /// 構成の分析の切り替え。
    fn composition_options(thresholds: Thresholds) -> Options {
        Options {
            threshold: 0.0,
            concatenation: ConcatenationRule::Composition,
            thresholds,
            unknown: UnknownMorphemes::Skip,
            document_domain: None,
        }
    }

    #[test]
    fn 完全一致のバケットが名前を決める() {
        // 見出しのバケットは辞書、高頻度は 2 以上、グレーは 1 である。
        let judge = judge_with(
            &[("公開鍵暗号", 7), ("分散処理", 3), ("検証手順", 1)],
            &[],
            0.0,
        );
        let dictionary = judge.judge("公開鍵暗号").unwrap();
        assert_eq!(dictionary.verdict, Verdict::Established);
        assert_eq!(dictionary.reason, Reason::Dictionary);
        let frequent = judge.judge("分散処理").unwrap();
        assert_eq!(frequent.verdict, Verdict::Established);
        assert_eq!(frequent.reason, Reason::HighFrequency);
        let gray = judge.judge("検証手順").unwrap();
        assert_eq!(gray.verdict, Verdict::Gray);
        assert_eq!(gray.reason, Reason::LowFrequency);
    }

    #[test]
    fn 対がすべて高頻度なら既存語の連結とする() {
        // 「公開鍵暗号」自体は登録がないが、「公開鍵」と「鍵暗号」がどちらも高頻度
        // なので既存語になる。
        let judge = judge_with(&[("公開鍵", 3), ("鍵暗号", 2)], &[], 0.0);
        let judgment = judge.judge("公開鍵暗号").unwrap();
        assert_eq!(judgment.verdict, Verdict::Established);
        assert_eq!(judgment.reason, Reason::Concatenation);
    }

    #[test]
    fn 対が1つでも欠ければ連結にしない() {
        // 「鍵暗号」の登録がないので、連結では決まらず造語の候補へ落ちる。
        let judge = judge_with(&[("公開鍵", 3)], &[], 0.0);
        let judgment = judge.judge("公開鍵暗号").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
    }

    #[test]
    fn 連結をグレーにも造語の候補にもできる() {
        // 「公開鍵」と「鍵暗号」がどちらも高頻度なので、対の検査に当たる。
        let keys = [("公開鍵", 3), ("鍵暗号", 2)];
        let gray = judge_with_options(
            &keys,
            &[],
            &[],
            concatenation_options(ConcatenationRule::Gray),
        );
        let judgment = gray.judge("公開鍵暗号").unwrap();
        assert_eq!(judgment.verdict, Verdict::Gray);
        assert_eq!(judgment.reason, Reason::Concatenation);
        // 検査をやめると、対が揃っていても造語の候補へ落ちる。
        let off = judge_with_options(
            &keys,
            &[],
            &[],
            concatenation_options(ConcatenationRule::Off),
        );
        let judgment = off.judge("公開鍵暗号").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
        assert_eq!(judgment.reason, Reason::Unregistered);
    }

    #[test]
    fn 未知語1形態素を候補にするか切り替える() {
        // 「Kubectl」は解析辞書に無い 1 形態素であり、`count` は表層形と小文字の
        // 正規化形の 2 つのキーで数える。
        let keys = [("Kubectl", 2), ("kubectl", 2)];
        let options = Options {
            threshold: 0.0,
            concatenation: ConcatenationRule::Established,
            thresholds: UNUSED_THRESHOLDS,
            unknown: UnknownMorphemes::Count,
            document_domain: None,
        };
        let counted = judge_with_options(&keys, &[], &[], options);
        let judgment = counted.judge("Kubectl").unwrap();
        assert_eq!(judgment.verdict, Verdict::Established);
        assert_eq!(judgment.reason, Reason::HighFrequency);
        // 候補にしない設定では、キーがあっても引かない。
        let skipped = judge_with_options(
            &keys,
            &[],
            &[],
            Options {
                unknown: UnknownMorphemes::Skip,
                ..options
            },
        );
        assert_eq!(skipped.judge("Kubectl").unwrap().verdict, Verdict::Coined);
    }

    /// 構成の分析の例に使う部品の頻度。`未認証データ` の 3 形態素である。
    const COMPOSITION_CONSTITUENTS: [(&str, u64); 3] =
        [("未", 5_000), ("認証", 30_000), ("データ", 90_000)];

    #[test]
    fn 構成の分析は圧縮された造語を拾い既存語を残す() {
        // 「未認証データ」は「未認証」と「認証データ」がどちらも高頻度なので、
        // 既定の 2 つ目の検査は既存語の連結と読む。
        let keys = [("未認証", 3), ("認証データ", 2), ("公開鍵暗号", 7)];
        let units = [("未認証", 400), ("認証データ", 12)];
        let thresholds = Thresholds {
            upper: 100,
            lower: 10,
        };
        let concatenation = judge_with_options(
            &keys,
            &COMPOSITION_CONSTITUENTS,
            &units,
            concatenation_options(ConcatenationRule::Established),
        );
        let judgment = concatenation.judge("未認証データ").unwrap();
        assert_eq!(judgment.verdict, Verdict::Established);
        assert_eq!(judgment.reason, Reason::Concatenation);
        // 構成の分析では、「未認証」400 と「データ」90,000 の分割が点数 400 を出し、
        // 上の閾値 100 を超えるので造語の候補になる。
        let composition = judge_with_options(
            &keys,
            &COMPOSITION_CONSTITUENTS,
            &units,
            composition_options(thresholds),
        );
        let judgment = composition.judge("未認証データ").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
        assert_eq!(judgment.reason, Reason::Composition);
        // 見出しにある「公開鍵暗号」は完全一致で決まるので、構成の分析に落ちない。
        let judgment = composition.judge("公開鍵暗号").unwrap();
        assert_eq!(judgment.verdict, Verdict::Established);
        assert_eq!(judgment.reason, Reason::Dictionary);
    }

    #[test]
    fn 構成の点数が2つの閾値の間ならグレーにする() {
        // 点数は「未認証」の 40 であり、下の閾値 10 以上で上の閾値 100 に満たない。
        let judge = judge_with_options(
            &[],
            &COMPOSITION_CONSTITUENTS,
            &[("未認証", 40)],
            composition_options(Thresholds {
                upper: 100,
                lower: 10,
            }),
        );
        let judgment = judge.judge("未認証データ").unwrap();
        assert_eq!(judgment.verdict, Verdict::Gray);
        assert_eq!(judgment.reason, Reason::Composition);
    }

    #[test]
    fn 構成の点数が下の閾値に届かなければ理由が未登録になる() {
        // 点数は「未認証」の 5 であり、下の閾値 10 に届かない。名前は造語の候補の
        // ままで、理由だけが変わる。
        let judge = judge_with_options(
            &[],
            &COMPOSITION_CONSTITUENTS,
            &[("未認証", 5)],
            composition_options(Thresholds {
                upper: 100,
                lower: 10,
            }),
        );
        let judgment = judge.judge("未認証データ").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
        assert_eq!(judgment.reason, Reason::Unregistered);
        assert!(!judgment.unknown_unit);
    }

    #[test]
    fn 頻度が不明な単位の分割を点数に使わない() {
        // 「未認証」は見出しのバケットを持つが回数の TSV に無いので、この分割は
        // 点数に使えない。残る分割の「認証データ」は単位にならないので、点数は
        // 0 のままである。
        let judge = judge_with_options(
            &[("未認証", 7)],
            &COMPOSITION_CONSTITUENTS,
            &[],
            composition_options(Thresholds {
                upper: 100,
                lower: 10,
            }),
        );
        let judgment = judge.judge("未認証データ").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
        assert_eq!(judgment.reason, Reason::Unregistered);
        assert!(judgment.unknown_unit);
    }

    #[test]
    fn 文書の分野に現れない既存語を転用の候補にする() {
        // 「樹形図」は文化とその他の理工に現れ、情報技術には現れない。
        let judge = judge_with_domains(
            &[
                (Domain::Culture, "樹形図"),
                (Domain::OtherStem, "樹形図"),
                (Domain::InformationTechnology, "公開鍵暗号"),
            ],
            &[],
            Domain::InformationTechnology,
        );
        let judgment = judge.judge("樹形図").unwrap();
        assert_eq!(judgment.verdict, Verdict::RepurposeCandidate);
        assert_eq!(judgment.reason, Reason::Domain);
        assert_eq!(judgment.domains.labels(), "その他の理工・文化");
        assert_eq!(judgment.to_string(), "転用の候補\t分野(その他の理工・文化)");
        // 文書の分野に現れる語は既存語のままである。
        let judgment = judge.judge("公開鍵暗号").unwrap();
        assert_eq!(judgment.verdict, Verdict::Established);
        assert_eq!(judgment.reason, Reason::HighFrequency);
    }

    #[test]
    fn 一般の語と分野の登録が無い語は既存語のままにする() {
        // 「完了条件」は一般で登録されているので、どの分野の文書でも既存語で
        // ある。「電子署名」は分野の登録が無いので、偏りの証拠が無い。
        let judge = judge_with_domains(
            &[(Domain::General, "完了条件"), (Domain::Culture, "樹形図")],
            &["電子署名"],
            Domain::InformationTechnology,
        );
        for word in ["完了条件", "電子署名"] {
            let judgment = judge.judge(word).unwrap();
            assert_eq!(judgment.verdict, Verdict::Established, "{word}");
            assert!(judgment.domains.is_empty(), "{word}");
        }
        // 同じ判定器でも、特定の分野だけの語は転用の候補になる。
        assert_eq!(
            judge.judge("樹形図").unwrap().verdict,
            Verdict::RepurposeCandidate
        );
    }

    #[test]
    fn 部品がありふれた未登録の語を造語の候補にする() {
        // 部品の最小の頻度 8220 が閾値 1000 を超えるので、理由は部品になる。
        let judge = judge_with(&[], &[("幽霊", 8220), ("参照", 42449)], 1000.0);
        let judgment = judge.judge("幽霊参照").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
        assert_eq!(judgment.reason, Reason::Constituent);
    }

    #[test]
    fn 部品も珍しい未登録の語は理由が未登録になる() {
        // 部品の最小の頻度 118 が閾値 1000 に届かない。名前は造語の候補のままで、
        // 理由だけが変わる。
        let judge = judge_with(&[], &[("貪欲", 118), ("分割", 32179)], 1000.0);
        let judgment = judge.judge("貪欲分割").unwrap();
        assert_eq!(judgment.verdict, Verdict::Coined);
        assert_eq!(judgment.reason, Reason::Unregistered);
    }

    /// 3 区分の例を持つ造語の例の TSV。`樹形図` は解析辞書に見出しがあるので意味の
    /// 転用、`公開鍵暗号` はプロジェクトが別の表記を使う言い換え、残る 2 語は造語で
    /// ある。
    const CATEGORIZED_TSV: &str = "語\t判定者\t出典\t区分\t区分の判定者\n\
         幽霊参照\tmaintainer\t記録\t造語\tmaintainer\n\
         貪欲分割\tmaintainer\t記録\t造語\tmaintainer\n\
         公開鍵暗号\tmaintainer\t記録\t言い換え\tmaintainer\n\
         樹形図\tmaintainer\t記録\t意味の転用\tmaintainer\n";

    /// 上の TSV を一時ファイルに書き、区分ごとに数えた 3 つの表を返す。
    fn tally_categorized_tsv(name: &str) -> CategoryTallies {
        let judge = judge_with(
            &[("樹形図", 7), ("公開鍵暗号", 7)],
            &[
                ("幽霊", 8220),
                ("参照", 42449),
                ("貪欲", 118),
                ("分割", 32179),
            ],
            1000.0,
        );
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("coined.tsv");
        std::fs::write(&path, CATEGORIZED_TSV).unwrap();
        let tallies = tally_by_category(&judge, &path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        tallies
    }

    #[test]
    fn 区分が造語の行だけを再現率の母数にする() {
        // 残る 2 区分の 2 行を母数に入れれば 2/4 になる。区分で分けるので 2/2 である。
        let tallies = tally_categorized_tsv("corpus-tool-coined-tally-test");
        assert_eq!(tallies.coined.total, 2);
        assert_eq!(tallies.coined.count(Verdict::Coined), 2);
        assert_eq!(tallies.coined.count(Verdict::Established), 0);
    }

    #[test]
    fn 区分が言い換えの行を別の表に数える() {
        // 言い換えの `公開鍵暗号` は見出しのバケットで既存語になる。造語の候補に
        // 入れば誤検出であり、表はその向きで読む。
        let tallies = tally_categorized_tsv("corpus-tool-reworded-tally-test");
        assert_eq!(tallies.reworded.total, 1);
        assert_eq!(tallies.reworded.count(Verdict::Established), 1);
        assert_eq!(tallies.reworded.count(Verdict::Coined), 0);
    }

    #[test]
    fn 区分が意味の転用の行を別の表に数える() {
        // 意味の転用の `樹形図` は見出しのバケットで既存語になる。造語の表には
        // 入らない。
        let tallies = tally_categorized_tsv("corpus-tool-repurposed-tally-test");
        assert_eq!(tallies.repurposed.total, 1);
        assert_eq!(tallies.repurposed.count(Verdict::Established), 1);
        assert_eq!(tallies.repurposed.count(Verdict::Coined), 0);
    }

    #[test]
    fn 割合には母数を添える() {
        assert_eq!(ratio(192, 200), "192/200 (96.0%)");
        // 語が 1 つもない群でも割り算をしない。
        assert_eq!(ratio(0, 0), "0/0");
    }

    #[test]
    fn 既存語の例を集合ごとに数える() {
        let dir = std::env::temp_dir().join("corpus-tool-judge-eval-sets-test");
        std::fs::create_dir_all(&dir).unwrap();
        let coined_path = dir.join("coined.tsv");
        std::fs::write(
            &coined_path,
            "語\t判定者\t出典\t区分\t区分の判定者\n幽霊参照\tmaintainer\t記録\t造語\tmaintainer\n",
        )
        .unwrap();
        let first = dir.join("established.tsv");
        std::fs::write(&first, "語\t判定者\t出典\n型注釈\tmaintainer\t技術書\n").unwrap();
        let second = dir.join("established-security.tsv");
        std::fs::write(
            &second,
            "語\t判定者\t出典\n脆弱性情報\tmaintainer\t注意喚起\n",
        )
        .unwrap();
        // 登録があるのは `型注釈` だけなので、2 つの集合で誤検出の数が変わる。
        let judge = judge_with(&[("型注釈", 3)], &[], 0.0);

        let (categories, established) = tally_eval(&judge, &coined_path, &[first, second]).unwrap();

        assert_eq!(categories.coined.total, 1);
        assert_eq!(categories.reworded.total, 0);
        assert_eq!(categories.repurposed.total, 0);
        // 集合は混ざらず、渡した順に 1 つずつ表になる。
        assert_eq!(established.len(), 2);
        assert_eq!(established[0].label, "既存語の例(established.tsv)");
        assert_eq!(established[0].total, 1);
        assert_eq!(established[0].count(Verdict::Coined), 0);
        assert_eq!(established[1].label, "既存語の例(established-security.tsv)");
        assert_eq!(established[1].total, 1);
        assert_eq!(established[1].count(Verdict::Coined), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
