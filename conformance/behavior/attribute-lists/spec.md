# attribute-lists

Conformance examples for the tier-1 `[[attribute_list]]` definition.
The package carries one inline span, `==…==`, whose template exposes
`{id}`, `{class}` and `{title}` slots; an attribute list that attaches
to the span shows in the output, while a list that attaches to a core
paragraph can only be observed as a consumed line. Any implementation
of format version 1 must render every example byte for byte.

## Examples

A list on a line of its own attaches to the preceding block and is
consumed; the preceding paragraph's output does not change.

```````````````````````````````` example
hello

{: #intro .note k=v}
.
<p>hello</p>
````````````````````````````````

A list that ends a paragraph attaches to the paragraph itself and is
consumed either way.

```````````````````````````````` example
hello
{: .note}
.
<p>hello</p>
````````````````````````````````

With no preceding block there is nothing to attach to and the line
stays literal text.

```````````````````````````````` example
{: .x}
.
<p>{: .x}</p>
````````````````````````````````

A malformed list is not an attribute list and stays literal text.

```````````````````````````````` example
hello

{: .}
.
<p>hello</p>
<p>{: .}</p>
````````````````````````````````

An inline list attaches to the inline span it follows; the class slot
fills with the attached value, and the other declared attributes
render empty.

```````````````````````````````` example
a ==b=={:.c} d
.
<p>a <mark id="" class="c" title="">b</mark> d</p>
````````````````````````````````

The id item fills the id slot.

```````````````````````````````` example
a ==b=={:#intro} c
.
<p>a <mark id="intro" class="" title="">b</mark> c</p>
````````````````````````````````

A quoted value is entity-decoded when it attaches and escaped again
when it renders, so an attribute value that holds an ampersand round
trips.

```````````````````````````````` example
a ==b=={: title="a &amp; b"} c
.
<p>a <mark id="" class="" title="a &amp; b">b</mark> c</p>
````````````````````````````````

At most 32 entries attach per list; a longer list is not an attribute
list and stays literal text.

```````````````````````````````` example
{: k0 k1 k2 k3 k4 k5 k6 k7 k8 k9 k10 k11 k12 k13 k14 k15 k16 k17 k18 k19 k20 k21 k22 k23 k24 k25 k26 k27 k28 k29 k30 k31 k32}
.
<p>{: k0 k1 k2 k3 k4 k5 k6 k7 k8 k9 k10 k11 k12 k13 k14 k15 k16 k17 k18 k19 k20 k21 k22 k23 k24 k25 k26 k27 k28 k29 k30 k31 k32}</p>
````````````````````````````````
