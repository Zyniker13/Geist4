// The cross-binding suite's Swift runner: renders the case list through
// the Swift binding over the C ABI and writes one JSON line per case,
// the same shape the Rust reference (`cross_reference`) and the other
// bindings' runners write. The driver (`tools/cross_binding.py`)
// requires every binding's html for every case to match the reference's
// byte for byte.
//
// Usage: Geist4Cross <cases.jsonl> <results.jsonl>

import Foundation
import Geist4

let arguments = CommandLine.arguments
guard arguments.count == 3 else {
    FileHandle.standardError.write("usage: Geist4Cross <cases.jsonl> <results.jsonl>\n".data(using: .utf8)!)
    exit(2)
}
let casesURL = URL(fileURLWithPath: arguments[1])
let resultsURL = URL(fileURLWithPath: arguments[2])

guard let raw = try? String(contentsOf: casesURL, encoding: .utf8) else {
    FileHandle.standardError.write("Geist4Cross: cannot read \(arguments[1])\n".data(using: .utf8)!)
    exit(2)
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write("Geist4Cross: \(message)\n".data(using: .utf8)!)
    exit(1)
}

var output = ""
var cache: [String: Geist4Options] = [:]
for line in raw.split(separator: "\n", omittingEmptySubsequences: true) {
    guard let lineData = line.data(using: .utf8),
        let object = try? JSONSerialization.jsonObject(with: lineData),
        let testCase = object as? [String: Any],
        let name = testCase["name"] as? String,
        let source = testCase["source"] as? String
    else {
        fail("a case line is not a JSON object with name and source")
    }
    let names = testCase["options"] as? [String] ?? []
    let key = names.joined(separator: "\u{0}")
    let options: Geist4Options
    if let cached = cache[key] {
        options = cached
    } else {
        guard let fresh = try? Geist4Options() else {
            fail("the options object could not be created")
        }
        for name in names {
            do {
                try fresh.enable(builtin: name)
            } catch {
                fail("case \(name): \(error)")
            }
        }
        cache[key] = fresh
        options = fresh
    }
    let html: String
    do {
        html = try renderHTML(source, options: options)
    } catch {
        fail("case \(name): \(error)")
    }
    let record: [String: Any] = ["name": name, "html": html]
    guard let resultData = try? JSONSerialization.data(withJSONObject: record),
        let resultLine = String(data: resultData, encoding: .utf8)
    else {
        fail("case \(name): the result could not be serialized")
    }
    output += resultLine + "\n"
}

do {
    try output.write(to: resultsURL, atomically: true, encoding: .utf8)
} catch {
    FileHandle.standardError.write("Geist4Cross: cannot write \(arguments[2])\n".data(using: .utf8)!)
    exit(2)
}
