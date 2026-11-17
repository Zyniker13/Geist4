//! The hostile fuzz fixture's guest: a component that plays by its
//! own rules. Three probes, each one the host must survive:
//!
//! * `%%bad:` lines open a block of the *undeclared* kind `bogus` —
//!   the adapter must latch a protocol violation, never route a
//!   foreign kind into the tree.
//! * `[[…]]` groups claim an inline node with an `end` far past the
//!   content — the consumer must keep the whole content, never panic.
//! * a render call with the `probe="huge"` attribute builds a
//!   20-million-byte string — the sandbox's memory cap must stop it
//!   as a defined error, never a host crash.
//! * `%%spin` lines answer the block-start hook with an endless loop
//!   — the sandbox's fuel must end it as a defined error, never a
//!   host hang.
//!
//! The kind `hug` is the one declared kind, used by the bracket probe.

wit_bindgen::generate!({
    path: "../../../crates/geist4-ext-loader/wit/geist4-extension.wit",
    world: "geist4-extension",
});

use crate::exports::geist4::extension::hooks::{
    Consume, ContinueDecision, InlineAction, InlineNode, KindDecl, OptionValue, Start,
};

struct Hostile;

/// An endless loop: the only way out is the sandbox's fuel. The
/// counter keeps every iteration side-effecting, so the optimizer
/// cannot fold the loop away.
fn spin() -> u32 {
    let mut counter = 0u32;
    loop {
        counter = counter.wrapping_add(1);
        std::hint::black_box(counter);
    }
}

impl crate::exports::geist4::extension::hooks::Guest for Hostile {
    fn init(_kinds: Vec<KindDecl>, _options: Vec<(String, OptionValue)>) -> Result<(), String> {
        Ok(())
    }

    fn block_start_look(line: String, _indented: bool, _column: u32) -> bool {
        if line.starts_with("%%spin") {
            spin();
        }
        line.starts_with("%%")
    }

    fn block_start(
        line: String,
        _indented: bool,
        _column: u32,
        _container_kind: Option<String>,
    ) -> Option<Start> {
        if line.starts_with("%%spin") {
            // The engine reaches block-start for every line, so the
            // endless loop burns the sandbox's fuel here.
            spin();
        }
        if line.starts_with("%%bad:") {
            Some(Start {
                kind: "bogus".to_owned(),
                consume: Consume::WholeLine,
                attrs: Vec::new(),
            })
        } else {
            None
        }
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

    fn inline_trigger(_line: String, _pos: u32) -> Option<InlineAction> {
        None
    }

    fn inline_bracket(content: String) -> Option<InlineAction> {
        let probe = if content == "big" { "huge" } else { "span" };
        Some(InlineAction::Node(InlineNode {
            kind: "hug".to_owned(),
            attrs: vec![("probe".to_owned(), probe.to_owned())],
            end: 999_999,
        }))
    }

    fn post_process(text: String) -> String {
        text
    }

    fn render(
        _kind: String,
        attrs: Vec<(String, String)>,
        children: String,
    ) -> Result<String, String> {
        let probe = attrs
            .iter()
            .find(|(name, _)| name == "probe")
            .map(|(_, value)| value.as_str());
        if probe == Some("huge") {
            // Twenty million zeroed bytes: linear memory beyond the
            // sandbox's cap. The grow must fail as a defined error.
            let blob: Vec<u8> = vec![b'x'; 20_000_000];
            return Ok(String::from_utf8_lossy(&blob).into_owned());
        }
        Ok(format!("<hug>{children}</hug>"))
    }
}

export!(Hostile);
