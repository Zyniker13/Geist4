import Foundation
import Geist4

// The Swift side of the cross-binding contract: the same byte-exact
// expectations the C contract test (crates/geist4-ffi/c-tests/test_ffi.c)
// pins, so every binding agrees on the engine's output.
//
// A plain executable rather than an XCTest suite: the Command Line Tools
// alone do not ship XCTest.framework, and the C and Node contract suites
// are plain executables too. `swift run Geist4Contract` exits non-zero
// when a check fails.
//
// The C-only cases (null input, invalid UTF-8) are unreachable from
// Swift: a Swift String is always valid UTF-8 and never null.

struct ContractFailure: Error, CustomStringConvertible {
    let description: String
}

var checks = 0
var failures = 0

func run(_ name: String, _ body: () throws -> Void) {
    checks += 1
    do {
        try body()
        print("ok - \(name)")
    } catch {
        failures += 1
        print("FAIL - \(name): \(error)")
    }
}

func require(_ condition: Bool, _ message: String) throws {
    if !condition {
        throw ContractFailure(description: message)
    }
}

func requireEqual<T: Equatable>(
    _ actual: T, _ expected: T, _ what: String
) throws {
    if actual != expected {
        throw ContractFailure(
            description: "\(what): got \(actual), want \(expected)")
    }
}

run("version is nonempty") {
    try require(!geist4Version().isEmpty, "geist4Version() is empty")
}

run("heading renders byte for byte") {
    try requireEqual(try renderHTML("# hi\n"), "<h1>hi</h1>\n", "html")
}

run("empty document renders empty") {
    try requireEqual(try renderHTML(""), "", "html")
}

run("NUL byte becomes a replacement character") {
    try requireEqual(
        try renderHTML("a\u{0}b\n"), "<p>a\u{FFFD}b</p>\n", "html")
}

run("gfm bundle enables strikethrough") {
    let options = try Geist4Options()
    try options.enable(builtin: "gfm")
    try requireEqual(
        try renderHTML("~~gone~~\n", options: options),
        "<p><del>gone</del></p>\n", "html")
}

run("gfm table renders byte for byte") {
    let options = try Geist4Options()
    try options.enable(builtin: "gfm")
    let source = "| a | b |\n| --- | --- |\n| 1 | 2 |\n"
    try require(source.utf8.count == 34, "source is 34 bytes")
    try requireEqual(
        try renderHTML(source, options: options),
        "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n"
            + "</thead>\n<tbody>\n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n"
            + "</tbody>\n</table>\n",
        "html")
}

run("unknown extension is rejected with valid names") {
    let options = try Geist4Options()
    do {
        _ = try options.enable(builtin: "nope")
        throw ContractFailure(description: "no error thrown")
    } catch let error as Geist4Error {
        try require(
            error.message.contains("valid names"),
            "rejection lists the valid names: \(error.message)")
    }
}

run("strikethrough enables alone") {
    let options = try Geist4Options()
    try options.enable(builtin: "strikethrough")
    try requireEqual(
        try renderHTML("~~gone~~\n", options: options),
        "<p><del>gone</del></p>\n", "html")
}

run("document outlives options and renders twice") {
    let source = "~~x~~ [text][missing]\n"
    let document: Geist4Document
    do {
        let options = try Geist4Options()
        try options.enable(builtin: "strikethrough")
        document = try Geist4Document(source, options: options)
    }
    // The options object is released at the end of the do block, before
    // the document renders: the document keeps its own copy of the
    // options.
    let html = try document.html()
    try requireEqual(html, "<p><del>x</del> [text][missing]</p>\n", "html")
    try requireEqual(try document.html(), html, "second render")
}

run("problems name the missing reference with a span") {
    let options = try Geist4Options()
    try options.enable(builtin: "strikethrough")
    let source = "~~x~~ [text][missing]\n"
    let document = try Geist4Document(source, options: options)
    let problems = try document.problems()
    try require(!problems.isEmpty, "at least one problem")
    var sawMissing = false
    for problem in problems {
        let sourceLength = source.utf8.count
        try require(problem.start < problem.end, "span is non-empty")
        try require(problem.end <= sourceLength, "span is inside the source")
        if problem.message.contains("missing") {
            sawMissing = true
        }
    }
    try require(sawMissing, "a problem names the missing reference")
}

// ---------------------------------------------------------------------------
// Host extensions: the tier-3 surface, registered through the C ABI's
// `Geist4HostVTable` with Swift closures bridged by trampolines.
// ---------------------------------------------------------------------------

run("host extension renders a fenced block through the hooks") {
    let ext = Geist4HostExtension(
        name: "notes",
        kinds: [Geist4KindSpec(name: "note", container: true, attributes: ["class"])])
    ext.hooks.blockStartLook = { line, _, _ in line.hasPrefix(":::") }
    ext.hooks.blockStart = { line, _, _, _ in
        line.dropFirst(3).trimmingCharacters(in: .whitespaces) == "note"
            ? Geist4Start(
                kind: "note", action: .wholeLine,
                attributes: [Geist4Attribute(name: "class", value: .text("tip"))])
            : nil
    }
    ext.hooks.blockContinue = { line, _, _, _ in
        line.trimmingCharacters(in: .whitespaces) == ":::"
            ? Geist4Continue.closeLine
            : Geist4Continue.keep(columns: 0)
    }
    ext.hooks.render = { kind, attributes, children in
        guard kind == "note", attributes == [Geist4Attribute(name: "class", value: .text("tip"))]
        else { return nil }
        return "<aside class=\"tip\">\(children)</aside>\n"
    }
    let options = try Geist4Options()
    try ext.register(on: options)
    try requireEqual(
        try renderHTML(":::note\nhello *world*\n:::\n", options: options),
        "<aside class=\"tip\"><p>hello <em>world</em></p>\n</aside>\n",
        "the render hook owns the block's output with its children inside")
}

run("host block close owns the whole output") {
    let ext = Geist4HostExtension(
        name: "signs", kinds: [Geist4KindSpec(name: "sign", container: true)])
    ext.hooks.blockStart = { line, _, _, _ in
        line.hasPrefix("%%%")
            ? Geist4Start(kind: "sign", action: .wholeLine)
            : nil
    }
    ext.hooks.blockContinue = { line, _, _, _ in
        line.hasPrefix("%%%")
            ? Geist4Continue.closeLine
            : Geist4Continue.keep(columns: 0)
    }
    ext.hooks.blockClose = { "<hr class=\"sign\">\n" }
    let options = try Geist4Options()
    try ext.register(on: options)
    try requireEqual(
        try renderHTML("%%%\nhidden\n%%%\n", options: options),
        "<hr class=\"sign\">\n",
        "the close hook's HTML is the whole output; the children render nowhere")
}

run("host claim renders through the override") {
    let ext = Geist4HostExtension(name: "todos")
    ext.hooks.blockStartLook = { line, _, _ in line.hasPrefix("TODO: ") }
    ext.hooks.blockStart = { line, _, _, _ in
        line.hasPrefix("TODO: ")
            ? Geist4Start(
                kind: "", action: .claim(6),
                attributes: [Geist4Attribute(name: "data-todo", value: .boolean(true))])
            : nil
    }
    ext.hooks.renderOverride = { _, _, childCount, child in
        var html = "<div class=\"todo\">"
        for index in 0..<childCount {
            guard let piece = child(index) else { break }
            html += piece
        }
        return html + "</div>"
    }
    let options = try Geist4Options()
    try ext.register(on: options)
    try requireEqual(
        try renderHTML("- TODO: buy milk\n", options: options),
        "<ul>\n<div class=\"todo\">buy milk</div></ul>\n",
        "the claimed item renders only through the override")
}

run("host inline trigger tags and renders a run") {
    let ext = Geist4HostExtension(
        name: "marks",
        kinds: [Geist4KindSpec(name: "mark", inlineText: true)],
        triggers: ["="])
    ext.hooks.inlineTrigger = { _, _ in Geist4Inline.delimiter(kind: "mark") }
    ext.hooks.render = { kind, _, children in
        kind == "mark" ? "<mark>\(children)</mark>" : nil
    }
    let options = try Geist4Options()
    try ext.register(on: options)
    try requireEqual(
        try renderHTML("a ==bold== b\n", options: options),
        "<p>a <mark>bold</mark> b</p>\n",
        "the paired runs wrapped their content through the hook")
    try requireEqual(
        try renderHTML("a ==x b\n", options: options),
        "<p>a ==x b</p>\n",
        "an unclosed run stays literal text")
}

run("host bracket claims an unmatched group") {
    let ext = Geist4HostExtension(
        name: "refs",
        kinds: [Geist4KindSpec(name: "note-ref", inlineText: true)])
    ext.hooks.inlineBracket = { content in
        content == "note"
            ? Geist4Inline.node(kind: "note-ref", end: .max, attributes: [])
            : nil
    }
    ext.hooks.render = { kind, _, children in
        kind == "note-ref" ? "<a class=\"note-ref\">\(children)</a>" : nil
    }
    let options = try Geist4Options()
    try ext.register(on: options)
    try requireEqual(
        try renderHTML("see [note] here\n", options: options),
        "<p>see <a class=\"note-ref\">note</a> here</p>\n",
        "the unmatched bracket group rendered through the claim")
}

run("host post-process rewrites text but not code") {
    let ext = Geist4HostExtension(name: "zones")
    ext.hooks.postProcess = { text in
        text.replacingOccurrences(of: "GMT", with: "UTC")
    }
    let options = try Geist4Options()
    try ext.register(on: options)
    try requireEqual(
        try renderHTML("Time GMT, see `GMT` code\n\n    GMT in code\n", options: options),
        "<p>Time UTC, see <code>GMT</code> code</p>\n"
            + "<pre><code>GMT in code\n</code></pre>\n",
        "text nodes are offered; code is not")
}

run("a failing host hook declines and surfaces through problems") {
    let ext = Geist4HostExtension(name: "boom")
    ext.hooks.postProcess = { _ in
        throw ContractFailure(description: "host callback exploded")
    }
    let options = try Geist4Options()
    try ext.register(on: options)
    let document = try Geist4Document("hello GMT\n", options: options)
    try requireEqual(try document.html(), "<p>hello GMT</p>\n", "the failed hook declined")
    let problems = try document.problems()
    try requireEqual(problems.count, 1, "one latched problem")
    try requireEqual(
        problems[0].message,
        "extension 'boom': the post-process hook failed with status -1",
        "the latched C-path message surfaces")
}

run("host extension validation rejects bad declared data") {
    func rejects(_ make: () -> Geist4HostExtension, _ within: String) throws {
        do {
            try make().register(on: Geist4Options())
            throw ContractFailure(description: "no error thrown for \(within)")
        } catch let error as Geist4Error {
            try require(
                error.message.contains(within),
                "'\(error.message)' mentions \(within)")
        }
    }
    try rejects({ Geist4HostExtension(name: "Bad Name") }, "host extension:")
    try rejects({ Geist4HostExtension(name: "tables") }, "reserved")
    try rejects(
        { Geist4HostExtension(name: "ok", kinds: [Geist4KindSpec(name: "Nope")]) },
        "host extension kind \"Nope\":")
    try rejects(
        {
            Geist4HostExtension(
                name: "ok",
                kinds: [Geist4KindSpec(name: "fine", attributes: ["bad name"])])
        },
        "attribute \"bad name\":")
    try rejects(
        { Geist4HostExtension(name: "ok", triggers: [String](repeating: "=", count: 33)) },
        "host extension triggers:")
}

run("a duplicate host extension name is rejected") {
    let options = try Geist4Options()
    try Geist4HostExtension(name: "dup").register(on: options)
    do {
        try Geist4HostExtension(name: "dup").register(on: options)
        throw ContractFailure(description: "no error thrown for the duplicate")
    } catch let error as Geist4Error {
        try require(
            error.message.contains("already registered"),
            "'\(error.message)' names the duplicate")
    }
}

print("\(checks - failures)/\(checks) checks passed")
if failures > 0 {
    exit(1)
}
