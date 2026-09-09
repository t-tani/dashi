//! 複合語の回数を run へ書き出し、キー順に併合して 1 つの表にする。
//!
//! 全記事の複合語のキーは数千万件になり、1 つのハッシュ表で持つと数 GB を占める。
//! そこで表が一定の大きさに達したら run として書き出し、表を空にする。run は
//! キーの昇順に並ぶので、最後に run を同時に読み進め、同じキーの回数を足しながら
//! 1 つの表へ書ける。メモリは run 1 つ分のキーと、run ごとの 1 行で済む。

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::tsv;

/// 書き出した run の一覧。
pub struct Runs {
    /// run を置くディレクトリ。
    dir: PathBuf,
    /// run のファイル名の頭。この後ろに通し番号と `.tsv` が続く。表ごとに違う
    /// 頭を渡すので、同じディレクトリへ書き出しても run が混ざらない。
    prefix: &'static str,
    /// 書き出した順の run のディレクトリ。
    paths: Vec<PathBuf>,
}

impl Runs {
    /// `dir` に、`prefix` で始まる名前の run を置く。
    pub fn new(dir: &Path, prefix: &'static str) -> Self {
        Self {
            dir: dir.to_path_buf(),
            prefix,
            paths: Vec::new(),
        }
    }

    /// `table` を新しい run へ書き、`table` を空にする。空の表では run を作らない。
    ///
    /// # Errors
    ///
    /// run を書けない場合に返す。
    pub fn spill(&mut self, table: &mut HashMap<Box<str>, u32>) -> Result<()> {
        if table.is_empty() {
            return Ok(());
        }
        let path = self
            .dir
            .join(format!("{}{:04}.tsv", self.prefix, self.paths.len()));
        tsv::write(&path, table)?;
        table.clear();
        // clear は確保した容量をそのまま残す。書き出しの目的はメモリを空けること
        // なので、容量も手放す。
        table.shrink_to_fit();
        self.paths.push(path);
        Ok(())
    }

    /// すべての run を併合して `path` へ書き、run を消す。返すのはキーの数である。
    ///
    /// # Errors
    ///
    /// run を読めない場合、run の行が `<キー>\t<回数>` の形でない場合、書き出しが
    /// 失敗する場合に返す。
    pub fn merge_into(self, path: &Path) -> Result<u64> {
        let mut runs: Vec<Run> = self
            .paths
            .iter()
            .map(|path| Run::open(path))
            .collect::<Result<_>>()?;
        let mut heap: BinaryHeap<Reverse<(Box<str>, usize)>> = BinaryHeap::new();
        for index in 0..runs.len() {
            push_head(&mut runs, index, &mut heap)?;
        }

        let file = File::create(path).with_context(|| format!("{} を作れない", path.display()))?;
        let mut writer = BufWriter::new(file);
        let mut keys = 0;
        while let Some(Reverse((key, index))) = heap.pop() {
            // run の中でキーは狭義に増えるので、次の行が同じキーになることはない。
            // 同じキーは必ず別の run から来る。
            let mut total = runs[index].count;
            push_head(&mut runs, index, &mut heap)?;
            while heap.peek().is_some_and(|head| head.0.0 == key) {
                if let Some(Reverse((_, other))) = heap.pop() {
                    total = total.saturating_add(runs[other].count);
                    push_head(&mut runs, other, &mut heap)?;
                }
            }
            writeln!(writer, "{key}\t{total}")
                .with_context(|| format!("{} を書けない", path.display()))?;
            keys += 1;
        }
        writer
            .flush()
            .with_context(|| format!("{} を書けない", path.display()))?;

        drop(runs);
        for run_path in &self.paths {
            std::fs::remove_file(run_path)
                .with_context(|| format!("{} を消せない", run_path.display()))?;
        }
        Ok(keys)
    }
}

/// 読み進めている run 1 つ。`count` は heap が持つキーの回数である。
struct Run {
    /// run のディレクトリ。読めない行を報告するために持つ。
    path: PathBuf,
    reader: BufReader<File>,
    /// 読んだ行を持つ緩衝。行ごとに確保し直さないために使い回す。
    line: String,
    /// 最後に読んだ行の回数。
    count: u64,
}

impl Run {
    /// `path` の run を開く。
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("{} を開けない", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            reader: BufReader::new(file),
            line: String::new(),
            count: 0,
        })
    }

    /// 次の行のキー。回数は [`Run::count`] に置く。run が尽きたら `None` を返す。
    fn next_key(&mut self) -> Result<Option<Box<str>>> {
        self.line.clear();
        if self
            .reader
            .read_line(&mut self.line)
            .with_context(|| format!("{} を読めない", self.path.display()))?
            == 0
        {
            return Ok(None);
        }
        let row = self.line.trim_end_matches('\n');
        let Some((key, count)) = row.rsplit_once('\t') else {
            bail!("{} の行にタブが無い", self.path.display());
        };
        self.count = count
            .parse()
            .with_context(|| format!("{} の回数が数でない", self.path.display()))?;
        Ok(Some(key.into()))
    }
}

/// `index` の run から次のキーを読み、heap へ積む。run が尽きていれば積まない。
fn push_head(
    runs: &mut [Run],
    index: usize,
    heap: &mut BinaryHeap<Reverse<(Box<str>, usize)>>,
) -> Result<()> {
    if let Some(key) = runs[index].next_key()? {
        heap.push(Reverse((key, index)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 同じキーの回数を足して昇順に併合する() {
        let dir = std::env::temp_dir().join("corpus-tool-runs-test");
        std::fs::create_dir_all(&dir).unwrap();
        let mut runs = Runs::new(&dir, "compound-run-");
        let mut table: HashMap<Box<str>, u32> = HashMap::new();
        table.insert("公開鍵".into(), 2);
        table.insert("暗号方式".into(), 3);
        runs.spill(&mut table).unwrap();
        // 書き出した表は空になり、次の run は続きだけを持つ。
        assert!(table.is_empty());
        table.insert("公開鍵".into(), 5);
        table.insert("電子署名".into(), 7);
        runs.spill(&mut table).unwrap();
        // 空の表では run を作らないので、3 本目は増えない。
        runs.spill(&mut table).unwrap();

        let merged = dir.join("compound-counts.tsv");
        assert_eq!(runs.merge_into(&merged).unwrap(), 3);
        assert_eq!(
            std::fs::read_to_string(&merged).unwrap(),
            "公開鍵\t7\n暗号方式\t3\n電子署名\t7\n"
        );
        // 併合し終えた run は残らない。
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().starts_with("compound-run-"))
            .collect();
        assert!(left.is_empty(), "run が残っている: {left:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
