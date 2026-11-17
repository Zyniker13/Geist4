# inline-spans

Conformance examples for the tier-1 `[[inline_span]]` definition. The
package declares three spans: `==…==` wraps in `<mark>`, `%…%` wraps in
`<del>` (a one-byte delimiter, which pairs as runs), and `$…$` wraps in
`<span>` with the inline math guards. Any implementation of format
version 1 must render every example byte for byte.

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

A one-byte delimiter pairs as runs, with the rule of three: two and
one sum to three, so that pair is blocked while the single-byte pair
wraps.

```````````````````````````````` example
x%y%z and x%%y%z
.
<p>x<del>y</del>z and x%%y%z</p>
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

The `math` flag applies the inline math border guards: interior spaces
are allowed, but a leading space, a trailing space, a digit after the
opener and a digit before the opener all keep the delimiters literal.

```````````````````````````````` example
$a b$ and $ x$ and $5$ and 5$x$ and $x $.
.
<p><span>a b</span> and $ x$ and $5$ and 5$x$ and $x $.</p>
````````````````````````````````
