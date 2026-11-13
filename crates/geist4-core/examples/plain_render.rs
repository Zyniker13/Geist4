//! Renders stdin with plain CommonMark (no extensions), for probing
//! against the reference implementations without extension behavior.
fn main() {
    use std::io::Read;
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source).expect("stdin");
    let options = geist4_core::extension::Options::new();
    let doc = geist4_core::parser::parse_with(&source, &options);
    print!("{}", geist4_core::basic_html::render_with(&doc, &options));
}
