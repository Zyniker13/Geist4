import CGeist4

// The tier-3 host-extension surface for Swift: extensions whose behavior
// is supplied as Swift closures, registered through the C ABI's
// `Geist4HostVTable`. The C ABI is a function-pointer table, so each hook
// is bridged by a static trampoline: the extension's context is a box
// holding the closures, and every trampoline unpacks it, calls the
// closure, and marshals the answer across the ABI.
//
// Buffer ownership follows the ABI's contract: every byte buffer the
// engine passes a hook is borrowed for the call only, and every buffer a
// hook hands back is read before the next hook runs. The trampolines keep
// the buffers they hand back in a pending arena that is cleared when the
// next hook of the same extension runs and when the box is released, so
// no buffer is freed while the engine can still read it.

/// One declared attribute value, tagged the way the ABI's `Geist4Attr`
/// carries it: a string, an integer, or a boolean.
public struct Geist4Attribute: Equatable {
    public enum Value: Equatable {
        case text(String)
        case integer(Int)
        case boolean(Bool)
    }

    public let name: String
    public let value: Value

    public init(name: String, value: Value) {
        self.name = name
        self.value = value
    }
}

/// One declared extension kind: a name with its shape flags and the
/// attribute names the extension may set on nodes of that kind.
public struct Geist4KindSpec {
    public let name: String
    /// The kind opens a block that holds blocks.
    public let container: Bool
    /// The kind may hold inline text.
    public let inlineText: Bool
    public let attributes: [String]

    public init(name: String, container: Bool = false,
                inlineText: Bool = false, attributes: [String] = []) {
        self.name = name
        self.container = container
        self.inlineText = inlineText
        self.attributes = attributes
    }
}

/// A `block-start` decision: which consume applies to the line, the kind
/// to open (ignored for the claim action), and the attributes to set on
/// the opened or claimed node.
public struct Geist4Start {
    public enum Action: Equatable {
        /// Consume the whole line, open the block.
        case wholeLine
        /// Seek past `columns` columns, open, keep the rest of the line
        /// as the block's first paragraph.
        case columns(Int)
        /// Seek to the content, open, keep the whole rest of the line.
        case rest
        /// The task-list pattern: consume `bytes` bytes and claim the
        /// deepest plain container; nothing opens.
        case claim(Int)
    }

    public let kind: String
    public let action: Action
    public let attributes: [Geist4Attribute]

    public init(kind: String, action: Action, attributes: [Geist4Attribute] = []) {
        self.kind = kind
        self.action = action
        self.attributes = attributes
    }
}

/// A `block-continue` decision: keep the block open, or close it and
/// consume the line whole.
public enum Geist4Continue: Equatable {
    /// Consume `columns` more columns and keep going; `.max` is the
    /// whole line.
    case keep(columns: UInt32)
    /// Close the block and consume the line whole.
    case closeLine
}

/// An inline decision: leave the bytes as text, tag the scanned run with
/// a kind, or claim the bracket group's content as a node.
public enum Geist4Inline: Equatable {
    /// Leave the bytes as text.
    case literal
    /// Tag the scanned run with the kind; a later run of the same byte
    /// with the same kind closes against it.
    case delimiter(kind: String)
    /// Claim the bracket group's content as a node of the kind, keeping
    /// `end` bytes of the content as its children; `.max` is the whole
    /// group.
    case node(kind: String, end: UInt32, attributes: [Geist4Attribute])
}

/// The extension's hooks. Each is optional; a `nil` hook declines
/// everything that hook would have decided. A hook that throws reports
/// failure to the engine, which latches a message and declines for that
/// parse — the C ABI carries no message channel, so the thrown error's
/// text stays Swift-side.
public struct Geist4HostHooks {
    /// Called once per parse before any other hook. A thrown error
    /// rejects the parse — the extension latches and declines.
    public var initHook: (() throws -> Void)?
    /// Whether the extension would start a block on this line.
    public var blockStartLook: ((_ line: String, _ indented: Bool,
                                 _ column: Int) throws -> Bool)?
    /// Decides whether this line starts one of the extension's blocks.
    public var blockStart: ((_ line: String, _ indented: Bool,
                             _ column: Int,
                             _ containerKind: String?) throws -> Geist4Start?)?
    /// Feeds one line to an open block.
    public var blockContinue: ((_ line: String, _ indented: Bool,
                                _ column: Int, _ blank: Bool)
        throws -> Geist4Continue?)?
    /// Called when the block's last line is consumed; returning HTML
    /// makes the block's output exactly that HTML, children and all.
    public var blockClose: (() throws -> String?)?
    /// The trigger byte was scanned at `pos` in the line.
    public var inlineTrigger: ((_ line: String, _ pos: Int)
        throws -> Geist4Inline?)?
    /// A bracket group's content matched no link or footnote.
    public var inlineBracket: ((_ content: String) throws -> Geist4Inline?)?
    /// Rewrites text nodes; code spans and blocks are never offered.
    public var postProcess: ((_ text: String) throws -> String?)?
    /// Renders a node of a declared kind: the kind, its attributes, and
    /// the children's HTML already rendered.
    public var render: ((_ kind: String, _ attributes: [Geist4Attribute],
                         _ childrenHTML: String) throws -> String?)?
    /// Renders a claimed node: the kind, its attributes, the child
    /// count, and a fetcher for each child's HTML by index; a fetcher
    /// call past the last child returns `nil`. Returning HTML owns the
    /// node's whole output — no built-in fallback.
    public var renderOverride: ((_ kind: String,
                                 _ attributes: [Geist4Attribute],
                                 _ childCount: Int,
                                 _ child: (Int) -> String?) throws -> String?)?

    public init() {}
}

/// A host extension built from Swift closures, ready to register on an
/// options object with `Geist4Options.enableHost(_:)`.
public final class Geist4HostExtension {
    public let name: String
    public let kinds: [Geist4KindSpec]
    public let triggers: [String]
    /// Var so a caller can fill hooks after construction:
    /// `ext.hooks.blockStart = { ... }`.
    public var hooks: Geist4HostHooks

    public init(name: String, kinds: [Geist4KindSpec] = [],
                triggers: [String] = [], hooks: Geist4HostHooks = Geist4HostHooks()) {
        self.name = name
        self.kinds = kinds
        self.triggers = triggers
        self.hooks = hooks
    }

    /// Registers the extension on the options. Throws when the declared
    /// data breaks the loader's rules (bad names, kinds, attributes or
    /// too many triggers) or when the name is already registered — a
    /// built-in's name is reserved, and hosts may never shadow it.
    public func register(on options: Geist4Options) throws {
        try options.enableHost(self)
    }
}

extension Geist4Options {
    /// Registers a host extension on the options.
    public func enableHost(_ host: Geist4HostExtension) throws {
        // The box holds the closures and the buffers handed back to the
        // engine; the engine releases it through `free_ctx` when its
        // last reference to the extension drops.
        let box = HostBox(hooks: host.hooks)

        // The declared data is read and copied during the registration
        // call, so the spans and arrays only need to live that long.
        var pinned: [UnsafeMutableRawPointer] = []
        func pin<T>(_ bytes: [UInt8], as type: T.Type) -> UnsafePointer<UInt8> {
            let count = max(bytes.count, 1)
            let raw = UnsafeMutableRawPointer.allocate(
                byteCount: count * MemoryLayout<T>.stride,
                alignment: MemoryLayout<T>.alignment)
            pinned.append(raw)
            raw.copyMemory(from: bytes, byteCount: bytes.count)
            return UnsafePointer(raw.assumingMemoryBound(to: UInt8.self))
        }
        func pinText(_ text: String) -> Geist4Span {
            let bytes = Array(text.utf8)
            guard !bytes.isEmpty else { return Geist4Span(data: nil, len: 0) }
            return Geist4Span(data: pin(bytes, as: UInt8.self), len: bytes.count)
        }

        let nameSpan = pinText(host.name)
        let triggerBytes = host.triggers.flatMap { Array($0.utf8) }
        let triggers: UnsafePointer<UInt8>? = triggerBytes.isEmpty
            ? nil : pin(triggerBytes, as: UInt8.self)

        // Per kind: the declared attribute names as spans, then the
        // declaration itself.
        var attrArrays: [UnsafeMutablePointer<Geist4Span>?] = []
        var kindDecls: [Geist4KindDecl] = []
        for kind in host.kinds {
            let spans = kind.attributes.map { pinText($0) }
            if spans.isEmpty {
                attrArrays.append(nil)
            } else {
                let buffer = UnsafeMutablePointer<Geist4Span>.allocate(capacity: spans.count)
                pinned.append(UnsafeMutableRawPointer(buffer))
                for (index, span) in spans.enumerated() { buffer[index] = span }
                attrArrays.append(buffer)
            }
            kindDecls.append(Geist4KindDecl(
                kind: pinText(kind.name),
                container: kind.container ? 1 : 0,
                inline_text: kind.inlineText ? 1 : 0,
                attrs: attrArrays.last ?? nil,
                attrs_len: spans.count))
        }
        let kinds: UnsafeMutablePointer<Geist4KindDecl>? = kindDecls.isEmpty
            ? nil
            : UnsafeMutablePointer<Geist4KindDecl>.allocate(capacity: kindDecls.count)
        if let kinds {
            pinned.append(UnsafeMutableRawPointer(kinds))
            for (index, decl) in kindDecls.enumerated() { kinds[index] = decl }
        }

        var vtable = Geist4HostVTable(
            ctx: Unmanaged.passRetained(box).toOpaque(),
            free_ctx: { ctx in Unmanaged<HostBox>.fromOpaque(ctx!).release() },
            name: nameSpan,
            kinds: kinds, kinds_len: kindDecls.count,
            triggers: triggers, triggers_len: triggerBytes.count,
            init: box.hooks.initHook == nil ? nil : { ctx in hostInit(ctx) },
            block_start_look:
                box.hooks.blockStartLook == nil ? nil : { ctx, line, len, indented, column in
                    hostBlockStartLook(ctx, line, len, indented, column)
                },
            block_start:
                box.hooks.blockStart == nil
                    ? nil
                    : { ctx, line, len, indented, column, container, containerLen, out in
                        hostBlockStart(ctx, line, len, indented, column,
                                       container, containerLen, out)
                    },
            block_continue:
                box.hooks.blockContinue == nil
                    ? nil
                    : { ctx, line, len, indented, column, blank, out in
                        hostBlockContinue(ctx, line, len, indented, column, blank, out)
                    },
            block_close: box.hooks.blockClose == nil ? nil : { ctx, out in
                hostBlockClose(ctx, out)
            },
            inline_trigger:
                box.hooks.inlineTrigger == nil
                    ? nil
                    : { ctx, line, len, pos, out in
                        hostInlineTrigger(ctx, line, len, pos, out)
                    },
            inline_bracket:
                box.hooks.inlineBracket == nil
                    ? nil
                    : { ctx, content, len, out in
                        hostInlineBracket(ctx, content, len, out)
                    },
            post_process:
                box.hooks.postProcess == nil
                    ? nil
                    : { ctx, text, len, out in hostPostProcess(ctx, text, len, out) },
            render:
                box.hooks.render == nil
                    ? nil
                    : { ctx, kind, kindLen, attrs, attrsLen, children, childrenLen, out in
                        hostRender(ctx, kind, kindLen, attrs, attrsLen,
                                   children, childrenLen, out)
                    },
            render_override:
                box.hooks.renderOverride == nil
                    ? nil
                    : { ctx, kind, kindLen, attrs, attrsLen, count, fetch, fetchCtx, out in
                        hostRenderOverride(ctx, kind, kindLen, attrs, attrsLen,
                                           count, fetch, fetchCtx, out)
                    })

        do {
            try call { err in
                geist4_options_enable_host(handle, &vtable, err, errCapacity)
            }
        } catch {
            // The engine never took the box; free it here.
            Unmanaged.passRetained(box).release()
            for raw in pinned { raw.deallocate() }
            throw error
        }
        for raw in pinned { raw.deallocate() }
    }
}

// MARK: - Trampolines

/// The engine-side state of one registration: the Swift closures plus
/// the buffers the trampolines handed back on the previous hook call.
final class HostBox {
    let hooks: Geist4HostHooks
    private var pending: [UnsafeMutableRawPointer] = []

    init(hooks: Geist4HostHooks) { self.hooks = hooks }

    /// The previous hook call's buffers are dead: the engine copied
    /// every one of them before any other hook could run. Drop them.
    func clearPending() {
        for raw in pending { raw.deallocate() }
        pending.removeAll()
    }

    deinit { clearPending() }

    /// Copies `bytes` into arena memory and hands the pointer back.
    /// Empty input gives a null pointer; the span keeps its zero length.
    func copy(_ bytes: [UInt8]) -> UnsafePointer<UInt8>? {
        guard !bytes.isEmpty else { return nil }
        let raw = UnsafeMutableRawPointer.allocate(byteCount: bytes.count, alignment: 1)
        pending.append(raw)
        raw.copyMemory(from: bytes, byteCount: bytes.count)
        return UnsafePointer(raw.assumingMemoryBound(to: UInt8.self))
    }

    func copy(_ text: String) -> UnsafePointer<UInt8>? { copy(Array(text.utf8)) }

    /// Builds the ABI's attribute array for a decision, with every byte
    /// payload in the same arena.
    func attributes(_ list: [Geist4Attribute]) -> (ptr: UnsafePointer<Geist4Attr>?, len: Int) {
        guard !list.isEmpty else { return (nil, 0) }
        let raw = UnsafeMutableRawPointer.allocate(
            byteCount: list.count * MemoryLayout<Geist4Attr>.stride,
            alignment: MemoryLayout<Geist4Attr>.alignment)
        pending.append(raw)
        let buffer = raw.assumingMemoryBound(to: Geist4Attr.self)
        for (index, attribute) in list.enumerated() {
            let tag: Int32
            let number: Int64
            let textSpan: Geist4Span
            switch attribute.value {
            case .text(let value):
                tag = GEIST4_ATTR_STR
                number = 0
                let bytes = Array(value.utf8)
                textSpan = Geist4Span(data: copy(bytes), len: bytes.count)
            case .integer(let value):
                tag = GEIST4_ATTR_INT
                number = Int64(truncatingIfNeeded: value)
                textSpan = Geist4Span(data: nil, len: 0)
            case .boolean(let value):
                tag = GEIST4_ATTR_BOOL
                number = value ? 1 : 0
                textSpan = Geist4Span(data: nil, len: 0)
            }
            buffer[index] = Geist4Attr(
                name: Geist4Span(data: copy(attribute.name), len: attribute.name.utf8.count),
                tag: tag, number: number, text: textSpan)
        }
        return (ptr: UnsafePointer(buffer), len: list.count)
    }
}

/// The name bytes, or `""` when the pointer is null.
private func text(_ span: Geist4Span) -> String {
    guard let data = span.data, span.len > 0 else { return "" }
    return String(decoding: UnsafeRawBufferPointer(start: data, count: span.len),
                  as: UTF8.self)
}

/// The bytes as text, or `nil` for a null pointer or zero length.
private func text(_ data: UnsafePointer<UInt8>?, _ len: Int) -> String? {
    guard let data, len > 0 else { return nil }
    return String(decoding: UnsafeRawBufferPointer(start: data, count: len),
                  as: UTF8.self)
}

/// The attributes the ABI points at, translated to their Swift shape.
private func readAttrs(_ attrs: UnsafePointer<Geist4Attr>?, _ len: Int)
    -> [Geist4Attribute] {
    guard let attrs, len > 0 else { return [] }
    return (0..<len).map { index in
        let attribute = attrs[index]
        let name = text(attribute.name)
        switch attribute.tag {
        case GEIST4_ATTR_INT:
            return Geist4Attribute(name: name,
                                   value: .integer(Int(truncatingIfNeeded: attribute.number)))
        case GEIST4_ATTR_BOOL:
            return Geist4Attribute(name: name, value: .boolean(attribute.number != 0))
        default:
            return Geist4Attribute(name: name, value: .text(text(attribute.text)))
        }
    }
}

/// Fails the hook: the thrown error is lost (the C ABI has no message
/// channel), the engine latches its own message and declines.
private func fail(_ error: Error) -> Int32 {
    _ = error
    return -1
}

private func hostInit(_ ctx: UnsafeMutableRawPointer?) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.initHook else { return 0 }
    do { try hook() } catch { return fail(error) }
    return 0
}

private func hostBlockStartLook(
    _ ctx: UnsafeMutableRawPointer?,
    _ line: UnsafePointer<UInt8>?, _ lineLen: Int,
    _ indented: Int32, _ column: UInt32
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.blockStartLook,
          let text = text(line, lineLen) else { return 0 }
    do {
        return try hook(text, indented != 0, Int(column)) ? 1 : 0
    } catch { return fail(error) }
}

private func hostBlockStart(
    _ ctx: UnsafeMutableRawPointer?,
    _ line: UnsafePointer<UInt8>?, _ lineLen: Int,
    _ indented: Int32, _ column: UInt32,
    _ containerKind: UnsafePointer<UInt8>?, _ containerKindLen: Int,
    _ out: UnsafeMutablePointer<Geist4StartDecision>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.blockStart, let out,
          let body = text(line, lineLen) else { return 0 }
    let container = text(containerKind, containerKindLen)
    do {
        guard let decision = try hook(body, indented != 0, Int(column), container)
        else { return 0 }
        out.pointee = startDecision(decision, box)
        return 1
    } catch { return fail(error) }
}

/// Marshals a Swift start decision into the ABI's out parameter.
private func startDecision(
    _ decision: Geist4Start, _ box: HostBox
) -> Geist4StartDecision {
    let kindBytes = Array(decision.kind.utf8)
    let tag: Int32
    let columns: UInt32
    switch decision.action {
    case .wholeLine:
        tag = GEIST4_START_WHOLE_LINE
        columns = 0
    case .columns(let count):
        tag = GEIST4_START_COLUMNS
        columns = UInt32(truncatingIfNeeded: max(0, count))
    case .rest:
        tag = GEIST4_START_REST
        columns = 0
    case .claim(let count):
        tag = GEIST4_START_CLAIM
        columns = UInt32(truncatingIfNeeded: max(0, count))
    }
    let attrs = box.attributes(decision.attributes)
    return Geist4StartDecision(
        tag: tag,
        kind: Geist4Span(data: box.copy(kindBytes), len: kindBytes.count),
        attrs: attrs.ptr, attrs_len: attrs.len,
        columns: columns)
}

private func hostBlockContinue(
    _ ctx: UnsafeMutableRawPointer?,
    _ line: UnsafePointer<UInt8>?, _ lineLen: Int,
    _ indented: Int32, _ column: UInt32, _ blank: Int32,
    _ out: UnsafeMutablePointer<Geist4ContinueDecision>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.blockContinue, let out,
          let text = text(line, lineLen) else { return 0 }
    do {
        guard let decision = try hook(text, indented != 0, Int(column), blank != 0)
        else { return 0 }
        switch decision {
        case .keep(let columns):
            out.pointee = Geist4ContinueDecision(
                tag: GEIST4_CONTINUE_KEEP, columns: columns)
        case .closeLine:
            out.pointee = Geist4ContinueDecision(
                tag: GEIST4_CONTINUE_CLOSE_LINE, columns: 0)
        }
        return 1
    } catch { return fail(error) }
}

/// Marshals a rendering outcome into the ABI's out parameter: `nil`
/// means no HTML, a string means the HTML applies.
private func rendered(_ html: String?, _ box: HostBox) -> Geist4Rendered {
    guard let html else { return Geist4Rendered(tag: GEIST4_OUT_NONE, html: Geist4Span(data: nil, len: 0)) }
    let bytes = Array(html.utf8)
    return Geist4Rendered(tag: GEIST4_OUT_HTML,
                          html: Geist4Span(data: box.copy(bytes), len: bytes.count))
}

private func hostBlockClose(
    _ ctx: UnsafeMutableRawPointer?, _ out: UnsafeMutablePointer<Geist4Rendered>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.blockClose, let out else { return 0 }
    do {
        out.pointee = rendered(try hook(), box)
        return 1
    } catch { return fail(error) }
}

/// Marshals an inline decision into the ABI's out parameter.
private func inlineDecision(_ decision: Geist4Inline, _ box: HostBox)
    -> Geist4InlineDecision {
    switch decision {
    case .literal:
        return Geist4InlineDecision(
            tag: GEIST4_INLINE_LITERAL, kind: Geist4Span(data: nil, len: 0),
            end: 0, attrs: nil, attrs_len: 0)
    case .delimiter(let kind):
        let bytes = Array(kind.utf8)
        return Geist4InlineDecision(
            tag: GEIST4_INLINE_DELIMITER,
            kind: Geist4Span(data: box.copy(bytes), len: bytes.count),
            end: 0, attrs: nil, attrs_len: 0)
    case .node(let kind, let end, let attributes):
        let bytes = Array(kind.utf8)
        let attrs = box.attributes(attributes)
        return Geist4InlineDecision(
            tag: GEIST4_INLINE_NODE,
            kind: Geist4Span(data: box.copy(bytes), len: bytes.count),
            end: end, attrs: attrs.ptr, attrs_len: attrs.len)
    }
}

private func hostInlineTrigger(
    _ ctx: UnsafeMutableRawPointer?,
    _ line: UnsafePointer<UInt8>?, _ lineLen: Int, _ pos: UInt32,
    _ out: UnsafeMutablePointer<Geist4InlineDecision>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.inlineTrigger, let out,
          let text = text(line, lineLen) else { return 0 }
    do {
        guard let decision = try hook(text, Int(pos)) else { return 0 }
        out.pointee = inlineDecision(decision, box)
        return 1
    } catch { return fail(error) }
}

private func hostInlineBracket(
    _ ctx: UnsafeMutableRawPointer?,
    _ content: UnsafePointer<UInt8>?, _ contentLen: Int,
    _ out: UnsafeMutablePointer<Geist4InlineDecision>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.inlineBracket, let out,
          let text = text(content, contentLen) else { return 0 }
    do {
        guard let decision = try hook(text) else { return 0 }
        out.pointee = inlineDecision(decision, box)
        return 1
    } catch { return fail(error) }
}

private func hostPostProcess(
    _ ctx: UnsafeMutableRawPointer?,
    _ original: UnsafePointer<UInt8>?, _ originalLen: Int,
    _ out: UnsafeMutablePointer<Geist4Span>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.postProcess, let out,
          let text = text(original, originalLen) else { return 0 }
    do {
        guard let replacement = try hook(text) else { return 0 }
        let bytes = Array(replacement.utf8)
        out.pointee = Geist4Span(data: box.copy(bytes), len: bytes.count)
        return 1
    } catch { return fail(error) }
}

private func hostRender(
    _ ctx: UnsafeMutableRawPointer?,
    _ kind: UnsafePointer<UInt8>?, _ kindLen: Int,
    _ attrs: UnsafePointer<Geist4Attr>?, _ attrsLen: Int,
    _ children: UnsafePointer<UInt8>?, _ childrenLen: Int,
    _ out: UnsafeMutablePointer<Geist4Rendered>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.render, let out,
          let kind = text(kind, kindLen),
          let children = text(children, childrenLen) else { return 0 }
    do {
        out.pointee = rendered(
            try hook(kind, readAttrs(attrs, attrsLen), children), box)
        return 1
    } catch { return fail(error) }
}

private func hostRenderOverride(
    _ ctx: UnsafeMutableRawPointer?,
    _ kind: UnsafePointer<UInt8>?, _ kindLen: Int,
    _ attrs: UnsafePointer<Geist4Attr>?, _ attrsLen: Int,
    _ childCount: UInt32, _ fetch: Geist4ChildFetch?, _ fetchCtx: UnsafeMutableRawPointer?,
    _ out: UnsafeMutablePointer<Geist4Rendered>?
) -> Int32 {
    let box = Unmanaged<HostBox>.fromOpaque(ctx!).takeUnretainedValue()
    box.clearPending()
    guard let hook = box.hooks.renderOverride, let out,
          let kind = text(kind, kindLen) else { return 0 }
    let child: (Int) -> String? = { index in
        guard let fetch, index >= 0, index < Int(childCount) else { return nil }
        var span = Geist4Span(data: nil, len: 0)
        guard fetch(fetchCtx, UInt32(truncatingIfNeeded: index), &span) >= 0 else {
            return nil
        }
        return text(span)
    }
    do {
        out.pointee = rendered(
            try hook(kind, readAttrs(attrs, attrsLen), Int(childCount), child), box)
        return 1
    } catch { return fail(error) }
}
