//! The host extension surface, end to end: extensions built from
//! `HostHooks` run through a real parse and render, exactly as a
//! binding's callbacks will, and every failure path declines
//! instead of unwinding into the engine.

use std::sync::Arc;

use geist4_core::extension::Extension;
use geist4_ffi::safe::{
    AttrValue, BracketClaim, ContinueConsume, ContinueDecision, HostError, HostExtension,
    HostHooks, KindDecl, Options, OwnedDocument, StartConsume, StartDecision,
};

fn extension(
    name: &str,
    kinds: Vec<KindDecl>,
    triggers: Vec<u8>,
    hooks: HostHooks,
) -> Arc<dyn Extension> {
    Arc::new(
        HostExtension::from_hooks(name.to_string(), kinds, triggers, hooks)
            .expect("valid extension"),
    )
}

// ---------------------------------------------------------------------------
// Blocks.
// ---------------------------------------------------------------------------

/// A fenced block: `:::note` opens, `:::` closes, the render hook
/// wraps the children. The close hook declines, so the node keeps its
/// children and the render hook owns the output.
#[test]
fn fenced_block_starts_continues_and_renders() {
    let hooks = HostHooks {
        block_start_look: Some(Box::new(|line: &str, _indented: bool, _column: u32| {
            Ok(line.starts_with(":::"))
        })),
        block_start: Some(Box::new(
            |line: &str, _indented: bool, _column: u32, _container: Option<&str>| {
                if line.trim_start_matches(":::").trim() == "note" {
                    Ok(Some(StartDecision {
                        consume: StartConsume::WholeLine,
                        kind: "note".to_string(),
                        attrs: vec![("class".to_string(), AttrValue::Str("tip".to_string()))],
                    }))
                } else {
                    Ok(None)
                }
            },
        )),
        block_continue: Some(Box::new(
            |line: &str, _indented: bool, _column: u32, _blank: bool| {
                if line.trim() == ":::" {
                    Ok(Some(ContinueDecision::CloseLine))
                } else {
                    Ok(Some(ContinueDecision::Keep(ContinueConsume::Rest)))
                }
            },
        )),
        render: Some(Box::new(
            |kind: &str, attrs: &[(String, String)], children: &str| {
                assert_eq!(kind, "note");
                assert_eq!(attrs, &[("class".to_string(), "tip".to_string())]);
                Ok(Some(format!("<aside class=\"tip\">{children}</aside>\n")))
            },
        )),
        ..HostHooks::default()
    };
    let kinds = vec![KindDecl {
        kind: "note".to_string(),
        container: true,
        inline_text: false,
        attrs: vec!["class".to_string()],
    }];
    let mut options = Options::new();
    options
        .enable_host(extension("notes", kinds, vec![], hooks))
        .unwrap();
    let html = OwnedDocument::parse(":::note\nhello *world*\n:::\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(
        html, "<aside class=\"tip\"><p>hello <em>world</em></p>\n</aside>\n",
        "the render hook owns the block's output with its children inside"
    );
}

/// A block whose close hook returns HTML owns its whole output and
/// the children render nowhere — the raw-HTML realization.
#[test]
fn block_close_html_replaces_the_children() {
    let hooks = HostHooks {
        block_start: Some(Box::new(
            |line: &str, _indented: bool, _column: u32, _container: Option<&str>| {
                // Decline is Ok(None); only a genuine error is Err (it latches).
                Ok(line.starts_with("%%%").then(|| StartDecision {
                    consume: StartConsume::WholeLine,
                    kind: "sign".to_string(),
                    attrs: vec![],
                }))
            },
        )),
        block_close: Some(Box::new(|| Ok(Some("<hr class=\"sign\">\n".to_string())))),
        block_continue: Some(Box::new(
            |line: &str, _indented: bool, _column: u32, _blank: bool| {
                if line.starts_with("%%%") {
                    Ok(Some(ContinueDecision::CloseLine))
                } else {
                    Ok(Some(ContinueDecision::Keep(ContinueConsume::Rest)))
                }
            },
        )),
        ..HostHooks::default()
    };
    let kinds = vec![KindDecl {
        kind: "sign".to_string(),
        container: true,
        inline_text: false,
        attrs: vec![],
    }];
    let mut options = Options::new();
    options
        .enable_host(extension("signs", kinds, vec![], hooks))
        .unwrap();
    let html = OwnedDocument::parse("%%%\nhidden\n%%%\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(
        html, "<hr class=\"sign\">\n",
        "the close hook's HTML is the whole output; the children render nowhere"
    );
}

/// The claim consume: the host claims the deepest plain container
/// (a list item), marks it, and the render override owns the node's
/// whole output — no built-in fallback.
#[test]
fn claim_marks_the_container_and_the_override_renders() {
    let hooks = HostHooks {
        block_start_look: Some(Box::new(|line: &str, _indented: bool, _column: u32| {
            Ok(line.starts_with("TODO: "))
        })),
        block_start: Some(Box::new(
            |line: &str, _indented: bool, _column: u32, _container: Option<&str>| {
                if line.starts_with("TODO: ") {
                    Ok(Some(StartDecision {
                        consume: StartConsume::Claim(6),
                        kind: String::new(),
                        attrs: vec![("data-todo".to_string(), AttrValue::Bool(true))],
                    }))
                } else {
                    Ok(None)
                }
            },
        )),
        render_override: Some(Box::new(|_node, children: &[String], out: &mut String| {
            out.push_str(&format!("<div class=\"todo\">{}</div>", children.concat()));
            Ok(true)
        })),
        ..HostHooks::default()
    };
    let mut options = Options::new();
    options
        .enable_host(extension("todos", vec![], vec![], hooks))
        .unwrap();
    let html = OwnedDocument::parse("- TODO: buy milk\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(
        html, "<ul>\n<div class=\"todo\">buy milk</div></ul>\n",
        "the claimed item renders only through the override; a tight \
         list's paragraphs render bare"
    );
}

/// An inline trigger tags the run at the cursor with a kind; a later
/// run of the same byte with the same kind closes against it, the way
/// paired delimiters pair, and the render hook renders the node with
/// its inline text children. An unclosed run stays literal text.
#[test]
fn trigger_tags_a_run_and_renders() {
    let hooks = HostHooks {
        inline_trigger: Some(Box::new(|_line: &str, _pos: u32| {
            Ok(Some("mark".to_string()))
        })),
        render: Some(Box::new(
            |kind: &str, _attrs: &[(String, String)], children: &str| {
                if kind == "mark" {
                    Ok(Some(format!("<mark>{children}</mark>")))
                } else {
                    Ok(None)
                }
            },
        )),
        ..HostHooks::default()
    };
    let kinds = vec![KindDecl {
        kind: "mark".to_string(),
        container: false,
        inline_text: true,
        attrs: vec![],
    }];
    let mut options = Options::new();
    options
        .enable_host(extension("marks", kinds, vec![b'='], hooks))
        .unwrap();
    let html = OwnedDocument::parse("a ==bold== b\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(
        html, "<p>a <mark>bold</mark> b</p>\n",
        "the paired runs wrapped their content through the hook"
    );
    let html = OwnedDocument::parse("a ==x b\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(
        html, "<p>a ==x b</p>\n",
        "an unclosed run stays literal text"
    );
}

/// An inline bracket claims `[note]` (which matched no link) as a
/// note-ref node with the whole content kept as its children.
#[test]
fn bracket_claims_an_unmatched_group() {
    let hooks = HostHooks {
        inline_bracket: Some(Box::new(|content: &str| {
            if content == "note" {
                Ok(Some(BracketClaim {
                    kind: "note-ref".to_string(),
                    attrs: vec![],
                    end: u32::MAX,
                }))
            } else {
                Ok(None)
            }
        })),
        render: Some(Box::new(
            |kind: &str, _attrs: &[(String, String)], children: &str| {
                Ok((kind == "note-ref").then(|| format!("<a class=\"note-ref\">{children}</a>")))
            },
        )),
        ..HostHooks::default()
    };
    let kinds = vec![KindDecl {
        kind: "note-ref".to_string(),
        container: false,
        inline_text: true,
        attrs: vec![],
    }];
    let mut options = Options::new();
    options
        .enable_host(extension("refs", kinds, vec![], hooks))
        .unwrap();
    let html = OwnedDocument::parse("see [note] here\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(
        html, "<p>see <a class=\"note-ref\">note</a> here</p>\n",
        "the unmatched bracket group rendered through the claim"
    );
}

/// Post-process rewrites text nodes; code spans and blocks are never
/// offered.
#[test]
fn post_process_rewrites_text_only() {
    let hooks = HostHooks {
        post_process: Some(Box::new(|text: &str| Ok(Some(text.replace("GMT", "UTC"))))),
        ..HostHooks::default()
    };
    let mut options = Options::new();
    options
        .enable_host(extension("zones", vec![], vec![], hooks))
        .unwrap();
    let html = OwnedDocument::parse(
        "Time GMT, see `GMT` code\n\n    GMT in code\n",
        Some(&options),
    )
    .unwrap()
    .render_html()
    .unwrap();
    assert_eq!(
        html, "<p>Time UTC, see <code>GMT</code> code</p>\n<pre><code>GMT in code\n</code></pre>\n",
        "text nodes are offered; code is not"
    );
}

/// A callback failure latches: the extension declines for that parse,
/// the document renders plain, and `take_error` hands the message
/// back exactly once.
#[test]
fn hook_failure_latches_and_surfaces_once() {
    let hooks = HostHooks {
        post_process: Some(Box::new(|_text: &str| {
            Err("host callback exploded".to_string())
        })),
        ..HostHooks::default()
    };
    let host =
        Arc::new(HostExtension::from_hooks("boom".to_string(), vec![], vec![], hooks).unwrap());
    let mut options = Options::new();
    options.enable_host(host.clone()).unwrap();
    let html = OwnedDocument::parse("hello GMT\n", Some(&options))
        .unwrap()
        .render_html()
        .unwrap();
    assert_eq!(html, "<p>hello GMT</p>\n", "the failed hook declined");
    assert_eq!(
        host.take_error().as_deref(),
        Some("host callback exploded"),
        "the latched message is handed back once"
    );
    assert!(host.take_error().is_none(), "taken means taken");
}

/// The declared data is validated at construction, the way the loader
/// validates a tier-2 manifest.
#[test]
fn declared_data_is_validated() {
    let invalid_name =
        HostExtension::from_hooks("Bad Name".to_string(), vec![], vec![], HostHooks::default());
    assert!(matches!(invalid_name, Err(HostError::Name { .. })));

    let reserved =
        HostExtension::from_hooks("tables".to_string(), vec![], vec![], HostHooks::default());
    assert!(matches!(reserved, Err(HostError::Reserved { .. })));

    let bad_kind = HostExtension::from_hooks(
        "ok".to_string(),
        vec![KindDecl {
            kind: "Nope".to_string(),
            container: false,
            inline_text: false,
            attrs: vec![],
        }],
        vec![],
        HostHooks::default(),
    );
    assert!(matches!(bad_kind, Err(HostError::Kind { .. })));

    let bad_attr = HostExtension::from_hooks(
        "ok".to_string(),
        vec![KindDecl {
            kind: "fine".to_string(),
            container: false,
            inline_text: false,
            attrs: vec!["bad name".to_string()],
        }],
        vec![],
        HostHooks::default(),
    );
    assert!(matches!(bad_attr, Err(HostError::Attr { .. })));

    let many_triggers = HostExtension::from_hooks(
        "ok".to_string(),
        vec![],
        vec![b'a'; 33],
        HostHooks::default(),
    );
    assert!(matches!(many_triggers, Err(HostError::Triggers { .. })));
}

/// A second extension with a name already registered is rejected. A
/// built-in's name is rejected earlier, at construction — hosts may
/// never shadow a built-in.
#[test]
fn duplicate_names_are_rejected() {
    let builtin =
        HostExtension::from_hooks("tables".to_string(), vec![], vec![], HostHooks::default());
    assert!(
        matches!(builtin, Err(HostError::Reserved { .. })),
        "a host may not take a built-in's name"
    );

    let mut options = Options::new();
    options
        .enable_host(extension("dup", vec![], vec![], HostHooks::default()))
        .unwrap();
    let again = options.enable_host(extension("dup", vec![], vec![], HostHooks::default()));
    assert!(matches!(again, Err(HostError::Duplicate { .. })));
}
