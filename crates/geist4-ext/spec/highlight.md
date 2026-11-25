# highlight

Conformance examples for the built-in highlight extension: `==…==`
renders as `<mark>` with the parsed inline content as children. The
delimiters follow the declarative tier's `[[inline_span]]` semantics —
the flanking rules, the two-byte run rule, backslash escapes, code-span
immunity, and nesting with emphasis — exactly as
`conformance/behavior/inline-spans` pins them. Any output change here
is a real behavioral change.

## Examples

The flanking rules: a delimiter run with a space on the outward side
stays literal; a run at a word's edge opens and closes.

```````````````````````````````` example
a == b and a ==b== c.
.
<p>a == b and a <mark>b</mark> c.</p>
````````````````````````````````

A two-byte delimiter is one unit, so a run of four is one delimiter,
not two; runs of different lengths do not pair, and a lone `=` is not
an occurrence of `==` at all.

```````````````````````````````` example
====x==== and =x= and ===x==
.
<p><mark>x</mark> and =x= and ===x==</p>
````````````````````````````````

Backslash escapes: an escaped opener does not open, and an escaped
closer inside a span stays literal text of the span's content.

```````````````````````````````` example
\==x== and ==a\==b==
.
<p>==x== and <mark>a==b</mark></p>
````````````````````````````````

The first close after the open wins; the same pair never nests, and a
closer without an opener stays literal.

```````````````````````````````` example
==a==b==
.
<p><mark>a</mark>b==</p>
````````````````````````````````

Code spans are immune: the delimiters inside backticks are text.

```````````````````````````````` example
`x ==y== z`
.
<p><code>x ==y== z</code></p>
````````````````````````````````

The spans nest with the core emphasis machinery from either side.

```````````````````````````````` example
*==bold highlight==* and ==*highlight bold*==.
.
<p><em><mark>bold highlight</mark></em> and <mark><em>highlight bold</em></mark>.</p>
````````````````````````````````

Marks may cross a soft break inside one paragraph, like the tier's
`multiline` default.

```````````````````````````````` example
==a
b==
.
<p><mark>a
b</mark></p>
````````````````````````````````

The delimiters close at a paragraph's end: an unterminated opener
stays literal text.

```````````````````````````````` example
==a

b
.
<p>==a</p>
<p>b</p>
````````````````````````````````
