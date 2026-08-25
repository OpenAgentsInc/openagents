# NIP-90 provider loop

Status: **retired**. The loop this page used to describe was deleted on
2026-07-14 by commit `21e82ce829` (`feat(vp1): retire money sites and wallet
authority`). Nothing in the tree serves NIP-90 jobs today.

This page is kept because the code it describes is the port source for
[#30](https://openagents.com/OpenAgentsInc/openagents/issues/30), the issue that
tracks reviving provider mode. It records what was removed, what survived, and
what `pylon provider` actually does now, so nobody spends an afternoon looking
for a command that no longer exists.

## Commands that no longer exist

Earlier revisions of this page told you to run these. Do not:

| Removed command                 | Status                                                     |
| ------------------------------- | ---------------------------------------------------------- |
| `pylon provider once`           | Deleted. The CLI catalog offers no `once` subcommand.      |
| `pnpm run provider:serve`       | Deleted with `apps/pylon/scripts/nip90-provider-serve.ts`. |
| `pnpm run smoke:nip90-provider` | Deleted. No such script is defined.                        |
| `pylon provider approve-labor`  | Deleted. See `labor-market-provider-loop.md`.              |

The files behind them went in the same commit: `apps/pylon/src/provider-nip90.ts`,
`apps/pylon/src/labor-market.ts`, `apps/pylon/src/multi-earning-ledger.ts`, the
Spark wallet stack, and the MDK treasury and tips services.

## What `pylon provider` does today

`apps/pylon/src/cli-catalog.ts` declares exactly two subcommands:

```sh
pylon provider go-online
pylon provider go-offline
```

`go-online` is local bookkeeping. It writes runtime lifecycle `online`, probes
the Claude, Codex, and Apple Foundation Models readiness this device actually
has, records the resulting capability and blocker refs, and reports per-account
coding capacity as quantities — available, busy, queued, and ready slots per
connected Codex account. It opens no socket, publishes no Nostr event, quotes
no job, and earns nothing. `go-offline` reverses the lifecycle.

Whatever runs on that capacity arrives through the assignment and dispatch
paths, not through a market.

## What survived the removal

- **The protocol package.** `packages/nip90` was kept and extended through the
  retirement. It carries the NIP-LBR lane — `lbr.ts` (request, quote,
  acceptance, result), `lbr-closeout.ts` (a content-addressed, public-safe
  receipt binding one whole lifecycle), and `lbr-bond.ts`, a post-retirement
  addition covering forfeitable provider bonds.
- **The presence lane refs.** `packages/pylon-core/src/presence/nip90-lane-refs.ts`
  still resolves relay URLs from `PYLON_NIP90_RELAYS` and builds the
  `lane.public.nip90.*` refs that `presence.ts` would attach to a registration.
  In practice it never fires: `providerDiscoveryFields` only attaches those
  fields to a Pylon whose capability refs include
  `PYLON_NIP90_PROVIDER_CAPABILITY_REF`, and no code path adds that ref any
  more.
- **The labor admission rules.** `apps/pylon/src/labor.ts` still holds workspace
  bounding, provider-auth exfiltration detection, the public-safe result
  projection, and the local agent command builders. Nothing imports it;
  `makeConfiguredLaborRuntime` is listed in
  `scripts/uncalled-production-symbol-baseline.json`.
- **The buyer half of the negotiation.** `packages/sarah/src/lbr-request-quote/`
  was recovered on 2026-07-24 as request and quote only.

## What was revived, and how far

`openagents provider settle` (`packages/openagents-cli/src/provider-command.ts`
and `provider-settlement.ts`, commit `f0c36de6a0`) revives the earning half at
the one seam that can be proven offline. It reads a lease and an LBR closeout
receipt from files and decides what the job earned.

An earning requires a receipt that names this job and this provider, was not
issued by the provider to itself, carries both a verification command and the
evidence that command produced, carries the platform's own closeout, is
content-addressable, landed inside the lease window, and prices the job exactly
as the lease did. Ten named refusals cover the ways that fails, and each earns
zero.

Nothing moves. A settled decision reports `payout_rail: "not_connected"` and
`custody: "none"`. There is no key, no wallet daemon, and no spend authority.

## Why the rest stays unbuilt

`INVARIANTS.md` still says that payments, markets, wallet custody, payout,
billing credits, and settlement are not part of the accepted MVP, and that any
revival requires a fresh owner-approved design, custody model, invariant change,
and proof program. `scripts/vp1-retired-money-surface-guard.mjs` enforces it on
every push.

Presence with capacity-as-quantity and the claim, lease, and submit transport
also need a market transport the do-not-build register defers. When that lane
arrives it takes the relay, provider-daemon, and skeptical-client shape, not a
NIP-90 or DVM revival.

One failure from the original loop is worth carrying forward into any
replacement. The live run on 2026-06-12 treated 60 seconds of relay silence as
fatal, so every registered provider went dark about a minute after going online.
A responder needs idle keepalive and resubscribe, and an idle relay has to be
read as keep-waiting rather than as an error.
