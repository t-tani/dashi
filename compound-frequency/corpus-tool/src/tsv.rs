//! 回数の TSV の読み書き。1 行が `<キー>\t<回数>` である。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

/// 回数の表を `path` へ書く。行はキーの昇順に並べる。
///
/// # Errors
///
/// ファイルを作れない場合、書き出しが失敗する場合に返す。
pub fn write(path: &Path, table: &HashMap<Box<str>, u32>) -> Result<()> {
    let mut rows: Vec<(&str, u32)> = table
        .iter()
        .map(|(key, count)| (key.as_ref(), *count))
        .collect();
    rows.sort_unstable();
    let file = File::create(path).with_context(|| format!("{} を作れない", path.display()))?;
    let mut writer = BufWriter::new(file);
    for (key, count) in rows {
        writeln!(writer, "{key}\t{count}")
            .with_context(|| format!("{} を書けない", path.display()))?;
    }
    writer
        .flush()
        .with_context(|| format!("{} を書けない", path.display()))
}

/// `path` の回数の表を読む。
///
/// # Errors
///
/// ファイルを開けない場合、行が `<キー>\t<回数>` の形でない場合に返す。
pub fn read(path: &Path) -> Result<Vec<(String, u64)>> {
    let mut rows = Vec::new();
    for_each_row(path, |key, count| rows.push((key.to_owned(), count)))?;
    Ok(rows)
}

/// `path` の回数の表を 1 行ずつ読み、キーと回数を `visit` へ渡す。
///
/// 全記事を数えた複合語の回数は 38,021,885 行あり、[`read`] のように 1 つの
/// `Vec` へ集めると数 GB を占める。行を選んで持つ呼び出し側は、こちらを使う。
///
/// # Errors
///
/// ファイルを開けない場合、行が `<キー>\t<回数>` の形でない場合に返す。
pub fn for_each_row(path: &Path, mut visit: impl FnMut(&str, u64)) -> Result<()> {
    let file = File::open(path).with_context(|| format!("{} を開けない", path.display()))?;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("{} を読めない", path.display()))?;
        let Some((key, count)) = line.rsplit_once('\t') else {
            bail!("{} の {} 行目にタブが無い", path.display(), index + 1);
        };
        let count: u64 = count
            .parse()
            .with_context(|| format!("{} の {} 行目の回数が数でない", path.display(), index + 1))?;
        visit(key, count);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 書いた表を読み直せる() {
        let dir = std::env::temp_dir().join("corpus-tool-tsv-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("counts.tsv");
        let mut table = HashMap::new();
        table.insert("公開鍵".into(), 12_u32);
        table.insert("暗号方式".into(), 3);
        write(&path, &table).unwrap();
        // 行はキーの昇順に並ぶ。
        assert_eq!(
            read(&path).unwrap(),
            vec![("公開鍵".to_owned(), 12_u64), ("暗号方式".to_owned(), 3)]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
