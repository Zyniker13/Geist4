//! A differential probe: reads a document on stdin, parses and renders
//! it with the tables and strikethrough extensions, and prints the
//! HTML for comparison against the reference binary.
use std::io::Read;

fn main() {
    let mut source = String::new();
    std::io::stdin()
        .read_to_string(&mut source)
        .expect("read stdin");
    let mut options = geist4_core::extension::Options::new();
    options.extension(geist4_ext::tables::Tables::extension());
    options.extension(geist4_ext::strikethrough());
    let document = geist4_core::parser::parse_with(&source, &options);
    print!(
        "{}",
        geist4_core::basic_html::render_with(&document, &options)
    );
}
