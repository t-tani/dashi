//! 取得した日本語の技術文書を、1 文書 1 ファイルの平文にする。
//!
//! 入力は取得の側が書いた `manifest.tsv` と、その隣に並ぶソースごとの
//! リポジトリである。形式の欄が変換の実装を選び、md と mdx と po と rst を変換
//! する。残る形式は変換せず、書き出す manifest に文書数 0 の行として残す。
//!
//! 正解集合の採り先のソースも平文にする。頻度に入れるかどうかは `count` の
//! `--exclude` が決めるので、評価用の成果物ではそこで外し、配布用では入れる。

mod markdown;
mod po;
mod rst;

use std::fs;
use std::path::{Path, PathBuf};

use aku_core::{ScanConfig, ScanResult};
use anyhow::{Context, Result, bail};

/// 取得の記録と平文化の記録の両方に使う manifest のファイル名。
const MANIFEST_FILE: &str = "manifest.tsv";

/// 平文のディレクトリ。
const TEXT_DIR: &str = "text";

/// 変換しなかったソースの、パスの欄に書く値。
const NO_OUTPUT_DIR: &str = "-";

/// 平文に残すのに要る、日本語の文字の比率の下限。akunuki が検査で使う既定と同じ
/// 値である。訳し残した英語の段落と、識別子だけを並べた表のセルがこの下限で落ちる。
const MIN_JAPANESE_RATIO: f64 = 0.20;

/// `manifest.tsv` の 1 行。取得の側が書く 8 欄をそのまま持つ。
struct Source {
    name: String,
    repo: String,
    branch: String,
    commit: String,
    commit_date: String,
    paths: String,
    format: String,
    license: String,
}

/// 1 ソースを平文にした結果。
struct Flattened {
    /// 平文を置いたディレクトリの名前。変換しない形式では [`NO_OUTPUT_DIR`]。
    output_dir: &'static str,
    /// 書き出した文書の数。
    documents: u64,
    /// 書き出した平文のバイト数。
    text_bytes: u64,
}

/// 平文にする形式。`manifest.tsv` の形式の欄がこれを選ぶ。
#[derive(Clone, Copy)]
enum Format {
    /// Markdown。mdx も同じ走査で読む。
    Markdown,
    /// gettext の po。
    Po,
    /// reStructuredText。
    Rst,
}

impl Format {
    /// `format` の欄に対応する変換。対応する変換が無い形式では `None` を返す。
    fn from_manifest(format: &str) -> Option<Self> {
        match format {
            "md" | "mdx" => Some(Self::Markdown),
            "po" => Some(Self::Po),
            "rst" => Some(Self::Rst),
            _ => None,
        }
    }

    /// 読む拡張子。形式が md のソースも mdx のファイルを混ぜて持つので、Markdown は
    /// 両方を読む。
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Markdown => &["md", "mdx"],
            Self::Po => &["po"],
            Self::Rst => &["rst"],
        }
    }

    /// 1 文書の原文を平文にする。
    fn flatten(self, source: &str) -> String {
        match self {
            Self::Markdown => markdown::flatten(source),
            Self::Po => po::flatten(source),
            Self::Rst => rst::flatten(source),
        }
    }
}

/// `raw_dir` の取得物を平文にし、`out_dir` の下へソースごとに書く。
///
/// # Errors
///
/// manifest を読めない場合、取得物のディレクトリを読めない場合、平文を書けない
/// 場合に返す。
pub fn run(raw_dir: &Path, out_dir: &Path) -> Result<()> {
    let manifest_path = raw_dir.join(MANIFEST_FILE);
    let sources = read_manifest(&manifest_path)?;
    let mut results = Vec::with_capacity(sources.len());
    for source in &sources {
        let result = flatten_source(raw_dir, out_dir, source)?;
        eprintln!(
            "{}: {} 文書、{} バイト({})",
            source.name, result.documents, result.text_bytes, result.output_dir
        );
        results.push(result);
    }
    let out_manifest = out_dir.join(TEXT_DIR).join(MANIFEST_FILE);
    write_manifest(&out_manifest, &sources, &results)?;

    let documents: u64 = results.iter().map(|result| result.documents).sum();
    let text_bytes: u64 = results.iter().map(|result| result.text_bytes).sum();
    println!(
        "{} ソースから {documents} 文書、{text_bytes} バイトを書いた",
        sources.len()
    );
    println!("manifest: {}", out_manifest.display());
    Ok(())
}

/// 1 ソースの取得物を読み、平文を書く。形式を変換しない場合は何も書かない。
fn flatten_source(raw_dir: &Path, out_dir: &Path, source: &Source) -> Result<Flattened> {
    let Some(format) = Format::from_manifest(&source.format) else {
        return Ok(Flattened {
            output_dir: NO_OUTPUT_DIR,
            documents: 0,
            text_bytes: 0,
        });
    };
    let source_root = raw_dir.join(&source.name);
    let out_root = out_dir.join(TEXT_DIR).join(&source.name);
    let mut documents = 0;
    let mut text_bytes = 0;
    for path in documents_in(&source_root, format)? {
        let Some(original) = read_utf8(&path)? else {
            continue;
        };
        let text = format.flatten(&original);
        if text.is_empty() {
            continue;
        }
        let relative = path.strip_prefix(&source_root).with_context(|| {
            format!("{} が {} の下にない", path.display(), source_root.display())
        })?;
        let out_path = output_path(&out_root, relative);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("{} を作れない", parent.display()))?;
        }
        fs::write(&out_path, &text)
            .with_context(|| format!("{} を書けない", out_path.display()))?;
        documents += 1;
        text_bytes += text.len() as u64;
    }
    Ok(Flattened {
        output_dir: TEXT_DIR,
        documents,
        text_bytes,
    })
}

/// 平文のディレクトリ。原文のディレクトリの形を保ち、ファイル名の末尾に `.txt` を足す。
///
/// 拡張子を置き換えると、同じディレクトリの `a.md` と `a.mdx` が同じ名前になる。
fn output_path(out_root: &Path, relative: &Path) -> PathBuf {
    let path = out_root.join(relative);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".txt");
    path.with_file_name(name)
}

/// `path` を UTF-8 として読む。UTF-8 でないファイルは、名前を書き出して `None` を
/// 返す。1 ファイルの文字コードで 20 ソースの走行を落とさないためである。
fn read_utf8(path: &Path) -> Result<Option<String>> {
    let bytes = fs::read(path).with_context(|| format!("{} を読めない", path.display()))?;
    let Ok(text) = String::from_utf8(bytes) else {
        eprintln!("{} を飛ばす: UTF-8 でない", path.display());
        return Ok(None);
    };
    Ok(Some(text))
}

/// `root` の下から、`format` が読む拡張子のファイルをパスの昇順で集める。
/// `.git` のディレクトリは読まない。
fn documents_in(root: &Path, format: Format) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    collect_documents(root, format, &mut found)?;
    found.sort();
    Ok(found)
}

/// `dir` を再帰で辿り、`format` が読むファイルを `found` へ足す。
fn collect_documents(dir: &Path, format: Format, found: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("{} を読めない", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("{} を読めない", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("{} の種類を読めない", path.display()))?;
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            collect_documents(&path, format, found)?;
        } else if path
            .extension()
            .is_some_and(|extension| format.extensions().iter().any(|name| extension == *name))
        {
            found.push(path);
        }
    }
    Ok(())
}

/// 取得の manifest を読む。
fn read_manifest(path: &Path) -> Result<Vec<Source>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("{} を読めない", path.display()))?;
    let mut sources = Vec::new();
    for (index, line) in text.lines().enumerate().skip(1) {
        let columns: Vec<&str> = line.split('\t').collect();
        let [
            name,
            repo,
            branch,
            commit,
            commit_date,
            paths,
            format,
            license,
        ] = columns[..]
        else {
            bail!("{} の {} 行目の欄が 8 つでない", path.display(), index + 1);
        };
        sources.push(Source {
            name: name.to_owned(),
            repo: repo.to_owned(),
            branch: branch.to_owned(),
            commit: commit.to_owned(),
            commit_date: commit_date.to_owned(),
            paths: paths.to_owned(),
            format: format.to_owned(),
            license: license.to_owned(),
        });
    }
    Ok(sources)
}

/// 平文の manifest を書く。取得の 8 欄に、平文のディレクトリと文書数とバイト数を足す。
fn write_manifest(path: &Path, sources: &[Source], results: &[Flattened]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("{} を作れない", parent.display()))?;
    }
    let mut rows = vec![String::from(
        "name\trepo\tbranch\tcommit\tcommit_date\tpaths\tformat\tlicense\toutput_dir\tdocuments\ttext_bytes",
    )];
    for (source, result) in sources.iter().zip(results) {
        rows.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            source.name,
            source.repo,
            source.branch,
            source.commit,
            source.commit_date,
            source.paths,
            source.format,
            source.license,
            result.output_dir,
            result.documents,
            result.text_bytes,
        ));
    }
    let text = rows.join("\n") + "\n";
    fs::write(path, text).with_context(|| format!("{} を書けない", path.display()))
}

/// 走査の設定。引用ブロックは注記の書き方として本文を持つので検査の対象に含める。
fn scan_config() -> ScanConfig {
    ScanConfig {
        include_blockquote: true,
        min_japanese_ratio: MIN_JAPANESE_RATIO,
        ..ScanConfig::default()
    }
}

/// 走査が返した断片を段落ごとに連ね、段落を改行で区切った平文にする。
///
/// 段落の原文([`aku_core::Paragraph`] の `text`)は、インラインのコードと
/// リンク先の URL を含んだままである。断片だけを連ねれば、走査が検査の対象から
/// 外したものは平文に残らない。
fn plain_text(result: &ScanResult<'_>) -> String {
    let mut text = String::new();
    let mut fragments = result.fragments.iter().peekable();
    for paragraph in &result.paragraphs {
        let mut line = String::new();
        while let Some(fragment) =
            fragments.next_if(|fragment| fragment.range.start < paragraph.range.end)
        {
            if fragment.range.start >= paragraph.range.start {
                line.push_str(fragment.text);
            }
        }
        let line = line.trim();
        if !line.is_empty() {
            text.push_str(line);
            text.push('\n');
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 平文のディレクトリは原文の名前に拡張子を足す() {
        // 拡張子を置き換えると、同じディレクトリの md と mdx が衝突する。
        assert_eq!(
            output_path(Path::new("text/astro"), Path::new("guides/deploy.mdx")),
            PathBuf::from("text/astro/guides/deploy.mdx.txt")
        );
    }

    #[test]
    fn 変換しない形式は形式の欄で外れる() {
        assert!(Format::from_manifest("md").is_some());
        assert!(Format::from_manifest("mdx").is_some());
        assert!(Format::from_manifest("po").is_some());
        assert!(Format::from_manifest("rst").is_some());
        // 取得はするが平文にしない 5 つの形式。
        for format in ["vimhelp", "sgml", "docbook", "rd", "asciidoc"] {
            assert!(Format::from_manifest(format).is_none(), "{format}");
        }
    }
}
