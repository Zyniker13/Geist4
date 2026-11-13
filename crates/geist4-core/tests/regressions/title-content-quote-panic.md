//// Bug: a link definition whose title content itself begins and ends
//// with the same quote character (here the content is a single
//// apostrophe) made clean_title strip the content again as if it were
//// the outer quoting, slicing an empty range and panicking.
//// Found by the parse_document fuzz target.
[a]: /u "'"

[a]
