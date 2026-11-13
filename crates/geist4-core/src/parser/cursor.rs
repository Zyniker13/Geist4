//! A cursor over one line with tab-aware virtual columns.
//!
//! Tabs advance to the next multiple of four columns. When only part of a
//! tab's columns are consumed (for example, one column after a `>` marker),
//! the cursor stays on the tab byte and records how many columns of it
//! remain; those become spaces when the line's content is taken.

/// A cursor over a line of source text.
#[derive(Debug, Clone)]
pub(super) struct Cursor<'t> {
    /// The line's bytes, without its line ending.
    line: &'t [u8],
    /// The byte position within the line.
    pos: usize,
    /// The virtual column at `pos`, with tabs expanded to tab stops of four.
    col: usize,
    /// Columns still to be consumed of the tab at `pos`, when `pos` sits on a
    /// partially consumed tab. Zero when `pos` is not on a tab or the tab was
    /// not partially consumed.
    partial_tab: usize,
}

impl<'t> Cursor<'t> {
    /// Creates a cursor at the start of `line`.
    pub(super) fn new(line: &'t [u8]) -> Self {
        Cursor {
            line,
            pos: 0,
            col: 0,
            partial_tab: 0,
        }
    }

    /// The next byte, without advancing.
    pub(super) fn peek(&self) -> Option<u8> {
        self.line.get(self.pos).copied()
    }

    /// The byte `n` positions ahead, without advancing.
    pub(super) fn peek_at(&self, n: usize) -> Option<u8> {
        self.line.get(self.pos + n).copied()
    }

    /// Whether the cursor is at the end of the line.
    pub(super) fn at_end(&self) -> bool {
        self.pos >= self.line.len()
    }

    /// The current byte position within the line.
    pub(super) fn pos(&self) -> usize {
        self.pos
    }

    /// The current virtual column.
    pub(super) fn col(&self) -> usize {
        self.col
    }

    /// Advances one byte. A tab advances to the next tab stop of four.
    pub(super) fn bump(&mut self) {
        if self.pos >= self.line.len() {
            return;
        }
        if self.line[self.pos] == b'\t' {
            self.col = (self.col / 4 + 1) * 4;
        } else {
            self.col += 1;
        }
        self.pos += 1;
        self.partial_tab = 0;
    }

    /// Skips spaces and tabs.
    pub(super) fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t')) {
            self.bump();
        }
    }

    /// The position and virtual column of the next byte that is not a space
    /// or tab, counting from the cursor.
    pub(super) fn find_nonspace(&self) -> (usize, usize) {
        let mut pos = self.pos;
        let mut col = self.col;
        while let Some(&b) = self.line.get(pos) {
            if b == b'\t' {
                col = (col / 4 + 1) * 4;
            } else if b == b' ' {
                col += 1;
            } else {
                break;
            }
            pos += 1;
        }
        (pos, col)
    }

    /// Whether only spaces and tabs remain from the cursor.
    pub(super) fn blank_from_here(&self) -> bool {
        let (pos, _) = self.find_nonspace();
        pos >= self.line.len()
    }

    /// Advances exactly `n` virtual columns.
    ///
    /// Returns false if the line ends first. When the advance lands inside a
    /// tab, the cursor stays on the tab byte and records the remaining
    /// columns, which the content-taking methods turn into spaces.
    pub(super) fn advance_cols(&mut self, n: usize) -> bool {
        let mut left = n;
        while left > 0 {
            let Some(&b) = self.line.get(self.pos) else {
                return false;
            };
            if b == b'\t' {
                let chars_to_tab = 4 - self.col % 4;
                if chars_to_tab > left {
                    self.col += left;
                    self.partial_tab = chars_to_tab - left;
                    return true;
                }
                self.col += chars_to_tab;
                self.pos += 1;
                left -= chars_to_tab;
                self.partial_tab = 0;
            } else {
                self.pos += 1;
                self.col += 1;
                left -= 1;
                self.partial_tab = 0;
            }
        }
        true
    }

    /// Moves the cursor to the first non-space byte from here.
    pub(super) fn skip_to_content(&mut self) {
        let (pos, col) = self.find_nonspace();
        self.pos = pos;
        self.col = col;
        self.partial_tab = 0;
    }

    /// The line's bytes, without its line ending.
    pub(super) fn line(&self) -> &'t [u8] {
        self.line
    }

    /// The number of bytes left from the cursor to the line's end.
    pub(super) fn rest_len(&self) -> usize {
        self.line.len() - self.pos
    }

    /// Whether the rest of the line is a plain slice of the line's bytes.
    /// A partially consumed tab makes `rest` synthesize its bytes instead,
    /// so source offsets no longer describe it.
    pub(super) fn rest_is_slice(&self) -> bool {
        self.partial_tab == 0
    }

    /// Moves the cursor to byte `pos` at virtual column `col`, clearing any
    /// partially consumed tab.
    pub(super) fn set_pos_col(&mut self, pos: usize, col: usize) {
        self.pos = pos;
        self.col = col;
        self.partial_tab = 0;
    }

    /// Advances `n` bytes, tabs expanding to tab stops of four.
    pub(super) fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            self.bump();
        }
    }

    /// Advances to the end of the line.
    pub(super) fn bump_all(&mut self) {
        while !self.at_end() {
            self.bump();
        }
    }

    /// Consumes only spaces and tabs, up to `n` virtual columns, leaving the
    /// cursor on the first other byte. Used to strip a block's fixed
    /// indentation from content lines without eating content.
    pub(super) fn skip_up_to_cols(&mut self, n: usize) {
        let mut left = n;
        while left > 0 {
            match self.peek() {
                Some(b' ') => {
                    self.bump();
                    left -= 1;
                }
                Some(b'\t') => {
                    let to_tab = 4 - self.col % 4;
                    if to_tab > left {
                        self.advance_cols(left);
                        left = 0;
                    } else {
                        self.bump();
                        left -= to_tab;
                    }
                }
                _ => break,
            }
        }
    }

    /// The rest of the line from the cursor, with a partially consumed tab
    /// at the cursor turned into its remaining columns of spaces.
    pub(super) fn rest(&self) -> Vec<u8> {
        if self.partial_tab > 0 {
            let mut out = Vec::with_capacity(self.line.len() - self.pos + self.partial_tab);
            out.extend(std::iter::repeat_n(b' ', self.partial_tab));
            out.extend_from_slice(&self.line[self.pos + 1..]);
            out
        } else {
            self.line[self.pos..].to_vec()
        }
    }
}
