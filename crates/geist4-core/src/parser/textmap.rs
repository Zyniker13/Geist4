//! Mapping a leaf block's text buffer back to source offsets.
//!
//! A leaf block's inline text is a buffer: per-line content joined with
//! newlines, built after each line's block prefixes were consumed, with
//! partially consumed tabs turned into spaces and NUL bytes replaced by
//! U+FFFD. To give inline nodes spans in the source document, the parser
//! records one anchor per appended line — where the line's bytes sit in
//! the buffer and where they came from — and the inline phase maps cell
//! ranges through those anchors.

/// One line's placement inside a leaf block's text buffer.
///
/// Buffer bytes `buf_start .. buf_start + line_len` map to source bytes
/// starting at `src_start` and ending at `src_end` (the offset one past
/// the line's ending, so the join byte maps to the ending's last byte).
/// Within the line the mapping is linear, except that the first
/// `tab_span` buffer bytes all stand for the tab byte at `src_start` —
/// the synthesized spaces of a partially consumed tab — and that a NUL
/// replacement gives each U+FFFD byte its NUL's own source offset, which
/// `nul_map` lists per buffer byte.
#[derive(Debug, Clone)]
pub(crate) struct LineAnchor {
    /// The buffer offset of the line's first byte, joins included on the
    /// left, its own content excluded from `src_end`.
    pub(crate) buf_start: usize,
    /// The source offset the tab's synthesized spaces map to: the tab
    /// byte when the line has a partially consumed tab, else the line's
    /// first mapped byte.
    pub(crate) src_start: usize,
    /// The source offset the line's first non-tab buffer byte maps to.
    pub(crate) lin_start: usize,
    /// How many buffer bytes are synthesized spaces of a partially
    /// consumed tab, all mapping to `src_start`.
    pub(crate) tab_span: u16,
    /// The source offset just past the line's ending.
    pub(crate) src_end: usize,
    /// Per-buffer-byte source offsets when the line was rebuilt with NUL
    /// replacements; `None` keeps the linear mapping.
    pub(crate) nul_map: Option<Box<[usize]>>,
}

impl LineAnchor {
    /// An anchor for a line whose buffer bytes are a plain slice of the
    /// source from `src_start` to `src_end`.
    pub(crate) fn whole_line(buf_start: usize, src_start: usize, src_end: usize) -> Self {
        LineAnchor {
            buf_start,
            src_start,
            lin_start: src_start,
            tab_span: 0,
            src_end,
            nul_map: None,
        }
    }
}

/// The buffer-to-source map of one leaf block's inline text.
#[derive(Debug, Clone, Default)]
pub(crate) struct TextMap {
    anchors: Vec<LineAnchor>,
    /// The buffer length the anchors cover, joins included.
    next: usize,
}

impl TextMap {
    /// Records one appended line of `text_len` buffer bytes at the
    /// buffer's end.
    pub(crate) fn record(&mut self, text_len: usize, anchor: LineAnchor) {
        self.anchors.push(LineAnchor {
            buf_start: self.next,
            ..anchor
        });
        self.next += text_len;
    }

    /// Records one appended line of `text_len` buffer bytes after the
    /// joining newline: the join shifts the line's buffer start by one.
    pub(crate) fn record_after_join(&mut self, text_len: usize, anchor: LineAnchor) {
        self.next += 1;
        self.record(text_len, anchor);
    }

    /// The source span of the buffer range `[start, end)`. With no
    /// anchors — a block whose bytes came straight from one source
    /// region — offsets shift by `fallback_start`.
    pub(crate) fn map_span(
        &self,
        start: usize,
        end: usize,
        fallback_start: usize,
    ) -> crate::span::Span {
        if self.anchors.is_empty() {
            return crate::span::Span::new(fallback_start + start, fallback_start + end);
        }
        if end <= start {
            return crate::span::Span::empty(self.byte_before(start));
        }
        crate::span::Span::new(self.byte_before(start), self.map_byte(end - 1) + 1)
    }

    /// The source offset of the byte just before buffer offset `o`,
    /// where offsets sit between characters.
    pub(crate) fn byte_before(&self, o: usize) -> usize {
        if o == 0 {
            return self.first_src();
        }
        self.map_byte(o - 1) + 1
    }

    /// Drops the buffer's first `consumed` bytes: consumed leading
    /// reference definitions. Anchors inside the consumed region go away;
    /// an anchor straddling the cut is rebased to the buffer start.
    pub(crate) fn shift(&mut self, consumed: usize) {
        if consumed == 0 || consumed >= self.next {
            if consumed >= self.next {
                self.anchors.clear();
                self.next = 0;
            }
            return;
        }
        // Line lengths before the cut, since rebasing an anchor must not
        // disturb the neighbour positions the lengths read from.
        let lens: Vec<usize> = self.anchors.iter().map(|a| self.line_len(a)).collect();
        let mut index = 0usize;
        self.anchors.retain_mut(|anchor| {
            let line_len = lens[index];
            index += 1;
            if anchor.buf_start >= consumed {
                anchor.buf_start -= consumed;
                true
            } else if anchor.buf_start + line_len > consumed {
                // The cut falls inside this line: keep the tail.
                let cut = consumed - anchor.buf_start;
                if cut <= usize::from(anchor.tab_span) {
                    anchor.tab_span -= cut as u16;
                } else {
                    let past_tab = cut - usize::from(anchor.tab_span);
                    anchor.src_start = anchor.lin_start + past_tab;
                    anchor.lin_start = anchor.src_start;
                    anchor.tab_span = 0;
                }
                if let Some(nul_map) = &mut anchor.nul_map {
                    *nul_map = nul_map[cut..].into();
                    if let Some(&first) = nul_map.first() {
                        anchor.src_start = first;
                    }
                }
                anchor.buf_start = 0;
                true
            } else {
                false
            }
        });
        self.next -= consumed;
    }

    /// The line's buffer length, joins excluded.
    fn line_len(&self, anchor: &LineAnchor) -> usize {
        match self
            .anchors
            .iter()
            .position(|a| a.buf_start == anchor.buf_start)
        {
            Some(i) if i + 1 < self.anchors.len() => {
                self.anchors[i + 1].buf_start - anchor.buf_start - 1
            }
            _ => self.next - anchor.buf_start,
        }
    }

    /// The source offset of the buffer byte at `o`.
    pub(crate) fn map_byte(&self, o: usize) -> usize {
        if self.anchors.is_empty() {
            return 0;
        }
        let index = match self.anchors.partition_point(|anchor| anchor.buf_start <= o) {
            0 => 0,
            i => i - 1,
        };
        let anchor = &self.anchors[index];
        let within = o - anchor.buf_start;
        let line_len = self.line_len(anchor);
        if within >= line_len {
            // The newline joining this line to the next: the ending's
            // last byte.
            return anchor.src_end - 1;
        }
        if let Some(nul_map) = &anchor.nul_map {
            return nul_map.get(within).copied().unwrap_or(anchor.src_start);
        }
        if within < usize::from(anchor.tab_span) {
            anchor.src_start
        } else {
            anchor.lin_start + within - usize::from(anchor.tab_span)
        }
    }

    /// The source offset of the buffer's first byte.
    pub(crate) fn first_src(&self) -> usize {
        self.map_byte(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setext_span_after_consumed_defs() {
        let mut map = TextMap::default();
        map.record(7, LineAnchor::whole_line(0, 0, 8));
        map.record_after_join(4, LineAnchor::whole_line(0, 8, 13));
        assert_eq!(map.map_span(8, 12, 0), crate::span::Span::new(8, 12));
    }
}
