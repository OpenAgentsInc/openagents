# Cloudflare World Actor Command Authority Model

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-22

Status: invariant model note for the Cloudflare Verse World cutover.

## Boundary

The Cloudflare Verse World Service owns public-safe presence, local interaction,
interest-scoped fanout, diagnostics, and replayable projection rows derived
from public source refs. It does not own training truth, product promises,
receipt validation, accepted-work authority, settlement, payout, wallet state,
provider credentials, private prompts, private repo content, or customer-private
data. Those authorities remain with the `openagents.com` Worker/D1 product
surface and its existing ledgers.

## Actors

Command envelopes carry an `actorClass` and `actorRef`.

- `browser`, `agent`, and `operator` actors may send interaction commands only:
  join, leave, bounded avatar pose, pylon focus, local/pylon chat, emote, and
  ephemeral intent.
- `service` actors may send projection commands only: upsert public run/entity/
  edge/proof/settlement/region/station rows, append world events, advance
  projection cursors, record bridge health, write system messages, and expire
  interaction rows.

The model deliberately has no compatibility actor for the deleted backend and
no stringly backend-kind switch. A command that needs a new authority class must
update `packages/world-contract`, this note, `INVARIANTS.md`, and the command
tests in the same change.

## State

For the authority model, state is the tuple:

```text
S = {
  hotRegionState,
  publicProjectionRows,
  commandReceiptLog
}
```

The safety property is:

```text
For every command c:
  if c.command is service-only and c.actorClass != "service",
    then apply(c, S).status = rejected
    and publicProjectionRows is unchanged.

  if c.command is browser/interaction and c.actorClass == "service",
    then apply(c, S).status = rejected
    and hotRegionState is unchanged.
```

The projection redaction property is:

```text
For every accepted service projection row r:
  r.safety.publicProjectionAllowed == true
  and r contains no private refs, private paths, raw prompts, provider payloads,
  wallet secrets, or customer-private text.
```

## Counterexamples Converted To Tests

- A browser actor tries `upsert_training_run`; expected rejected auth receipt
  and diagnostic delta.
- An agent actor tries `append_world_event`; expected rejected auth receipt and
  no projected row.
- An operator actor tries `record_bridge_health`; expected rejected auth
  receipt and no bridge-health row.
- A service actor tries `send_local_message`; expected rejected auth receipt and
  no local-chat row.
- A service actor writes a private/unsafe projection row; expected rejected
  redaction receipt and no payload echo.

These are covered in `apps/openagents-world/src/commands.test.ts`. The model is
bounded to command authority; it does not prove Cloudflare runtime delivery,
network ordering, or cross-service identity issuance. Those are covered by the
Worker service tests, two-client smokes, and deploy runbooks.
