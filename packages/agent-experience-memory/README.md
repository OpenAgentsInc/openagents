# @openagentsinc/agent-experience-memory

Optional, owner-local, redacted experience memory for the Apple FM router and
the coding loop (AFS-10).

Memory is a conservative, default-OFF addition. With the flag off, no bank is
frozen, no record is read or written, and no recalled slice enters any prompt:
the router and the IDE behave byte-identically to a build without this package.
Memory is never a dependency of a router or IDE path. It is only ever an
advisory input a host MAY consult when the owner turns it on.

## What it is

The package adds the distilled global-pattern layer and a bounded, one-shot
recall path on top of an existing substrate. It reuses:

- the ATIF redaction boundary (`@openagentsinc/atif`) for one rule set over
  traces and memory, and
- the reviewed ranking, tie-break, and packing ideas of the unwired Pylon TAS
  kit (`repo-memory.ts`, `session-memory.ts`, `semantic-retrieval.ts`,
  `context-assembly.ts`). It imports nothing from that kit. The TAS files carry
  no schema, persistence, consent, delete, or owner-scope authority.

## Guarantees

- **Default OFF.** `MEMORY_DEFAULT_ENABLED` is `false`. `defaultMemoryConfig`
  returns a disabled, local-only config. A disabled adapter is a true no-op:
  zero reads, zero writes.
- **Redacted.** Every stored fact and every recalled slice passes the ATIF
  redactor. A fact carrying a secret, wallet or payment value, or a local path
  is rejected outright. Soft PII (for example an email) is scrubbed. `assertRecallClean`
  is a backstop on recall output.
- **Owner-scoped and project-scoped.** One owner scope never reads another
  owner's memory. Recall stays inside one project without a separate scope
  grant. A distilled pattern inherits no access to the private cases behind it.
- **Consent.** Consent defaults to `withheld`. A withheld record is visible to
  its owner through `inspect` but never enters a frozen bank or a recall result.
- **Local-only.** This portable package holds no cloud, SQL, provider, or Node
  host. The durable adapter that writes private local app storage lives in the
  app composition root. The Apple FM path stays strictly on-device. A non-local
  adapter needs a separate owner decision and an admitted Google Cloud design.
- **Frozen and one-shot.** Exactly one eligible bank is frozen at turn start.
  At most one pre-turn adaptation runs, bound by an `effectiveAdaptationDigest`,
  so current-turn data cannot change current-turn input. A corrupt bank fails
  closed to no-memory.
- **Structured to measure.** `computeBenefitReport` compares flag-off and
  flag-on acceptance and correction deltas. Version one ships OFF with NO
  measured live benefit. The harness exists so a future promotion rests on
  evidence, never assertion.

## Owner lifecycle

`inspect`, `exportScope`, and `forget` give the owner inspect, export, and
delete over their own memory in a scope.

## Portable graph memory

`GraphMemoryStore` is a separate optional contract for a graph corpus. It does
not change `MemoryStore`. The contract uses the
`@openagentsinc/graph-corpus` schemas and validators. It binds each graph to one
owner, one project, one source set, one manifest, one policy, and one
generation. `graphMemoryScopeRefFor` calculates the required graph scope from
the owner and project. The store accepts only input that has consent and the
`already_redacted` state.

`GraphMemoryStateStore` is the portable persistence port. A host supplies an
atomic `load` and `compareAndSet` implementation. The graph contract owns
schema decoding, two-phase mutation recovery, idempotency, archive validation,
delete-plan validation, and lifecycle receipts. A Desktop adapter can encrypt
and store the opaque state in SQLite. This package does not import SQLite or a
cryptography API.

Archive export returns bytes to the caller. The graph store keeps a bounded
replay record so that an operation retry returns the exact prior result.
An accepted source deletion and `forget` remove each internal replay record.
Their receipts distinguish an internal replay deletion from a caller-held
export. The caller controls deletion of those external bytes.

Receipt history is bounded. When the receipt limit is full, a normal mutation
fails before it writes pending state. A full-scope `forget` is not blocked. It
compacts the prior receipt set into a count and a digest, and it keeps the exact
forget receipt for restart inspection.

Lifecycle fact arrays use a separate derived limit. This limit covers the
maximum admitted graph actions, artifact actions, ranking snapshots, and replay
records in one operation. Thus, a valid near-limit delete or forget operation
does not create an invalid receipt.

## Boundaries

The package is governed by `scripts/check-afs-boundaries.ts`: no app, platform
API, provider SDK, SQL driver, cloud client, or Node host import, and no
unchecked casts.
