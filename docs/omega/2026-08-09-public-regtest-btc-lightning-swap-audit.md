# Omega public-regtest BTC-to-Lightning swap audit

- Date: 2026-08-09
- Class: audit and historical receipt
- Status: settled regtest swap; broader historical priority unproven
- Source thread: `760a4d36-f2bf-491c-951e-9bfac92e495e`
- Source store: owner-local `~/.omega-dev/threads/threads.db`
- Thread bounds: created `2026-08-09T14:59:11.949752+00:00`; updated
  `2026-08-09T17:17:10.578723+00:00`
- Scope: one Omega Agent conversation and its recorded market-tool results

## Verdict

Omega recorded a settled swap of 100,000 valueless Bitcoin regtest satoshis
from the on-chain Bitcoin rail to the Lightning regtest rail. The requester
record verified evidence on both rails. The exchange used the OpenAgents
public-regtest market, where provider profiles and offerings were read from two
ready Nostr relays and two providers competed for the quote. The selected
provider was `provider-a`.

This is the first requester-verified BTC-to-LN regtest settlement present in
the inspected Omega development thread store: 35 threads and 789 events were
searched, and the only regtest `omega.market-demo.swap.v1` result was this swap,
recorded twice in the same thread (execution and later status inspection).

The evidence does **not** support an unqualified “first-ever BTC↔LN swap over
decentralized Nostr relays” claim. Electrum 4.6.0 publicly shipped submarine
swaps over Nostr on 2025-07-16, including provider discovery and swap RPCs over
Nostr. The record also shows that Omega submitted execution to a single HTTPS
API, while its market discovery read the Nostr relays directly. No Nostr event
IDs or relay delivery receipts for the private execution transcript appear in
the thread.

The strongest claim this receipt supports is:

> On 2026-08-09, Omega recorded its first requester-verified, public-regtest
> BTC-to-Lightning swap through the OpenAgents Nostr-coordinated,
> multi-provider market.

“First” in that sentence is limited to the inspected Omega development thread
store. “Nostr-coordinated” describes provider and offering discovery without
claiming that every execution message traveled only through relays.

## What was swapped

| Field | Recorded value |
| --- | --- |
| Direction | `BTC` → `LN` |
| Input | 100,000 sats on Bitcoin regtest |
| Output rail | Lightning regtest |
| Network identifier | `bip122:0f9188f13cb7b2c9e5c72a6b65eeada4` |
| Quote | `regtest-route-2` |
| Request | `5a9a24c6e89cb9e6a2a041bd449616045c70ce5973350e46d5b6d12f44eb5194` |
| Swap | `5def90138254c0ec54572b2739a0608da99b20b7467a7b8b93f862d724482393` |
| Selected provider | `232aa9c2d3642abf9ba89e4c9f704b018630acfaf3e2c9faa2faa2b708341b18` (`provider-a`) |
| Final state | `settled` |
| Settlement authority | requester-local verification |

This was a submarine swap in the Bitcoin-layer sense: the requester supplied
value on the on-chain Bitcoin side and received settlement on Lightning. It
did not exchange Bitcoin for a different asset. It changed the settlement rail
used for bitcoin-denominated value.

The record calls the input amount 100,000 sats but does not record the exact
Lightning output amount. The indicative quote reported 100 basis points, or
1%, while the settled result set `fee_bps` to `null`. The evidence therefore
does not establish an exact fee paid or justify deriving a 99,000-sat output.

All funds were regtest funds with no monetary value. This was not a mainnet
payment, did not move spendable mainnet BTC, and did not settle a real-world
financial obligation.

## Timeline

1. The requester asked for a 50,000-sat BTC-to-LN regtest swap.
2. The tool refused because the public service accepted 100,000 through
   1,000,000 sats.
3. Omega requested the minimum amount, 100,000 sats. `regtest-route-1`
   returned an indicative route through “Immortal funded provider” at 100 bps.
4. The first execution attempt failed because the API returned an empty or
   invalid JSON response. Omega reported that no confirmed swap had been
   created.
5. At the requester’s instruction, Omega obtained the fresh quote
   `regtest-route-2`, again naming “Immortal funded provider” and 100 bps.
6. The second execution returned the settled swap record. Its selected
   provider was `provider-a`, identified by public key rather than the display
   name from the indicative route.
7. A later status read returned the same swap ID and settlement evidence.
8. A contemporaneous network-status read reported two ready relays and four
   ready provider profiles.

The failed first attempt and successful retry are separate execution outcomes.
The first quote does not belong to the settled swap. The settled record is
bound to `regtest-route-2`.

## Participants and coordination path

```text
Omega requester
  ├─ reads provider profiles and offerings (Nostr kinds 39600/39601)
  │    ├─ relay-a ─ provider-a and other advertised providers
  │    └─ relay-b ─ provider-b and other advertised providers
  └─ sends a NIP-98-signed HTTPS execution request
       └─ OpenAgents public-regtest swap API
            ├─ obtains competing signed provider quotes
            ├─ selects provider-a
            ├─ coordinates Bitcoin-regtest funding and Lightning settlement
            └─ returns evidence for requester-local verification
```

The network snapshot recorded these relay endpoints:

| Relay | URL | State | Trust |
| --- | --- | --- | --- |
| `relay-a` | `wss://relay-a.34-41-78-122.nip.io` | ready | pinned |
| `relay-b` | `wss://relay-b.34-41-78-122.sslip.io` | ready | pinned |

It also recorded four ready providers:

| Provider | Public key | Discovery state | Relay | Advertised fee |
| --- | --- | --- | --- | --- |
| Immortal funded provider | `4abf91baeb62b561288ec27ada3a81edf3855689de593f13792fbec428a20d83` | discovered | relay-a | 100 bps |
| Local demo provider | `31de792d6c6eb78606220534db6533a151738c66f18e765b20f4e72b8f9c1ac2` | discovered | relay-a | unknown |
| provider-a | `232aa9c2d3642abf9ba89e4c9f704b018630acfaf3e2c9faa2faa2b708341b18` | pinned | relay-a | 100 bps |
| provider-b | `2ad9b8e40ab714f4f5230fd721b8600237d24eda9ed5a5af3ed608dd1abcdb59` | pinned | relay-b | 100 bps |

The same snapshot identified Bazaar revision
`b46d75ec2293c270ad1a1fc8a815eca46fd969bb` and Immortal revision
`907b693b1765b3e9e631cae81aa27fa83efa444b`. It reported no aggregate
network statistics; missing volume and activity figures must not be read as
zero.

The execution result listed provider-a and provider-b as quote participants,
selected provider-a, and marked the unselected reservation released. The
network snapshot also contained two additional ready provider profiles, but
the swap result did not say they supplied signed quotes for this request.

This is decentralized at the discovery and provider-choice layer in a bounded
sense: multiple relay endpoints carried signed provider identities and
offerings, and multiple provider keys were eligible. It is not evidence of a
fully serverless execution path. Omega's recorded implementation used
`https://api.openagents.com/v1/market/regtest/swaps` as the execution endpoint.
The relays coordinated market information and did not hold funds or operate
the Bitcoin or Lightning nodes.

## Settlement and verification evidence

The returned lifecycle was one linked four-stage projection:

| Sequence | Stage | Authority |
| ---: | --- | --- |
| 0 | `contract` | `requester_local` |
| 1 | `funding` | `provider_claim` |
| 2 | `executing` | `provider_claim` |
| 3 | `settled` | `requester_local` |

Every status pointed to the preceding status. The projection reported no gaps,
no forks, and `local_effects_verified: true`. Funding and execution remained
provider claims until the requester verified the terminal effects.

Omega generated this four-stage view from the API's terminal settled response.
The thread does not contain four independently captured raw status events.
Accordingly, “no gaps or forks” describes Omega's returned projection, not an
independent replay of a complete relay event history.

The rail evidence was:

| Rail | Reference | State |
| --- | --- | --- |
| Bitcoin | `7af1666bf70cf59dd377c373a04100344cd87df2049f7803848e78bbad72e801` | verified |
| Lightning | `88ca3da5c89e19b0bab3d8820b9667d318a995edd3ed93ae4b65c507f67c9bfe` | verified |

The thread does not include the raw Bitcoin transaction, its decoded outputs or
confirmation depth, the Lightning invoice, payment preimage, node RPC output,
or independent explorer data. The two references are therefore durable
identifiers in the returned receipt, not enough material for a third party to
reperform rail verification from this document alone.

## What the record proves

- Omega reached the live public-regtest service rather than the deterministic
  demo fixture.
- The requested direction was on-chain Bitcoin regtest to Lightning regtest.
- The second 100,000-sat attempt returned a settled result.
- The requester-side projection accepted Bitcoin and Lightning evidence.
- The recorded status chain had no gaps or forks.
- Two provider public keys participated in quote selection, provider-a won,
  and the unselected reservation was released.
- At the later network read, two configured relays and four provider profiles
  were ready.
- This is the only unique settled regtest swap ID found in the inspected local
  Omega thread store.

## What the record does not prove

- A global or industry-wide first. Electrum predates this receipt with a
  released Nostr submarine-swap implementation.
- That no earlier OpenAgents or Immortal swap occurred outside this local
  thread store.
- That every private RFQ, quote, contract, status, or close message traversed
  both Nostr relays. The thread has no event IDs or per-relay receipts.
- That the two relays were operated by independent organizations or failure
  domains. Different hostnames alone do not establish organizational
  decentralization.
- The exact Lightning amount received or fee paid.
- Atomicity, non-custody, refund readiness, or loss safety from the receipt
  alone. Those properties require the contract, scripts, invoice commitments,
  exit package, and verifier inputs.
- Mainnet readiness, economic liquidity, or movement of funds with market
  value.
- Independent verification. The terminal authority was the requester that
  initiated the swap.

## Historical-priority assessment

The broad “first-ever” hypothesis is rejected on currently available public
evidence. Electrum's 4.6.0 release notes state that its client used Nostr to
discover submarine-swap providers and perform related RPCs, that providers
advertised fees and liquidity on Nostr, and that providers did not require an
HTTP endpoint. That release predates this receipt by more than a year:

- [Electrum 4.6.0 release](https://github.com/spesmilo/electrum/releases/tag/4.6.0)
- [Electrum submarine-swap provider documentation](https://electrum.readthedocs.io/en/latest/swapserver.html)

Those sources do not, by themselves, identify the first successful Electrum
swap or prove that an earlier swap matched this exact OpenAgents regtest
topology. They are enough to prevent a responsible global novelty claim.

The narrower Omega milestone remains useful. The local store contains one
unique regtest swap ID, and this receipt captures the first such settlement
available in that store. A stronger OpenAgents-wide priority claim would need
a search of the Immortal provider database, relay archives, API request ledger,
deployment logs, and any other Omega profiles or machines, with a common lower
time bound and deduplication by request and swap ID.

## Sources and reproducibility

The owner-local evidence was read without modifying the database:

```sql
SELECT sequence, parent_sequence, event_json
FROM thread_events
WHERE thread_id = '760a4d36-f2bf-491c-951e-9bfac92e495e'
ORDER BY sequence;
```

The material events were sequences 31–34 for the rejected amount, first quote,
and failed execution; 37–38 for the fresh quote and settlement; 41 for the
status and network read; and 48 for the final plain-language identification.
`thread_events` does not store an event timestamp, so the thread creation and
update values above bound the conversation but do not timestamp settlement
more precisely.

The local first-in-store check searched all 35 threads and 789 events for
regtest `omega.market-demo.swap.v1` objects and deduplicated their `swap_id`
values. It found this swap at event sequences 38 and 41 in the source thread.

Omega commit
[`0d7ca48399223d71b6716e9aa520ce408dc0b94d`](https://github.com/OpenAgentsInc/omega/tree/0d7ca48399223d71b6716e9aa520ce408dc0b94d)
documents the client boundary used by the receipt:

- [`market_demo_tools.rs`](https://github.com/OpenAgentsInc/omega/blob/0d7ca48399223d71b6716e9aa520ce408dc0b94d/crates/agent/src/tools/market_demo_tools.rs)
  pins the manifest, relay fallbacks, public-regtest API, and regtest
  disclosures.
- Network status loads the manifest, queries provider-profile and offering
  events of kinds 39600 and 39601 from each relay, folds their current heads,
  and labels providers as pinned or discovered.
- Execution signs an HTTPS POST with NIP-98, requires a settled
  `openagents.market.regtest-swap.v1` response, and projects the returned
  provider, request, rail evidence, quote participants, and reservation release
  into the Omega tool receipt.

This audit preserves the result fields but does not publish wallet secrets,
preimages, credentials, or raw private swap messages.
