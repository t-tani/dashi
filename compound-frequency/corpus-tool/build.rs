//! `Cargo.lock` が固定している akunuki の commit を、実行ファイルへ環境変数
//! `CORPUS_TOOL_AKUNUKI_REV` として渡す。成果物の manifest はこの値を書く。
//!
//! commit を写した定数をソースに置くと、依存を上げたときに manifest だけが
//! 古い commit を指す。lock ファイルから読めば、依存と manifest は必ず揃う。

use std::path::Path;

/// `Cargo.lock` で akunuki の crate に付く取得元の接頭辞。この後ろに
/// `?rev=<指定>#<commit>` が続く。
const AKUNUKI_SOURCE_PREFIX: &str = "git+https://github.com/t-tani/akunuki.git";

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo が渡すこと");
    let lock_path = Path::new(&manifest_dir).join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());
    let lock = std::fs::read_to_string(&lock_path)
        .unwrap_or_else(|error| panic!("{} を読めない: {error}", lock_path.display()));
    let rev = akunuki_rev(&lock).unwrap_or_else(|| {
        panic!(
            "{} に {AKUNUKI_SOURCE_PREFIX} の commit が無い",
            lock_path.display()
        )
    });
    println!("cargo:rustc-env=CORPUS_TOOL_AKUNUKI_REV={rev}");
}

/// lock ファイルの本文から、akunuki の取得元の行が持つ commit を取り出す。
/// 行は `source = "<接頭辞>?rev=<指定>#<commit>"` の形である。
fn akunuki_rev(lock: &str) -> Option<&str> {
    lock.lines()
        .filter_map(|line| line.trim().strip_prefix("source = \""))
        .find(|source| source.starts_with(AKUNUKI_SOURCE_PREFIX))
        .and_then(|source| source.trim_end_matches('"').rsplit('#').next())
}
