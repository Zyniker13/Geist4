# block-containers

Conformance examples for the tier-1 `[[block_container]]` definition.
The package declares two containers: `::: frame`, whose template wraps
the children in a `<div>`, and `+++ callout`, a second entry with the
default empty `info_html`. Any implementation of format
version 1 must render every example byte for byte.

## Examples

Ordinary block parsing continues inside the container: paragraphs,
lists, blank lines between them.

```````````````````````````````` example
::: frame
First paragraph.

- a list item
:::
.
<div class="frame">
<p>First paragraph.</p>
<ul>
<li>a list item</li>
</ul>
</div>
````````````````````````````````

Containers of the same name nest; the close fence closes the
innermost open container of that name.

```````````````````````````````` example
::: frame
outer

::: frame
inner
:::

still outer
:::
.
<div class="frame">
<p>outer</p>
<div class="frame">
<p>inner</p>
</div>
<p>still outer</p>
</div>
````````````````````````````````

A close fence must be at least as long as the open fence. A shorter or
longer run is paragraph text inside the container, not a close.

```````````````````````````````` example
::: frame
hi
::
still in
::::
and this
:::
.
<div class="frame">
<p>hi
::
still in
::::
and this</p>
</div>
````````````````````````````````

The marker interrupts a paragraph, the close fence is consumed and
leaves no trace, and parsing continues after it.

```````````````````````````````` example
a paragraph
::: frame
interrupted
:::

after
.
<p>a paragraph</p>
<div class="frame">
<p>interrupted</p>
</div>
<p>after</p>
````````````````````````````````

A container left open at the end of the document is closed by the end
of the document (`unclosed_ok` defaults to true).

```````````````````````````````` example
::: frame
never closed
.
<div class="frame">
<p>never closed</p>
</div>
````````````````````````````````

Up to three spaces of indentation may precede an open fence and a
close fence; four spaces would make them an indented code block.

```````````````````````````````` example
   ::: frame
hi
   :::
after
.
<div class="frame">
<p>hi</p>
</div>
<p>after</p>
````````````````````````````````

The info string is any text after the name. An entry without an
`info_html` template (the default) ignores it entirely; the fenced
content renders normally.

```````````````````````````````` example
+++ callout See *this*
body
+++
.
<aside class="callout">
<p>body</p>
</aside>
````````````````````````````````

The marker is recognized only in block position: inside a block quote
the continuation lines keep the container's paragraph open, and a lazy
line joins it like any other paragraph line.

```````````````````````````````` example
> ::: frame
> text
lazy
.
<blockquote>
<div class="frame">
<p>text
lazy</p>
</div>
</blockquote>
````````````````````````````````
