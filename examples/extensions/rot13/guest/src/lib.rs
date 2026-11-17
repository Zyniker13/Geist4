//! The `rot13` example's tier-2 guest: `%secret%` renders with every
//! letter rotated thirteen places. The hook logic — scanning a
//! delimiter run and transforming text outside tags — is what the
//! declarative tier cannot express.

wit_bindgen::generate!({
    path: "../../../../crates/geist4-ext-loader/wit/geist4-extension.wit",
    world: "geist4-extension",
});

use crate::exports::geist4::extension::hooks::{
    ContinueDecision, InlineAction, InlineDelimiter, KindDecl, OptionValue,
};

/// The one kind this guest implements.
const KNOWN: &[&str] = &["rot13"];

struct Rot13;

/// Rotates ASCII letters by thirteen places; everything else stands.
fn rot13_char(c: char) -> char {
    match c {
        'a'..='z' => ((c as u8 - b'a' + 13) % 26 + b'a') as char,
        'A'..='Z' => ((c as u8 - b'A' + 13) % 26 + b'A') as char,
        other => other,
    }
}

/// Rotates the letters of `text`, leaving anything between `<` and `>`
/// (a tag the engine's inline renderer emitted) untouched.
fn rot13_outside_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        if in_tag {
            out.push(c);
            if c == '>' {
                in_tag = false;
            }
        } else if c == '<' {
            in_tag = true;
            out.push(c);
        } else {
            out.push(rot13_char(c));
        }
    }
    out
}

impl crate::exports::geist4::extension::hooks::Guest for Rot13 {
    fn init(kinds: Vec<KindDecl>, _options: Vec<(String, OptionValue)>) -> Result<(), String> {
        for kind in &kinds {
            if !KNOWN.contains(&kind.kind.as_str()) {
                return Err(format!("unexpected kind: {}", kind.kind));
            }
        }
        Ok(())
    }

    fn block_start_look(_line: String, _indented: bool, _column: u32) -> bool {
        false
    }

    fn block_start(
        _line: String,
        _indented: bool,
        _column: u32,
        _container_kind: Option<String>,
    ) -> Option<crate::exports::geist4::extension::hooks::Start> {
        None
    }

    fn block_continue(
        _line: String,
        _indented: bool,
        _column: u32,
        _blank: bool,
    ) -> Option<ContinueDecision> {
        None
    }

    fn block_close() -> Option<crate::exports::geist4::extension::hooks::RenderedBlock> {
        None
    }

    fn inline_trigger(line: String, pos: u32) -> Option<InlineAction> {
        if line.as_bytes().get(pos as usize).copied() != Some(b'%') {
            return None;
        }
        // One run of consecutive `%` bytes: the engine pairs matching
        // runs and makes the content between them the node's inline
        // children, the way it pairs tier-1 span delimiters.
        let bytes = line.as_bytes();
        let mut end = pos as usize;
        while end < bytes.len() && bytes[end] == b'%' {
            end += 1;
        }
        Some(InlineAction::Delimiter(InlineDelimiter {
            kind: "rot13".to_owned(),
            end: end as u32,
        }))
    }

    fn inline_bracket(_content: String) -> Option<InlineAction> {
        None
    }

    fn post_process(text: String) -> String {
        text
    }

    fn render(
        kind: String,
        _attrs: Vec<(String, String)>,
        children: String,
    ) -> Result<String, String> {
        match kind.as_str() {
            "rot13" => Ok(format!("<rot13>{}</rot13>", rot13_outside_tags(&children))),
            _ => Err(format!("unknown kind: {kind}")),
        }
    }
}

export!(Rot13);
