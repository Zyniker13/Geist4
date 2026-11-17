# shout

Fences a block with `::: shout` and renders its content uppercased.
A tier-2 (WebAssembly) extension written in **C**: the guest component
opens the container at the marker line, keeps every non-blank line as
content, and its `render` hook uppercases the letters — logic the
declarative tier cannot express.

The guest is built from `guest/` (C, via wit-bindgen's C bindings):

    wit-bindgen c crates/geist4-ext-loader/wit/geist4-extension.wit \
      --out-dir examples/extensions/shout/guest
    clang --target=wasm32-unknown-unknown -O2 -nostdlib \
      -isystem examples/extensions/shout/guest/compat \
      -Wl,--no-entry -Wl,--export-dynamic \
      examples/extensions/shout/guest/shout.c \
      examples/extensions/shout/guest/shim.c \
      examples/extensions/shout/guest/geist4_extension.c \
      examples/extensions/shout/guest/geist4_extension_component_type.o \
      -o shout-core.wasm
    wasm-tools component new shout-core.wasm -o examples/extensions/shout/shout.wasm

The component is checked in at `shout.wasm`.

    geist4 ext check examples/extensions/shout

## Examples

A paragraph inside the container.

```````````````````````````````` example
::: shout
hey there
:::
.
<div class="shout">
<p>HEY THERE</p>
</div>
````````````````````````````````

Emphasis inside: the guest uppercases the text and leaves the
engine's tags alone.

```````````````````````````````` example
::: shout
*em* and **strong**
:::
.
<div class="shout">
<p><em>EM</em> AND <strong>STRONG</strong></p>
</div>
````````````````````````````````

A list inside the container.

```````````````````````````````` example
::: shout
- one
- two
:::
.
<div class="shout">
<ul>
<li>ONE</li>
<li>TWO</li>
</ul>
</div>
````````````````````````````````

The container closes at the fence; the following paragraph is
outside it.

```````````````````````````````` example
::: shout
hi
:::

after
.
<div class="shout">
<p>HI</p>
</div>
<p>after</p>
````````````````````````````````

A different marker is plain text.

```````````````````````````````` example
::: shoutbox
x
:::
.
<p>::: shoutbox
x
:::</p>
````````````````````````````````
