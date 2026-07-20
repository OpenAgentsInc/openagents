# SBX-04 managed-sandbox turn runtime

- Date: 2026-07-19
- Issue: [#9024](https://github.com/OpenAgentsInc/openagents/issues/9024)
- Status: deterministic component accepted. Live GCP provider proof remains SBX-09.
- Public facade: default off
- Private control schema: `openagents.managed_sandbox_turn_runtime.v1`

## Outcome

SBX-04 connects Box-compatible prompt, prompt status, ordered events, and
interrupt to the canonical managed-sandbox turn authority. A turn records its
provider, model, SDK harness, prompt digest, work unit, attachment, exact
resource generation, dense event cursor, usage, terminal reason, and terminal
receipt.

The implementation does not infer completion from quiet output. Dispatch
settles only its short admission command when `RuntimeStarted` is durable. The
turn remains independently active. A terminal turn requires structural
completion, explicit interruption, a declared lease/budget guardrail, or a
typed failure.

## Components

| Component                           | Responsibility                                                                                                           |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `packages/managed-sandbox-contract` | Runtime identity, turn, usage, native event, input-event, and terminal-receipt schemas plus lifecycle model              |
| Cloud SQL migration `0081`          | Canonical turn JSON, per-turn sequence, interrupt command, receipt, and native-event coordinates                         |
| `PostgresManagedSandboxStore`       | Dense append, identical-byte replay, changed-byte conflict, cursor replay, terminal reconciliation, and generation fence |
| Worker Box adapter                  | Prompt/status/events/interrupt translation and bounded Box event projection                                              |
| `oa-codex-control`                  | Private fail-closed dispatch/sync/interrupt adapter to one configured guest SDK helper                                   |

## Driver contract

Set `OA_MANAGED_SANDBOX_TURN_DRIVER` to an absolute executable path in the
control/guest deployment. Relative paths are refused. The executable receives
one JSON request on stdin and the literal argument
`--managed-sandbox-turn`. It returns one JSON response on stdout.

The admitted actions are:

- `dispatch`: includes the bounded raw prompt on the private channel. It must
  begin with turn event 1, `RuntimeStarted`.
- `sync`: includes `afterTurnSequence` and returns zero or more next dense
  events.
- `interrupt`: includes the exact turn, generation, reason, idempotency ref,
  and last turn sequence and must begin with `RuntimeInterruptRequested`.

The helper is expected to use the ordinary `@openai/codex-sdk` or
`@anthropic-ai/claude-agent-sdk` path already owned by Pylon. It owns private
SDK session/process handles. Those handles, credentials, auth homes, raw
paths, stderr, and provider diagnostics do not enter the response. Only the
closed native event vocabulary crosses back.

Each call is a short control operation. The daemon sets no turn-completion
timer and does not treat a driver response gap as completion. Lease and budget
enforcement remain declared guardrails, not an arbitrary provider wall clock.

Each provider stream has a one-terminal fence. The guest helper emits the first
structural completion, interruption, or typed failure and ignores later SDK
records for that stream. A provider failure followed by a trailing error cannot
append a second terminal event or roll back the first terminal receipt.

## Configuration

The Box facade still requires all SBX-03 configuration:

- `MANAGED_SANDBOX_BOX_V1_ENABLED` — remains false in production through
  SBX-09.
- `OA_MANAGED_SANDBOX_IMAGE_DIGEST` — exact admitted image digest.
- `OA_CLOUD_CONTROL_URL` — private `oa-codex-control` URL.
- `OA_CLOUD_CONTROL_TOKEN` — private bearer secret.

The control service additionally requires:

- `OA_MANAGED_SANDBOX_TURN_DRIVER` — absolute SDK-helper executable path.

Absent Worker or driver configuration returns typed `503`. It never falls
back to the Worker host, a fixture, another provider, or weaker isolation.

## Reconnect and interrupt

Native events retain both sandbox-global and turn-local order. A client may
re-read any ordered turn cursor. Exact replayed bytes append no data. A gap
or changed event at a present sequence is a conflict.

Interrupt reserves a new exact command only after dispatch admission releases
the sandbox command lock. The provider-visible request and native
`RuntimeInterruptRequested` event share the turn/generation fence. A repeated
idempotency key retries the same target and cannot interrupt a newer turn.
The turn remains `interrupting` until the provider reports
`RuntimeInterrupted` or typed failure.

## Resume truth

Stop and resume preserve only the facts proven by their own contracts:
durable filesystem/checkpoint state and the Cloud SQL turn/event ledger.
Services may restart in the guest. SBX-04 does not claim that memory, a PID,
transport connection, or provider-hidden session state was snapshotted.

## Verification

```bash
pnpm --dir packages/managed-sandbox-contract run typecheck
pnpm --dir packages/managed-sandbox-contract run test
pnpm --dir packages/khala-sync-server run typecheck
pnpm --dir packages/khala-sync-server exec vp test --run src/managed-sandbox-store.test.ts
pnpm --dir apps/openagents.com/workers/api run typecheck
pnpm --dir apps/openagents.com/workers/api exec vp test --run src/managed-sandbox-box-v1-routes.test.ts
cargo test -p oa-codex-control managed_sandbox_turn_runtime
pnpm run check:fast
```

The store test covers exact provider/model/harness identity, text, tool,
usage, replay, changed bytes, reconnect, interrupt, terminal receipt, and stale
generation. The route suite runs the unmodified Box SDK through both Codex and
Claude component event streams. The Rust suite proves the private executable
adapter accepts both provider identities and rejects stale/invisible
interrupts.

## Owner-gated live SDK harness

SBX-09 owns
`apps/openagents.com/workers/api/scripts/managed-sandbox-box-live-acceptance.ts`.
The script is default-off and requires both `--apply` and
`OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST`. Its remaining inputs
are the external staging `/v1` base path, two programmatic agent tokens with
different owners, the GCP project and zone, and the exact admitted source,
image, and profile digests. Tokens remain environment-only and are not
written to evidence.

The script runs the unmodified `@asciidev/box-sdk@0.0.24`. It requires real
Codex and Claude structural completion, then interrupts a separate exact turn.
It also covers lifecycle replay, cross-owner denial, ascending event reconnect,
file/command/artifact policy, stop/resume persistence, stale-generation cursor
denial, typed `501` behavior, delete replay, and zero exact-name GCP residue.
If API cleanup cannot converge, the script may remove only its deterministic
guest, disk, and five generation-owned firewall names. Any such emergency
cleanup makes the acceptance fail even when the final residue is zero.

Lifecycle settlement preserves the external control receipt without importing
its provider-specific URI grammar. The owner broker hashes the exact external
receipt identifier and records only
`artifact.managed-sandbox.lifecycle-receipt.<sha256>` in the native receipt.
The native ref decoder therefore stays fail-closed while evidence remains
content-addressed and replay-stable.

## Proof boundary

This packet proves the default-off software component and deterministic fault
boundary. Its receipt is
`docs/sol/evidence/2026-07-19-sbx04-managed-sandbox-turns.json`.

It does not claim that a real Codex or Claude account ran inside the admitted
GCP image. It also does not claim that the public facade is active or that the
production tests passed. SBX-09 is the only issue that may satisfy and
independently review the live isolation, cost, cleanup, and rollback claims.
