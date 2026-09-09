//! 成果物ファイルの検証用ハッシュ。
//!
//! `build` が組む 3 つの成果物ファイルは追跡しないので、`manifest.json` の
//! SHA-256 が唯一の照合手段になる。取得した配布物がこの値と一致するかで、
//! 壊れた転送やすり替えを検出できる。

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

/// `bytes` の SHA-256 を 16 進の小文字表記で返す。
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 既知の入力のハッシュを返す() {
        // 空文字列の SHA-256 は既知の値である。
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
