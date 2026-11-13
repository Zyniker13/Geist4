//! Renders stdin with front matter alone, for generating the
//! reference-verified spec file (the spec harness runs one extension
//! at a time). An optional first argument overrides the delimiter
//! (`---` by default).
fn main() {
    use geist4_core::extension::Options;
    use std::io::Read;
    let delimiter = std::env::args().nth(1).unwrap_or_else(|| "---".to_owned());
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).expect("stdin");
    let mut options = Options::new();
    options.extension(geist4_ext::front_matter_with(delimiter));
    let doc = geist4_core::parser::parse_with(&source, &options);
    print!("{}", geist4_core::basic_html::render_with(&doc, &options));
}
