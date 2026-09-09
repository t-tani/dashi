//! cirrussearch のダンプを 1 記事ずつ流して読み、絞り込みに当たった記事の名前と
//! 本文と話題の予測のタグとリダイレクトの名前を返す。
//!
//! ダンプは gzip の JSON Lines で、`{"index":...}` の行と記事の行が交互に並ぶ。
//! 記事の行は数十 KB から数 MB あるので、行を 1 つずつ読んでは捨て、全体を
//! メモリに載せない。

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::MultiGzDecoder;
use serde::Deserialize;

/// 記事の絞り込み。
#[derive(Clone, Copy)]
pub enum Selection {
    /// 名前空間 0 の記事をすべて採る。
    AllArticles,
    /// 名前空間 0 のうち、情報技術の話題を持つ記事だけを採る。
    TechnologyTopics,
}

impl Selection {
    /// 記事を採る条件。manifest に書く文言でもある。
    #[must_use]
    pub fn condition(self) -> &'static str {
        match self {
            Self::AllArticles => "namespace が 0 の記事をすべて採る",
            Self::TechnologyTopics => {
                "namespace が 0 で、articletopic の予測が STEM.Computing か STEM.Technology のどちらかで 500 点以上"
            }
        }
    }

    /// `--topics` を渡したかを条件にする。
    #[must_use]
    pub fn from_topics(topics: bool) -> Self {
        if topics {
            Self::TechnologyTopics
        } else {
            Self::AllArticles
        }
    }

    /// 話題で絞るか。
    fn filters_by_topic(self) -> bool {
        matches!(self, Self::TechnologyTopics)
    }
}

/// 採る話題。ダンプの話題の名前で書く。
const TOPICS: [&str; 2] = ["STEM.Computing", "STEM.Technology"];

/// 採る点数の下限。点数は 0 から 1000 である。話題の分野を畳む
/// [`crate::topic`] もこの下限を使う。
pub(crate) const MIN_SCORE: u32 = 500;

/// 話題の予測のタグに付く接頭辞。この後ろに `<話題>|<点数>` が続き、話題の末尾に
/// `*` が付くことがある。
const TAG_PREFIX: &str = "classification.prediction.articletopic/";

/// 記事の名前空間。
const ARTICLE_NAMESPACE: i64 = 0;

/// 記事の行の前に並ぶ、投入先を書いた行の先頭。この行は記事として数えない。
const INDEX_LINE_PREFIX: &str = "{\"index\"";

/// 読み込むバッファの大きさ。1 行が数 MB になるので、既定の 8 KB では読み出しの
/// 回数が増える。
const BUFFER_BYTES: usize = 4 << 20;

/// ダンプの行のうち、絞り込みと本文に要る項目。`{"index":...}` の行はどの項目も
/// 持たないので、すべて省略できる形で受ける。
#[derive(Deserialize)]
struct Record {
    namespace: Option<i64>,
    title: Option<String>,
    text: Option<String>,
    weighted_tags: Option<Vec<String>>,
    redirect: Option<Vec<Redirect>>,
}

/// 記事へのリダイレクト 1 件。記事の名前空間と同じく、名前空間 0 のものだけを
/// 記事名として採る。
#[derive(Deserialize)]
struct Redirect {
    namespace: Option<i64>,
    title: Option<String>,
}

/// 絞り込みに当たった 1 記事。
pub struct Article {
    /// 記事名。ダンプが名前を持たない記事では空になる。
    pub title: String,
    /// 本文。
    pub text: String,
    /// 話題の予測のタグ。タグを持たない記事では空になる。
    pub weighted_tags: Vec<String>,
    /// この記事を指す、名前空間 0 のリダイレクトの名前。
    pub redirect_titles: Vec<String>,
}

/// ダンプの読み出しの位置と、読んだ量の記録。
pub struct Dump {
    reader: BufReader<MultiGzDecoder<File>>,
    /// 読み終わった行を持つ緩衝。行ごとに確保し直さないために使い回す。
    line: String,
    /// 記事の絞り込み。
    selection: Selection,
    /// 読み終わるまでに数える記事の上限。
    limit: Option<u64>,
    /// 読んだ記事の数。`{"index":...}` の行は数えない。
    pub scanned: u64,
    /// 条件に当たった記事の数。
    pub selected: u64,
    /// 条件に当たった記事の本文のバイト数。
    pub text_bytes: u64,
}

impl Dump {
    /// `path` のダンプを `selection` の絞り込みで開く。`limit` を渡すと、先頭の
    /// その数の記事で読み終わる。
    ///
    /// # Errors
    ///
    /// ファイルを開けない場合に返す。
    pub fn open(path: &Path, selection: Selection, limit: Option<u64>) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("{} を開けない", path.display()))?;
        Ok(Self {
            reader: BufReader::with_capacity(BUFFER_BYTES, MultiGzDecoder::new(file)),
            line: String::new(),
            selection,
            limit,
            scanned: 0,
            selected: 0,
            text_bytes: 0,
        })
    }

    /// 絞り込みに当たった次の記事。ダンプの終わりか上限に達したら `None` を
    /// 返す。
    ///
    /// # Errors
    ///
    /// gzip を展開できない場合、行が JSON でない場合に返す。
    pub fn next_article(&mut self) -> Result<Option<Article>> {
        loop {
            if self.limit.is_some_and(|limit| self.scanned >= limit) {
                return Ok(None);
            }
            self.line.clear();
            if self
                .reader
                .read_line(&mut self.line)
                .context("ダンプを読めない")?
                == 0
            {
                return Ok(None);
            }
            if self.line.starts_with(INDEX_LINE_PREFIX) {
                continue;
            }
            self.scanned += 1;
            // 話題で絞る場合は、条件に当たらない記事が大半なので、JSON にする前に
            // 話題の名前を素の文字列で探す。話題の名前は ASCII なので JSON の脱出を
            // 受けない。
            if self.selection.filters_by_topic()
                && !TOPICS.iter().any(|topic| self.line.contains(topic))
            {
                continue;
            }
            let record: Record = serde_json::from_str(&self.line).context("行が JSON でない")?;
            if record.namespace != Some(ARTICLE_NAMESPACE) {
                continue;
            }
            if self.selection.filters_by_topic() && !is_technology(&record) {
                continue;
            }
            let Some(text) = record.text else {
                continue;
            };
            self.selected += 1;
            self.text_bytes += text.len() as u64;
            return Ok(Some(Article {
                title: record.title.unwrap_or_default(),
                text,
                weighted_tags: record.weighted_tags.unwrap_or_default(),
                redirect_titles: article_redirect_titles(record.redirect),
            }));
        }
    }
}

/// リダイレクトのうち、名前空間 0 のものの名前を取り出す。他の名前空間の
/// リダイレクトは、記事名として数えない。
fn article_redirect_titles(redirect: Option<Vec<Redirect>>) -> Vec<String> {
    redirect
        .unwrap_or_default()
        .into_iter()
        .filter(|redirect| redirect.namespace == Some(ARTICLE_NAMESPACE))
        .filter_map(|redirect| redirect.title)
        .collect()
}

/// 記事が情報技術の話題を [`MIN_SCORE`] 点以上で持つか。
fn is_technology(record: &Record) -> bool {
    record.weighted_tags.iter().flatten().any(|tag| {
        topic_score(tag).is_some_and(|(topic, score)| score >= MIN_SCORE && TOPICS.contains(&topic))
    })
}

/// 話題の予測のタグから、話題の名前と点数を取り出す。話題の予測でないタグと、
/// 点数を読めないタグでは `None` を返す。[`crate::topic`] が分野を畳むために
/// 再利用する。
pub(crate) fn topic_score(tag: &str) -> Option<(&str, u32)> {
    let (topic, score) = tag.strip_prefix(TAG_PREFIX)?.rsplit_once('|')?;
    Some((topic.trim_end_matches('*'), score.parse().ok()?))
}

/// ダンプのファイル名。
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// ダンプの版。ファイル名に現れる最初の 8 桁の数字であり、`jawiki-20251229-...`
/// なら `20251229` である。数字が無ければ `None` を返す。
pub fn version(path: &Path) -> Option<String> {
    let name = file_name(path);
    let bytes = name.as_bytes();
    bytes
        .windows(8)
        .find(|window| window.iter().all(u8::is_ascii_digit))
        .map(|window| String::from_utf8_lossy(window).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn 話題の名前と点数を取り出す() {
        // 末尾に `*` が付く形と付かない形の両方が現れる。
        assert_eq!(
            topic_score("classification.prediction.articletopic/STEM.Computing*|803"),
            Some(("STEM.Computing", 803))
        );
        assert_eq!(
            topic_score("classification.prediction.articletopic/Culture.Linguistics|538"),
            Some(("Culture.Linguistics", 538))
        );
        // 話題の予測でないタグは None である。
        assert_eq!(topic_score("recommendation.link.exists|1"), None);
    }

    #[test]
    fn 点数が500以上の情報技術の記事だけを採る() {
        let record = |tag: &str| Record {
            namespace: Some(0),
            title: None,
            text: None,
            weighted_tags: Some(vec![tag.to_owned()]),
            redirect: None,
        };
        assert!(is_technology(&record(
            "classification.prediction.articletopic/STEM.Computing|500"
        )));
        assert!(is_technology(&record(
            "classification.prediction.articletopic/STEM.Technology*|999"
        )));
        // 点数が下限に届かない記事と、別の話題の記事は採らない。
        assert!(!is_technology(&record(
            "classification.prediction.articletopic/STEM.Computing|499"
        )));
        assert!(!is_technology(&record(
            "classification.prediction.articletopic/Culture.Linguistics|991"
        )));
    }

    #[test]
    fn 名前空間0のリダイレクトの名前だけを採る() {
        let redirect = |namespace: i64, title: &str| Redirect {
            namespace: Some(namespace),
            title: Some(title.to_owned()),
        };
        assert_eq!(
            article_redirect_titles(Some(vec![
                redirect(ARTICLE_NAMESPACE, "公開鍵暗号方式"),
                // 名前空間 14 は分類の頁であり、記事名ではない。
                redirect(14, "暗号技術"),
            ])),
            vec!["公開鍵暗号方式".to_owned()]
        );
        // リダイレクトを持たない記事では空になる。
        assert!(article_redirect_titles(None).is_empty());
    }

    #[test]
    fn ファイル名から版を取り出す() {
        let path = PathBuf::from("/a/jawiki-20251229-cirrussearch-content.json.gz");
        assert_eq!(version(&path).as_deref(), Some("20251229"));
        assert_eq!(
            file_name(&path),
            "jawiki-20251229-cirrussearch-content.json.gz"
        );
        // 8 桁の数字が無いファイル名では版を取れない。
        assert_eq!(version(&PathBuf::from("/a/dump.json.gz")), None);
    }
}
