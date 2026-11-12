//! Parse and run test suites written in the CommonMark spec example format.
//!
//! The format is the one the CommonMark specification itself uses: prose with
//! ATX headings as sections, and test examples delimited by fences of exactly
//! thirty-two backticks. Inside an example, a line holding a single `.` splits
//! the Markdown input from the expected output, and the arrow character `→`
//! stands for a tab, so tabs stay visible in the source.
//!
//! ````````````````````````````````````text
//! # A section
//!
//! ```````````````````````````````` example
//! *hello* world
//! .
//! <p><em>hello</em> world</p>
//! ````````````````````````````````
//! ````````````````````````````````````
//!
//! The harness is a small, dependency-free library, so it can be used from
//! the engine's own crates, from built-in and third-party extensions, and
//! from outside projects: parse a spec file, run its examples through a
//! render function, and get a report of what passed, what failed, and where.
//!
//! # Example
//!
//! ```
//! use geist4_spec_harness::{parse, run};
//!
//! let fence = "`".repeat(32);
//! let spec = format!(
//!     "# Runs\n\n{fence} example\n*x*\n.\n<p><em>x</em></p>\n{fence}\n"
//! );
//!
//! let examples = parse(&spec).unwrap();
//! let report = run(&examples, &mut |_example| String::from("<p>no</p>"));
//! assert_eq!(report.total(), 1);
//! assert_eq!(report.passed(), 0);
//! assert_eq!(report.failed(), 1);
//! assert_eq!(report.failures()[0].number, 1);
//! ```

use std::fmt;
use std::fmt::Write as _;

/// The example fence used by the format: a line of exactly thirty-two backticks.
const FENCE: &str = "````````````````````````````````";

/// One test example from a spec file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    /// The example's position in its file, counting from one in file order.
    /// This is the numbering the CommonMark project itself uses when it
    /// reports test results.
    pub number: usize,
    /// The text of the heading last seen before the example's opening fence.
    pub section: String,
    /// The Markdown input, with tab arrows expanded to real tabs.
    pub markdown: String,
    /// The expected output, with tab arrows expanded to real tabs.
    pub html: String,
    /// The extra words after the word `example` on the opening fence line,
    /// for render functions that support options (for example `toc`).
    pub options: Vec<String>,
}

/// Why a spec file could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The one-based line in the input where the problem starts.
    pub line: usize,
    /// What the problem is.
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses a spec file and returns its examples in file order.
///
/// Sections come from ATX heading lines (lines starting with `#`) outside
/// examples: an example keeps the heading text last seen before its opening
/// fence. Inside examples, tab arrows are expanded to real tabs in both the
/// input and the expected output.
///
/// # Errors
///
/// Returns a [`ParseError`] when an example fence is never closed, or when an
/// example has no `.` separator line between input and expected output.
pub fn parse(input: &str) -> Result<Vec<Example>, ParseError> {
    let mut examples = Vec::new();
    let mut section = String::new();
    let mut number = 0usize;
    let mut lines = input.lines().enumerate();

    while let Some((index, line)) = lines.next() {
        if line.starts_with('#') {
            section = heading_text(line);
            continue;
        }
        let Some(rest) = line.strip_prefix(FENCE) else {
            continue;
        };
        let mut words = rest.split_whitespace();
        if words.next() != Some("example") {
            continue;
        }
        let options: Vec<String> = words.map(str::to_string).collect();
        number += 1;
        let open_line = index + 1;

        let mut content: Vec<&str> = Vec::new();
        let mut closed = false;
        for (_, content_line) in lines.by_ref() {
            if content_line.trim_end() == FENCE {
                closed = true;
                break;
            }
            content.push(content_line);
        }
        if !closed {
            return Err(ParseError {
                line: open_line,
                message: format!("example {number} is not closed"),
            });
        }

        let Some(separator) = content
            .iter()
            .position(|content_line| content_line.trim_end() == ".")
        else {
            return Err(ParseError {
                line: open_line,
                message: format!("example {number} has no '.' separator line"),
            });
        };

        let markdown = expand_tabs(&content[..separator].join("\n"));
        let html = expand_tabs(&content[separator + 1..].join("\n"));
        examples.push(Example {
            number,
            section: section.clone(),
            markdown,
            html,
            options,
        });
    }

    Ok(examples)
}

/// The text of an ATX heading line: leading hashes and padding removed.
fn heading_text(line: &str) -> String {
    line.trim_start_matches('#').trim().to_string()
}

/// Expands the format's tab arrows to real tabs.
fn expand_tabs(text: &str) -> String {
    text.replace('→', "\t")
}

/// One failing example: the input, the expected output, and what the render
/// function produced instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The example's number in its file, counting from one.
    pub number: usize,
    /// The section the example is in.
    pub section: String,
    /// The Markdown input.
    pub markdown: String,
    /// The expected output.
    pub expected: String,
    /// What the render function produced.
    pub actual: String,
}

/// The failing examples of one section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionFailures<'a> {
    /// The section's heading text.
    pub section: &'a str,
    /// The example numbers that failed, in file order.
    pub numbers: Vec<usize>,
}

/// The outcome of running a set of examples through a render function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    total: usize,
    passed: usize,
    failures: Vec<Failure>,
}

impl Report {
    /// How many examples were run.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// How many examples produced exactly the expected output.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.passed
    }

    /// How many examples failed.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.total - self.passed
    }

    /// The failures, in file order.
    #[must_use]
    pub fn failures(&self) -> &[Failure] {
        &self.failures
    }

    /// The failures grouped by section, in the order the sections first
    /// failed.
    #[must_use]
    pub fn failing_sections(&self) -> Vec<SectionFailures<'_>> {
        let mut sections: Vec<(&str, Vec<usize>)> = Vec::new();
        for failure in &self.failures {
            match sections
                .iter_mut()
                .find(|(name, _)| *name == failure.section)
            {
                Some((_, numbers)) => numbers.push(failure.number),
                None => sections.push((&failure.section, vec![failure.number])),
            }
        }
        sections
            .into_iter()
            .map(|(section, numbers)| SectionFailures { section, numbers })
            .collect()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "passed {} of {}", self.passed, self.total)?;
        for SectionFailures { section, numbers } in self.failing_sections() {
            writeln!(f)?;
            write!(
                f,
                "  {}: {} failing (examples {})",
                section,
                numbers.len(),
                compress_numbers(&numbers)
            )?;
        }
        Ok(())
    }
}

/// Runs every example through `render` and compares each output with the
/// example's expected output, byte for byte.
///
/// The render function receives the whole [`Example`], so suites that use
/// fence options can switch behavior on `example.options`.
pub fn run<F>(examples: &[Example], render: &mut F) -> Report
where
    F: FnMut(&Example) -> String,
{
    let mut passed = 0usize;
    let mut failures = Vec::new();
    for example in examples {
        let actual = render(example);
        if actual == example.html {
            passed += 1;
        } else {
            failures.push(Failure {
                number: example.number,
                section: example.section.clone(),
                markdown: example.markdown.clone(),
                expected: example.html.clone(),
                actual,
            });
        }
    }
    Report {
        total: examples.len(),
        passed,
        failures,
    }
}

/// Formats example numbers compactly: consecutive numbers become ranges, as
/// in `1-3, 7, 9-12`. The numbers must be in ascending order, which the
/// runner produces.
fn compress_numbers(numbers: &[usize]) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < numbers.len() {
        let start = numbers[index];
        let mut end = start;
        while index + 1 < numbers.len() && numbers[index + 1] == end + 1 {
            index += 1;
            end += 1;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        if end == start {
            let _ = write!(out, "{start}");
        } else {
            let _ = write!(out, "{start}-{end}");
        }
        index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a fence line of exactly `n` backticks, plus `suffix`.
    fn fence(n: usize, suffix: &str) -> String {
        format!("{}{suffix}", "`".repeat(n))
    }

    #[test]
    fn the_example_fence_is_thirty_two_backticks() {
        assert_eq!(FENCE.len(), 32, "the spec example fence is 32 backticks");
    }

    #[test]
    fn parse_reads_sections_numbers_and_options() {
        let open_plain = fence(32, " example");
        let open_option = fence(32, " example toc");
        let close = fence(32, "");
        let spec = format!(
            "# Outer\n\n{open_plain}\n*one*\n.\n<p>one</p>\n{close}\n\
             ## Inner\n\n{open_option}\na→b\n.\n<p>a→b</p>\n{close}\n"
        );

        let examples = parse(&spec).expect("parses");
        assert_eq!(examples.len(), 2);

        assert_eq!(examples[0].number, 1);
        assert_eq!(examples[0].section, "Outer");
        assert_eq!(examples[0].markdown, "*one*");
        assert_eq!(examples[0].html, "<p>one</p>");
        assert!(examples[0].options.is_empty());

        assert_eq!(examples[1].number, 2);
        assert_eq!(examples[1].section, "Inner");
        assert_eq!(examples[1].options, vec!["toc".to_string()]);
        assert_eq!(
            examples[1].markdown, "a\tb",
            "tab arrows expand in the input"
        );
        assert_eq!(
            examples[1].html, "<p>a\tb</p>",
            "tab arrows expand in the output"
        );
    }

    #[test]
    fn parse_errors_on_a_missing_separator() {
        let open = fence(32, " example");
        let close = fence(32, "");
        let spec = format!("# S\n\n{open}\nno separator here\n{close}\n");

        let error = parse(&spec).expect_err("missing separator is an error");
        assert_eq!(error.line, 3, "the error points at the opening fence");
        assert!(error.message.contains("separator"), "{}", error.message);
    }

    #[test]
    fn parse_errors_on_an_unclosed_example() {
        let open = fence(32, " example");
        let spec = format!("# S\n\n{open}\nnever closed\n");

        let error = parse(&spec).expect_err("an unclosed example is an error");
        assert_eq!(error.line, 3);
        assert!(error.message.contains("closed"), "{}", error.message);
    }

    #[test]
    fn ordinary_fenced_code_is_not_an_example() {
        let code_fence = fence(3, "rust");
        let close = fence(3, "");
        let spec = format!("# S\n\n{code_fence}\nfn main() {{}}\n{close}\n");

        let examples = parse(&spec).expect("parses");
        assert!(examples.is_empty(), "shorter fences are not examples");
    }

    #[test]
    fn run_reports_passes_failures_and_sections() {
        let open = fence(32, " example");
        let close = fence(32, "");
        let spec = format!(
            "# A\n\n{open}\na\n.\n<p>a</p>\n{close}\n\n\
             {open}\nb\n.\n<p>b</p>\n{close}\n\n\
             # B\n\n{open}\nc\n.\n<p>c</p>\n{close}\n\n\
             {open}\nd\n.\n<p>d</p>\n{close}\n\n\
             {open}\ne\n.\n<p>e</p>\n{close}\n"
        );
        let examples = parse(&spec).expect("parses");

        // Fail on b, d and e; pass a and c.
        let mut render = |example: &Example| {
            if example.number == 2 || example.number >= 4 {
                String::from("<p>wrong</p>")
            } else {
                example.html.clone()
            }
        };
        let report = run(&examples, &mut render);

        assert_eq!(report.total(), 5);
        assert_eq!(report.passed(), 2);
        assert_eq!(report.failed(), 3);
        assert_eq!(
            report
                .failures()
                .iter()
                .map(|failure| failure.number)
                .collect::<Vec<_>>(),
            vec![2, 4, 5]
        );

        let sections = report.failing_sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].section, "A");
        assert_eq!(sections[0].numbers, vec![2]);
        assert_eq!(sections[1].section, "B");
        assert_eq!(sections[1].numbers, vec![4, 5]);

        let text = report.to_string();
        assert!(text.contains("passed 2 of 5"), "{text}");
        assert!(text.contains("A: 1 failing (examples 2)"), "{text}");
        assert!(text.contains("B: 2 failing (examples 4-5)"), "{text}");
    }

    #[test]
    fn failures_carry_the_details_a_developer_needs() {
        let open = fence(32, " example");
        let close = fence(32, "");
        let spec = format!("{open}\n*hi*\n.\n<p>expected</p>\n{close}\n");
        let examples = parse(&spec).expect("parses");

        let mut render = |_example: &Example| String::from("<p>actual</p>");
        let report = run(&examples, &mut render);

        let failure = &report.failures()[0];
        assert_eq!(failure.number, 1);
        assert_eq!(failure.markdown, "*hi*");
        assert_eq!(failure.expected, "<p>expected</p>");
        assert_eq!(failure.actual, "<p>actual</p>");
    }
}
