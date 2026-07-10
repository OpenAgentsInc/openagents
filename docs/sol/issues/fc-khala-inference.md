# FC-BRAIN: Sarah inference through the Khala gateway

> **Historical closed substrate:** #8600 is closed. Retain persona-neutral
> gateway, caps, fallback, and receipt contracts. Sarah-specific presentation
> has no open product lane and this file is not a current dispatch queue.

## Outcome

Sarah's text and avatar brain use the persona-neutral Khala gateway with exact
receipts, caps, and typed fallback. This production-hardening lane runs in
parallel with P0 Fleet Command and does not block the first local dogfood run.

## Scope

- Remove direct raw provider-key inference from `apps/sarah`.
- Route Gemma/Sarah calls through an internal persona-neutral model identity.
- Coalesce VAD fragments into bounded turns so sustained speech cannot create
  a quota storm.
- Enforce Sarah-specific session/turn caps and spend alerts before inference.
- Replace hand-written retry/model fallback with gateway policy and typed
  events.
- Write exact per-turn usage rows and reconcile the private task receipt; public
  counter movement alone is not proof.
- Preserve role/tool authority above inference routing.

## Exit

A live sustained-speech Sarah session completes without raw app provider keys
or fragment-driven quota failure; every model turn has an exact receipt and
every fallback is typed and visible to operations.
