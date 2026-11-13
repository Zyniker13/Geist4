#!/usr/bin/env python3
"""Generate the benchmark corpus: deterministic real-world-style
documents, checked in under bench/corpus/ with a manifest of SHA-256
hashes so the benchmark set is pinned and reproducible. Rerunning the
script must be a no-op diff.

The documents imitate the shapes a Markdown renderer sees in practice:
prose paragraphs, headings, lists, quotes, fenced code, links, inline
emphasis, tables of contents, and long reference-definition sections.
"""
import hashlib
import pathlib
import random
import sys

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent / "bench" / "corpus"

WORDS = (
    "the quick parser renders markdown with an emphasis on correctness "
    "speed follows and the specification is the contract every example "
    "passes byte for byte without special cases or skipped input text "
    "links images quotes code and lists arrive from untrusted authors "
    "so the engine bounds its work and never panics on arbitrary bytes"
).split()

HEADINGS = (
    "Overview", "Design", "Installation", "Usage", "Examples",
    "Configuration", "Advanced topics", "Frequently asked questions",
    "See also", "History", "Implementation notes", "Benchmarks",
)


def sentence(rng: "random.Random", n: int) -> str:
    words = [WORDS[rng.randrange(len(WORDS))] for _ in range(n)]
    text = " ".join(words)
    return text[0].upper() + text[1:] + "."


def paragraph(rng: "random.Random") -> str:
    return " ".join(sentence(rng, 4 + rng.randrange(12)) for _ in range(2 + rng.randrange(4)))


def code_block(rng: "random.Random", i: int) -> str:
    lines = [f"let value_{i} = compute({k});" for k in range(3 + rng.randrange(4))]
    return "```rust\n" + "\n".join(lines) + "\n```"


def list_block(rng: "random.Random") -> str:
    out = []
    for k in range(3 + rng.randrange(5)):
        out.append(f"- item {k}: {sentence(rng, 3 + rng.randrange(8))}")
    return "\n".join(out)


def quote_block(rng: "random.Random") -> str:
    return "> " + paragraph(rng)


def reference_section(rng: "random.Random", n: int) -> str:
    out = []
    for k in range(n):
        out.append(f"[ref{k}]: https://example.com/{k} \"Reference {k}\"")
    return "\n".join(out)


def paragraph_with_links(rng: "random.Random") -> str:
    base = paragraph(rng)
    return base + f" See [ref{rng.randrange(20)}] and [ref{rng.randrange(20)}] for details."


def document(seed: int) -> str:
    rng = random.Random(seed)
    parts = [f"# {HEADINGS[seed % len(HEADINGS)]}", ""]
    parts.append(paragraph(rng))
    parts.append("")
    parts.append(paragraph_with_links(rng))
    parts.append("")
    parts.append(f"## {HEADINGS[(seed + 3) % len(HEADINGS)]}")
    parts.append("")
    parts.append(list_block(rng))
    parts.append("")
    parts.append(quote_block(rng))
    parts.append("")
    parts.append(code_block(rng, seed))
    parts.append("")
    parts.append(paragraph(rng))
    parts.append("")
    parts.append(f"## {HEADINGS[(seed + 7) % len(HEADINGS)]}")
    parts.append("")
    for k in range(2 + seed % 3):
        parts.append(paragraph_with_links(rng))
        parts.append("")
    parts.append(reference_section(rng, 10 + (seed % 3) * 10))
    return "\n".join(parts) + "\n"


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = []
    for i in range(1, 41):
        path = OUT / f"doc-{i:03d}.md"
        text = document(i)
        path.write_text(text)
        digest = hashlib.sha256(text.encode()).hexdigest()
        manifest.append(f"{digest}  {path.name}")
    (OUT / "MANIFEST.sha256").write_text("\n".join(manifest) + "\n")
    print(f"wrote {len(manifest)} documents and the manifest under {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
