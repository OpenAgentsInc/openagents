# Khala CLI And Own-Capacity Promise Reconciliation

Date: 2026-06-27

Inputs:

- `docs/transcripts/242.md`
- `docs/transcripts/243.md`
- `docs/transcripts/244.md`
- `clients/khala-cli/README.md`
- `clients/khala-cli/src/changelog.ts`
- `docs/khala/2026-06-25-pylon-linked-coding-capacity-routing-spec.md`
- `docs/afteraction/2026-06-26-khala-pylon-codex-delegation-afteraction.md`
- `docs/traces/2026-06-27-pylon-codex-live-trace-status-audit.md`

## Registry Updates

`2026-06-27.1` adds scoped green records for:

- `inference.khala_free_openai_compatible_api.v1`
- `metrics.khala_tokens_served_public.v1`
- `khala.cli_terminal_client.v1`

It adds conservative yellow records for:

- `metrics.khala_model_family_mix_public.v1`
- `khala.own_capacity_codex_delegation.v1`
- `data.khala_free_tier_trace_capture.v1`
- `privacy.khala_paid_capture_optout.v1`

## Owner-Capacity Boundary

The Khala -> Pylon -> Codex claim is intentionally narrow. It covers explicit
typed delegation only:

```sh
pylon khala request --workflow codex_agent_task --pylon-ref <caller-owned-pylon>
pylon assignment run-no-spend --assignment-ref <assignment-ref> --json
```

Green proof for a particular assignment requires an accepted closeout and exact
`token_usage_events` rows with:

- `provider = pylon-codex-own-capacity`
- `model = openagents/pylon-codex`
- `usage_truth = exact`
- `demand_kind = own_capacity`
- `demand_source = khala_coding_delegation`

The public headline token counter includes those exact rows after closeout, but
counter movement alone is not assignment proof.

Fresh dummy runbook delegation from this reconciliation pass, 2026-06-27:

- Pylon preflight: `pylon.33afd48282a649047e3a`, Codex ready, owner-capacity
  dispatch advertising local Codex.
- Public counter baseline before the run: `417,285,489` tokens.
- Three parallel typed fixture requests returned delegation frames and then
  accepted no-spend closeouts:
  - `assignment.public.khala_coding.chatcmpl_17078db5d05343198f63dd007632866a`
    -> `assignment.closeout.0ba622400bc6ff1055523d2f`
  - `assignment.public.khala_coding.chatcmpl_31fbf4ba89794bb087de539578e3c202`
    -> `assignment.closeout.108206bb42738b16cb10282a`
  - `assignment.public.khala_coding.chatcmpl_d5f732c130d44cabba2a59d71fd9bf09`
    -> `assignment.closeout.a283c446b0e56e3ee4b6082b`

Each fixture closeout reported `paymentMode: "no-spend"`,
`settlementState: "not_applicable"`, `payoutClaimAllowed: false`, and
`result.public.pylon.codex_agent_task.fixture_repair_passed`.

## Still Blocked

Do not claim:

- third-party Pylon capacity pooling or resale;
- payout eligibility for owner-local Codex no-spend work;
- guaranteed continuous dispatch availability;
- public raw Codex event visibility;
- a live full-assignment trace/status UI;
- broad automatic coding-prompt routing without the typed/semantic selector and
  caller-owned capacity resolver.

The main blockers are dispatch capacity-read reliability, owner-only Pylon/Codex
trace read-scope mismatch, assignment-level trace/status presentation, and the
broad semantic router being verified without ad hoc keyword routing.

## CLI Boundary

The current CLI evidence is the v0.1.16 source and changelog, not the v0.1.11
sneak peek in Episode 244. The old OpenTUI single-line plan in
`docs/khala-cli/` is superseded: the shipped client uses normal terminal
scrollback/raw-mode input, supports interactive and headless operation, metadata
commands, feedback, changelog, token counter, login/logout, optional local Codex
delegation, and the owner-authenticated Artanis channel.
