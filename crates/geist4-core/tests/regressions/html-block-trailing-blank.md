//// Bug: blank lines inside an HTML block were normalized away by the
//// renderer (the literal was trimmed and one newline re-added), so a
//// block ending at end-of-input lost its trailing blank line and the
//// output no longer matched the reference implementation byte for byte.
<!--a

