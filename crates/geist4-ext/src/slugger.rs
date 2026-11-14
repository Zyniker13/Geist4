//! An exact Rust port of github-slugger 2.0.0 (MIT,
//! <https://github.com/Flet/github-slugger>), the library that generates
//! GitHub-compatible heading anchor slugs.
//!
//! Ported from the reference `index.js`/`regex.js`. The character-removal
//! regex is pre-expanded into the code-point range table in
//! `slug_table`, so no regular-expression engine is used. Behavior is pinned by the frozen oracle fixture
//! `tests/fixtures/slugger_oracle.json`, generated from the reference
//! implementation by `tools/slugger_probe.mjs`; if a case fails there,
//! suspect the port, never the fixture.

use std::collections::HashMap;

use crate::slug_table::slug_removed;

/// Slugifies `value` the way github-slugger 2.0.0's free `slug` export
/// does: lowercase unless `maintain_case`, remove the slug-removed
/// characters, and replace spaces with `-`.
#[must_use]
pub fn slug(value: &str, maintain_case: bool) -> String {
    if maintain_case {
        return remove_and_dash(value);
    }
    remove_and_dash(&value.to_lowercase())
}

/// Removes the slug-removed characters and replaces spaces with `-`.
///
/// The reference does this as two regex `replace` passes (remove, then
/// space-to-dash); a single pass per character is equivalent because
/// ` ` is never itself removed, so removing first can never expose or
/// hide a space that the second pass would treat differently.
fn remove_and_dash(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if c == ' ' {
            out.push('-');
        } else if !slug_removed(c) {
            out.push(c);
        }
    }
    out
}

/// Stateful slugger mirroring github-slugger 2.0.0's default export:
/// like [`slug`], but repeated inputs within one slugger get `-1`, `-2`,
/// ... suffixes so every slug is unique until [`Slugger::reset`].
#[derive(Debug, Default)]
pub struct Slugger {
    /// Times each candidate slug has already been handed out.
    occurrences: HashMap<String, u64>,
}

impl Slugger {
    /// Creates a slugger with an empty occurrence table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the unique slug for `value`, counting prior uses exactly
    /// like github-slugger 2.0.0: while the candidate slug was seen
    /// before, the original slug's counter advances and the candidate
    /// becomes `original-<counter>`; the returned slug is then recorded
    /// with counter 0.
    ///
    /// The counter advanced in the loop is the *original* slug's, so a
    /// candidate that collides with an earlier `original-<n>` slug is
    /// retried against the same advancing counter — the reference's
    /// `while (self.occurrences[result])` recheck loop, verbatim.
    pub fn slug(&mut self, value: &str, maintain_case: bool) -> String {
        let original = slug(value, maintain_case);
        let mut result = original.clone();
        while self.occurrences.contains_key(&result) {
            let count = self.occurrences.entry(original.clone()).or_insert(0);
            *count += 1;
            result = format!("{original}-{count}");
        }
        self.occurrences.insert(result.clone(), 0);
        result
    }

    /// Clears the occurrence table, like github-slugger 2.0.0's `reset`.
    pub fn reset(&mut self) {
        self.occurrences.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{Slugger, slug};

    #[test]
    fn free_slug_matches_reference_basics() {
        assert_eq!(slug("Hello World", false), "hello-world");
        assert_eq!(slug("", false), "");
        assert_eq!(slug("  ", false), "--");
        assert_eq!(slug("FOO", true), "FOO");
        assert_eq!(slug("FOO", false), "foo");
    }

    #[test]
    fn repeated_slugs_get_numbered_suffixes() {
        let mut slugger = Slugger::new();
        assert_eq!(slugger.slug("A", false), "a");
        assert_eq!(slugger.slug("A", false), "a-1");
        assert_eq!(slugger.slug("A", false), "a-2");
    }

    #[test]
    fn dash_collision_rechecks_the_original_counter() {
        let mut slugger = Slugger::new();
        assert_eq!(slugger.slug("a-1", false), "a-1");
        assert_eq!(slugger.slug("a", false), "a");
        assert_eq!(slugger.slug("a", false), "a-2");
    }

    #[test]
    fn reset_restores_first_use() {
        let mut slugger = Slugger::new();
        assert_eq!(slugger.slug("hello world", false), "hello-world");
        assert_eq!(slugger.slug("hello world", false), "hello-world-1");
        slugger.reset();
        assert_eq!(slugger.slug("hello world", false), "hello-world");
    }
}
