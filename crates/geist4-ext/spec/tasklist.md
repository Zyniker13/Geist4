# Task list items


These examples run with the `tasklist` extension enabled. Every
expected output agrees with the reference implementation and with the
engine, byte for byte.

The reference's tasklist scanner takes the required whitespace after
the box from its scanner class, so a vertical tab or form feed counts;
the engine matches that at the box. Whitespace at the *ends* of the
item's content follows the core's CommonMark 0.31.2 whitespace
definition instead, which trims vertical tab and form feed — the
cmark-gfm fork carries an older table that keeps them, so its output
differs from these examples on an item whose content ends in one of
those bytes (decision 0016).


## Task list items

```````````````````````````````` example
- [ ] foo
- [x] bar
.
<ul>
<li><input type="checkbox" disabled="" /> foo</li>
<li><input type="checkbox" checked="" disabled="" /> bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

- [X] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" checked="" disabled="" /> 
<p>bar</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

- [x] bar

- [ ] baz
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" checked="" disabled="" /> 
<p>bar</p>
</li>
<li><input type="checkbox" disabled="" /> 
<p>baz</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [ ] foo

- [x] bar
.
<ul>
<li><input type="checkbox" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" checked="" disabled="" /> 
<p>bar</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]
- [ ] foo
.
<ul>
<li>[x]</li>
<li><input type="checkbox" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
- [x] bar
- [ ] baz
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
<li><input type="checkbox" checked="" disabled="" /> bar</li>
<li><input type="checkbox" disabled="" /> baz</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [ ] foo

- [x] bar

- [ ] baz
.
<ul>
<li><input type="checkbox" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" checked="" disabled="" /> 
<p>bar</p>
</li>
<li><input type="checkbox" disabled="" /> 
<p>baz</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
  - [ ] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo
<ul>
<li><input type="checkbox" disabled="" /> bar</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
foo
- [x] bar
- [ ] baz
.
<p>foo</p>
<ul>
<li><input type="checkbox" checked="" disabled="" /> bar</li>
<li><input type="checkbox" disabled="" /> baz</li>
</ul>
````````````````````````````````


## Marker and box forms

```````````````````````````````` example
* [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
+ [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
1. [x] foo
.
<ol>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ol>
````````````````````````````````

```````````````````````````````` example
1) [x] foo
.
<ol>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ol>
````````````````````````````````

```````````````````````````````` example
12) [x] foo
.
<ol start="12">
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ol>
````````````````````````````````

```````````````````````````````` example
123. [ ] foo
.
<ol start="123">
<li><input type="checkbox" disabled="" /> foo</li>
</ol>
````````````````````````````````

```````````````````````````````` example
12 [ ] foo
.
<p>12 [ ] foo</p>
````````````````````````````````

```````````````````````````````` example
- [X] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
* [X] foo
- [x] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
<ul>
<li><input type="checkbox" checked="" disabled="" /> bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
-  [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
-	[x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
 -	[x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
 - [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
   - [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [ ]   foo
.
<ul>
<li><input type="checkbox" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]  foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo   
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] 
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> </li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]	
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> </li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
````````````````````````````````


## What is not a task list

```````````````````````````````` example
- [y] foo
.
<ul>
<li>[y] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [Y] foo
.
<ul>
<li>[Y] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [] foo
.
<ul>
<li>[] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]foo
.
<ul>
<li>[x]foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]
.
<ul>
<li>[x]</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- foo

  [x] bar
.
<ul>
<li>
<p>foo</p>
<p>[x] bar</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
-
  [x] foo
.
<ul>
<li>[x] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
> - [x] foo
.
<blockquote>
<ul>
<li>[x] foo</li>
</ul>
</blockquote>
````````````````````````````````

```````````````````````````````` example
1x [ ] foo
.
<p>1x [ ] foo</p>
````````````````````````````````

```````````````````````````````` example
1x [x] foo
- [ ] bar
.
<p>1x [x] foo</p>
<ul>
<li><input type="checkbox" disabled="" /> bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] [x] not a box
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [x] not a box</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] [x foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [x foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] [[x]]
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [[x]]</li>
</ul>
````````````````````````````````


## The checked state

```````````````````````````````` example
- [ ] [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [x] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [ ] [X] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [X] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [ ] [x] done [x]
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [x] done [x]</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [ ] done [x] now
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> done [x] now</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] [x] foo
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> [x] foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] **bold [x] bold**
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <strong>bold [x] bold</strong></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] [link](http://example.com) and [x]
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <a href="http://example.com">link</a> and [x]</li>
</ul>
````````````````````````````````


## Structure around items

```````````````````````````````` example
- [ ] foo
  - [x] bar
    - [ ] baz
.
<ul>
<li><input type="checkbox" disabled="" /> foo
<ul>
<li><input type="checkbox" checked="" disabled="" /> bar
<ul>
<li><input type="checkbox" disabled="" /> baz</li>
</ul>
</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

  - [x] bar

    - [ ] baz
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>bar</p>
<ul>
<li><input type="checkbox" disabled="" /> baz</li>
</ul>
</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

- [ ] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" disabled="" /> 
<p>bar</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]

  foo
.
<ul>
<li>
<p>[x]</p>
<p>foo</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
- [ ] bar

- [ ] baz
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" disabled="" /> 
<p>bar</p>
</li>
<li><input type="checkbox" disabled="" /> 
<p>baz</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

text

- [ ] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
<p>text</p>
<ul>
<li><input type="checkbox" disabled="" /> bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

  > - [ ] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
<blockquote>
<ul>
<li>[ ] bar</li>
</ul>
</blockquote>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

      bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
<pre><code>bar
</code></pre>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo

  <div>x</div>
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
<div>x</div>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
- bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
<li>bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
-
- [ ] bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
<li></li>
<li><input type="checkbox" disabled="" /> bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
    - [x] foo
.
<pre><code>- [x] foo
</code></pre>
````````````````````````````````

```````````````````````````````` example
1. [x] foo
2. [ ] bar
3. [x] baz
.
<ol>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
<li><input type="checkbox" disabled="" /> bar</li>
<li><input type="checkbox" checked="" disabled="" /> baz</li>
</ol>
````````````````````````````````

```````````````````````````````` example
5. [x] foo
.
<ol start="5">
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ol>
````````````````````````````````

```````````````````````````````` example
1) [x] foo
2. [ ] bar
.
<ol>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ol>
<ol start="2">
<li><input type="checkbox" disabled="" /> bar</li>
</ol>
````````````````````````````````

```````````````````````````````` example
1. [x] foo

2. [ ] bar
.
<ol>
<li><input type="checkbox" checked="" disabled="" /> 
<p>foo</p>
</li>
<li><input type="checkbox" disabled="" /> 
<p>bar</p>
</li>
</ol>
````````````````````````````````

```````````````````````````````` example
- [x] foo
  bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo
bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a

  - [x] b

    - [x] c
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>a</p>
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>b</p>
<ul>
<li><input type="checkbox" checked="" disabled="" /> c</li>
</ul>
</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a
  - [x] b
    - [x] c
      - [x] d
        - [x] e
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a
<ul>
<li><input type="checkbox" checked="" disabled="" /> b
<ul>
<li><input type="checkbox" checked="" disabled="" /> c
<ul>
<li><input type="checkbox" checked="" disabled="" /> d
<ul>
<li><input type="checkbox" checked="" disabled="" /> e</li>
</ul>
</li>
</ul>
</li>
</ul>
</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a

  - [ ] b

    - [x] c
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>a</p>
<ul>
<li><input type="checkbox" disabled="" /> 
<p>b</p>
<ul>
<li><input type="checkbox" checked="" disabled="" /> c</li>
</ul>
</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
1. [x] a
   1. [ ] b
      1. [x] c
.
<ol>
<li><input type="checkbox" checked="" disabled="" /> a
<ol>
<li><input type="checkbox" disabled="" /> b
<ol>
<li><input type="checkbox" checked="" disabled="" /> c</li>
</ol>
</li>
</ol>
</li>
</ol>
````````````````````````````````

```````````````````````````````` example
- [x] a
* [ ] b
+ [x] c
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
</ul>
<ul>
<li><input type="checkbox" disabled="" /> b</li>
</ul>
<ul>
<li><input type="checkbox" checked="" disabled="" /> c</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a

* [ ] b

+ [x] c
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
</ul>
<ul>
<li><input type="checkbox" disabled="" /> b</li>
</ul>
<ul>
<li><input type="checkbox" checked="" disabled="" /> c</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a
1. [ ] b
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
</ul>
<ol>
<li><input type="checkbox" disabled="" /> b</li>
</ol>
````````````````````````````````

```````````````````````````````` example
- [x] - not a list
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> - not a list</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] # not a heading
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> # not a heading</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] ``` not a fence
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> ``` not a fence</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo
bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
# bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo</li>
</ul>
<h1>bar</h1>
````````````````````````````````

```````````````````````````````` example
- [x]
  foo
.
<ul>
<li>[x]
foo</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]

  foo
.
<ul>
<li>
<p>[x]</p>
<p>foo</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- a

  - [x] b
.
<ul>
<li>
<p>a</p>
<ul>
<li><input type="checkbox" checked="" disabled="" /> b</li>
</ul>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
> foo

- [x] bar
.
<blockquote>
<p>foo</p>
</blockquote>
<ul>
<li><input type="checkbox" checked="" disabled="" /> bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a

    [ ] b
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 
<p>a</p>
<p>[ ] b</p>
</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x]

next
.
<ul>
<li>[x]</li>
</ul>
<p>next</p>
````````````````````````````````

```````````````````````````````` example
- [x] a

```
fence
```
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
</ul>
<pre><code>fence
</code></pre>
````````````````````````````````

```````````````````````````````` example
- [x] a

# heading
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
</ul>
<h1>heading</h1>
````````````````````````````````

```````````````````````````````` example
- [x] a

---
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
</ul>
<hr />
````````````````````````````````


## Inline content

```````````````````````````````` example
- [x] <b>bold</b>
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <b>bold</b></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a &amp; b
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a &amp; b</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] <http://example.com>
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <a href="http://example.com">http://example.com</a></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a `code` b
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a <code>code</code> b</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] *em* _em_
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <em>em</em> <em>em</em></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] _a_
__b__
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <em>a</em>
<strong>b</strong></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] ![alt](http://example.com/i.png)
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <img src="http://example.com/i.png" alt="alt" /></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] ![alt][foo]

[foo]: http://example.com/i.png
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> <img src="http://example.com/i.png" alt="alt" /></li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] [foo]: http://example.com "t"

[foo]
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> </li>
</ul>
<p><a href="http://example.com" title="t">foo</a></p>
````````````````````````````````

```````````````````````````````` example
- [x] foo  
bar
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo<br />
bar</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo\
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> foo\</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] 1. more
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 1. more</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] aaaa bbbb cccc dddd eeee ffff gggg
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> aaaa bbbb cccc dddd eeee ffff gggg</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] a
- [ ] b
- [x] c
- [ ] d
- [x] e
- [ ] f
- [x] g
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> a</li>
<li><input type="checkbox" disabled="" /> b</li>
<li><input type="checkbox" checked="" disabled="" /> c</li>
<li><input type="checkbox" disabled="" /> d</li>
<li><input type="checkbox" checked="" disabled="" /> e</li>
<li><input type="checkbox" disabled="" /> f</li>
<li><input type="checkbox" checked="" disabled="" /> g</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [X]  spaced  content
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> spaced  content</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] résumé — dashes ✓
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> résumé — dashes ✓</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] 中文内容测试
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> 中文内容测试</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] done ✓ done
.
<ul>
<li><input type="checkbox" checked="" disabled="" /> done ✓ done</li>
</ul>
````````````````````````````````

```````````````````````````````` example
- [x] foo
.
<ul>
<li>[x] foo</li>
</ul>
````````````````````````````````
