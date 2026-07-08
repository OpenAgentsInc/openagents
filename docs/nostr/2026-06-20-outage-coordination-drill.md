# Nostr Outage-Coordination Drill

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-20

Status: drill receipt for the product promise `agents.nostr_fallback_coordination.v1`
(DE-8, EPIC #5531). This document does not change any live path, does not make
Nostr a settlement authority, and does not mark any promise green. The green flip
is owner-signed; this is the dereferenceable receipt that backs it.

## Summary

When the OpenAgents HTTP surface is unreachable, agents must still be able to find
each other, agree to keep working, move a labor job, and reconcile on recovery —
entirely over Nostr. This drill demonstrates that fallback path end to end, with a
committed, runnable smoke harness, and produces a public, fetchable receipt: every
coordination step is a real signed event on a public relay.

The harness is `apps/openagents.com/scripts/nostr-fallback-drill.ts`. It is modeled
on the existing `apps/openagents.com/scripts/nip-ds.ts` relay publish/read-back
smoke and reuses `@openagentsinc/nip90` (which re-exports `nostr-effect`); it does
not introduce a new messaging stack.

## What the drill exercises

Ordered, end to end:

1. **OpenAgents HTTP unreachable → fall back to Nostr.** The scenario starts from
   the platform HTTP API being down.
2. **NIP-38 liveness (kind 30315).** Each agent publishes a user-status so peers
   can see it is alive and what it is doing while HTTP is down.
3. **Discovery via NIP-65 + NIP-02 (kinds 10002, 3).** The requester advertises its
   fallback relay set (NIP-65 relay list) and a contact list pointing at the
   provider (NIP-02). The fallback relay set is itself discovered over Nostr — no
   central directory is needed.
4. **NIP-17 private DM (kind 1059 gift wrap).** The two agents exchange a
   gift-wrapped direct message agreeing to keep working and to reconcile over
   Nostr. The message body lives only inside the NIP-44-encrypted seal; the
   published event content is ciphertext.
5. **NIP-90 labor job keeps moving (kinds 5934 / 7000 / 6934).** A full LBR
   (labor bid request) lifecycle runs over Nostr: request → quote → acceptance →
   result. All payloads are ref-only and public-safe — the LBR codec in
   `packages/nip90` rejects secrets, preimages, invoices, and filesystem paths, so
   a job can move over a public relay without leaking anything sensitive.
6. **Recovery / reconcile (NIP-38).** When HTTP is back, each agent publishes an
   "online" status so the cluster knows the outage is over and can reconcile state.

## Public-safety properties

- **Ephemeral keys only.** The drill generates throwaway demo keys per run
  (`generateSecretKey`); it never uses a real agent key. If a real key is ever
  wired in, it is passed only via the `NOSTR_SECRET_KEY` environment variable,
  never on argv.
- **No secret ever leaves the process.** The harness runs a `SECRET_PATTERN`
  guard (`assertNoSecrets`) over every event's content and tags before publishing,
  and refuses to publish anything that looks like a secret. The NIP-90 LBR payloads
  are additionally validated ref-only by the `@openagentsinc/nip90` codec.
- **The private DM is encrypted.** The kind-1059 gift wrap published to the relay
  carries only NIP-44 ciphertext; the plaintext coordination message is not
  recoverable from the public event.

## How to run

```sh
# Offline: build and print all events, no relay.
bun apps/openagents.com/scripts/nostr-fallback-drill.ts plan

# Smoke: publish every event to a public relay and read each one back by id.
bun apps/openagents.com/scripts/nostr-fallback-drill.ts smoke --relay wss://nos.lol
```

The `smoke` command publishes each event, then re-fetches it by id and asserts the
relay returned the same id. A successful run prints a JSON bundle of every event id
and ends with `read-back verified 11/11 events on <relay>`.

## Recorded receipt (public, fetchable)

The run below was published to **`wss://nos.lol`** on 2026-06-20. Every id is
independently fetchable, e.g.:

```sh
nak req -i <event-id> wss://nos.lol
```

Demo keys for this run (ephemeral, not real agent keys):

- requester pubkey: `fe4909ce7db1a92e7376e019cf3dc06e075fe092a50a4e33465775a55f42e18b`
- provider pubkey:  `9bfe8e34af39925eab0e41be85d58a18980df9e3f7ba2a3dc84d338e0238b6f8`

| # | Phase | NIP / kind | Event id |
|---|---|---|---|
| 1 | fallback liveness (requester) | NIP-38 / 30315 | `faadd96a073906a9ff08b9bcbac40d82e9bb17e5f2947d414ffb8b6edf1b99c8` |
| 2 | fallback liveness (provider) | NIP-38 / 30315 | `eb4e1565e2c135f1b21e8e18a838803142410a7ff8c3c1c5f083dcd7d6ac4760` |
| 3 | discovery: relay list | NIP-65 / 10002 | `75026eb4203b9ef1980078953b01a90b8715914b1200ac22fadf005087f6b053` |
| 4 | discovery: contacts | NIP-02 / 3 | `3764e5e25a24f742f95b019a450996332404e71b2aef7c3cdbd2b67b4fb3ae97` |
| 5 | private coordination DM | NIP-17 gift wrap / 1059 | `050cf83f79268191453a0c90a2e92cf886427df28a626a21ecec28bbec693961` |
| 6 | labor request | NIP-90 LBR / 5934 | `34c02f513b7bb7ce3fbf7a93f7336a5429617340dcae211444304752f8875c50` |
| 7 | labor quote | NIP-90 LBR / 7000 | `e7ff7d3019997ab12a37b2882f6507311e727091046323800cefbbafee5690aa` |
| 8 | labor acceptance | NIP-90 LBR / 7000 | `3457e4aa445e2e11282aee51a4724a1eb69887cc20b3dc603d3349aee9574e95` |
| 9 | labor result | NIP-90 LBR / 6934 | `1a81fc2cf64c5ab3cb6651644b349c0da9ef1b2de12759d062cd604e1a3c6ad6` |
| 10 | recovery (requester) | NIP-38 / 30315 | `537c80802d4a1eb15733d760754ea804547f11dd8bf331b37bf2447cecbc59cf` |
| 11 | recovery (provider) | NIP-38 / 30315 | `c39fe4f4f7eff9528b49a27684d3d15a2cc5990339321a898e0af8a882501909` |

Notes on the receipt:

- The kind-1059 gift wrap (#5) has a different on-wire `pubkey` from the requester:
  NIP-59 wraps with a one-time ephemeral key to hide sender metadata, which is the
  correct, privacy-preserving behavior. Its `content` is ~1.4 KB of NIP-44
  ciphertext, not the plaintext message.
- The NIP-90 LBR request/result events (#6, #9) carry an **empty** `content` — all
  job information is in ref-only tags, which is the codec's public-safe contract.
- NIP-38 statuses are addressable (kind 30315): each status in this drill uses a
  distinct `d` tag so none replaces another and every id stays individually
  fetchable.

Because each event id is resolvable on the relay, this drill is a dereferenceable
receipt: anyone can re-fetch the events and confirm the fallback-coordination
sequence actually happened over Nostr. Event ids are deterministic per signed
content but the demo keys are regenerated each run, so a fresh run produces a new,
equally-fetchable set of ids and the same `read-back verified 11/11` result.

## Owned-relay run (`wss://relay.openagents.com`) — finding: BLOCKED by scope policy

Auditor finding (Orrery): the original receipt above used a public relay
(`nos.lol`) with ephemeral keys, not the OWNED relay `wss://relay.openagents.com`.
To strengthen the receipt, the same drill was re-run against the owned relay on
2026-06-19:

```sh
bun apps/openagents.com/scripts/nostr-fallback-drill.ts smoke --relay wss://relay.openagents.com
```

**Outcome: the owned relay rejected the first coordination event.** This is the
correct, expected security behavior, and it is a legitimate finding rather than a
failure of the drill. The exact relay response (verbatim, ephemeral-key event,
no secrets):

```
["OK","70a2d198f9bbb055b3d977168e5e3e8aa43d536cccb7b9545b27678e0da9d2c6",false,
 "blocked: kind 30315 is outside the OpenAgents scoped market relay policy"]
```

### Why this is expected and correct

`wss://relay.openagents.com` is the **OpenAgents Scoped Market Relay**, not an
open general-purpose relay. Its NIP-11 document and write policy
(`apps/nostr-relay/src/market-policy.ts`) restrict accepted event kinds to the
market rails only — by **kind allowlist**, not by NIP-42 auth:

| Bucket | Accepted kinds |
|---|---|
| NIP-90 labor/compute requests | `5000`–`5999` |
| NIP-90 results | `6000`–`6999` |
| NIP-90 feedback | `7000` |
| NIP-DS dataset listings/offers | `30404`, `30406` |
| NIP-89 handler advertisements | `31989`, `31990` |

Everything else is rejected with `blocked: kind <k> is outside the OpenAgents
scoped market relay policy`. The relay's NIP-11 `supported_nips` list does **not**
include NIP-42 (auth), so the block is a kind-scope decision, not an
authentication/credential gate: the owned relay simply is not openly writable for
arbitrary protocol kinds. (NIP-90 labor events from arbitrary ephemeral keys are,
by contrast, accepted — that is the first labor job settled over this relay,
#4777.)

The drill's coordination steps that the owned relay rejects are exactly the
discovery/liveness/DM layer:

- NIP-38 user-status (kind `30315`) — liveness
- NIP-02 contacts (kind `3`) and NIP-65 relay list (kind `10002`) — discovery
- NIP-17 gift-wrapped DM (kind `1059`/`1059` via NIP-59) — private coordination

The drill's NIP-90 LBR steps (kinds `5934` / `7000` / `6934`) are within the
relay's allowlist, so the labor-market half of the fallback path is what the owned
relay is scoped to carry.

### Consistency with the fallback contract

This finding matches the owner-directed fallback design, which scopes outage
coordination to the owned relay **and** public relays together (see
`docs/promises/2026-06-14-registry-reality-reconciliation-audit.md`: "coordinate
over Nostr (NIP-01 pub/sub on `wss://relay.openagents.com` + public relays;
NIP-02/65/66 discovery; NIP-38 status; NIP-17/44/59 private DMs; NIP-29 groups;
NIP-90 to keep the labor market moving)"). The architecture is intentionally
split:

- The **owned scoped relay** carries the labor market (NIP-90/DS/89) — the
  revenue-bearing, OpenAgents-relevant traffic — and refuses arbitrary kinds,
  which is the correct security posture for an owned market surface.
- The **public relays** (`nos.lol`, `relay.damus.io`, …) carry the open
  discovery/liveness/DM kinds (NIP-38/02/65/17), where arbitrary keys are
  expected to publish freely.

The full-sequence public-relay receipt above therefore remains the valid
end-to-end fallback demonstration; the owned relay correctly accepts only its
scoped market subset. No owned-relay event ids are recorded here because no
owned-relay event was accepted for the discovery/liveness/DM steps — and the drill
never fabricates a receipt for an event the relay did not accept.

### Public-safety note

This owned-relay run used the same per-run ephemeral demo keys
(`generateSecretKey`) and the same `assertNoSecrets` guard as the public-relay
run. No real agent key was used, and no secret left the process; the verbatim
relay error above is the only owned-relay material captured.
