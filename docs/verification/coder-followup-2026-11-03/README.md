# Coder follow-up qualification, 2026-11-03

This implements the four follow-ups to the repository pilot: availability-aware
automatic routing, observed Codex tool/model/usage records, structural coding
fixtures, and interruption after real effects. Product behavior and boundaries
are specified in [observability v1](../../protocol/coder-observability-v1.md).
Commands and case descriptions are in [the pilot guide](../../coder/repository-pilot.md#structural-follow-up-suite).

## Results

| Qualification | Result |
| --- | --- |
| Combined Coder and observability checks | 417/417 pass; the subsequently added low-confidence routing regression also passes |
| Pure availability policy, mutation | 89/89 killed, zero survivors; no equivalent markings |
| Structural fixture references | All four begin failing and pass external acceptance after the reference repair |
| Synthetic provider protocol through actual CLI | Typed provider failure persists; reported reset retained; automatic route and run refuse dispatch; explicit retry starts another process; invalid allowlist closes |
| SIGKILL after non-idempotent append | Effect remains exactly `x`; status unknown; replay inert; explicit read-only fork preserves bytes |
| SIGKILL during partial edit | Effect remains `partial`; status unknown; replay does not complete the edit; explicit read-only fork preserves bytes |
| Live Codex / decimal parser | Pass, 23 tool starts recorded; resolved `gpt-6.1-sol`, effort `xhigh`; cumulative usage snapshots observed |
| Live Microcoder / epoch state machine | Pass after fixture wording correction; model, effort, usage and cost unobserved |
| Book | 16 programs, 109 source-linked listings, local links pass; all 32 executions match |
| New protobuf source | Standalone descriptor compilation passes with installed protoc 36.2 |
| Tools gate | Existing repository failures remain; see [tools-check.txt](tools-check.txt) |

The pinned complete protocol gate was not qualified: this environment has
protoc 36.2 rather than the repository's required 29.3, and its default Python
lacks the pinned protobuf package. The new source was compiled independently;
that does not substitute for the complete pinned gate.

The initial Microcoder state task said rejection must preserve “state,” which
conflicted with resetting its `accepted` flag. That run is retained in
`invalidated-micro-epochs/`, explicitly excluded from scoring. The task now
requires preserving epoch and sequence while returning a flag for the current
call. The corrected run passes. No passing live measurement was repeated.
The decimal reference initially exposed an unsupported escape in its acceptance
fixture; that fixture was corrected and only its failed qualification rerun.

## Evidence and limits

`codex-decimal/` and `micro-epochs/` contain synthetic task transcripts, final
source, receipts and compact observations. Usage is a cumulative thread total,
including cached input, not a bill or a sum of snapshots. Cost is unknown.
These two different tasks do not compare the engines. Case 2 is reference-only;
case 3's live continuation remains unused as a reserved public case. No private
project conversation was used.

`availability/` contains the synthetic failure, refusal, routing masks and the
two-start counter. No live provider quota was used for that test. `append/`
and `partial-edit/` preserve actual interrupted bytes and recovery observations.
Reference receipts are in `reference/`. Exactly-once execution, atomic edits,
power-loss recovery and provider-internal retries remain unqualified.

Work began from `38b219dbaf` in a separate worktree. Live smoke runs were taken
during implementation, before later metadata identifiers and diagnostic
refinements. They are development observations, not a frozen comparative
benchmark. Their raw transcript hashes are in each observations file.
[`delivered-source.sha256`](delivered-source.sha256) pins the final delivered
sources and fixtures; it does not retroactively pin those earlier executions.
The unchanged qualified compiler SHA-256 is
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
The v1 pilot source and historic receipts remain untouched.

Larger real repositories, repeated trials, private holdouts, long conversation
stress tests and additional provider reset adapters remain further work. This
change supplies usable checks and observations without claiming those results.
