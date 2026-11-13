# 0019: The renderer ends its output with a newline

## Context

The specification example format cannot express a trailing newline: the
newline before a closing fence is the delimiter between the example and
the next one, so every example's expected HTML is stored without a final
newline. The renderer matched that convention by stripping the newline
after the last block. The reference implementations do the opposite:
cmark, cmark-gfm and comrak all end every document's HTML with a
newline after the last block. Differential comparisons made through
shell command substitution hid the difference, because both sides were
stripped alike.

## Decision

The HTML renderer emits the trailing newline, exactly as the reference
implementations do; an empty document renders as nothing. The spec
harness normalizes exactly one trailing newline on both sides before
comparing, which is the most the example format can hide. Test helpers
that compare against expectations written without the newline strip one
newline locally with a comment saying why.

## Consequences

The engine's HTML output is byte-for-byte comparable to the reference
CLIs' output without any shell-level normalization, so a consumer
diffing against cmark or comrak sees no difference. The spec suite
still compares exactly modulo the format's one unrepresentable newline;
a missing or doubled newline inside a document is still a failure. The
one-byte change is observable by anyone comparing rendered output
against an older Geist4 build.
