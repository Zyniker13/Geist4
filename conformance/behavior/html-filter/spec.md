# html-filter

Conformance examples for the tier-1 `[[html_filter]]` definition. The
package denies `script`, `iframe` and `style`: each `<` that starts
such a tag in raw HTML renders as `&lt;`. Any implementation of
format version 1 must render every example byte for byte.

## Examples

Both the opening and the closing tag of a denied element are
substituted in inline HTML.

```````````````````````````````` example
a <script>x</script> b
.
<p>a &lt;script>x&lt;/script> b</p>
````````````````````````````````

The filter applies to HTML blocks too, line by line.

```````````````````````````````` example
<script>
document.write("ok");
</script>
.
&lt;script>
document.write("ok");
&lt;/script>
````````````````````````````````

The match is case-insensitive, and the tag's own case is preserved
in the substituted output.

```````````````````````````````` example
x <SCRIPT>y</SCRIPT> z
.
<p>x &lt;SCRIPT>y&lt;/SCRIPT> z</p>
````````````````````````````````

A name that merely starts like a denied tag is not denied.

```````````````````````````````` example
<scripty>x</scripty>
.
<p><scripty>x</scripty></p>
````````````````````````````````

A tag that is not on the deny list passes through untouched.

```````````````````````````````` example
q <span data-x="1">a</span>
.
<p>q <span data-x="1">a</span></p>
````````````````````````````````

Attributes and the self-closing form are part of the raw HTML and
pass through after the substitution.

```````````````````````````````` example
x <style media="all" /> y
.
<p>x &lt;style media="all" /> y</p>
````````````````````````````````

Any denied tag is substituted, with its attributes kept.

```````````````````````````````` example
x <iframe src="y">z</iframe> w
.
<p>x &lt;iframe src="y">z&lt;/iframe> w</p>
````````````````````````````````

The filter applies to raw HTML only: a denied tag inside a code span
is code output, not raw HTML, and escapes the normal way.

```````````````````````````````` example
`<script>`
.
<p><code>&lt;script&gt;</code></p>
````````````````````````````````
