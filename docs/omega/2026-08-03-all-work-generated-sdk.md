# Generated All Work SDK

- Date: 2026-08-03
- Issue: OpenAgents `#9305` (`OAW-010`)
- Contract: `openagents.all_work_boundary.v1`
- Status: implemented and verified in the owned reference process

## Outcome

The All Work Contract Profile now generates a TypeScript Effect client from
the same OpenAgents-owned definition that emits Effect schemas, Rust types,
JSON Schema, fixtures, and compatibility metadata. The SDK exposes only
implemented request/result pairs. Structural-only resources cannot appear as
callable production methods.

The generated surface includes typed query and command methods, typed protocol
failures, strict builders, fresh fixture factories, implemented-resource
metadata, and a client artifact digest in the compatibility manifest.

The reference `omega-effectd.v2` processor now serves
`work.index.subscribe`. It returns a ready snapshot and cursor, supports an
unchanged-cursor resume, rejects malformed cursors, and reports an explicit
gap when the projection changed but the process does not retain the missing
event history. It does not invent an upsert or remove event.

## Journey and falsifiers

The focused reference journey uses the generated client to:

1. read a canonical Work snapshot;
2. assign the Work through `work.command.execute`;
3. replay the same idempotency key and receive the same result;
4. subscribe to the Work index and receive a resume cursor;
5. resume against a changed query projection and receive a gap; and
6. retry the mutation with a stale revision and receive a typed conflict.

Client tests also prove retryable unavailable failures remain typed and that
malformed successful payloads fail closed. Existing signed Workroom tests cover
multi-relay partial delivery and idempotent retry.

## Compatibility and language boundary

TypeScript wire DTOs are structural types backed by strict constrained Effect
schemas. Rust boundary scalars remain validated newtypes. Wire clients do not
need unsafe casts to construct validated requests, while runtime decoding still
preserves patterns, lengths, safe integers, absent versus null, unknown-field
refusal, and tagged-union refusal.

Every generated artifact is byte-compared by `check:generated`. The manifest
binds definition, Effect schema, TypeScript client, Rust, JSON Schema, and
fixture-index digests. A wire or SDK change without regenerated reviewed bytes
fails the gate.

## Source and license record

The generator and client implementation are OpenAgents-authored. No external
generator or SDK source was copied. The package license is MIT. The pinned
Linear GraphQL schema in the dogfood plan supplied MIT-licensed vocabulary and
pattern evidence only. It is not a runtime dependency or authority.

## Verification

```sh
pnpm --dir packages/all-work-contract generate
pnpm --dir packages/all-work-contract check:generated
pnpm --dir packages/all-work-contract typecheck
pnpm --dir packages/all-work-contract test
cargo test -p openagents-all-work-contract
pnpm --dir packages/omega-effectd typecheck
pnpm exec vp test --root packages/omega-effectd --config vitest.config.ts --run src/protocol/server.test.ts src/protocol/all-work-process.test.ts
```
