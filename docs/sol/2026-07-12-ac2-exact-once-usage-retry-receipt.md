# AC-2 exact-once usage retry receipt

Issue: [#8720](https://github.com/OpenAgentsInc/openagents/issues/8720)  
Parent: [#8547](https://github.com/OpenAgentsInc/openagents/issues/8547)

## Outcome

The Agent Computer usage route now has an integration proof against the actual
D1 token ledger for the lost-response retry required by #8547. The executor's
`usageRef` is deliberately not part of the idempotency identity. The server
derives one digest from the immutable
owner/thread/turn/lane/provider/model tuple and uses it for both the token event
ID and ledger idempotency key.

The proof posts an exact Codex owner-capacity receipt, then repeats the same
turn with a fresh client `usageRef`. It establishes:

- both responses name the same token event;
- the first response reports `insertedTokenUsage: true` and delta 18;
- the retry reports `insertedTokenUsage: false` and delta 0;
- the public-counter publisher receives exactly one delta; and
- the real ledger reads exactly 18 served tokens, not 36.

The existing route corpus continues to prove stable charge idempotency for
metered provider capacity and the no-charge disposition for owner capacity.

## Verification

- `bun run test -- src/khala-cloud-runtime-usage-routes.test.ts`: 9 passed,
  0 failed.
- Worker TypeScript typecheck passed.
- `git diff --check` passed before publication.

This closes the parent audit's usage-truth retry question. It does not claim a
new turn can reuse an old turn identity, nor does it replace #8547's grant and
live Firecracker/mobile acceptance.
