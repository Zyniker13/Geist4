/* @ts-self-types="./geist4.d.ts" */

/**
 * A parsed document: parse once, render or inspect many times.
 *
 * The document owns everything it borrows; dropping it (letting the
 * JavaScript reference go) frees the parse. The options used at parse
 * time are remembered, so rendering needs no other argument and the
 * options object may be dropped first.
 */
export class Document {
    static __wrap(ptr) {
        const obj = Object.create(Document.prototype);
        obj.__wbg_ptr = ptr;
        DocumentFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        DocumentFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_document_free(ptr, 0);
    }
    /**
     * The document's HTML, byte for byte as the Rust core renders it.
     *
     * # Errors
     * Only an engine bug would throw here.
     * @returns {string}
     */
    html() {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.document_html(this.__wbg_ptr);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Parses `source` with the given options.
     *
     * # Errors
     * Only an engine bug would throw here; valid input parses.
     * @param {string} source
     * @param {Options} options
     * @returns {Document}
     */
    static parseWith(source, options) {
        _assertClass(options, Options);
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.document_parseWith(ptr0, len0, options.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return Document.__wrap(ret[0]);
    }
    /**
     * Parses `source` as plain CommonMark. The `parse_with` form takes
     * an options object with built-ins enabled; the two produce
     * identical documents when no extension is registered.
     *
     * # Errors
     * Only an engine bug would throw here; valid input parses.
     * @param {string} source
     * @returns {Document}
     */
    static parse(source) {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.document_parse(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return Document.__wrap(ret[0]);
    }
    /**
     * The problems the parse noticed: broken references and similar
     * issues the tree absorbed without changing the output, in
     * document order.
     * @returns {Problem[]}
     */
    problems() {
        const ret = wasm.document_problems(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
}
if (Symbol.dispose) Document.prototype[Symbol.dispose] = Document.prototype.free;

/**
 * A host extension: JavaScript callbacks the engine calls at its
 * extension points, the tier-3 counterpart of a built-in. Register
 * it on `Options` with `enableHost`.
 *
 * The config is a plain object: `{ name, kinds?, triggers?, hooks? }`.
 * `kinds` maps a kind name to a shape — a mapping with optional
 * `attrs` (attribute names), `container` and `inline_text` flags, or
 * just a list of attribute names (an inline kind) — and `triggers`
 * is a list of one-character strings. Both are validated when the
 * extension is enabled, as a manifest would be.
 *
 * `hooks` is an object of optional functions: `init()`,
 * `blockStartLook(line, indented, column)`, `blockStart(line,
 * indented, column, container)`, `blockContinue(line, indented,
 * column, blank)`, `blockClose()`, `inlineTrigger(text, column)`,
 * `inlineBracket(text)`, `postProcess(text)`, `render(kind, attrs,
 * children)` and `renderOverride(kind, attrs, children)`. An absent
 * hook declines; a hook that throws surfaces as a parse problem.
 */
export class HostExtension {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        HostExtensionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_hostextension_free(ptr, 0);
    }
    /**
     * The extension's name.
     * @returns {string}
     */
    get name() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.hostextension_name(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Builds the extension from its config object.
     * @param {any} config
     */
    constructor(config) {
        const ret = wasm.hostextension_new(config);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        HostExtensionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) HostExtension.prototype[Symbol.dispose] = HostExtension.prototype.free;

/**
 * The parse options: which built-in extensions are registered.
 *
 * `new` is plain CommonMark; `enable_builtin` registers built-ins by
 * name — `tables`, `strikethrough`, `autolink`, `tasklist`,
 * `tagfilter`, `footnotes`, `front_matter`, `heading_identifiers`,
 * `description_lists`, or the bundles `gfm` and `all`. An unknown name
 * throws with the valid names listed; a name registered twice does
 * nothing, so bundles and single names compose freely.
 */
export class Options {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        OptionsFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_options_free(ptr, 0);
    }
    /**
     * Enables built-in syntax highlighting for fenced code blocks in
     * HTML output. Off by default.
     */
    enableHighlighting() {
        wasm.options_enableHighlighting(this.__wbg_ptr);
    }
    /**
     * Registers a host extension: JavaScript callbacks at the engine's
     * hook points, the same vocabulary the tier-2 adapter exposes.
     *
     * The extension's declared data is validated exactly like a
     * tier-2 manifest's, and its name is checked against the
     * built-ins' and every earlier extension's.
     *
     * # Errors
     * Invalid declared data — a reserved name, a malformed kind, a
     * trigger that is not one byte — or a name already registered.
     * @param {HostExtension} extension
     */
    enableHost(extension) {
        _assertClass(extension, HostExtension);
        const ret = wasm.options_enableHost(this.__wbg_ptr, extension.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Registers a built-in extension by name.
     *
     * # Errors
     * An unknown name, with the valid names in the message.
     * @param {string} name
     */
    enable_builtin(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.options_enable_builtin(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Plain CommonMark, no extensions registered.
     */
    constructor() {
        const ret = wasm.options_new();
        this.__wbg_ptr = ret;
        OptionsFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
}
if (Symbol.dispose) Options.prototype[Symbol.dispose] = Options.prototype.free;

/**
 * One problem the parse noticed, with its span into the source.
 *
 * Spans are byte offsets into the UTF-8 encoding of the source, the
 * same convention the core uses; JavaScript callers indexing the
 * source as UTF-16 should convert with `TextEncoder`.
 */
export class Problem {
    static __wrap(ptr) {
        const obj = Object.create(Problem.prototype);
        obj.__wbg_ptr = ptr;
        ProblemFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ProblemFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_problem_free(ptr, 0);
    }
    /**
     * The span's end, exclusive, as a byte offset into the source.
     * @returns {number}
     */
    get end() {
        const ret = wasm.problem_end(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * What the problem is, as one line.
     * @returns {string}
     */
    get message() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.problem_message(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * The span's start, as a byte offset into the source.
     * @returns {number}
     */
    get start() {
        const ret = wasm.problem_start(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) Problem.prototype[Symbol.dispose] = Problem.prototype.free;

/**
 * Parses and renders in one call, as plain CommonMark.
 *
 * The convenient path for callers who only want HTML. The `renderHtml`
 * form of the same call takes an options object.
 *
 * # Errors
 * Only an engine bug would throw here.
 * @param {string} source
 * @returns {string}
 */
export function renderHtml(source) {
    let deferred3_0;
    let deferred3_1;
    try {
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.renderHtml(ptr0, len0);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * Parses and renders in one call with the given options.
 *
 * # Errors
 * Only an engine bug would throw here.
 * @param {string} source
 * @param {Options} options
 * @returns {string}
 */
export function renderHtmlWith(source, options) {
    let deferred3_0;
    let deferred3_1;
    try {
        _assertClass(options, Options);
        const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.renderHtmlWith(ptr0, len0, options.__wbg_ptr);
        var ptr2 = ret[0];
        var len2 = ret[1];
        if (ret[3]) {
            ptr2 = 0; len2 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred3_0 = ptr2;
        deferred3_1 = len2;
        return getStringFromWasm0(ptr2, len2);
    } finally {
        wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
    }
}

/**
 * The engine version, as in the C header's `geist4_version`.
 * @returns {string}
 */
export function version() {
    let deferred1_0;
    let deferred1_1;
    try {
        const ret = wasm.version();
        deferred1_0 = ret[0];
        deferred1_1 = ret[1];
        return getStringFromWasm0(ret[0], ret[1]);
    } finally {
        wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
}
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_208446807d0d1e30: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg___wbindgen_boolean_get_4f1baf9a00be3c74: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_f249a2a76caa9a55: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_741ff2bb612836f5: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_c22311b1cc228e3a: function(arg0) {
            const ret = arg0 === null;
            return ret;
        },
        __wbg___wbindgen_is_object_8b968c3ab8e96595: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_undefined_be5b34249ff1fc15: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_number_get_934b640d0889af78: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64((arg0 >>> 0) + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_7ce8eee7fd327545: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_call_23089ef18de8d8c3: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            const ret = arg0.call(arg1, arg2, arg3, arg4, arg5);
            return ret;
        }, arguments); },
        __wbg_call_275b78b3b6520b92: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            const ret = arg0.call(arg1, arg2, arg3, arg4);
            return ret;
        }, arguments); },
        __wbg_call_40b008ba32f73991: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_call_a410eb4b59ed5965: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_call_fc92fee6137d452d: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.call(arg1, arg2, arg3);
            return ret;
        }, arguments); },
        __wbg_entries_7bb1459bd9b5c2b9: function(arg0) {
            const ret = Object.entries(arg0);
            return ret;
        },
        __wbg_get_2080c49015e35254: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_d7176cfb76d71114: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_get_unchecked_e6325ad374eff861: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_instanceof_Array_ff00140877e0a911: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Error_74e3cca0c4a69161: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Error;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_isArray_bcf1376d0d732d08: function(arg0) {
            const ret = Array.isArray(arg0);
            return ret;
        },
        __wbg_length_b03a0fabb109b1e9: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_new_37eb45aaf5eeb820: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_cd4348290afae586: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_problem_new: function(arg0) {
            const ret = Problem.__wrap(arg0);
            return ret;
        },
        __wbg_push_c9f4893fc209f768: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_set_2793479888329cb6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_toString_4d043badaf1f4f69: function(arg0) {
            const ret = arg0.toString();
            return ret;
        },
        __wbindgen_generic_0000000000000001: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./geist4_bg.js": import0,
    };
}

const DocumentFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_document_free(ptr, 1));
const HostExtensionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_hostextension_free(ptr, 1));
const OptionsFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_options_free(ptr, 1));
const ProblemFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_problem_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
}

function debugString(val) {
    switch (typeof val) {
        // primitive types
        case 'number':
        case 'boolean':
        case 'undefined':
        return `${val}`;
        case 'string':
        return JSON.stringify(val);
        case 'bigint':
        return `${val}n`;
        case 'symbol':
        return val.description !== undefined
            ? `Symbol(${val.description})`
            : 'Symbol';
        case 'function':
        return typeof val.name === 'string' && val.name !== ''
            ? `Function(${val.name})`
            : 'Function';
        case 'object': {
            if (val === null) {
                return 'null';
            }
            // objects
            if (Array.isArray(val)) {
                return '[' + Array.from(val, debugString).join(', ') + ']';
            }
            // errors
            if (val instanceof Error) {
                return `${val.name}: ${val.message}\n${val.stack}`;
            }

            const toStringOutput = toString.call(val);
            // Test for built-in
            const className = /\[object ([^\]]+)\]/.exec(toStringOutput)?.[1];

            // Failed to match the standard '[object ClassName]'
            if (className === undefined) {
                return toStringOutput;
            }
            if (className === 'Object') {
                // we're a user defined class or Object
                // JSON.stringify avoids problems with cycles, and is generally much
                // easier than looping through ownProperties of `val`.
                try {
                    return 'Object(' + JSON.stringify(val) + ')';
                } catch (_) {
                    return 'Object';
                }
            }
            // TODO we could test for more things here, like `Set`s and `Map`s.
            return className;
        }
        // Per MDN: 'This list of values is exhaustive.'
        // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/typeof#description
    }
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('geist4_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
