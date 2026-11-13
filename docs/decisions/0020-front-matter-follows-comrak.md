# 0020: Front matter follows comrak's split, claimed by byte offset

## Context

GFM carries no front matter specification, and cmark-gfm has no front
matter branch, so no cmark-based oracle exists. comrak implements front
matter as a parser option (`front_matter_delimiter`) whose observable
HTML behavior is: the prologue renders as nothing, and the rest of the
document parses as if the prologue were not part of the source.

comrak implements that by splitting the source before parsing and
re-parsing the remainder from scratch, which shifts every offset after
the prologue relative to the original input.

## Decision

The splitting rules follow comrak exactly, quirks included: one leading
byte order mark may precede the opener; the opener is the delimiter
followed by a line ending (the delimiter must end its line); the closer
is the first later occurrence of a line ending, the delimiter, and then
a line ending or the end of input, searched as three patterns in
comrak's priority order so a CRLF closer anywhere in the body wins over
an earlier LF-only closer; the closer's own line ending and one
following blank line belong to the prologue; an empty body never
closes. The default delimiter is `---`; `front_matter_with` accepts any
delimiter, matching comrak's one-delimiter option.

Unlike comrak, the prologue is claimed by byte offset before the line
loop and the rest of the document parses from its real position, so
source spans after the prologue stay true to the original input. The
HTML output is unaffected by the difference. The prologue is a
`front_matter` node whose raw text is the node's `source` attribute,
declared through the ordinary extension API, so loaded extensions can
claim front matter the same way.

## Consequences

Parity is pinned by a 47-case differential battery against the comrak
0.51.0 library (its CLI does not expose the option) and a 34-example
spec file of oracle-agreeing cases; CRLF cases cannot be expressed in
the example format, so they live in the differential battery and core
unit tests. Spans are more faithful than the oracle's, which is a
deliberate, documented divergence. Documents whose body legitimately
starts with the delimiter at position zero parse as front matter, as in
comrak; a document that begins with a thematic break must start with a
blank line or other content first.
