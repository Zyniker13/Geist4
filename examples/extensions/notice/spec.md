# notice

A fenced container: a line `::: notice` opens a block that holds other
blocks until a closing `:::` (or the document end). A tier-1
(declarative) extension: the engine runs the container, so nesting and
ordinary block parsing inside it behave exactly as anywhere else.

    geist4 ext check examples/extensions/notice

## Examples

A paragraph inside the container.

```````````````````````````````` example
::: notice
Watch this space.
:::
.
<div class="notice">
<p>Watch this space.</p>
</div>
````````````````````````````````

Several blocks, and blank lines between them.

```````````````````````````````` example
::: notice
First paragraph.

- a list item
:::
.
<div class="notice">
<p>First paragraph.</p>
<ul>
<li>a list item</li>
</ul>
</div>
````````````````````````````````

One container inside another; the close fence closes the innermost
open container of that name.

```````````````````````````````` example
::: notice
outer

::: notice
inner
:::

still outer
:::
.
<div class="notice">
<p>outer</p>
<div class="notice">
<p>inner</p>
</div>
<p>still outer</p>
</div>
````````````````````````````````

The marker interrupts a paragraph, like a code fence.

```````````````````````````````` example
a paragraph
::: notice
interrupted
:::
.
<p>a paragraph</p>
<div class="notice">
<p>interrupted</p>
</div>
````````````````````````````````

At the document end without a close fence, EOF closes the container.

```````````````````````````````` example
::: notice
never closed
.
<div class="notice">
<p>never closed</p>
</div>
````````````````````````````````
