// A host extension from Swift: a fenced `note` block rendered through
// hooks, plus a render override that claims `TODO:` list items.
//
// A host extension is behavior supplied from the embedding language at
// run time, declared with the same manifest the loader validates for a
// tier-2 extension (see examples/extensions/). Run it from this package:
//
//     swift run Geist4HostNotes

import Foundation
import Geist4

let notes = Geist4HostExtension(
    name: "notes",
    kinds: [Geist4KindSpec(name: "note", container: true, attributes: ["class"])])
notes.hooks.blockStartLook = { line, _, _ in line.hasPrefix(":::") }
notes.hooks.blockStart = { line, _, _, _ in
    line.trimmingCharacters(in: .whitespaces) == ":::note"
        ? Geist4Start(
            kind: "note", action: .wholeLine,
            attributes: [Geist4Attribute(name: "class", value: .text("tip"))])
        : nil
}
notes.hooks.blockContinue = { line, _, _, _ in
    line.trimmingCharacters(in: .whitespaces) == ":::"
        ? Geist4Continue.closeLine
        : Geist4Continue.keep(columns: 0)
}
notes.hooks.render = { kind, attributes, children in
    guard kind == "note",
        attributes == [Geist4Attribute(name: "class", value: .text("tip"))]
    else { return nil }
    return "<aside class=\"tip\">\(children)</aside>\n"
}

let todos = Geist4HostExtension(name: "todos")
todos.hooks.blockStartLook = { _, _, _ in true }
todos.hooks.blockStart = { line, _, _, _ in
    line.hasPrefix("TODO: ")
        ? Geist4Start(
            kind: "", action: .claim(6),
            attributes: [Geist4Attribute(name: "data-todo", value: .boolean(true))])
        : nil
}
todos.hooks.renderOverride = { _, _, childCount, child in
    var html = "<div class=\"todo\">"
    for index in 0..<childCount {
        guard let piece = child(index) else { break }
        html += piece
    }
    return html + "</div>"
}

let options = try Geist4Options()
try notes.register(on: options)
try todos.register(on: options)

let text = ":::note\nhello *world*\n:::\n\n- TODO: buy milk\n"
let document = try Geist4Document(text, options: options)
print(try document.html())
