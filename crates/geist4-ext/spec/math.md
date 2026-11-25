# math

Conformance examples for the built-in math extension: `$…$` renders as
`<span data-math-style="inline">` and `$$…$$` as
`<span data-math-style="display">`, with the parsed inline content as
children. The delimiters follow the declarative tier's
`[[inline_span]]` semantics — the flanking rules, one-byte runs pairing
as runs, backslash escapes, and code-span immunity — with the tier's
inline math border guards on the single-dollar runs. Display runs carry
no border guards; the flanking rules still gate them. Any output
change here is a real behavioral change.

## Examples

The basic inline and display forms render with the parsed content as
children.

```````````````````````````````` example
$a+b$ and $$c+d$$
.
<p><span data-math-style="inline">a+b</span> and <span data-math-style="display">c+d</span></p>
````````````````````````````````

The border guards: interior spaces are allowed, but a leading space, a
trailing space, a digit after the opener and a digit before the opener
all keep the delimiters literal.

```````````````````````````````` example
$a b$ and $ x$ and $5$ and 5$x$ and $x $.
.
<p><span data-math-style="inline">a b</span> and $ x$ and $5$ and 5$x$ and $x $.</p>
````````````````````````````````

Runs of one dollar are inline and runs of two or more are display, and
a pair matches only within its own kind: the outer runs of a
three-dollar opener and closer form a display pair around the middle
single dollar, which becomes the display content.

```````````````````````````````` example
$$$x$$$
.
<p><span data-math-style="display">x</span></p>
````````````````````````````````

Runs of different lengths do not pair across kinds: a lone dollar
inside a display pair is content.

```````````````````````````````` example
$$a$b$$
.
<p><span data-math-style="display">a$b</span></p>
````````````````````````````````

Display runs carry no border guards but keep the flanking rules: a run
with a space on the outward side cannot open or close.

```````````````````````````````` example
$$ x $$ and $$a$$
.
<p>$$ x $$ and <span data-math-style="display">a</span></p>
````````````````````````````````

Backslash escapes: an escaped opener does not open, and an escaped
closer inside a span stays literal text of the span's content.

```````````````````````````````` example
\$a$ and $a\$b$
.
<p>$a$ and <span data-math-style="inline">a$b</span></p>
````````````````````````````````

Code spans are immune: the dollars inside backticks are text, and a
code span inside a math span is the span's content.

```````````````````````````````` example
`$a$` and $`a`$
.
<p><code>$a$</code> and <span data-math-style="inline"><code>a</code></span></p>
````````````````````````````````

The spans nest with the core emphasis machinery from either side.

```````````````````````````````` example
*$a$* and $*a*$
.
<p><em><span data-math-style="inline">a</span></em> and <span data-math-style="inline"><em>a</em></span></p>
````````````````````````````````

A closer without an opener stays literal.

```````````````````````````````` example
b $ c$
.
<p>b $ c$</p>
````````````````````````````````

The delimiter stack pairs the first matching closer with the nearest
opener of its kind; an inline opener that cannot close its position
stays literal while a later display pair forms.

```````````````````````````````` example
$a$$b$$
.
<p>$a<span data-math-style="display">b</span></p>
````````````````````````````````

Display math may hold a soft break: the span stays open across one
line break inside a paragraph.

```````````````````````````````` example
$$a
b$$
.
<p><span data-math-style="display">a
b</span></p>
````````````````````````````````

Inline math follows the same rule.

```````````````````````````````` example
$a
b$
.
<p><span data-math-style="inline">a
b</span></p>
````````````````````````````````

The delimiters close at a paragraph's end: an unterminated opener
stays literal text.

```````````````````````````````` example
$a

b
.
<p>$a</p>
<p>b</p>
````````````````````````````````
