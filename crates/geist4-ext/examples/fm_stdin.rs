//! Renders stdin with front matter (and the other built-ins) enabled,
//! for the differential battery against the reference implementation.
//! An optional first argument overrides the front matter delimiter
//! (`---` by default).
fn main() {
    use geist4_core::extension::Options;
    use std::io::Read;
    let delimiter = std::env::args().nth(1).unwrap_or_else(|| "---".to_owned());
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).expect("stdin");
    let mut options = Options::new();
    options.extension(geist4_ext::front_matter_with(delimiter));
    options.extension(geist4_ext::tables());
    options.extension(geist4_ext::strikethrough());
    options.extension(geist4_ext::autolink());
    options.extension(geist4_ext::tasklist());
    options.extension(geist4_ext::tagfilter());
    options.extension(geist4_ext::footnotes());
    let doc = geist4_core::parser::parse_with(&source, &options);
    print!("{}", geist4_core::basic_html::render_with(&doc, &options));
}
