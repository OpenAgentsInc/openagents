# Omega-effectd generation-fenced host bridge

- Date: 2026-07-24
- Scope: first OpenAgents-side runtime batch for Omega issue `#26`
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Protocol: `openagents.omega.effectd.v1`
- Admission: implementation evidence only. This does not admit a release or public claim.

## Result

The framed service no longer invents an Omega workspace, thread, lane state,
turn result, interruption result, or evidence snapshot. It issues typed reverse
host requests for:

- workspace resolution
- thread creation
- lane readiness
- exact leased-turn dispatch
- thread and turn evidence refresh
- live-turn interruption
- owner-visible system notes

The existing `reconcileFullAutoThreads` lease path remains the only dispatch
decision mechanism. Host dispatch acknowledges admission of the exact leased
`turnRef`. The service does not mint a second identity.

## Failure boundaries

- Every host request and response carries the active supervisor generation.
- Reinitialization rejects pending work from the prior generation.
- Unknown, duplicate, late, and stale replies do not update cached state.
- Input and output frames are bounded to 64 KiB before parsing or emission.
- At most 32 host requests may be in flight.
- Unanswered host requests expire after a bounded 30-second deadline. The
  service ignores their late replies.
- An absent bridge returns typed `host_unavailable`.
- Host-confirmed absence of a bound thread disables its thread lease and
  settles the run to `stalled` / `host_thread_missing` without dispatch.
- Public run lists remain free of objectives, done conditions, transcripts,
  credentials, and raw host evidence.

## Verification

- `pnpm --dir packages/omega-effectd typecheck`
- `pnpm --dir packages/omega-effectd test` — 198 passed

The test matrix covers exactly-one leased dispatch, missing-host refusal,
missing-thread typed stall, generation rollover and stale reply rejection,
frame bounds, and secret-free public projections.

## Remaining Omega-side work

Omega Rust must multiplex `host_request` frames while waiting for the matching
service response, execute them through the real GPUI/workspace/provider host
adapter, and return generation-matched `host_response` frames. This receipt
does not claim that packaged owner journey or publish package bytes.
