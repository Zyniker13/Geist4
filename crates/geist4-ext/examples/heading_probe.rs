//! Prints heading-identifier output for a handful of sources, for
//! eyeballing against the reference pipeline's rendering.
use geist4_core::basic_html::render_with;
use geist4_core::extension::Options;
use geist4_core::parser::parse_with;

fn main() {
    let mut options = Options::new();
    options.extension(geist4_ext::heading_identifiers());
    for source in [
        "# Hello\n",
        "## ##\n",
        "# A `great` worry\n",
        "# a\n\n# a\n\n# a a\n",
        "Setext\n=======\n",
        "> # Quoted\n",
        "# <https://example.com>\n",
        "# a ![alt](/img.png) b\n",
        "# <!-- a > b --> title\n",
        "# &lt;tag&gt;\n",
        "# ΟΔΟΣ\n",
        "# 1. Some text & 2. Some code\n",
        "text\n\n# Tail\n",
    ] {
        let doc = parse_with(source, &options);
        println!("--- {source:?}\n{}", render_with(&doc, &options));
    }
}
