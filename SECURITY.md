# Security

## Reporting a Vulnerability

Please report security problems privately through GitHub's [private vulnerability reporting](../../security/advisories/new) for this repository, not in a public issue. Include the input that triggers the problem and the version or commit you tested.

## What Counts

The engine is meant to process untrusted Markdown safely. Any of these is a security bug:

- a panic, crash, hang, or super-linear running time on any input;
- unbounded memory use on any input;
- output that lets script run in a page under the default safe settings (for example a `javascript:` link, or raw HTML passed through when the policy says to escape it);
- an extension escaping its sandbox, or exceeding its time or memory limits without being stopped.

The threat model and the defaults are documented in `docs/security-model.md` (from milestone M3).
