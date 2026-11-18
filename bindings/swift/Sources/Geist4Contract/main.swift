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

print("\(checks - failures)/\(checks) checks passed")
if failures > 0 {
    exit(1)
}
