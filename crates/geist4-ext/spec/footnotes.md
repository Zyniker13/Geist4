# Footnotes


These examples run with the `footnotes` extension enabled. Every
expected output agrees with the reference implementation and with the
engine, byte for byte.

The feature follows comrak's implementation, which is where GitHub's
footnote rendering lives; cmark-gfm carries no public footnotes branch
(decision 0018). Definitions parse as ordinary containers: their lines
continue while indented four columns — exactly four are consumed, so a
six-space continuation is a two-indented paragraph inside the
definition — or blank. A post-processing pass numbers references in
document order, moves referenced definitions to the end of the
document in first-reference order, restores unmatched references to
literal text, and detaches definitions nothing referenced. A
definition whose label a later definition overwrote never enters the
map: it stays where it was written and renders without backrefs.

Two details come from the reference's inline structure. The reference
is claimed only when the node right after the `[` starts with `^`, so
`[\^a]` (whose escape sits in its own node) and `[ ^a]` stay literal;
and an image-shaped `![^1]` still claims, leaving the `!` as text,
because the marker is a node of its own that the claim does not touch.

The reference renders the footnote section wherever the first
definition appears and closes it at the end of the document, so a
block between two definitions renders inside the `<ol>`; a definition
the pass moved to the end sits after those. Backrefs splice into a
definition's last paragraph when it has one; the first backref is
bare and later ones carry their number.


## References and definitions

```````````````````````````````` example
x[^1]

[^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
a[^x] b[^x]

[^x]: y
.
<p>a<sup class="footnote-ref"><a href="#fn-x" id="fnref-x" data-footnote-ref>1</a></sup> b<sup class="footnote-ref"><a href="#fn-x" id="fnref-x-2" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-x">
<p>y <a href="#fnref-x" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a> <a href="#fnref-x-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-2" aria-label="Back to reference 1-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]
.
<p>x[^1]</p>
````````````````````````````````

```````````````````````````````` example
[^1]: note
.

````````````````````````````````

```````````````````````````````` example
a[^nope]
.
<p>a[^nope]</p>
````````````````````````````````

```````````````````````````````` example
x

[^a]: unused
.
<p>x</p>
````````````````````````````````

```````````````````````````````` example
a[^b] c[^a]

[^a]: one

[^b]: two
.
<p>a<sup class="footnote-ref"><a href="#fn-b" id="fnref-b" data-footnote-ref>1</a></sup> c<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>2</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-b">
<p>two <a href="#fnref-b" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
<li id="fn-a">
<p>one <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
a[^1] b[^2] c[^3] d[^1] e[^2] f[^3]

[^1]: one

[^2]: two

[^3]: three
.
<p>a<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup> b<sup class="footnote-ref"><a href="#fn-2" id="fnref-2" data-footnote-ref>2</a></sup> c<sup class="footnote-ref"><a href="#fn-3" id="fnref-3" data-footnote-ref>3</a></sup> d<sup class="footnote-ref"><a href="#fn-1" id="fnref-1-2" data-footnote-ref>1</a></sup> e<sup class="footnote-ref"><a href="#fn-2" id="fnref-2-2" data-footnote-ref>2</a></sup> f<sup class="footnote-ref"><a href="#fn-3" id="fnref-3-2" data-footnote-ref>3</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>one <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a> <a href="#fnref-1-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-2" aria-label="Back to reference 1-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
<li id="fn-2">
<p>two <a href="#fnref-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a> <a href="#fnref-2-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2-2" aria-label="Back to reference 2-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
<li id="fn-3">
<p>three <a href="#fnref-3" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="3" aria-label="Back to reference 3">↩</a> <a href="#fnref-3-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="3-2" aria-label="Back to reference 3-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a][^a][^a]

[^a]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup><sup class="footnote-ref"><a href="#fn-a" id="fnref-a-2" data-footnote-ref>1</a></sup><sup class="footnote-ref"><a href="#fn-a" id="fnref-a-3" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>note <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a> <a href="#fnref-a-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-2" aria-label="Back to reference 1-2">↩<sup class="footnote-ref">2</sup></a> <a href="#fnref-a-3" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-3" aria-label="Back to reference 1-3">↩<sup class="footnote-ref">3</sup></a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
c[^c] a[^a] b[^b]

[^a]: A

[^b]: B

[^c]: C
.
<p>c<sup class="footnote-ref"><a href="#fn-c" id="fnref-c" data-footnote-ref>1</a></sup> a<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>2</a></sup> b<sup class="footnote-ref"><a href="#fn-b" id="fnref-b" data-footnote-ref>3</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-c">
<p>C <a href="#fnref-c" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
<li id="fn-a">
<p>A <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
<li id="fn-b">
<p>B <a href="#fnref-b" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="3" aria-label="Back to reference 3">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
[^a]: first

x[^a]

[^a]: second
.
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>first </p>
</li>
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup></p>
<li id="fn-a">
<p>second <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a]y

[^a]: note

[^a]: z
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup>y</p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>note </p>
</li>
<li id="fn-a">
<p>z <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
[^1]
.
<p>[^1]</p>
````````````````````````````````

```````````````````````````````` example
[^1]:
.

````````````````````````````````


## Definition bodies

```````````````````````````````` example
x[^1]

[^1]: note

  - one
  - two
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<ul>
<li>one</li>
<li>two</li>
</ul>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note

  > quoted
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<blockquote>
<p>quoted</p>
</blockquote>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: para

      code
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>para</p>
<p>code <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: first
    second line
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>first
second line <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: first

    second para
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>first</p>
<p>second para <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: first

  after def
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<p>after def</p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>first <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: first
lazy line
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>first
lazy line <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: first
more
lines
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>first
more
lines <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a] y[^b]

[^a]: one
[^b]: two
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup> y<sup class="footnote-ref"><a href="#fn-b" id="fnref-b" data-footnote-ref>2</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>one <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
<li id="fn-b">
<p>two <a href="#fnref-b" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note

plain
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<p>plain</p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
[^1]: note

x[^1]
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]:
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1"><a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]:note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1] : note
.
<p>x[^1]</p>
<p>[^1] : note</p>
````````````````````````````````

```````````````````````````````` example
x[^1]

   [^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

    [^1]: note
.
<p>x[^1]</p>
<pre><code>[^1]: note
</code></pre>
````````````````````````````````

```````````````````````````````` example
x[^1]


[^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
> [^1]: note
> cont
.
<blockquote>
</blockquote>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: has [a](u) link
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>has <a href="u">a</a> link <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: ![alt](u)
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p><img src="u" alt="alt" /> <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: <b>bold</b>
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p><b>bold</b> <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: line one  
    line two
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>line one<br />
line two <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: line one
    line two
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>line one
line two <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note

  ---
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<hr />
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note

  # head
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<h1>head</h1>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note

  ```
  code
  ```
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<pre><code>code
</code></pre>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
[^1]: note with [^1]
.
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note with <sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup> <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a]

[^a]: text [^a] more
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>text <sup class="footnote-ref"><a href="#fn-a" id="fnref-a-2" data-footnote-ref>1</a></sup> more <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a> <a href="#fnref-a-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-2" aria-label="Back to reference 1-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a]

[^a]: outer

  [^b]: inner

y[^b]
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup></p>
<p>y<sup class="footnote-ref"><a href="#fn-b" id="fnref-b" data-footnote-ref>2</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>outer <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
<li id="fn-b">
<p>inner <a href="#fnref-b" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a]

[^a]: y[^b]

[^b]: z
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>y<sup class="footnote-ref"><a href="#fn-b" id="fnref-b" data-footnote-ref>2</a></sup> <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
<li id="fn-b">
<p>z <a href="#fnref-b" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="2" aria-label="Back to reference 2">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````


## Label shapes

```````````````````````````````` example
x[^a b]

[^a b]: note
.
<p>x<a href="note">^a b</a></p>
````````````````````````````````

```````````````````````````````` example
x[^a   b]

[^a   b]: note
.
<p>x<a href="note">^a   b</a></p>
````````````````````````````````

```````````````````````````````` example
x[^a	b]

[^a	b]: note
.
<p>x<a href="note">^a	b</a></p>
````````````````````````````````

```````````````````````````````` example
x[^a]b]

[^a]b]: note
.
<p>x[^a]b]</p>
<p>[^a]b]: note</p>
````````````````````````````````

```````````````````````````````` example
x[\^a]

[^a]: note
.
<p>x[^a]</p>
````````````````````````````````

```````````````````````````````` example
x[^a\]b]

[^a]b]: note
.
<p>x[^a]b]</p>
<p>[^a]b]: note</p>
````````````````````````````````

```````````````````````````````` example
x[^]

[^]: note
.
<p>x<a href="note">^</a></p>
````````````````````````````````

```````````````````````````````` example
x[^ ]

[^ ]: note
.
<p>x<a href="note">^ </a></p>
````````````````````````````````

```````````````````````````````` example
x[^
.
<p>x[^</p>
````````````````````````````````

```````````````````````````````` example
x[^a[^b]

[^b]: inner
.
<p>x[^a<sup class="footnote-ref"><a href="#fn-b" id="fnref-b" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-b">
<p>inner <a href="#fnref-b" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^ABC]

[^abc]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-abc" id="fnref-abc" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-abc">
<p>note <a href="#fnref-abc" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^Straße]

[^strasse]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-strasse" id="fnref-strasse" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-strasse">
<p>note <a href="#fnref-strasse" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^Å]

[^Å]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-%C3%85" id="fnref-%C3%85" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-%C3%85">
<p>note <a href="#fnref-%C3%85" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^ a ]

[^ a ]: note
.
<p>x<a href="note">^ a </a></p>
````````````````````````````````

```````````````````````````````` example
x[^a 	 b]

[^a b]: note
.
<p>x<a href="note">^a 	 b</a></p>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note

y[^1]
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<p>y<sup class="footnote-ref"><a href="#fn-1" id="fnref-1-2" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a> <a href="#fnref-1-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-2" aria-label="Back to reference 1-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^this is a very long footnote label indeed]

[^this is a very long footnote label indeed]: note
.
<p>x<a href="note">^this is a very long footnote label indeed</a></p>
````````````````````````````````

```````````````````````````````` example
x[^a_b_c]

[^a_b_c]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-a_b_c" id="fnref-a_b_c" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a_b_c">
<p>note <a href="#fnref-a_b_c" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a|b]

[^a|b]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-a%7Cb" id="fnref-a%7Cb" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a%7Cb">
<p>note <a href="#fnref-a%7Cb" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^**a**]

[^**a**]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-**a**" id="fnref-**a**" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-**a**">
<p>note <a href="#fnref-**a**" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^<b>a</b>]

[^<b>a</b>]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-%3Cb%3Ea%3C/b%3E" id="fnref-%3Cb%3Ea%3C/b%3E" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-%3Cb%3Ea%3C/b%3E">
<p>note <a href="#fnref-%3Cb%3Ea%3C/b%3E" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^\a]

[^\a]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-%5Ca" id="fnref-%5Ca" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-%5Ca">
<p>note <a href="#fnref-%5Ca" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^&amp;]

[^&amp;]: note
.
<p>x[^&amp;]</p>
````````````````````````````````

```````````````````````````````` example
x[^  ]

[^  ]: note
.
<p>x<a href="note">^  </a></p>
````````````````````````````````

```````````````````````````````` example
[^]: note
.

````````````````````````````````

```````````````````````````````` example
x[^]

[^]: note
.
<p>x<a href="note">^</a></p>
````````````````````````````````


## References in context

```````````````````````````````` example
# Head[^1]

[^1]: note
.
<h1>Head<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></h1>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
> quote[^1]
>
> [^1]: in-quote
.
<blockquote>
<p>quote<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
</blockquote>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>in-quote <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
- item[^1]

[^1]: note
.
<ul>
<li>item<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></li>
</ul>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
- item

  [^1]: inside

x[^1]
.
<ul>
<li>
<p>item</p>
</li>
</ul>
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>inside <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
> x[^1]
>
> [^1]: note
.
<blockquote>
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
</blockquote>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
> x[^1]

[^1]: note
.
<blockquote>
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
</blockquote>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
> x[^1]
>
> [^1]: note

y[^1]
.
<blockquote>
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
</blockquote>
<p>y<sup class="footnote-ref"><a href="#fn-1" id="fnref-1-2" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a> <a href="#fnref-1-2" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1-2" aria-label="Back to reference 1-2">↩<sup class="footnote-ref">2</sup></a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
`x[^1]`

[^1]: note
.
<p><code>x[^1]</code></p>
````````````````````````````````

```````````````````````````````` example
```
x[^1]
```

[^1]: note
.
<pre><code>x[^1]
</code></pre>
````````````````````````````````

```````````````````````````````` example
	x[^1]

[^1]: note
.
<pre><code>x[^1]
</code></pre>
````````````````````````````````

```````````````````````````````` example
<http://example.com/[^1]>

[^1]: note
.
<p><a href="http://example.com/%5B%5E1%5D">http://example.com/[^1]</a></p>
````````````````````````````````

```````````````````````````````` example
[t](u "x[^1]")

[^1]: note
.
<p><a href="u" title="x[^1]">t</a></p>
````````````````````````````````

```````````````````````````````` example
[see[^1]](u)

[^1]: note
.
<p><a href="u">see<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></a></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
![^1]

[^1]: note
.
<p>!<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
![^1]

[^1]: note

[^2]: other
.
<p>!<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a](y)

[^a]: note
.
<p>x<a href="y">^a</a></p>
````````````````````````````````

```````````````````````````````` example
[l](u)[^1]

[^1]: note
.
<p><a href="u">l</a><sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a]

[^a]: note

[a]: u
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>note <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^a][]

[^a]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-a" id="fnref-a" data-footnote-ref>1</a></sup>[]</p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-a">
<p>note <a href="#fnref-a" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^*a*]

[^*a]: note
.
<p>x[^*a*]</p>
````````````````````````````````

```````````````````````````````` example
*x[^1]*

[^1]: note
.
<p><em>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></em></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
<div>
</div>

x[^1]

[^1]: note
.
<div>
</div>
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

[^1]: note   
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]   

[^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^é]

[^é]: café
.
<p>x<sup class="footnote-ref"><a href="#fn-%C3%A9" id="fnref-%C3%A9" data-footnote-ref>1</a></sup></p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-%C3%A9">
<p>café <a href="#fnref-%C3%A9" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````

```````````````````````````````` example
x[^1]

para between

[^1]: note
.
<p>x<sup class="footnote-ref"><a href="#fn-1" id="fnref-1" data-footnote-ref>1</a></sup></p>
<p>para between</p>
<section class="footnotes" data-footnotes>
<ol>
<li id="fn-1">
<p>note <a href="#fnref-1" class="footnote-backref" data-footnote-backref data-footnote-backref-idx="1" aria-label="Back to reference 1">↩</a></p>
</li>
</ol>
</section>
````````````````````````````````
