//! md と mdx の本文を平文にする。

use aku_core::scan;

use super::{plain_text, scan_config};

/// `source` の Markdown から、日本語の本文だけを取り出す。
///
/// フロントマター、コードブロック、コードスパン、HTML のタグ、リンク先の URL は
/// 走査が検査の対象から外すので、返す平文に残らない。mdx の JSX のタグは HTML と
/// して外れ、`import` の行は日本語の文字の比率が下限に届かないので落ちる。
pub fn flatten(source: &str) -> String {
    plain_text(&scan(source, scan_config()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 変換前の md。frontmatter・見出し・コードブロック・コードスパン・リンク・
    /// 英語だけの段落を持つ。
    const MARKDOWN: &str = r#"---
title: 設定ファイルの書き方
---

# 設定ファイル

`aku.toml` に既定値を書く。詳しくは [設定の一覧](https://example.com/config) を読む。

```toml
# 秘密の値をここに書く
token = "検査に出てはいけない文字列"
```

This paragraph is written in English and carries no Japanese prose.

> 注記の本文は引用ブロックに置く。
"#;

    #[test]
    fn mdの変換後に日本語の本文だけが残る() {
        assert_eq!(
            flatten(MARKDOWN),
            "設定ファイル\n\
             に既定値を書く。詳しくは 設定の一覧 を読む。\n\
             注記の本文は引用ブロックに置く。\n"
        );
    }

    #[test]
    fn コードブロックの中の文字列は本文に残らない() {
        let text = flatten(MARKDOWN);
        assert!(!text.contains("検査に出てはいけない文字列"));
        assert!(!text.contains("秘密の値"));
        // コードスパンの中身とリンク先の URL も残らない。
        assert!(!text.contains("aku.toml"));
        assert!(!text.contains("example.com"));
        // 英語だけの段落は日本語の文字の比率で落ちる。
        assert!(!text.contains("English"));
    }

    #[test]
    fn mdxのタグとimportの行は本文に残らない() {
        let source = "import Card from '../../components/Card.astro';\n\
                      \n\
                      <Card title=\"はじめに\">\n\
                      \n\
                      配置の手順をここに書く。\n\
                      \n\
                      </Card>\n";
        let text = flatten(source);
        assert_eq!(text, "配置の手順をここに書く。\n");
    }
}
