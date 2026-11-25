# wiki_links

Conformance examples for the built-in wiki links extension:
`[[target]]` or `[[target|label]]` renders as
`<a href="{target}" class="wikilink">`. The semantics are the
declarative tier's `[[wiki_link]]` rules exactly, as the tier-1
conformance package `conformance/behavior/wiki-links` pins them. Any
output change here is a real behavioral change.

## Examples

The target fills the `{target}` slot and also becomes the label when
no label is given.

```````````````````````````````` example
[[Page]]
.
<p><a href="Page" class="wikilink">Page</a></p>
````````````````````````````````

A label overrides the target as the link's content.

```````````````````````````````` example
[[Page|see the page]]
.
<p><a href="Page" class="wikilink">see the page</a></p>
````````````````````````````````

A real link wins: the bracketed text loses the wiki claim and renders
as literal text inside the ordinary link.

```````````````````````````````` example
[[Page]](x)
.
<p><a href="x">[Page]</a></p>
````````````````````````````````

Spaces are percent-encoded into the target but stay spaces in the
label.

```````````````````````````````` example
[[Page name]]
.
<p><a href="Page%20name" class="wikilink">Page name</a></p>
````````````````````````````````

The reserved characters are percent-encoded into the target and stay
literal in the label.

```````````````````````````````` example
[[a#b?c]]
.
<p><a href="a%23b%3Fc" class="wikilink">a#b?c</a></p>
````````````````````````````````

A single closing bracket breaks the suffix and the text stays
literal.

```````````````````````````````` example
[[a]b]
.
<p>[[a]b]</p>
````````````````````````````````

The claim happens before the inline machinery runs, so emphasis
inside the brackets stays literal in both the target and the label.

```````````````````````````````` example
[[a *b* c]]
.
<p><a href="a%20*b*%20c" class="wikilink">a *b* c</a></p>
````````````````````````````````

An unterminated opening bracket is not a wiki link and stays literal
text.

```````````````````````````````` example
[[Page
.
<p>[[Page</p>
````````````````````````````````

The label is HTML-escaped like inline text.

```````````````````````````````` example
[[a|<b>]]
.
<p><a href="a" class="wikilink">&lt;b&gt;</a></p>
````````````````````````````````

A wiki link is inline content: it nests inside emphasis and carries
its neighbors with it.

```````````````````````````````` example
*[[a]]*
.
<p><em><a href="a" class="wikilink">a</a></em></p>
````````````````````````````````

An empty target renders an empty `href` and the empty label.

```````````````````````````````` example
[[|x]]
.
<p><a href="" class="wikilink">x</a></p>
````````````````````````````````

The first `|` splits; a second one stays in the label.

```````````````````````````````` example
[[a|b|c]]
.
<p><a href="a" class="wikilink">b|c</a></p>
````````````````````````````````
