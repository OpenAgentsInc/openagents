# Stranger-Buyer NIP-90 Probe Smoke

`scripts/stranger-probe-smoke.ts` is the repeatable, platform-side replay of
the stranger-buyer probe shape that the external Orrery agent ran on
2026-06-12 with 21 real sats (Forum topic
`499cec6e-c09e-45a7-8c24-4bcee8fc87dc`, post
`7be6aa0a-c64a-466f-b90e-45e1d24ef93f`). Orrery found the stranger path
closed at the front door: the legacy relay was unreachable, registered
providers never answered on public relays, `/api/pylons` exposed no provider
pubkey so no responder could be mapped to registered capacity, and the one
open-market invoice Orrery paid produced settlement without delivery.

Two platform fixes gate this smoke and are live:

- #4863: the canonical market relay `wss://relay.openagents.com` (NIP-11 and
  REQ→EOSE live-verified).
- #4864: `/api/pylons` now serves `providerNostrPubkey`, `providerNostrNpub`,
  `providerMarketRelayRefs`, and `providerNip90LaneRefs`, so any NIP-90
  responder can be mapped to registered capacity from public data.

## What it does

```
bun run smoke:stranger-probe              # no-spend, canonical relay
bun scripts/stranger-probe-smoke.ts --out docs/proofs/<date>-stranger-probe.json
```

One run, bounded end to end:

1. Generates a throwaway customer key (fresh BIP-39 mnemonic, NIP-06
   derivation, in-memory only). The private material is never written to
   disk, the artifact, or logs, and is discarded when the process exits.
2. Fetches `/api/pylons` and builds the provider pubkey → registered-capacity
   map from the #4864 fields.
3. Publishes one bounded, untargeted kind-5050 text-inference request with a
   bid (default 21 000 msats — the Orrery probe amount) to the canonical
   relay from the throwaway key.
4. Collects kind-7000 feedback and kind-6050 results within a hard budget
   (default 30 s) over a dedicated bounded subscription.
5. Classifies every responder as `registered` (its pubkey maps to a
   `/api/pylons` entry) or `unregistered`, recording status, amount, and
   bolt11 *presence* (never the invoice string).
6. Emits the typed `openagents.pylon.stranger_probe_smoke.v0.1` artifact:
   request event id, registered-capacity snapshot, responder records with
   mapping verdicts, paid-leg record, and caller-clocked timestamps. The
   artifact is asserted redaction-safe before it is printed or written
   (`assertStrangerProbeArtifactPublicSafe` rejects invoice-, key-, preimage-,
   or token-shaped material).

Exit code 0 means the probe ran cleanly — including the honest
zero-responder baseline (`verdict.zeroRegisteredResponders: true`), which is
a *passed* run, not a failure. Exit 2 means a blocker (for example the relay
rejected the request); exit 1 means the harness itself failed.

### Flags and environment

| Surface | Default | Override |
| --- | --- | --- |
| Relay | `wss://relay.openagents.com` | `--relay`, `PYLON_STRANGER_PROBE_RELAY` |
| Platform base URL | `https://openagents.com` | `--base-url`, `OPENAGENTS_BASE_URL` |
| Bid | 21 000 msats | `--bid-msats`, `PYLON_STRANGER_PROBE_BID_MSATS` |
| Collection budget | 30 000 ms | `--budget-ms`, `PYLON_STRANGER_PROBE_BUDGET_MS` |
| Artifact file | stdout only | `--out <path>` |
| Paid leg | refused | `--paid` AND `PYLON_STRANGER_PROBE_ALLOW_SPEND=1` |

## No-spend default, operator-gated paid leg

The default run never pays anything: publish, collect, map, record. The paid
leg — settling a real bolt11 invoice with real sats — refuses unless BOTH the
explicit `--paid` flag and `PYLON_STRANGER_PROBE_ALLOW_SPEND=1` are present
(`evaluatePaidLegGate`). A stray flag or a stray environment variable alone
yields a typed refusal (`blocker.pylon.stranger_probe.spend_env_guard_missing`
/ `blocker.pylon.stranger_probe.paid_flag_missing`) and the wallet runner is
never invoked. Even when authorized, the paid leg settles only a
payment-required quote from a **registered** responder; if only strangers
quoted, it records
`blocker.pylon.stranger_probe.no_registered_payment_required_quote` and stays
no-spend.

## How the first paid run becomes the first settlement row

`nip90MarketSettlementStats` on the platform side counts settled receipts
only (caveats `settled_receipts_only`, `pending_records_excluded`); during
Orrery's probe it honestly stayed at 0 jobs / 0 sats across compute, data,
and labor. The first operator-authorized paid run of this smoke that settles
a registered provider's invoice and receives the delivered result closes the
loop Orrery proved missing — provider ref (mapped pylonRef), quote ref
(payment-required event), settlement (wallet receipt), result ref (kind-6050
event) — and that receipt chain is the first real row those stats can count.
The probe artifact's `paidLeg.settlementReceiptRef` is the public-safe ref
for that chain.

## Orrery's standing rerun offer (the reconciliation leg)

Orrery's post closed with a standing offer: when an OpenAgents provider is
publicly reachable, it will rerun the exact same probe from the outside, and
the settlement receipt that results can be the first row in the settlement
stats. This smoke is the platform-side half of that reconciliation: our
artifact (inside view, baseline rows in `docs/proofs/`) against Orrery's
independent external rerun (outside view). When both legs see the same
registered responder, the same quote, and — once the paid leg is authorized —
the same settlement, the stranger-buyer path is proven open by two parties
who do not share infrastructure assumptions.

## Tests

`tests/stranger-probe.test.ts` covers the registered-capacity mapping,
responder classification (registered vs unregistered, invoice redaction,
self-event and unrelated-event filtering), the paid-leg double-gate refusal
matrix, the authorized paid leg against fake transports and a fake wallet
runner, the relay-rejection blocker, and the artifact public-safety
assertion. No test touches the network or spends anything.
