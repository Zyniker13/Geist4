# Working in This Repository

This guide is for anyone, human or agent, changing this repository. Read `ROADMAP.md` and `STATUS.md` first: they say what the project is and where the work stands.

## Priorities

Correctness, then security, then speed, then features. Never trade a spec example for speed.

## Rules

- Run `scripts/check.sh` before every push. It runs exactly what CI runs; keep the two identical.
- Every change that alters behavior comes with tests (see `TESTING.md`), and every bug fix starts with a regression test.
- Keep the core free of `unsafe`, regular expressions, I/O and global state (see the Fixed Choices in `ROADMAP.md`).
- Record each significant decision in `docs/decisions/` (Context, Decision, Consequences), numbered in order.
- Keep `STATUS.md` current: current milestone, last done, next step, blocked on, and a log line.
- Work on `main` in small commits that keep CI green. Never force-push or rewrite pushed history.
- Add dependencies only from crates.io and npm, prefer well-known maintained ones, and make sure `cargo deny check` passes. Record any security-relevant dependency in a decision record.
- Pin GitHub Actions to full commit SHAs and keep workflow permissions read-only.

## Public Repository

Everything here is public. Never commit secrets of any kind, email addresses, hostnames, IP addresses, local paths outside the repository, or anything about the machines or services used to develop it. Test data uses obviously fake values (`alice`, `example.com`). Commit messages describe the code only.
