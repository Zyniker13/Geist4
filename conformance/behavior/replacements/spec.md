# replacements

Conformance examples for the tier-1 `[[replace]]` definition. The
package declares six replacements; their needles are chosen so no
example's text triggers another's. Any implementation of format
version 1 must render every example byte for byte.

## Examples

With `once = false` every occurrence in a text node is replaced.

```````````````````````````````` example
a (c) b (c)
.
<p>a © b ©</p>
````````````````````````````````

`once` defaults to true: only the first occurrence in a text node is
replaced, and the replacement text is emitted verbatim, here the dash
character; the un-replaced occurrence is escaped as plain text.

```````````````````````````````` example
a -- b -- c
.
<p>a – b -- c</p>
````````````````````````````````

Entries are applied in declaration order, each on the result of the
previous, so the chain replaces the doubled letters twice.

```````````````````````````````` example
qq qq
.
<p>W W</p>
````````````````````````````````

Code spans are immune.

```````````````````````````````` example
`(c)` and (c)
.
<p><code>(c)</code> and ©</p>
````````````````````````````````

Code blocks are immune.

```````````````````````````````` example
```
(c)
```

(c)
.
<pre><code>(c)
</code></pre>
<p>©</p>
````````````````````````````````

Raw HTML is immune, both block-level and inline, in the tag and in
attribute values; the text between tags is replaced.

```````````````````````````````` example
<div>(c)</div>

q <span title="(c)">yes (c)</span>
.
<div>(c)</div>
<p>q <span title="(c)">yes ©</span></p>
````````````````````````````````

A replacement matches only within one text node and never crosses a
node boundary, so the letters on either side of an emphasis run do
not combine into a needle.

```````````````````````````````` example
x*b*
.
<p>x<em>b</em></p>
````````````````````````````````

The substitution is a single pass over each text node: the text a
replacement produces is not scanned again, which is what lets a
self-referential replacement terminate.

```````````````````````````````` example
mail x@y
.
<p>mail x@@y</p>
````````````````````````````````
