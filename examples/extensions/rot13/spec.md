# rot13

Marks text with `%percent signs%` and renders every letter rotated
thirteen places. A tier-2 (WebAssembly) extension: the guest component
scans a delimiter run at the registered `%` trigger byte, the engine
pairs matching runs with its standard machinery, and the guest's
`render` hook ROT13-encodes the content — logic the declarative tier
cannot express. The guest is built from `guest/` (Rust, via
wit-bindgen); the component is checked in at `rot13.wasm`.

    geist4 ext check examples/extensions/rot13

## Examples

Plain letters rotate; digits and punctuation stand.

```````````````````````````````` example
The secret is %uryyb%.
.
<p>The secret is <rot13>hello</rot13>.</p>
````````````````````````````````

Round-tripping: encode, then decode by applying it again.

```````````````````````````````` example
%Uryyb jbeyq%! and %hello world%!
.
<p><rot13>Hello world</rot13>! and <rot13>uryyb jbeyq</rot13>!</p>
````````````````````````````````

Words, digits and punctuation inside the span.

```````````````````````````````` example
%Shipped 13 times — nice%!
.
<p><rot13>Fuvccrq 13 gvzrf — avpr</rot13>!</p>
````````````````````````````````

Emphasis inside the span: the guest rotates the text and leaves the
engine's tags alone.

```````````````````````````````` example
%*mixed* content%
.
<p><rot13><em>zvkrq</em> pbagrag</rot13></p>
````````````````````````````````

An unpaired `%` stays literal text.

```````````````````````````````` example
50% of a %100
.
<p>50% of a %100</p>
````````````````````````````````
