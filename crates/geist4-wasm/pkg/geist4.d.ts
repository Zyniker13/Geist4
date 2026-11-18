/* tslint:disable */
/* eslint-disable */

/**
 * A parsed document: parse once, render or inspect many times.
 *
 * The document owns everything it borrows; dropping it (letting the
 * JavaScript reference go) frees the parse. The options used at parse
 * time are remembered, so rendering needs no other argument and the
 * options object may be dropped first.
 */
export class Document {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The document's HTML, byte for byte as the Rust core renders it.
     *
     * # Errors
     * Only an engine bug would throw here.
     */
    html(): string;
    /**
     * Parses `source` with the given options.
     *
     * # Errors
     * Only an engine bug would throw here; valid input parses.
     */
    static parseWith(source: string, options: Options): Document;
    /**
     * Parses `source` as plain CommonMark. The `parse_with` form takes
     * an options object with built-ins enabled; the two produce
     * identical documents when no extension is registered.
     *
     * # Errors
     * Only an engine bug would throw here; valid input parses.
     */
    static parse(source: string): Document;
    /**
     * The problems the parse noticed: broken references and similar
     * issues the tree absorbed without changing the output, in
     * document order.
     */
    problems(): Problem[];
}

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
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Registers a built-in extension by name.
     *
     * # Errors
     * An unknown name, with the valid names in the message.
     */
    enable_builtin(name: string): void;
    /**
     * Plain CommonMark, no extensions registered.
     */
    constructor();
}

/**
 * One problem the parse noticed, with its span into the source.
 *
 * Spans are byte offsets into the UTF-8 encoding of the source, the
 * same convention the core uses; JavaScript callers indexing the
 * source as UTF-16 should convert with `TextEncoder`.
 */
export class Problem {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The span's end, exclusive, as a byte offset into the source.
     */
    readonly end: number;
    /**
     * What the problem is, as one line.
     */
    readonly message: string;
    /**
     * The span's start, as a byte offset into the source.
     */
    readonly start: number;
}

/**
 * Parses and renders in one call, as plain CommonMark.
 *
 * The convenient path for callers who only want HTML. The `renderHtml`
 * form of the same call takes an options object.
 *
 * # Errors
 * Only an engine bug would throw here.
 */
export function renderHtml(source: string): string;

/**
 * Parses and renders in one call with the given options.
 *
 * # Errors
 * Only an engine bug would throw here.
 */
export function renderHtmlWith(source: string, options: Options): string;

/**
 * The engine version, as in the C header's `geist4_version`.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_document_free: (a: number, b: number) => void;
    readonly __wbg_options_free: (a: number, b: number) => void;
    readonly __wbg_problem_free: (a: number, b: number) => void;
    readonly document_html: (a: number) => [number, number, number, number];
    readonly document_parse: (a: number, b: number) => [number, number, number];
    readonly document_parseWith: (a: number, b: number, c: number) => [number, number, number];
    readonly document_problems: (a: number) => [number, number];
    readonly options_enable_builtin: (a: number, b: number, c: number) => [number, number];
    readonly options_new: () => number;
    readonly problem_end: (a: number) => number;
    readonly problem_message: (a: number) => [number, number];
    readonly problem_start: (a: number) => number;
    readonly renderHtml: (a: number, b: number) => [number, number, number, number];
    readonly renderHtmlWith: (a: number, b: number, c: number) => [number, number, number, number];
    readonly version: () => [number, number];
    readonly geist4_document_free: (a: number) => void;
    readonly geist4_document_html: (a: number, b: number, c: number, d: number) => number;
    readonly geist4_document_problems: (a: number, b: number, c: number, d: number) => number;
    readonly geist4_options_create: () => number;
    readonly geist4_options_enable_builtin: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly geist4_options_free: (a: number) => void;
    readonly geist4_parse: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly geist4_render_html: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly geist4_string_free: (a: number) => void;
    readonly geist4_version: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
