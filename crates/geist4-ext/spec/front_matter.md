# Front matter

Reference-verified examples: the source and HTML of every case
agree byte for byte with the comrak 0.51.0 library parser, the
oracle recorded in `docs/decisions/0018-footnotes-follow-comrak.md`.
The front matter extension claims a delimited prologue at the
document's beginning; the prologue renders as nothing.

Unlike the reference implementation, which re-parses the source
after the prologue and shifts every offset, this engine claims the
prologue by byte offset, so source spans after the prologue stay
true to the original input. The HTML is unaffected.

## Basic shapes

### 1. basic

```````````````````````````````` example
---
title: hi
---

# Hello
.
<h1>Hello</h1>
````````````````````````````````

### 2. no blank after closer

```````````````````````````````` example
---
a: b
---
text
.
<p>text</p>
````````````````````````````````

### 3. blank after closer

```````````````````````````````` example
---
a: b
---

text
.
<p>text</p>
````````````````````````````````

### 4. empty output

```````````````````````````````` example
---
a: b
---
.

````````````````````````````````

## Split rules

### 5. unclosed

```````````````````````````````` example
---
a: b
text
.
<hr />
<p>a: b
text</p>
````````````````````````````````

### 6. empty body does not close

```````````````````````````````` example
---
---
text
.
<hr />
<hr />
<p>text</p>
````````````````````````````````

### 7. opener line only

```````````````````````````````` example
---
.
<hr />
````````````````````````````````

### 8. closing line only

```````````````````````````````` example
---
---
.
<hr />
<hr />
````````````````````````````````

### 9. indented opener

```````````````````````````````` example
 ---
a: b
---
text
.
<hr />
<h2>a: b</h2>
<p>text</p>
````````````````````````````````

### 10. longer opener

```````````````````````````````` example
----
a: b
----
text
.
<hr />
<h2>a: b</h2>
<p>text</p>
````````````````````````````````

### 11. two dashes

```````````````````````````````` example
--
a: b
--
text
.
<h2>--
a: b</h2>
<p>text</p>
````````````````````````````````

### 12. trailing space on opener

```````````````````````````````` example
--- 
a: b
---
text
.
<hr />
<h2>a: b</h2>
<p>text</p>
````````````````````````````````

### 13. text on opener line

```````````````````````````````` example
---text
.
<p>---text</p>
````````````````````````````````

### 14. delimiter inside body line

```````````````````````````````` example
---
a: b

c: ---
---
text
.
<p>text</p>
````````````````````````````````

### 15. second block is not front matter

```````````````````````````````` example
---
a: b
---

text

---
more: c
---
.
<p>text</p>
<hr />
<h2>more: c</h2>
````````````````````````````````

### 16. blank body

```````````````````````````````` example
---

---
text
.
<p>text</p>
````````````````````````````````

### 17. byte order mark

```````````````````````````````` example
﻿---
a: b
---
text
.
<p>text</p>
````````````````````````````````

### 18. four dash closer

```````````````````````````````` example
---
a
----
text
.
<hr />
<h2>a</h2>
<p>text</p>
````````````````````````````````

### 19. space in closer

```````````````````````````````` example
---
a
--- 
text
.
<hr />
<h2>a</h2>
<p>text</p>
````````````````````````````````

### 20. indented closer

```````````````````````````````` example
---
a
 ---
text
.
<hr />
<h2>a</h2>
<p>text</p>
````````````````````````````````

### 21. one body line

```````````````````````````````` example
---
abc
---
.

````````````````````````````````

### 22. only blank body lines

```````````````````````````````` example
---


---
.

````````````````````````````````

### 23. indented body

```````````````````````````````` example
---
  a: b
---
text
.
<p>text</p>
````````````````````````````````

### 24. non-ascii body

```````````````````````````````` example
---
ключ: значение
---

tекст
.
<p>tекст</p>
````````````````````````````````

### 25. emoji body

```````````````````````````````` example
---
icon: 🚀
---

🚀 launch
.
<p>🚀 launch</p>
````````````````````````````````

### 26. toml delimiter

```````````````````````````````` example
+++
a = 1
+++
text
.
<p>+++
a = 1
+++
text</p>
````````````````````````````````

## Interaction

### 27. heading after

```````````````````````````````` example
---
lang: en
---

## Head

body
.
<h2>Head</h2>
<p>body</p>
````````````````````````````````

### 28. list after

```````````````````````````````` example
---
step: 2
---

- one
- two
.
<ul>
<li>one</li>
<li>two</li>
</ul>
````````````````````````````````

### 29. code fence containing the delimiter

```````````````````````````````` example
---
code: 3
---

```
---
```
.
<pre><code>---
</code></pre>
````````````````````````````````

### 30. block quote after

```````````````````````````````` example
---
q: 4
---

> quoted
.
<blockquote>
<p>quoted</p>
</blockquote>
````````````````````````````````

### 31. raw html after

```````````````````````````````` example
---
h: 5
---

<div>raw</div>
.
<div>raw</div>
````````````````````````````````

### 32. thematic break after

```````````````````````````````` example
---
seven: 7
---

---

after
.
<hr />
<p>after</p>
````````````````````````````````

### 33. two prologues in a row

```````````````````````````````` example
---
a: b
---
---
text
.
<hr />
<p>text</p>
````````````````````````````````

### 34. blank line before opener

```````````````````````````````` example

---
a: b
---
text
.
<hr />
<h2>a: b</h2>
<p>text</p>
````````````````````````````````
