//! Renders stdin with only the footnotes extension enabled, for
//! generating and checking the footnotes spec file.
fn main() {
    use geist4_core::extension::Options;
    use std::io::Read;
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).expect("stdin");
    let mut options = Options::new();
    options.extension(geist4_ext::footnotes());
    let doc = geist4_core::parser::parse_with(&source, &options);
    print!("{}", geist4_core::basic_html::render_with(&doc, &options));
}
