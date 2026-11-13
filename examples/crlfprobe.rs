fn main() {
    let src = std::fs::read_to_string("/tmp/crlf.md").unwrap();
    print!("{}", geist4_core::basic_html::render(&geist4_core::parser::parse_document(&src)));
}
