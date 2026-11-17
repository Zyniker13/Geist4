# highlight

Marks text with `==double equals==`, the way `==mark==` renders on
several Markdown flavors. A tier-1 (declarative) extension: the engine
scans the delimiters with the standard emphasis rules, so nesting,
escaping and flanking behave like the rest of Markdown.

    geist4 ext check examples/extensions/highlight

## Examples

```````````````````````````````` example
This is ==highlighted== text.
.
<p>This is <mark>highlighted</mark> text.</p>
````````````````````````````````

Two spans on one line, each pairing with its nearest opener.

```````````````````````````````` example
==one== and ==two==.
.
<p><mark>one</mark> and <mark>two</mark>.</p>
````````````````````````````````

The delimiters nest inside other inline constructs, and other
constructs nest inside the span.

```````````````````````````````` example
*==bold highlight==* and ==*highlight bold*==.
.
<p><em><mark>bold highlight</mark></em> and <mark><em>highlight bold</em></mark>.</p>
````````````````````````````````

A backslash-escaped `==` does not open a span.

```````````````````````````````` example
\==not a span==
.
<p>==not a span==</p>
````````````````````````````````

Code spans and code blocks are untouched.

```````````````````````````````` example
`x ==y== z` and

    x ==y== z
.
<p><code>x ==y== z</code> and</p>
<pre><code>x ==y== z
</code></pre>
````````````````````````````````
