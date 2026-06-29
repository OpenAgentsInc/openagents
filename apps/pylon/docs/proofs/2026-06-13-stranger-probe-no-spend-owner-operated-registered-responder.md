# 2026-06-13 stranger probe: first REGISTERED responder (owner-operated leg)

Companion note for
`2026-06-13-stranger-probe-no-spend-owner-operated-registered-responder.json`.

## Honest labeling

- This run is the **owner-operated** leg: the responding provider
  (`pylon.raynor.proof.beta.20260612`, home on the same Mac) and the
  stranger-probe buyer ran on the **same machine**, operated by the same
  operator. It proves registered capacity can serve a stranger-shaped
  kind-5050 request end to end; it is **NOT** the independent-stranger leg
  that the P6 proof needs (a buyer with no operator relationship to the
  provider). The probe's throwaway customer key and untargeted request shape
  are identical to the baseline, but machine independence is not claimed.
- **No spend**: the paid leg was not requested
  (`blocker.pylon.stranger_probe.paid_leg_not_requested`). The provider only
  issued a receive invoice (payment-required quote, 1000 msats); nothing was
  paid by either side.

## Delta vs the 2026-06-12 zero-responder baseline

Baseline `2026-06-12-stranger-probe-no-spend-baseline.json`: 3 registered
providers (pubkeys, heartbeat online, canonical relay declared), zero
responders of any kind.

This run: same 3-provider registered map, and the probe request
(`13a2413c8dcd...`) received the full NIP-90 lifecycle from a registered
provider — `payment-required` (1000 msats, bolt11 attached), `processing`,
kind-6050 `job_result` (Apple FM completion, 84 chars), `success`. Verdict:
`zeroRegisteredResponders: false`.

## Root cause of the baseline's silence

Confirmed in `pylon.raynor.proof.beta.20260612`'s own node logs: the
provider loop published NIP-89 handler info at 11:21:28Z and died at
11:22:29Z with `[NIP-90] Service stopped with error: ... relay message timed
out` — the relay subscription's hardcoded 60s message wait treated a quiet
relay as fatal, and the supervised service never resubscribed. Every
registered provider went dark ~61s after going online. Fixed in
`src/provider-nip90.ts` (idle keepalive + resubscribe-on-failure) with
regression tests in `tests/provider-nip90.test.ts`.

## Serving setup for this run

- Worktree at current `origin/main` + the persistence fix, serve entrypoint
  `scripts/nip90-provider-serve.ts` (`bun run provider:serve`).
- `PYLON_HOME` pointed at the registered raynor-proof-beta home; identity
  pubkey matches the registered `providerNostrPubkey`.
- Runtime: local Apple FM (`http://127.0.0.1:11435`, ready).
- Quotes: MDK agent wallet via `PYLON_MDK_WALLET_HOME` (receive-only).
- Relay: `wss://relay.openagents.com` (canonical), NIP-89 handler info
  published at loop start; loop verified alive well past the old 60s death
  window before the probe ran.

Refs: openagents#4866 #4777 #4781 #4782.
