# wiki-links

Conformance examples for the tier-1 `[[wiki_link]]` definition. The
package declares one wiki link, `[[target]]` or `[[target|label]]`,
rendered as `<a class="wikilink">`. Any implementation of format
version 1 must render every example byte for byte.

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
