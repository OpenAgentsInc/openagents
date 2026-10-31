# 201. Model-visible means logged

> **Status: unimplemented — design draft.** Nothing here runs yet.

The model sees only what the harness sends it. This chapter makes every one
of those bytes reconstructable. The harness writes an append-only session log
of durable events and derives the model's history from that log. You can
rebuild any past request byte for byte and replay it.

## What this chapter covers

- Event types: turn start and end, step start and end, user and assistant
  messages, tool calls and tool results. Each event type is a versioned
  message in `proto/`.
- Derivation. The request history is a pure function of the log. No request
  carries content that the log does not hold.
- Replay. Given a log and a step index, the harness rebuilds that step's
  request exactly and can resend it to the same model identity.
- Versioning. The log format has a version. Readers for old versions stay, so
  an old session still opens.
- Durability. Each event is written before its effect is reported, so a crash
  leaves a log that ends at a known boundary.

## What it builds

- A planned session log store and reader in `psionic/lib/harness/`, and
  event schemas in `proto/`.

## Dependencies

- [Chapter 200](ch200-the-simplest-loop.md): the turn and step boundaries that the log records.
- [Chapter 239](ch239-a-suite-is-three-digests.md): digests and append-only receipt chains, reused for log
  integrity.

## Done means

- For every step of a recorded session, the rebuilt request equals the sent
  request byte for byte.
- A session written by an earlier log version opens and replays with the
  current reader.
- A log truncated at any byte offset opens to the last complete event and
  reports the truncation.
- A request that contains bytes absent from the log fails a check before it is
  sent.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
