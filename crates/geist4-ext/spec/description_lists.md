# Description Lists



These examples run with the `description_lists` extension enabled.

Every expected output agrees with comrak 0.51.0, byte for byte.



## Basic term and definition

```````````````````````````````` example
Term
: Definition
.
<dl>
<dt>Term</dt>
<dd>Definition</dd>
</dl>
````````````````````````````````


## Blank line before definition

```````````````````````````````` example
Term

: Definition
.
<dl>
<dt>Term</dt>
<dd>
<p>Definition</p>
</dd>
</dl>
````````````````````````````````


## Multiple definitions for one term

```````````````````````````````` example
Term
: Definition 1
: Definition 2
.
<dl>
<dt>Term</dt>
<dd>Definition 1</dd>
<dd>Definition 2</dd>
</dl>
````````````````````````````````


## Multiple terms for one definition

```````````````````````````````` example
Term 1
Term 2
: Definition
.
<dl>
<dt>Term 1
Term 2</dt>
<dd>Definition</dd>
</dl>
````````````````````````````````


## Continuation line in definition

```````````````````````````````` example
Term
: Definition
  continued line
.
<dl>
<dt>Term</dt>
<dd>Definition
continued line</dd>
</dl>
````````````````````````````````


## Second paragraph in definition

```````````````````````````````` example
Term
: Definition

  second paragraph
.
<dl>
<dt>Term</dt>
<dd>Definitionsecond paragraph</dd>
</dl>
````````````````````````````````


## Two definitions in sequence

```````````````````````````````` example
Term
: One
  Two
: Three
.
<dl>
<dt>Term</dt>
<dd>One
Two</dd>
<dd>Three</dd>
</dl>
````````````````````````````````


## Tilde marker

```````````````````````````````` example
~ Definition of tilde
.
<p>~ Definition of tilde</p>
````````````````````````````````


## Colon without space

```````````````````````````````` example
:x no space
.
<p>:x no space</p>
````````````````````````````````


## Colon with no content

```````````````````````````````` example
: no content
.
<p>: no content</p>
````````````````````````````````


## Colon with only space

```````````````````````````````` example
: 
.
<p>:</p>
````````````````````````````````


## Definition without preceding term

```````````````````````````````` example
no term
: Definition only
.
<dl>
<dt>no term</dt>
<dd>Definition only</dd>
</dl>
````````````````````````````````


## Term break and new term

```````````````````````````````` example
Term
: def1

Term2

: def3
.
<dl>
<dt>Term</dt>
<dd>def1</dd>
<dt>Term2</dt>
<dd>
<p>def3</p>
</dd>
</dl>
````````````````````````````````


## Three definitions in sequence

```````````````````````````````` example
Term
: def1
: def2
: def3
.
<dl>
<dt>Term</dt>
<dd>def1</dd>
<dd>def2</dd>
<dd>def3</dd>
</dl>
````````````````````````````````


## Nested definition list

```````````````````````````````` example
Term
: def1
  : def2
.
<dl>
<dt>Term</dt>
<dd>
<dl>
<dt>def1</dt>
<dd>def2</dd>
</dl>
</dd>
</dl>
````````````````````````````````


## Inline markup in term and definition

```````````````````````````````` example
**Term** with *markup*
: Def with `code`
.
<dl>
<dt><strong>Term</strong> with <em>markup</em></dt>
<dd>Def with <code>code</code></dd>
</dl>
````````````````````````````````


## Two definitions

```````````````````````````````` example
Term
: def
: def2
.
<dl>
<dt>Term</dt>
<dd>def</dd>
<dd>def2</dd>
</dl>
````````````````````````````````


## Heading before definition list

```````````````````````````````` example
# Heading

Term
: def
.
<h1>Heading</h1>
<dl>
<dt>Term</dt>
<dd>def</dd>
</dl>
````````````````````````````````


## Quote block with definition

```````````````````````````````` example
> Quote
: def
.
<blockquote>
<p>Quote
: def</p>
</blockquote>
````````````````````````````````


## Code block in definition

```````````````````````````````` example
Term
: def
    code inside
.
<dl>
<dt>Term</dt>
<dd>def
code inside</dd>
</dl>
````````````````````````````````


## List in definition

```````````````````````````````` example
Term
: - list in details
.
<dl>
<dt>Term</dt>
<dd>
<ul>
<li>list in details</li>
</ul>
</dd>
</dl>
````````````````````````````````


## Definition after blank line

```````````````````````````````` example
Term
: def

: def2
.
<dl>
<dt>Term</dt>
<dd>def</dd>
<dd>def2</dd>
</dl>
````````````````````````````````


## Definition after blank line with paragraph

```````````````````````````````` example
Term
: def

: def2

def3 para
.
<dl>
<dt>Term</dt>
<dd>def</dd>
<dd>def2</dd>
</dl>
<p>def3 para</p>
````````````````````````````````


## Single definition

```````````````````````````````` example
Term
: def
.
<dl>
<dt>Term</dt>
<dd>def</dd>
</dl>
````````````````````````````````


## Definition at document start

```````````````````````````````` example
: def after nothing
.
<p>: def after nothing</p>
````````````````````````````````


## Definition after paragraphs

```````````````````````````````` example
a

b

: def for b
.
<p>a</p>
<dl>
<dt>b</dt>
<dd>
<p>def for b</p>
</dd>
</dl>
````````````````````````````````


## New paragraph in definition

```````````````````````````````` example
Term
: def
new para at col 0
.
<dl>
<dt>Term</dt>
<dd>def
new para at col 0</dd>
</dl>
````````````````````````````````


## Three definitions with breaks

```````````````````````````````` example
Term
: def

: def2
: def3
.
<dl>
<dt>Term</dt>
<dd>def</dd>
<dd>def2</dd>
<dd>def3</dd>
</dl>
````````````````````````````````


## Indented code in definition

```````````````````````````````` example
Term
: def
    indented 4
.
<dl>
<dt>Term</dt>
<dd>def
indented 4</dd>
</dl>
````````````````````````````````


## Tab after colon with space

```````````````````````````````` example
Term
:→ def with tab
.
<dl>
<dt>Term</dt>
<dd>def with tab</dd>
</dl>
````````````````````````````````


## Tab after colon without space

```````````````````````````````` example
Term
:→def tab after colon
.
<dl>
<dt>Term</dt>
<dd>def tab after colon</dd>
</dl>
````````````````````````````````

