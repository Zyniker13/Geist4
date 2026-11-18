import CGeist4

/// The status the C ABI reports, with the message from its error buffer.
/// `status` is nil when the C entry point failed by returning a null
/// handle, which carries no status of its own.
public struct Geist4Error: Error, Equatable, CustomStringConvertible {
    /// The `geist4_status` value from the C ABI, when it reported one.
    public let status: Int32?
    /// What went wrong, from the C ABI's error buffer.
    public let message: String

    public var description: String {
        status.map { "\(message) (status \($0))" } ?? message
    }
}

/// One author mistake found while parsing, with its span in the source.
public struct Geist4Problem: Equatable {
    public let message: String
    /// Byte offset of the first byte of the span.
    public let start: Int
    /// Byte offset one past the last byte of the span.
    public let end: Int
}

/// The set of enabled extensions, held as a C options handle and freed
/// when the object is released. A document keeps its own copy of the
/// options, so the document renders after its options object is released.
public final class Geist4Options {
    let handle: OpaquePointer?

    public init() throws {
        handle = try callHandle { _ in
            geist4_options_create()
        }
    }

    /// Enables a built-in extension by name ("gfm" is the bundle of the
    /// CommonMark extras). Unknown names throw with the valid names listed.
    public func enable(builtin name: String) throws {
        try call { err in
            withUTF8Pointer(name) { chars, count in
                geist4_options_enable_builtin(
                    handle, chars, count, err, errCapacity)
            }
        }
    }

    deinit { geist4_options_free(handle) }
}

/// A parsed document. The source text is copied in at parse time, so the
/// document keeps its own copy and renders after its options are freed.
public final class Geist4Document {
    let handle: OpaquePointer?

    /// Parses `markdown`, optionally with a set of enabled extensions.
    /// Throws when the input is not valid UTF-8; a Swift String always
    /// is, so from Swift this surfaces only through data-based paths.
    public init(_ markdown: String, options: Geist4Options? = nil) throws {
        handle = try callHandle { err in
            withUTF8Pointer(markdown) { chars, count in
                geist4_parse(
                    options?.handle, chars, count,
                    err, errCapacity)
            }
        }
    }

    /// The document rendered to HTML.
    public func html() throws -> String {
        var out = geist4_string(data: nil, len: 0)
        try call { err in
            geist4_document_html(handle, &out, err, errCapacity)
        }
        defer { geist4_string_free(out) }
        return string(from: out)
    }

    /// The problems found while parsing, in source order.
    public func problems() throws -> [Geist4Problem] {
        var list = geist4_problem_list(items: nil, len: 0)
        try call { err in
            geist4_document_problems(handle, &list, err, errCapacity)
        }
        guard list.len > 0, let items = list.items else { return [] }
        var problems: [Geist4Problem] = []
        problems.reserveCapacity(list.len)
        for index in 0..<list.len {
            let problem = items[index]
            problems.append(Geist4Problem(
                message: string(from: problem.message, len: problem.message_len),
                start: problem.start,
                end: problem.end))
        }
        return problems
    }

    deinit { geist4_document_free(handle) }
}

/// The engine version, the same string the C ABI's geist4_version reports.
public func geist4Version() -> String {
    String(cString: geist4_version())
}

/// Parses and renders in one call, without holding a document handle.
public func renderHTML(_ markdown: String,
                       options: Geist4Options? = nil) throws -> String {
    var out = geist4_string(data: nil, len: 0)
    try call { err in
        withUTF8Pointer(markdown) { chars, count in
            geist4_render_html(
                options?.handle, chars, count,
                &out, err, errCapacity)
        }
    }
    defer { geist4_string_free(out) }
    return string(from: out)
}

// MARK: - Internals

private let errCapacity: Int = 256

/// Runs `body` with the UTF-8 bytes of `text` as a C char pointer and
/// their count. `withUTF8` is mutating in Swift 6, so the text is copied
/// into a local `var` first; the bit cast from `UInt8` to `CChar` is
/// size-identical and safe.
private func withUTF8Pointer<T>(
    _ text: String,
    _ body: (UnsafePointer<CChar>?, Int) throws -> T
) rethrows -> T {
    var text = text
    return try text.withUTF8 { raw in
        try raw.withMemoryRebound(to: CChar.self) { chars in
            try body(chars.baseAddress, chars.count)
        }
    }
}

/// Runs a C entry point that returns a status; a non-OK status throws
/// with the message the entry point wrote to the error buffer.
private func call(
    _ body: (UnsafeMutablePointer<CChar>?) throws -> Int32
) throws {
    var err = [CChar](repeating: 0, count: errCapacity)
    let raw = try err.withUnsafeMutableBufferPointer { buffer in
        try body(buffer.baseAddress)
    }
    if raw != GEIST4_OK.rawValue {
        throw Geist4Error(status: raw, message: string(from: err))
    }
}

/// Runs a C entry point that returns a handle or null; a null handle
/// throws with the message from the error buffer.
private func callHandle(
    _ body: (UnsafeMutablePointer<CChar>?) throws -> OpaquePointer?
) throws -> OpaquePointer? {
    var err = [CChar](repeating: 0, count: errCapacity)
    let handle = try err.withUnsafeMutableBufferPointer { buffer in
        try body(buffer.baseAddress)
    }
    guard let handle else {
        var message = string(from: err)
        if message.isEmpty { message = "the call failed without a message" }
        throw Geist4Error(status: nil, message: message)
    }
    return handle
}

/// Reads a NUL-terminated buffer of at most `errCapacity` bytes.
private func string(from err: [CChar]) -> String {
    err.withUnsafeBufferPointer { buffer in
        guard let base = buffer.baseAddress else { return "" }
        return String(cString: base)
    }
}

/// Reads a `geist4_string`: an owned, NUL-terminated buffer and its length.
private func string(from value: geist4_string) -> String {
    string(from: value.data, len: value.len)
}

/// Reads `len` bytes of (possibly non-NUL-terminated) UTF-8 text.
private func string(from data: UnsafePointer<CChar>?, len: Int) -> String {
    guard let data, len > 0 else { return "" }
    let bytes = UnsafeRawBufferPointer(start: data, count: len)
        .bindMemory(to: UInt8.self)
    return String(decoding: bytes, as: UTF8.self)
}
