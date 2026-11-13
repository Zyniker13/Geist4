//! Renders stdin with all built-in extensions enabled, for the
//! differential battery against the reference implementation.
fn main() {
    use geist4_core::extension::Options;
    use std::io::Read;
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).expect("stdin");
    let mut options = Options::new();
    options.extension(geist4_ext::tables());
    options.extension(geist4_ext::strikethrough());
    options.extension(geist4_ext::autolink());
    options.extension(geist4_ext::tasklist());
    options.extension(geist4_ext::tagfilter());
    let doc = geist4_core::parser::parse_with(&source, &options);
    print!("{}", geist4_core::basic_html::render_with(&doc, &options));
}
