## Problem

Agents can't extract failure names cheaply, so truncated terminal output motivates full re-runs. In trajectory `2026-08-27T12:29:02` steps 48→49 cost ~300s because the only machine-readable artifact of a 2301-file vitest run was an ANSI summary tail.

## Recommendation

Document (and standardize) structured-report invocations per runner so post-hoc queries never need re-execution:

- **vitest**: `vp test --run --reporter=json --outputFile=$SESSION_DIR/vitest.json`; failures via `jq '.testResults[].assertionResults[] | select(.status=="failed") | .fullName'`.
- **cargo**: `cargo test -- --format json -Z unstable-options` or `cargo test -- --format terse` plus JUnit (`--reporter junit` where available / `-o results.xml`).
- Persist the report path into `$SESSION_DIR/cmd-N.log`/`.json` alongside raw output (pairs with the long-output persistence issue).

Wrap as a helper skill stanza so models use it by default instead of grep-piping live runs.

## Acceptance criteria

- [ ] Skill/system-prompt snippet lists canonical report commands + jq extraction for each runner used in this repo.
- [ ] A failing suite produces a queryable report file; a follow-up question about which tests failed is answerable without executing anything.
