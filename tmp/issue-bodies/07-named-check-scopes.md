## Problem

Distinguishing verification scopes ("is my diff green") from baseline sweeps ("does main cross-build") is currently ad-hoc. The full-suite commands used on `2026-08-27T12:29:02` conflated those jobs, so proving step-50-style results (the 11 failures were pre-existing) required stash-and-rerun gymnastics.

## Recommendation

Add first-class harness affordances separating diff-scoped verification from repo-level sweeps:

- A named repo config (e.g. `.openagents/checks.json` or package.json key) mapping scopes to runnable suites, e.g. `{ "diff": ["vp test --run $(git diff --name-only HEAD | grep .test.)"], "package": [...], "full": [...] }`.
- Tool-layer helper the model calls by scope name rather than assembling raw suite invocations mid-loop.
- Baseline mode runs against stashed/clean state cheaply once (not repeatedly), caching "pre-existing failures" lists so mid-loop failures can be classified as mine vs inherited without another sweep.

This is the substrate that makes the escalation-ladder policy enforceable rather than aspirational.

## Acceptance criteria

- [ ] Repo declares named check scopes the coder can invoke directly.
- [ ] A failing full-suite run leaves behind a machine-readable "known-failures at HEAD" artifact usable later for attribution (mine vs pre-existing).
- [ ] Mid-loop attribution questions resolve via cached baseline data instead of another suite execution.
