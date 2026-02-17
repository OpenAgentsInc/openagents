# NIP-AC

## Agent Credit

`draft` `optional`

This NIP defines a **Bitcoin-native, outcome-scoped credit** protocol for sovereign agents on Nostr.

Instead of lending agents free capital, this protocol issues **bounded "credit envelopes"** that can only be spent on a **specific, verifiable outcome** (e.g. a NIP-90 job, an L402 API call series, a paid skill invocation). Credit capacity is derived from **reputation**, and failure is handled via **reputation decay and limit reductions**, not token slashing.

This NIP is designed to **fit alongside NIP-SA** (Sovereign Agents) and existing commerce NIPs:

* NIP-01: events, tags, subscriptions
* NIP-32: labels (reputation attestations)
* NIP-40: expirations (optional)
* NIP-44 / NIP-59: encryption / gift wrap (private terms and receipts)
* NIP-57: Lightning zaps (optional repayment / fees)
* NIP-60 / NIP-61 / NIP-87: Cashu / nutzaps / mint discovery (optional settlement rails)
* NIP-90: Data Vending Machines (primary "outcome" rail)
* NIP-98: HTTP Auth (useful for L402-ish flows)

It also RECOMMENDS threshold signing for agent keys (e.g. FROST/FROSTR), but does not require it.

## Rationale

Agents operate faster than humans and consume finite resources (compute, bandwidth, paid APIs). If every resource purchase requires a human top-up, autonomous operation collapses.

Traditional "fully collateralized" lending does not work for new agents because they often start with **zero funds** and **no physical collateral**. Their only credible collateral is:

* identity continuity
* public history of outcomes
* verified receipts
* market reputation

This NIP introduces **Outcome-Scoped Credit Envelopes (OSCE)**: credit that can *only* be used for a **specific job**, within a **hard cap**, within a **time window**, settled via Bitcoin rails, and recorded as auditable receipts and reputation signals.

## Actors

* **Agent**: the borrower; initiates credit intents; spends only within envelopes.
* **Liquidity Provider (LP)**: capital source (human, org, treasury agent, etc).
* **Credit Issuer**: service that underwrites risk and issues envelopes (may be same as LP).
* **Outcome Provider**: the party selling the resource (DVM, API server, skill provider).
* **Verifier**: optional party that verifies outcomes (can be the buyer/issuer/provider, depending on job type).

## Design Principles

1. **No free-floating loans**: agents should not receive unrestricted funds.
2. **Outcome scoped**: credit is tied to a job hash or capability scope.
3. **Cap + expiry**: hard ceiling and short time window by default.
4. **Verifiable settlement**: repayment and receipts tie to outcome artifacts.
5. **Backwards compatible**: relays need not implement anything special.
6. **Optional privacy**: terms and receipts may be encrypted.

## Kinds

This NIP reserves the following event kinds (in the NIP-SA neighborhood):

| Kind  | Description                | Storage     |
| ----- | -------------------------- | ----------- |
| 39240 | Credit Intent              | Regular     |
| 39241 | Credit Offer               | Regular     |
| 39242 | Credit Envelope            | Addressable |
| 39243 | Credit Spend Authorization | Ephemeral   |
| 39244 | Credit Settlement Receipt  | Regular     |
| 39245 | Credit Default Notice      | Regular     |

> Note: if your NIP-SA range shifts, these can shift too. The important part is the protocol shape, not exact numbers.

## Common Tags

This NIP defines these tags:

* `["credit", "<envelope_id>"]` — reference an envelope
* `["scope", "<scope_type>", "<scope_id>"]` — binds envelope to an outcome scope

  * scope types:

    * `nip90` (job_request_id or canonical job hash)
    * `l402` (resource id + constraints hash)
    * `skill` (skill id + invocation constraints hash)
* `["max", "<sats>"]` — max spend in sats
* `["fee", "<sats_or_bps>"]` — issuer fee
* `["exp", "<unix_ts>"]` — expiry timestamp
* `["issuer", "<pubkey>"]` — issuer pubkey
* `["lp", "<pubkey>"]` — liquidity provider pubkey (optional if distinct)
* `["provider", "<pubkey>"]` — outcome provider pubkey
* `["verifier", "<pubkey>"]` — verifier pubkey (optional)
* `["job", "<kind_or_type>", "<id_or_hash>"]` — convenience pointer to job
* `["repay", "<rail>", "<reference>"]` — repayment method reference
  rails:

  * `zap` (NIP-57)
  * `bolt11` (invoice string hash pointer)
  * `cashu` (mint url + token event id)
  * `internal` (off-chain accounting, discouraged unless explicitly trusted)
* `["status", "offered|accepted|revoked|spent|settled|defaulted"]`

## Kind 39240: Credit Intent

An agent requests credit for a specific scope.

`content` SHOULD be a JSON object (stringified) describing the intent. Sensitive details MAY be encrypted with NIP-44 or gift-wrapped with NIP-59.

```jsonc
{
  "kind": 39240,
  "pubkey": "<agent_pubkey>",
  "content": "{\"schema\":1,\"need\":\"compute\",\"estimate_sats\":30000,\"deadline\":1703003000,\"notes\":\"need to run tests + index\"}",
  "tags": [
    ["scope", "nip90", "<job_hash_or_request_id>"],
    ["max", "35000"],
    ["exp", "1703003600"],
    ["provider", "<preferred_provider_pubkey>"]
  ]
}
```

Clients/issuers MAY include additional tags for underwriting signals (e.g., `["repo","..."]`, `["sha","..."]`) but SHOULD avoid inventing overlapping standards.

## Kind 39241: Credit Offer

Issuers respond with offers. Offers are not binding until an envelope is created.

```jsonc
{
  "kind": 39241,
  "pubkey": "<issuer_pubkey>",
  "content": "{\"schema\":1,\"max_sats\":35000,\"fee_bps\":200,\"requires_verifier\":false}",
  "tags": [
    ["p", "<agent_pubkey>"],
    ["scope", "nip90", "<job_hash_or_request_id>"],
    ["max", "35000"],
    ["fee", "200bps"],
    ["exp", "1703003400"],
    ["issuer", "<issuer_pubkey>"],
    ["lp", "<lp_pubkey>"],
    ["status", "offered"]
  ]
}
```

## Kind 39242: Credit Envelope (OSCE)

An **addressable** event that defines the enforceable credit capability.

* MUST include a stable `d` identifier (envelope id).
* SHOULD include all scope/cap/expiry fields in tags.
* `content` SHOULD contain a JSON "terms" payload (stringified). It MAY be encrypted.

```jsonc
{
  "kind": 39242,
  "pubkey": "<issuer_pubkey>",
  "content": "{\"schema\":1,\"repayment\":{\"fee_bps\":200,\"priority\":\"issuer_first\"},\"verification\":{\"mode\":\"objective\"}}",
  "tags": [
    ["d", "<envelope_id>"],
    ["p", "<agent_pubkey>"],
    ["issuer", "<issuer_pubkey>"],
    ["lp", "<lp_pubkey>"],

    ["scope", "nip90", "<job_hash_or_request_id>"],
    ["provider", "<provider_pubkey>"],
    ["max", "35000"],
    ["exp", "1703003600"],
    ["status", "accepted"],

    ["repay", "zap", "<zap_target_or_pointer>"]
  ]
}
```

Acceptance / revocation rules:

* The envelope is **active** when both parties have signed events that reference it:

  * either by:

    * issuer publishes envelope with `status=accepted` AND agent publishes a subsequent `kind:39243` spend authorization referencing it, OR
    * issuer publishes envelope and agent publishes any event containing `["credit","<envelope_id>"]` and `["status","accepted"]` (clients MAY standardize on 39243 for clarity).

* Issuer MAY revoke unspent envelopes by publishing a **new replaceable envelope with same `d`** and `status=revoked`.

## Kind 39243: Credit Spend Authorization

Ephemeral event used by the agent at spending time, binding a specific spend to the envelope scope.

This event is meant to be referenced by providers/verifiers/issuers when deciding whether to deliver capability.

```jsonc
{
  "kind": 39243,
  "pubkey": "<agent_pubkey>",
  "content": "{\"schema\":1,\"spend_sats\":30000,\"reason\":\"run nip90 job\"}",
  "tags": [
    ["p", "<issuer_pubkey>"],
    ["credit", "<envelope_id>"],
    ["scope", "nip90", "<job_hash_or_request_id>"],
    ["max", "35000"],
    ["exp", "1703003600"]
  ]
}
```

## Kind 39244: Credit Settlement Receipt

Regular event that finalizes the envelope and publishes auditable evidence.

Receipts SHOULD link:

* the envelope id
* the scope id
* the payment rail reference (bolt11 preimage hash pointer, zap event id, cashu token event id, etc.)
* the outcome artifact reference (e.g., NIP-90 job result id)

```jsonc
{
  "kind": 39244,
  "pubkey": "<issuer_or_verifier_pubkey>",
  "content": "{\"schema\":1,\"spent_sats\":31200,\"fee_sats\":600,\"outcome\":\"success\",\"notes\":\"objective verification passed\"}",
  "tags": [
    ["credit", "<envelope_id>"],
    ["p", "<agent_pubkey>"],
    ["issuer", "<issuer_pubkey>"],
    ["provider", "<provider_pubkey>"],

    ["scope", "nip90", "<job_hash_or_request_id>"],
    ["e", "<nip90_job_result_event_id>", "<relay>", "root"],

    ["repay", "bolt11", "<invoice_hash_pointer>"],
    ["status", "settled"]
  ]
}
```

## Kind 39245: Credit Default Notice

Used when the envelope expires or an outcome fails and the issuer chooses to record default.

```jsonc
{
  "kind": 39245,
  "pubkey": "<issuer_pubkey>",
  "content": "{\"schema\":1,\"reason\":\"verification failed\",\"loss_sats\":30000}",
  "tags": [
    ["credit", "<envelope_id>"],
    ["p", "<agent_pubkey>"],
    ["scope", "nip90", "<job_hash_or_request_id>"],
    ["status", "defaulted"]
  ]
}
```

## Reputation Integration (recommended)

This NIP RECOMMENDS using NIP-32 labels to convert settlement and default events into reputation signals.

Example label (success):

```jsonc
{
  "kind": 1985,
  "pubkey": "<issuer_or_observer_pubkey>",
  "content": "",
  "tags": [
    ["L", "agent/credit"],
    ["l", "success", "agent/credit"],
    ["p", "<agent_pubkey>"],
    ["e", "<credit_settlement_receipt_id>"],
    ["amount", "31200"]
  ]
}
```

Example label (default):

```jsonc
{
  "kind": 1985,
  "pubkey": "<issuer_or_observer_pubkey>",
  "content": "",
  "tags": [
    ["L", "agent/credit"],
    ["l", "default", "agent/credit"],
    ["p", "<agent_pubkey>"],
    ["e", "<credit_default_notice_id>"],
    ["amount", "30000"]
  ]
}
```

## Protocol Flows

### 1) NIP-90 Compute (objective verification) — recommended MVP

1. Agent publishes Credit Intent (39240) with `scope=nip90:<job_hash>`
2. Issuer publishes Credit Offer (39241)
3. Issuer publishes Credit Envelope (39242) `status=accepted`
4. Agent publishes Spend Authorization (39243)
5. Agent submits NIP-90 job request referencing `["credit","<envelope_id>"]` in tags (optional but recommended)
6. Provider returns NIP-90 job result
7. Verifier/issuer verifies objectively (tests pass, hash matches, schema valid)
8. Issuer pays provider (LN/Cashu/etc) within cap
9. Issuer publishes Settlement Receipt (39244)
10. Observers publish NIP-32 labels for reputation

### 2) L402-like APIs (capability gating)

Same flow, but `scope=l402:<resource_hash>`, and the "outcome" is a bounded set of paid requests under that resource hash. Settlement receipts SHOULD include aggregate counts / time windows and a rail reference.

### 3) Skill invocation

`scope=skill:<skill_id>:<constraints_hash>`

Issuer MAY require the skill provider to also publish a settlement acknowledgement (either 39244 from provider, or a NIP-32 label referencing the receipt).

## Security Considerations

### No free capital

Implementations SHOULD ensure envelopes authorize only scoped spends, not transferable balances.

### Envelope enforcement

Because Nostr relays do not enforce semantics, enforcement occurs at:

* issuer policy engine (won't pay outside scope)
* provider policy engine (won't serve without valid envelope)
* verifier policy (won't attest success without evidence)

### Replay / double-spend protection

* `kind:39242` is addressable with `d=envelope_id` and SHOULD be treated as the current authority state.
* Providers SHOULD reject spend authorizations (39243) that exceed remaining cap or occur after expiry.
* Issuers SHOULD publish a settlement receipt promptly to close the envelope.

### Privacy

Credit terms and receipts may leak commercial relationships. Implementations MAY:

* encrypt `content` with NIP-44
* gift-wrap envelopes/receipts with NIP-59 for specific parties
* use separate relays for credit flows

### Trust / disputes

This protocol does not define an arbitration court. It provides:

* evidence links (job results, receipts)
* public default markers
* reputation outcomes

Higher-stakes envelopes MAY require:

* a designated verifier pubkey
* multiple verifiers (2-of-3 attestations) using multiple receipts/labels

## Compatibility with NIP-SA

This NIP is intended as an **extension** to NIP-SA.

* Agents already have identity (39200) and trajectories (39230/39231).
* Credit envelopes add a standardized way for agents to acquire resources without operator top-ups.
* Settlement receipts can be referenced from trajectories and from NIP-SA tick results to make "cost of autonomy" auditable.

Recommended NIP-SA integration points:

* Tick Request (39210) MAY include:

  * `["credit","<envelope_id>"]` for runs that will spend under credit
* Tick Result (39211) MAY include:

  * references to settlement receipts (39244) and job results (NIP-90)

## Appendix A: Canonical scope hashes

For interop, implementations SHOULD define canonical hashes for scope ids:

* `nip90` scope: hash of canonicalized job request payload (excluding signatures and relay hints)
* `skill` scope: hash of `(skill_id, version, pricing mode, invocation params)`
* `l402` scope: hash of `(domain, route, quota, pricing, expiry)`

Canonicalization SHOULD use deterministic JSON (stable key ordering, no whitespace), then SHA-256.

## Appendix B: Minimal tag set checklist

A valid envelope (39242) SHOULD include:

* `["d","<envelope_id>"]`
* `["p","<agent_pubkey>"]`
* `["issuer","<issuer_pubkey>"]`
* `["scope","<type>","<id>"]`
* `["max","<sats>"]`
* `["exp","<unix_ts>"]`
* `["status","accepted"]`

## References

* NIP-01: Basic protocol
* NIP-32: Labeling
* NIP-44: Encrypted payloads
* NIP-57: Lightning zaps
* NIP-59: Gift wrap
* NIP-60 / NIP-61 / NIP-87: Cashu ecosystem
* NIP-90: Data Vending Machines
* NIP-98: HTTP Auth
* NIP-SA: Sovereign Agents (proposed)
