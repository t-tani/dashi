//! po から訳文を取り出して平文にする。

use aku_core::scan_plain_text;

use super::{plain_text, rst, scan_config};

/// 訳文が始まる行。複数形の訳文は `msgstr[0]` の形で並ぶ。
const MSGSTR: &str = "msgstr";

/// 原文が始まる行。
const MSGID: &str = "msgid ";

/// `source` の po から訳文を取り出し、平文にする。
///
/// 訳していない項目は `msgstr` が空なので落とす。原文が空の項目はファイルの
/// 見出しであり、訳文の欄に版と作業者の記録を持つので落とす。python の訳文は
/// rst のマークアップを持つので、インラインのマークアップは rst の関数で外す。
pub fn flatten(source: &str) -> String {
    let messages = messages(source).join("\n\n");
    plain_text(&scan_plain_text(&messages, scan_config()))
}

/// いま読んでいる欄。次の行が `"` で始まる続きの行なら、この欄へ足す。
#[derive(Clone, Copy)]
enum Field {
    /// msgid にも msgstr にも続かない。コメントと空行の後がこれである。
    None,
    /// 原文。
    Msgid,
    /// 訳文。
    Msgstr,
}

/// 1 項目の原文と訳文。
#[derive(Default)]
struct Entry {
    msgid: String,
    msgstrs: Vec<String>,
}

/// 本文を持つ訳文を、出現順に返す。
fn messages(source: &str) -> Vec<String> {
    let mut messages = Vec::new();
    let mut entry = Entry::default();
    let mut field = Field::None;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            field = Field::None;
        } else if let Some(literal) = line.strip_prefix(MSGID) {
            // 原文の行が項目の切れ目である。
            push_messages(&mut messages, &entry);
            entry = Entry {
                msgid: unquote(literal),
                msgstrs: Vec::new(),
            };
            field = Field::Msgid;
        } else if let Some(rest) = line.strip_prefix(MSGSTR) {
            let literal = rest.trim_start_matches(|character: char| character != '"');
            entry.msgstrs.push(unquote(literal));
            field = Field::Msgstr;
        } else if line.starts_with('"') {
            // 続きの行は、区切りを入れずに前の行へつなぐ。po は行の幅に収める
            // ためだけに文字列を割るので、語の途中で割れていることがある。
            match field {
                Field::Msgid => entry.msgid.push_str(&unquote(line)),
                Field::Msgstr => {
                    if let Some(last) = entry.msgstrs.last_mut() {
                        last.push_str(&unquote(line));
                    }
                }
                Field::None => {}
            }
        } else {
            // `msgid_plural` と `msgctxt` は msgid の側なので、本文には使わない。
            field = Field::None;
        }
    }
    push_messages(&mut messages, &entry);
    messages
}

/// 本文を持つ訳文を `messages` へ足す。msgid が空の見出しの項目は足さない。
fn push_messages(messages: &mut Vec<String>, entry: &Entry) {
    if entry.msgid.is_empty() {
        return;
    }
    for msgstr in &entry.msgstrs {
        let message = rst::strip_inline(msgstr);
        if !message.trim().is_empty() {
            messages.push(message);
        }
    }
}

/// po の文字列リテラルから中身を取り出し、脱出を戻す。
fn unquote(literal: &str) -> String {
    let literal = literal.trim();
    let inner = literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(literal);
    let mut text = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            text.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => text.push('\n'),
            Some('t') => text.push(' '),
            Some(escaped) => text.push(escaped),
            None => {}
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 変換前の po。見出しの項目、訳した項目、行を割った項目、訳していない項目が
    /// 並ぶ。
    const PO: &str = r#"# Translators:
# python-doc bot, 2026
#
#, fuzzy
msgid ""
msgstr ""
"Project-Id-Version: Python 3.15\n"
"Language: ja\n"

#: ../../library/os.rst:2
msgid ":mod:`!os` --- Miscellaneous operating system interfaces"
msgstr ":mod:`!os` --- 雑多なオペレーティングシステムインターフェース"

#: ../../library/os.rst:11
msgid ""
"This module provides a portable way of using operating system dependent "
"functionality."
msgstr ""
"このモジュールは、 OS 依存の機能を利用するポータブルな方法を提供しま"
"す。詳しくは :func:`open` を参照してください。"

#: ../../library/os.rst:19
msgid "Notes on the availability of these functions:"
msgstr ""
"#;

    #[test]
    fn poの変換後に訳文だけが残る() {
        assert_eq!(
            flatten(PO),
            "!os --- 雑多なオペレーティングシステムインターフェース\n\
             このモジュールは、 OS 依存の機能を利用するポータブルな方法を提供します。詳しくは open を参照してください。\n"
        );
    }

    #[test]
    fn 見出しと未訳の項目は落ちる() {
        let text = flatten(PO);
        // 原文が空の見出しの項目は、訳文の欄に版と言語の記録を持つ。
        assert!(!text.contains("Project-Id-Version"));
        assert!(!text.contains("Language: ja"));
        // msgid は残さない。
        assert!(!text.contains("This module provides"));
        assert!(!text.contains("Notes on the availability"));
    }

    #[test]
    fn 行を割った訳文を区切りなしでつなぐ() {
        // po は行の幅に収めるために語の途中で割る。
        let entry = "msgid \"x\"\nmsgstr \"\"\n\"提供しま\"\n\"す。\"\n";
        assert_eq!(messages(entry), vec!["提供します。".to_owned()]);
    }
}
