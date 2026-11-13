//// Bug: `--` inside an inline HTML comment was treated as a scan failure.
//// The reference scanner accepts dash runs inside an inline HTML
//// comment: `--` followed by any byte other than `>` is comment
//// content, so only a real `-->` closes the comment.
x <!-- a ------' b --> y
