# Disallowed raw HTML


These examples run with the `tagfilter` extension enabled. Every
expected output agrees with the reference implementation and with the
engine, byte for byte.

The filter runs at render time over raw HTML literals: each `<` whose
tag name (case-insensitively, optionally after a `/`) is one of the
blacklisted tags and is followed by whitespace, `>` or `/>` becomes
`&lt;`. The boundary whitespace is the reference's own byte class —
space, tab, carriage return and line feed — so a vertical tab or form
feed after the name does not filter, matching the reference (decision
0016 covers the class difference from the engine's content trims).


## The GFM spec example

```````````````````````````````` example
<strong> <title> <style> <em>

<blockquote>
  <xmp> is disallowed.  <XMP> is also disallowed.
</blockquote>
.
<p><strong> &lt;title> &lt;style> <em></p>
<blockquote>
  &lt;xmp> is disallowed.  &lt;XMP> is also disallowed.
</blockquote>
````````````````````````````````


## Filtered tags

```````````````````````````````` example
<title>
.
&lt;title>
````````````````````````````````

```````````````````````````````` example
<textarea>
.
&lt;textarea>
````````````````````````````````

```````````````````````````````` example
<style>
.
&lt;style>
````````````````````````````````

```````````````````````````````` example
<xmp>
.
&lt;xmp>
````````````````````````````````

```````````````````````````````` example
<iframe>
.
&lt;iframe>
````````````````````````````````

```````````````````````````````` example
<noembed>
.
&lt;noembed>
````````````````````````````````

```````````````````````````````` example
<noframes>
.
&lt;noframes>
````````````````````````````````

```````````````````````````````` example
<script>
.
&lt;script>
````````````````````````````````

```````````````````````````````` example
<plaintext>
.
&lt;plaintext>
````````````````````````````````

```````````````````````````````` example
a <script>alert(1)</script> b
.
<p>a &lt;script>alert(1)&lt;/script> b</p>
````````````````````````````````

```````````````````````````````` example
x <style>i{}</style> y
.
<p>x &lt;style>i{}&lt;/style> y</p>
````````````````````````````````

```````````````````````````````` example
q <title>t</title> r
.
<p>q &lt;title>t&lt;/title> r</p>
````````````````````````````````

```````````````````````````````` example
q <textarea>t</textarea> r
.
<p>q &lt;textarea>t&lt;/textarea> r</p>
````````````````````````````````

```````````````````````````````` example
q <xmp>t</xmp> r
.
<p>q &lt;xmp>t&lt;/xmp> r</p>
````````````````````````````````

```````````````````````````````` example
q <iframe src="u"></iframe> r
.
<p>q &lt;iframe src="u">&lt;/iframe> r</p>
````````````````````````````````

```````````````````````````````` example
q <noembed>t</noembed> r
.
<p>q &lt;noembed>t&lt;/noembed> r</p>
````````````````````````````````

```````````````````````````````` example
q <noframes>t</noframes> r
.
<p>q &lt;noframes>t&lt;/noframes> r</p>
````````````````````````````````

```````````````````````````````` example
q <plaintext>t</plaintext> r
.
<p>q &lt;plaintext>t&lt;/plaintext> r</p>
````````````````````````````````


## Case and boundary shapes

```````````````````````````````` example
<SCRIPT>
foo
</SCRIPT>
.
&lt;SCRIPT>
foo
&lt;/SCRIPT>
````````````````````````````````

```````````````````````````````` example
<ScRiPt>x</sCrIpT>
.
&lt;ScRiPt>x&lt;/sCrIpT>
````````````````````````````````

```````````````````````````````` example
<script/>
.
&lt;script/>
````````````````````````````````

```````````````````````````````` example
<script >
.
&lt;script >
````````````````````````````````

```````````````````````````````` example
<script src="a.js">
foo
</script>
.
&lt;script src="a.js">
foo
&lt;/script>
````````````````````````````````

```````````````````````````````` example
</script>
.
&lt;/script>
````````````````````````````````

```````````````````````````````` example
</script >
.
&lt;/script >
````````````````````````````````

```````````````````````````````` example
<script
.
&lt;script
````````````````````````````````

```````````````````````````````` example
<scriptx>
.
<scriptx>
````````````````````````````````

```````````````````````````````` example
< script>
.
<p>&lt; script&gt;</p>
````````````````````````````````

```````````````````````````````` example
<script/
.
<p>&lt;script/</p>
````````````````````````````````

```````````````````````````````` example
<script/x>
.
<p>&lt;script/x&gt;</p>
````````````````````````````````

```````````````````````````````` example
<script>a
</script>
<style>b
</style>
.
&lt;script>a
&lt;/script>
&lt;style>b
&lt;/style>
````````````````````````````````

```````````````````````````````` example
<div>
<script>x
</script>
</div>
.
<div>
&lt;script>x
&lt;/script>
</div>
````````````````````````````````

```````````````````````````````` example
para

<script>y</script>
.
<p>para</p>
&lt;script>y&lt;/script>
````````````````````````````````

```````````````````````````````` example
  <script>z</script>
.
  &lt;script>z&lt;/script>
````````````````````````````````

```````````````````````````````` example
    <script>z</script>
.
<pre><code>&lt;script&gt;z&lt;/script&gt;
</code></pre>
````````````````````````````````

```````````````````````````````` example
```
<script>z</script>
```
.
<pre><code>&lt;script&gt;z&lt;/script&gt;
</code></pre>
````````````````````````````````

```````````````````````````````` example
> <script>z</script>
.
<blockquote>
&lt;script>z&lt;/script>
</blockquote>
````````````````````````````````

```````````````````````````````` example
- <script>z</script>
.
<ul>
<li>
&lt;script>z&lt;/script>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
<div title="x">t</div>
.
<div title="x">t</div>
````````````````````````````````

```````````````````````````````` example
<textarea rows="2">t</textarea>
.
&lt;textarea rows="2">t&lt;/textarea>
````````````````````````````````

```````````````````````````````` example
a <SCRIPT>x</SCRIPT> b
.
<p>a &lt;SCRIPT>x&lt;/SCRIPT> b</p>
````````````````````````````````

```````````````````````````````` example
a <script/> b
.
<p>a &lt;script/> b</p>
````````````````````````````````

```````````````````````````````` example
a <script src="s.js">x</script> b
.
<p>a &lt;script src="s.js">x&lt;/script> b</p>
````````````````````````````````

```````````````````````````````` example
a <scriptx> b
.
<p>a <scriptx> b</p>
````````````````````````````````

```````````````````````````````` example
a <script b
.
<p>a &lt;script b</p>
````````````````````````````````

```````````````````````````````` example
a < b
.
<p>a &lt; b</p>
````````````````````````````````

```````````````````````````````` example
a <b c> d
.
<p>a <b c> d</p>
````````````````````````````````

```````````````````````````````` example
<style>a</style><script>b</script>
.
&lt;style>a&lt;/style>&lt;script>b&lt;/script>
````````````````````````````````

```````````````````````````````` example
*<script>x</script>*
.
<p><em>&lt;script>x&lt;/script></em></p>
````````````````````````````````

```````````````````````````````` example
[t](u) <script>x</script>
.
<p><a href="u">t</a> &lt;script>x&lt;/script></p>
````````````````````````````````

```````````````````````````````` example
a `<script>` b
.
<p>a <code>&lt;script&gt;</code> b</p>
````````````````````````````````

```````````````````````````````` example
x  
<script>y</script>
.
<p>x</p>
&lt;script>y&lt;/script>
````````````````````````````````

```````````````````````````````` example
&lt;script&gt;
.
<p>&lt;script&gt;</p>
````````````````````````````````

```````````````````````````````` example
\<script>x</script>
.
<p>&lt;script&gt;x&lt;/script></p>
````````````````````````````````

```````````````````````````````` example
<script>x</script>
.
<script>x&lt;/script>
````````````````````````````````

```````````````````````````````` example
<style>x</style>
.
<style>x&lt;/style>
````````````````````````````````
