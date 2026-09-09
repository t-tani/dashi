//! rst の本文を平文にする。
//!
//! pandoc は使わない。この機械に入っていないうえ、要るのは日本語の地の文だけで
//! あり、rst の全機能を解釈しなくても取れる。落とすのは、ディレクティブと字下げ
//! したリテラルブロック、コメント、見出しの飾りと表の罫線、フィールドの行である。
//! 入れ子のディレクティブと `include` の展開は扱わないので、含めた別ファイルの
//! 本文はそのファイルを読んだときに取れる。

use std::sync::LazyLock;

use aku_core::scan_plain_text;
use regex::{Captures, Regex};

use super::{plain_text, scan_config};

/// 本文がコードのディレクティブ。字下げした本体ごと落とす。ほかのディレクティブ
/// は注記や図の説明として日本語の本文を持つので、本体を残す。
const CODE_DIRECTIVES: [&str; 6] = [
    "code",
    "code-block",
    "sourcecode",
    "literalinclude",
    "parsed-literal",
    "math",
];

/// ディレクティブの行。`.. <名前>::` の後ろに引数が続く。
static DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^\.\.[ \t]+([A-Za-z][A-Za-z0-9_+.:-]*)::(.*)$")
        .expect("固定パターンのコンパイルに失敗しない")
});

/// フィールドの行。ディレクティブのオプション(`:depth: 3`)と、文書の冒頭に
/// 並ぶ書誌の欄がこの形である。閉じるコロンの後ろに空白が要る点で、行頭に来た
/// ロールと区別できる。ロールは閉じるコロンの後ろにバッククォートが続く。
static FIELD: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^:[A-Za-z][A-Za-z0-9_-]*:([ \t]|$)").expect("固定パターンのコンパイルに失敗しない")
});

/// 二重のバッククォートで囲んだリテラル。中身はコードなので落とす。
static LITERAL: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new("``[^`]*``").expect("固定パターンのコンパイルに失敗しない")
});

/// ロールを付けたテキスト。訳文は参照のロールの中に日本語の表題を置くので、
/// 表題を残し、ロールの名前と参照先を落とす。
static ROLE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r":[A-Za-z][A-Za-z0-9_+.:-]*:`([^`]*)`")
        .expect("固定パターンのコンパイルに失敗しない")
});

/// バッククォートで囲んだテキスト。リンクの表題に日本語を置くので、テキストだけを
/// 残す。末尾の `_` はリンクの目印である。
static BACKQUOTE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new("`([^`]*)`_{0,2}").expect("固定パターンのコンパイルに失敗しない")
});

/// リンクの参照先。ロールとリンクのテキストの末尾に付く。
static TARGET: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"[ \t]*<[^<>]*>").expect("固定パターンのコンパイルに失敗しない")
});

/// 置換のテキスト。`|version|` のような目印で、本文を持たない。
static SUBSTITUTION: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"\|[A-Za-z0-9_.+-]+\|").expect("固定パターンのコンパイルに失敗しない")
});

/// `source` の rst から、日本語の本文だけを取り出す。
pub fn flatten(source: &str) -> String {
    let body = strip_blocks(source);
    plain_text(&scan_plain_text(&body, scan_config()))
}

/// 行ごとに、ディレクティブとリテラルブロックとコメント、見出しの飾りと表の罫線、
/// フィールドの行を落とす。残った行はインラインのマークアップを外して返す。
fn strip_blocks(source: &str) -> String {
    let mut body = String::new();
    // 本体を落としているブロックの、始まりの行の字下げ。これより深い行を落とす。
    let mut dropping: Option<usize> = None;
    for line in source.lines() {
        let content = line.trim();
        if content.is_empty() {
            // 空行はブロックの終わりではない。リテラルブロックは空行をまたぐ。
            body.push('\n');
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if let Some(parent) = dropping {
            if indent > parent {
                continue;
            }
            dropping = None;
        }
        if let Some(captures) = DIRECTIVE.captures(content) {
            let (name, argument) = (&captures[1], captures[2].trim());
            if CODE_DIRECTIVES.contains(&name) {
                dropping = Some(indent);
            } else if !argument.is_empty() {
                // 注記や図の説明は、ディレクティブの行に引数として本文を置く。
                push_line(&mut body, argument);
            }
            continue;
        }
        if content.starts_with("..") {
            // コメントとリンクの目印。訳文のリポジトリは原文の英語をコメントで
            // 残すので、字下げした本体ごと落とさなければ英語が本文に混ざる。
            dropping = Some(indent);
            continue;
        }
        if FIELD.is_match(content) || is_adornment(content) {
            continue;
        }
        if let Some(head) = content.strip_suffix("::") {
            // 行末の `::` は、続く字下げしたリテラルブロックの始まりである。
            dropping = Some(indent);
            push_line(&mut body, head);
            continue;
        }
        push_line(&mut body, content);
    }
    body
}

/// インラインのマークアップを外した `content` を `body` へ足す。
fn push_line(body: &mut String, content: &str) {
    body.push_str(strip_inline(content).trim());
    body.push('\n');
}

/// 見出しの飾り(`====`)と表の罫線(`+----+`)なら true。ASCII の記号だけで
/// できた 2 文字以上の行がこれにあたり、地の文はこの形にならない。
fn is_adornment(content: &str) -> bool {
    content.chars().count() >= 2
        && content
            .chars()
            .all(|character| character.is_ascii_punctuation())
}

/// インラインのマークアップを外す。po の訳文も rst のマークアップを持つので、
/// [`super::po`] がこの関数を呼ぶ。
pub(super) fn strip_inline(text: &str) -> String {
    let text = LITERAL.replace_all(text, "");
    let text = ROLE.replace_all(&text, |captures: &Captures<'_>| {
        TARGET.replace_all(&captures[1], "").into_owned()
    });
    let text = BACKQUOTE.replace_all(&text, |captures: &Captures<'_>| {
        TARGET.replace_all(&captures[1], "").into_owned()
    });
    SUBSTITUTION.replace_all(&text, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 変換前の rst。訳文のリポジトリの書き方に合わせ、原文の英語をコメントで
    /// 残し、見出しに飾りを付け、コードブロックをディレクティブで書く。
    const RST: &str = r".. URL: https://example.com/compose/
.. SOURCE:
   doc version: v27
      https://example.com/source

.. Sharing configuration between files
.. _sharing-configuration:

==================
設定ファイルの共有
==================

.. contents::
    :depth: 3

.. note:: 引数に置いた注記の本文は残す。

.. Compose reads two files by default.

既定では、Compose は ``docker-compose.yml`` を読む。詳しくは
:ref:`設定の追加 <adding-configuration>` を読む。

次のように実行する::

    $ docker compose up
    秘密の値をここに書く

.. code-block:: bash

    $ docker compose down
    取り消しの手順をここに書く

外側の本文はここで続く。
";

    #[test]
    fn rstの変換後に日本語の本文だけが残る() {
        assert_eq!(
            flatten(RST),
            "設定ファイルの共有\n\
             引数に置いた注記の本文は残す。\n\
             既定では、Compose は  を読む。詳しくは\n設定の追加 を読む。\n\
             次のように実行する\n\
             外側の本文はここで続く。\n"
        );
    }

    #[test]
    fn コードブロックの中の文字列は本文に残らない() {
        let text = flatten(RST);
        // 行末の `::` が始めるリテラルブロックと、ディレクティブの本体の両方。
        assert!(!text.contains("秘密の値"));
        assert!(!text.contains("取り消しの手順"));
        assert!(!text.contains("docker compose up"));
        // 原文の英語を残したコメントと、その字下げした本体。
        assert!(!text.contains("Compose reads two files"));
        assert!(!text.contains("doc version"));
        // インラインのリテラルと、リンクの参照先。
        assert!(!text.contains("docker-compose.yml"));
        assert!(!text.contains("adding-configuration"));
    }

    #[test]
    fn インラインのマークアップを外す() {
        // ロールとリンクは日本語のテキストを残し、参照先だけを落とす。
        assert_eq!(strip_inline(":ref:`設定の追加 <adding>`"), "設定の追加");
        assert_eq!(
            strip_inline("`公式の手引き <https://example.com>`_"),
            "公式の手引き"
        );
        // 二重のバッククォートの中身はコードなので落とす。
        assert_eq!(strip_inline("既定は ``true`` である"), "既定は  である");
        // 置換の目印は本文を持たない。
        assert_eq!(strip_inline("版は |version| である"), "版は  である");
    }

    #[test]
    fn 見出しの飾りと罫線を落とす() {
        assert!(is_adornment("=================="));
        assert!(is_adornment("+------+------+"));
        // 地の文と、1 文字の記号は落とさない。
        assert!(!is_adornment("設定ファイルの共有"));
        assert!(!is_adornment("-"));
    }
}
