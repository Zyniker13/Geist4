# 0003 — Spec version policy

## Context
The engine must implement CommonMark 0.31.2. The vendored spec file must stay in sync with the
official spec.

## Decision
Upgrading the CommonMark spec is a decision-record item and a milestone task, never a silent
change. The spec file is vendored at `spec/spec.txt` with provenance recorded in
`spec/README.md`. The core test (`tests/spec.rs`) references this decision record so that
upgrades are never accidental.

## Consequences
Any spec upgrade requires: updating `spec/spec.txt`, updating `spec/progress.json`, updating
`spec/README.md` with the new version/commit, updating the decision record, and updating
`ROADMAP.md` and `STATUS.md`. The upgrade must pass all 652 examples before release.
