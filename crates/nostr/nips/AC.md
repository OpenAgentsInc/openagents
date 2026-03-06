NIP-AC
======

Agent Credit
------------

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
* Fedimint: federation ecash rails (federation discovery follows NIP-SA `federation` tag conventions; no dedicated NIP at time of writing)
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
| 39246 | Credit Cancel Spend        | Regular     |

> Note: if your NIP-SA range shifts, these can shift too. The important part is the protocol shape, not exact numbers.

## Common Tags

This NIP defines these tags:

* `["credit", "<envelope_id>"]` — reference an envelope
* `["scope", "<scope_type>", "<scope_id>"]` — binds envelope to an outcome scope

  * scope types:

    * `nip90` (job_request_id or canonical job hash)
    * `l402` (resource id + constraints hash)
    * `skill` (canonical SKL skill_scope_id + invocation constraints hash)
* `["a", "33400:<skill_npub>:<d-tag>"]` — recommended when `scope_type=skill`, points at SKL skill address
* `["e", "<skill_manifest_event_id>"]` — recommended version pin when `scope_type=skill`
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
  * `bolt12` (offer or settlement reference)
  * `cashu` (mint url + token event id)
  * `fedimint` (federation-id@domain + redemption reference)
  * `internal` (off-chain accounting, discouraged unless explicitly trusted)
* `["spend_rail", "<rail>", "<reference>"]` — provider-facing spending rail (distinct from `repay`)
  rails:

  * `lightning` (bare default; implies bolt11 when `spend_rail` is absent — backwards compatible)
  * `bolt11` (explicit: invoice hash pointer)
  * `bolt12` (offer or settlement reference)
  * `cashu` (mint url)
  * `fedimint` (federation-id@domain)
* `["spend_cashu_keyset", "<keyset-id>"]` — optional keyset pin when `spend_rail=cashu`
* `["guardian", "<pubkey>"]` — guardian pubkey required to co-approve high-spend ticks (SA-Guardian Profile)
* `["approval_threshold", "<sats>"]` — spend amount in sats above which `guardian` approval is required at the envelope level
* `["revoke_reason", "<reason>", "<detail>"]` — machine-readable reason for envelope revocation (e.g., `skl-safety-label`)
* `["status", "offered|accepted|revoked|spent|settled|defaulted"]`

Providers SHOULD declare the spending rails they accept on their NIP-90 `kind:31990` service announcements. Issuers MUST verify that the agent's declared `spend_rail` is accepted by the target provider before issuing an envelope. If `spend_rail=cashu`, the provider MUST accept Cashu tokens at the declared mint as payment for job results, and the agent MUST NOT use a different mint than declared in the envelope. When `spend_rail` is absent, `lightning` (bolt11) is assumed for backwards compatibility; implementations SHOULD prefer `bolt12` where supported. Parsers MUST handle `repay` and `spend_rail` tags with 3 or more elements and MUST NOT reject events containing additional elements.

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
    ["provider", "<preferred_provider_pubkey>"],

    // optional: declare preferred spending rail
    ["spend_rail", "lightning"],              // default — implies bolt11
    // or: ["spend_rail", "bolt11", "<invoice-hash-pointer>"]  // explicit bolt11
    // or: ["spend_rail", "bolt12", "<offer-hash-pointer>"]
    // or: ["spend_rail", "cashu", "<mint-url>"]
    // or: ["spend_rail", "fedimint", "<federation-id>@<domain>"]
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

    ["repay", "cashu", "<mint-url>", "<token-event-id>"],  // repayment rail back to issuer/LP

    // optional: provider-facing spending rail
    ["spend_rail", "cashu", "<mint-url>"],
    ["spend_cashu_keyset", "<keyset-id>"]
  ]
}
```

Issuers MAY include `spend_rail` and (optionally, when `spend_rail=cashu`) `spend_cashu_keyset` tags in the envelope to declare the provider-facing spending rail. This is distinct from `repay`, which defines the agent's repayment rail back to the issuer or LP. When `spend_rail` is absent, implementations SHOULD assume `lightning` (bolt11). Detailed proof construction for non-Lightning rails is intentionally out of AC core scope; `repay` and `spend_rail` tags SHOULD carry stable, rail-appropriate references agreed by participating implementations.

### Conditional Reversibility (Cancel Window)

Envelopes MAY include a `cancel_until` tag to create a reversibility window for consequential spends:

```jsonc
["cancel_until", "1703003300"]
```

When present, credit spends against this envelope are committed but not final until `cancel_until`. During this window, the guardian or issuer MAY publish a `kind:39246` Cancel Spend event to reclaim the committed tokens. If `cancel_until` is absent, or if it has already passed when the spend authorization is processed, the spend is final immediately.

#### Cancel Spend Event (`kind:39246`)

```jsonc
{
  "kind": 39246,
  "pubkey": "<guardian_or_issuer_pubkey>",
  "created_at": 1740500150,
  "tags": [
    ["e", "<spend_authorization_event_id>"],
    ["credit", "<envelope_id>"],
    ["reason", "suspicious_tool_invocation"]
  ],
  "content": ""
}
```

The cancel event MUST be published before `cancel_until`. Providers SHOULD treat canceled spends as void. After `cancel_until`, cancel events MUST be ignored.

#### Normative Requirements

- Envelopes that use reversible spending SHOULD include `cancel_until`.
- Providers receiving spend authorizations against cancel-window envelopes SHOULD wait until `cancel_until` before delivering irreversible resources.
- Guardians and issuers MAY publish `kind:39246` cancel events before `cancel_until`.
- Implementations MUST treat the cancel window as a soft escrow — the tokens are committed (deducted from remaining cap) but not transferred until finalization.

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
* the payment rail reference (bolt11 invoice hash, bolt12 offer hash, cashu token event id, fedimint redemption hash, etc.)
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

    // repayment rails (use one per receipt):
    ["repay", "bolt11", "<invoice-hash-pointer>"],
    // or: ["repay", "bolt12", "<offer-or-settlement-reference>"]
    // or: ["repay", "cashu", "<mint-url>", "<token-event-id>"]
    // or: ["repay", "fedimint", "<federation-id>@<domain>", "<redemption-reference>"]
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

`scope=skill:<skill_scope_id>:<constraints_hash>`

Where `skill_scope_id` SHOULD follow NIP-SKL canonical form:

`33400:<skill_npub>:<d-tag>:<version>`

Credit events for skill scopes SHOULD also include:

- `["a", "33400:<skill_npub>:<d-tag>"]`
- `["e", "<skill_manifest_event_id>"]`

This keeps underwriting and settlement tied to a concrete SKL identity/version pair.

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

### SKL Safety Label Revocation Trigger

When an agent holds a `scope=skill` envelope and a NIP-32 safety label (`kind:1985`) is published against the referenced skill manifest with a negative label value (for example, `malicious-confirmed`, `prompt-injection`, or `capability-violation`), issuers SHOULD treat this as a revocation trigger for any active envelopes scoped to that skill.

Issuers implementing this policy SHOULD:

1. Subscribe to `kind:1985` events referencing skill addresses they have issued envelopes against:

   ```json
   {"kinds": [1985], "#a": ["33400:<skill_pubkey>:<d-tag>"]}
   ```

2. On receipt of a negative label from a trusted labeler (per local quorum policy), publish a replacement `kind:39242` with `status=revoked` and include:

   ```jsonc
   ["revoke_reason", "skl-safety-label", "<label-value>"],
   ["e", "<kind:1985-label-event-id>"]
   ```

3. Publish a `kind:39245` Credit Default Notice if the envelope was partially spent.

This connects NIP-SKL's third-party safety attestation layer directly to NIP-AC's economic enforcement layer without changing either NIP's core semantics.

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

## Compatibility with NIP-SKL

When `scope_type=skill`, implementations SHOULD use NIP-SKL canonical manifest identity:

- `skill_scope_id = 33400:<skill_npub>:<d-tag>:<version>`
- `scope tag = ["scope","skill","<skill_scope_id>:<constraints_hash>"]`
- `a tag = ["a","33400:<skill_npub>:<d-tag>"]`
- `e tag = ["e","<skill_manifest_event_id>"]` (recommended version pin)

This keeps AC envelopes aligned to a specific SKL manifest version and prevents ambiguous credit authorization across upgraded skill versions.

### Guardian-gated envelopes (SA-Guardian Profile)

When an agent operates under a NIP-SA guardian approval profile, credit envelopes MAY be issued with a guardian constraint:

```jsonc
{
  "kind": 39242,
  "tags": [
    // ... existing tags ...
    ["guardian", "<guardian-pubkey>"],      // required co-approver
    ["approval_threshold", "5000"]          // sats above which gate fires (envelope-level)
  ]
}
```

Providers receiving a `kind:39243` Spend Authorization that references a guardian-gated envelope MUST verify that either:

1. The spend amount is below `approval_threshold`, or
2. A valid `kind:39213` Guardian Approval event (defined in NIP-SA) exists, signed by the declared `guardian` pubkey, referencing the spend authorization event.

This allows credit issuers to enforce human-in-the-loop approval without changing the core NIP-AC event flow: the guardian gate is a constraint on the envelope, not a protocol change.

## Appendix A: Canonical scope hashes

For interop, implementations SHOULD define canonical hashes for scope ids:

* `nip90` scope: hash of canonicalized job request payload (excluding signatures and relay hints)
* `skill` scope: hash of `(skill_scope_id, pricing mode, invocation params)`
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

Optional constraint tags (include when applicable):

* `["guardian","<pubkey>"]` — SA-Guardian Profile co-approver
* `["approval_threshold","<sats>"]` — envelope-level spend threshold above which guardian approval fires
* `["spend_rail","<rail>"]` — provider-facing payment rail (defaults to `lightning` / bolt11 when absent)
* `["spend_cashu_keyset","<keyset-id>"]` — optional Cashu-only keyset pin when `spend_rail=cashu`
* `["revoke_reason","<reason>","<detail>"]` — include on replacement event when revoking
* `["cancel_until","<unix_ts>"]` — reversibility window for consequential spends (see §Conditional Reversibility)

## Changelog

**v4 (2026-03-05) — NIST AI Agent Standards Alignment**

- Added Conditional Reversibility / Cancel Window (`cancel_until` tag, `kind:39246` Cancel Spend event) for reversible consequential spends (§Conditional Reversibility).
- Added `cancel_until` and `spend_cashu_keyset` to Appendix B optional tag checklist.
- Satisfies requirements from: CAISI RFI NIST-2025-0035 (Jan 2026) — rollback/undo for unwanted agent actions.

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
* NIP-SKL: Agent Skill Registry (proposed)
* BOLT 12: Lightning offers specification (https://bolt12.org)
