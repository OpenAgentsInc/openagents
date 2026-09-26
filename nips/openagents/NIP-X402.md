# NIP-X402 — Lightning-paid operations

`draft` `optional` — v1. The [shared contracts](contracts.md) are normative.
This specification is **Designed**, not implemented. It assigns no event
kinds. It combines x402 Lightning payments with Nostr capability discovery,
host spending policy, private purchase records, and recoverable execution.
It does not change [MKT](NIP-MKT.md) or [LAB](NIP-LAB.md) payment timing.

The upstream baseline is x402 v2 and its merged
[`exact` Lightning method](https://github.com/x402-foundation/x402/blob/4fcf836cc393174130e1358577ce5d37356da1c3/specs/schemes/exact/scheme_exact_lnbtc.md),
reviewed at commit `4fcf836cc393174130e1358577ce5d37356da1c3`. This NIP references
that contract; it is not a copy of its implementation. An upstream change
requires an explicit compatibility review, not silent reinterpretation.

## Scope and compatibility

There are three separately advertised roles:

| Role | Request binding | Compatibility |
| --- | --- | --- |
| HTTP client/server | Upstream `http:1`. | Standard x402 v2 HTTP headers and Lightning method, unchanged. Nostr can supply discovery, local authorization, and private evidence. |
| MCP client/server | Upstream `mcp:1`. | Standard x402 v2 MCP payment signaling, unchanged. MCP over HTTP still uses `mcp:1`. |
| Native Nostr buyer/provider | `nostr:openagents:1`, defined here. | An OpenAgents extension using x402 core types and Lightning validation. It is not an upstream-registered transport/profile and requires explicit support at both ends and the facilitator. |

A host MUST reject an unsupported profile. It MUST NOT disguise a native
Nostr purchase as HTTP, accept an unknown profile as `http:1`, or claim that
an ordinary upstream facilitator implements the native profile. Nostr
transport and x402 payment admission are distinct from wallet transport.
The first production implementation SHOULD support the standard HTTP path
before advertising native interoperability.

This contract purchases a single exact operation or resource before execution.
It supplies no escrow, credit, streaming balance, postpaid billing, automatic
refund, swap, or assurance of delivery. A provider can receive payment and
then fail. The client MUST admit that risk before paying. Long-running labor
paid after acceptance continues to use MKT/LAB. A worker's separately admitted
purchase of a tool is an input cost, not buyer acceptance of the worker's labor.

## Roles and identities

The buyer is the Nostr principal authorizing a native purchase; the provider
signs the capability and native payment records. The payment host controls a
separately admitted wallet binding. The receiver creates BOLT11 invoices;
`payTo` is its 33-byte compressed Lightning node key. A Nostr pubkey is a
32-byte x-only key. They are not interchangeable or implicitly derivable roles.

The resource server validates the actual request and controls execution. A
facilitator validates proofs and consumes them in durable state. It may run
inside the provider process. Neither a facilitator receipt nor a relay `OK`
proves delivery, quality, or wallet authorization. Lightning provides no stable
payer address: `SettlementResponse.payer` MUST remain absent. A separately
verified Nostr buyer remains application identity, never a fabricated Lightning
payer field.

The receiver key MUST have exclusive invoice-issuance authority for the
resource server's admitted merchant scope. A shared custodial key under which
untrusted tenants can issue invoices is not compatible with this method.
Nostr signatures cannot fix that attack. Hosts MUST pin the receiver-key
association and its operational assurance; discovery alone is insufficient.
All facilitators serving that receiver MUST share one authoritative replay
store, including across native, HTTP, and MCP entrances.

## Discovery without a new event kind

This NIP defines the CAP feature ID `oa-x402-v1`. A
[CAP](NIP-CAP.md) `adapter` definition using it includes that value in
`requires` and adds exactly one `x402` object to `binding_contract`. All
existing CAP fields and admission rules continue to apply. Other profiles do
not acquire this extension by implication. `binding_contract.interface` names
the operation input contract; `x402.v` names the payment descriptor below.

An `x402` object contains exactly these fields:

| Field | Meaning |
| --- | --- |
| `v` | `openagents.x402-discovery.v1`. |
| `protocol` | `x402-v2`. |
| `scheme`, `asset`, `method`, `flow` | Respectively `exact`, `BTC`, `bolt11`, `upfront`. |
| `bindings` | Nonempty distinct subset of `http:1`, `mcp:1`, `nostr:openagents:1`, compatible with the CAP transport. |
| `receivers` | Nonempty list of unique `{network, pay_to}` pairs using the concrete identifiers below and valid compressed receiver keys. |
| `merchant` | Provider-scoped nonempty opaque identifier, at most 128 ASCII bytes; a discovery identity, not a wallet credential. |
| `recovery` | `native-record-v1` for the native role; `none` or `provider-contract-v1` for HTTP/MCP. |
| `recovery_contract` | Exact ArtifactRef for the provider-specific recovery contract, or null unless `recovery` is `provider-contract-v1`. |

Use CAP's existing `remote` endpoint/worker/relay hints; native transport is
`nostr-cj` and requires `remote.worker` to equal the provider in this v1
profile. A definition may advertise several operations, but a purchase pins
one. HTTP/MCP locations and native worker keys MUST be verified against the
admitted host binding before disclosure. A recovery contract must identify
its authentication, lookup interface, idempotency, retention, and limits.
An unsupported recovery contract is not evidence of recoverability. The selected
challenge network and `payTo` must match an admitted descriptor receiver pair;
the buyer also checks the exact provider-signed CAP definition and operation.
A new key or endpoint requires a new reviewed binding, not automatic trust in
an invoice or redirect.

Discovery tags remain CAP tags. No invoice, preimage, purchase ID, account,
private request digest, or fee grant may appear in public tags or catalog
metadata. A descriptor is not a live challenge, price commitment, balance,
spend approval, or proof that the receiver/facilitator is correctly operated.
The provider MUST NOT advertise a binding before its configured path and
negative cases pass conformance. Relays advertise only envelope behavior,
not the enclosed x402 role, through NIP-11.

## Lightning wire rules

The upstream Lightning validation is mandatory for all three roles:

- Use x402 version `2`, scheme `exact`, asset `BTC`, method `bolt11`, and
  explicit `extra.paymentFlow: "upfront"`. A missing method defaults to
  `bolt11` only as upstream specifies. New producers SHOULD emit it explicitly.
- Mainnet is `lnbtc:000000000019d6689c085ae165831e93` with invoice currency `bc`.
  Testnet is `lnbtc:000000000933ea01ad0ee984209779ba` with currency `tb`.
  This v1 profile has no regtest, signet, on-chain, or `bip122:` alias.
- Amounts are positive decimal **millisatoshi strings**. One satoshi is
  `"1000"`. Bare user numbers never imply sats, dollars, or bitcoins.
  Conversions use checked exact arithmetic; invoice-less free operations do
  not construct zero-value x402 Lightning payments.
- Strictly verify BOLT11 encoding, signature and recovered/explicit signer,
  amount, currency, payment hash, creation time, exact invoice expiry versus
  `maxTimeoutSeconds`, and exactly one signed description hash with no inline
  description. The raw description hash MUST equal the locally constructed
  request digest. Unsigned copies in `extra` cannot replace this check.
- Payers must return a valid 32-byte preimage and an attributable paid result
  for the exact invoice, amount, and hash. A QR wallet with no preimage-return
  path is unsupported. An invoice, wallet notification, or claimed payment
  status alone cannot construct the proof.
- Settle consumes proof; it does not send the Lightning payment. Upfront flow
  MUST call settle before execution and MUST NOT call facilitator `/verify`.
  The proof preimage MUST hash to the accepted invoice's payment hash.
- Only `extra.invoice` is dynamic. The accepted invoice remains byte-identical
  to the one paid. The expected request binding and all other required terms
  come from actual request/configuration, never the client's echo.
- The invoice face amount is the x402 settled amount. Proof does not reveal
  actual received amount or routing fees. Overpayment grants no extra credit;
  this method defines no automatic refund. Unknown fees remain unknown.

The client pays only while its invoice is valid. The facilitator's distinct
paid-retry grace is `settlement_time <= invoice_end + skew`, with default
skew 60 seconds and inclusive equality. Grace is not permission to start a
new payment against an expired invoice. A paid proof that arrives too late
can be refused while the payment remains real. Recovery or a refund then
requires the separately admitted provider arrangement.

## HTTP and MCP binding

HTTP and MCP roles MUST implement the pinned upstream profiles exactly. Use
`PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`, and `PAYMENT-RESPONSE` for HTTP; use
upstream MCP payment signaling and `_meta["x402/payment"]` for tool calls.
The client must receive the actual outgoing request before payment, not only
`PaymentRequired`. The server independently reconstructs the operation it will
execute before matching accepted terms.

For HTTP, preserve the exact method, public target URI including query order
and escapes, content bytes at the specified decoding boundary, and every
configured operation/account-affecting header. Follow upstream RFC 9421
component rules and absent-versus-empty hashes. Do not follow redirects or
refresh a bound credential and reuse the old request digest silently.

[NIP-98](../official/98.md) can authenticate an HTTP request independently,
but an operation-relevant `Authorization` header must be in the upstream
binding. Refreshing its signed timestamp changes that binding. Keeping it
unchanged may exceed the service's freshness window while payment settles.
Initial paid admission MUST have a compatible bounded freshness policy, or a
separately specified prepayment authorization/reservation protocol. Otherwise
refuse this composition before payment. Separately authenticated result recovery
can retrieve only a purchase already durably admitted; it cannot fix expired
authorization on the first paid submission. A paid-but-unadmitted operation
retains that outcome without inventing a result. It MUST NOT omit the header
or hash only its pubkey instead. Normal authorization is rechecked on each
attempt.

For MCP, bind the configured server identity, tool name, unmodified arguments,
and all account/operation-affecting metadata. JSON-RPC IDs and progress tokens
are excluded upstream; their changes cannot buy a different operation.
Identity hidden only in a session or transport credential is insufficient;
represent account selection in bound arguments/metadata and separately check
that the authenticated caller may use it.

HTTP/MCP services can support application response caching or the upstream
`payment-identifier` extension. Neither defines a free pass around payment
validation. Scope cache keys by merchant and authenticated principal, bind
exact request and terms, and return only evidence of the original execution.
Do not use caller-chosen IDs alone as authorization or infer a durable cache
from the presence of an SDK helper. Keep upstream settle duplicate errors
unchanged; cached application retrieval is a separate operation.

## Native purchase records

Native records use the existing immutable `3188` private artifact envelope,
NIP-44 v2, exact author/recipient ACLs, random mailbox, and retained signed
provenance from the shared contracts. They are not new CJ kinds, public zap
receipts, plaintext NIP-78 data, or ephemeral-only payment history.

For this required CAP feature, `nostr-cj` names the purchased execution path;
purchase control itself uses `3188` on the pinned `remote.relays`. Both peers
authenticate using NIP-42 and subscribe to bounded private `3188` records
addressed to their exact pubkey before publishing. They verify/decrypt each
record, then route by its private purchase identity and verified issuer.
Random mailbox tags remain opaque. A control consumer must explicitly support
this NIP; a generic CJ worker does not understand these records automatically.
Retain/deduplicate exact artifact digests and retrieve bounded missing history;
a relay acknowledgment or missing subscription record is not settlement.

Every native record has exactly `v`, `requires`, `type`, `purchase`, `buyer`,
`provider`, `issuer`, `issued_at`, and `body`, plus optional inert `meta`:

- `v` is `openagents.x402-record.v1`; `requires` is empty in v1.
- `purchase` is a buyer-generated random 64-lowercase-hex nonce. Buyer and
  provider are distinct exact Nostr pubkeys. Their ordered triple identifies
  one application purchase independently of the invoice's payment hash.
- `issuer` equals the verified envelope signer. Recipient is the other party;
  local evidence copies to an admitted storage recipient retain original
  signed provenance and disclosure restrictions.
- `issued_at` is observational Unix seconds, not proof of payment or receipt.
- `type` and its closed `body` follow the table. ArtifactRefs include a schema;
  bytes, digest, and original declaring signature must all validate.

| Type and issuer | Required body |
| --- | --- |
| `request`, buyer | `{capability, operation, input, context, account, ceiling_msat, fee_ceiling_msat, valid_until, execute_until, recover_until}`. Capability is an exact CAP DefinitionRef; operation is an advertised ID; input is an ArtifactRef; context is an ArtifactRef or null. Account is a provider-scoped opaque string, or null when the operation has no account. Both ceilings are canonical nonnegative decimal strings; price ceiling is positive. `issued_at < valid_until <= execute_until < recover_until`; all are Unix seconds. |
| `challenge`, provider | `{request, required}`. Request is the exact buyer request ArtifactRef with provenance; required is an x402 `PaymentRequired` with exactly one native Lightning entry. |
| `claim`, buyer | `{request, challenge, payment}`. First two are ArtifactRefs; payment is the x402 `PaymentPayload` with the accepted invoice and secret proof. |
| `claim_rejected`, provider | `{request, claim, cause, status}`. The first two are ArtifactRefs to the same purchase; cause is a stable refusal code; status is the unchanged authoritative purchase status ArtifactRef or null. This per-claim refusal does not change the purchase phase. |
| `status_query`, buyer | `{request, after}`. Request is exact; after is a provider status ArtifactRef or null. Queries never dispatch or issue a new payment. |
| `status`, provider | `{request, seq, prev, claim, phase, settlement, run, output, cause, recover_until}` as defined below. |

No new record may mutate an existing request. A different request digest for
`(provider, buyer, purchase)` is `idempotency_conflict`, even if price/input
look similar. Envelope IDs may differ for retransmission or authorized copies;
the signed request artifact bytes and identity remain fixed. Missing bytes,
provenance, schema, or authority prevents payment/dispatch. Every referenced
request, challenge, claim, predecessor, and status must resolve to the same
buyer/provider/purchase triple and original request digest. Check each issuer
role independently. In particular, `claim.challenge.body.request` must equal
`claim.body.request`; a provider record from another purchase cannot be used
as authority merely because its amount or receiver matches.

A `status` chain starts with `seq: 0`, `prev: null`; each later status increments
one and references its predecessor. Conflicting bytes at the same sequence
are retained as equivocation and suspend automatic transitions. `claim` is an
ArtifactRef or null. `phase` is `offered`, `claim_pending`, `admitted`,
`running`, `completed`, `failed`, `refused`, or `unknown`. `settlement` is null
or an exact retained `SettlementResponse`; its presence does not imply
successful execution. `run` and `output` are ArtifactRefs or null; cause is a
stable code string or null. `recover_until` is a Unix-second provider commitment
to retain the purchase/status/result evidence, later than issuance.

Phase-specific evidence is mandatory:

| Phase | Required evidence and allowed next phase |
| --- | --- |
| `offered` | Claim, settlement, run, output, and cause are null. Next: `claim_pending` or `refused`. |
| `claim_pending` | Exact claim is present; settlement, run, output, and cause are null. Next: `admitted`, `refused`, or `unknown`. |
| `admitted` | Exact claim, successful settlement, and durable run-intent ArtifactRef; output and cause null. Next: `running`, `failed`, or `unknown`. |
| `running` | Preserve admission/claim/settlement and current run evidence; output and cause null. Next: `completed`, `failed`, or `unknown`. |
| `completed` | Successful admission plus output matching the purchased capability's pinned output schema and completed run evidence; cause null. Terminal. |
| `failed` | Preserve admitted claim, successful settlement, and run evidence; cause required. Output may be a retained partial result or null, never mislabeled completed. Terminal. |
| `refused` | No successful provider consumption or execution. Claim may be null; settlement is null or an explicit failure; run/output null; cause required. Buyer payment can still have happened. Terminal. |
| `unknown` | Retain every already established claim, settlement, and run fact; cause required. Resume only through authoritative reconciliation of that same attempt, not a fresh payment or execution. |

The first status is `offered` or a terminal pre-challenge `refused`, with
sequence zero. Offered may be committed together with its challenge. Later
records follow the transition table. Successful
settlement must match the referenced claim's concrete network and decoded
invoice payment hash, and omit payer. It cannot be replaced by a different
payment later. Unknown can resolve to the phase established by retained
evidence, but cannot erase known admission or bypass its execution identity.
Terminal status is immutable: queries return it, and a new claim rejection
is separate. No terminal phase reopens execution under the same purchase.

Run/intent/output evidence must bind this exact buyer/provider/purchase,
capability version, operation, input, context, account, and effective limits.
A signed unrelated run or merely resolvable ArtifactRef is insufficient.
Completion is the provider's attributable result, not independent LAB
acceptance. A missing status is unknown, not proof that the claim or operation
never happened. Clients record their wallet state separately from the provider
statement; refusal does not imply unpaid, and failed delivery does not imply
refunded.

`valid_until` is the latest time for a new purchase admission, not a ban on
retrieving already admitted work. `execute_until` bounds admitted execution;
`recover_until` is the requested minimum evidence/result-retention deadline.
By issuing a challenge, the provider commits to those bounds or refuses before
asking for payment. Status records must not shorten the promised recovery
window. Retention does not promise successful delivery. Hosts may require
longer retention for unresolved monetary liabilities. Challenge issuance,
wallet dispatch, and first purchase admission require `now < valid_until`;
equality refuses new admission. Execution dispatch requires `now < execute_until`
and a host-enforced deadline. These application bounds do not extend invoice
validity or settlement grace. A late paid claim may therefore remain spent but
unadmitted; it requires reconciliation rather than an automatic new payment.

## Native request binding

Native profile name is exactly `nostr:openagents:1`.
`extra.requestBindingParams` MUST be the empty object. No optional context
may influence the purchased operation outside the immutable request and its
pinned input/context bytes. The provider must separately authorize the exact
buyer for the bound account and resource. Invoice/challenge issuance is bounded
and requires that admission before allocating receiver resources.

Construct this object, with exactly these fields, from the validated request:

```json
{
  "domain": "x402:exact:lnbtc:bolt11:nostr:openagents:1",
  "buyer": "<buyer x-only pubkey>",
  "provider": "<provider x-only pubkey>",
  "purchase": "<64-hex purchase nonce>",
  "requestDigest": "sha256:<digest of the entire request artifact's JCS bytes>"
}
```

`requestHash = SHA256(UTF8(JCS(binding)))`, encoded lowercase hex in `extra`,
with its raw 32 bytes in BOLT11 `h`. The native resource URL is
`nostr:` followed by the NIP-19 `npub` encoding of the provider. It identifies
the provider only; operation authority comes from the bound request, never
from that URL. `PaymentRequired.resource.url` must equal that derived value.
The facilitator validates native profile syntax, empty parameters, matching
server-supplied expected hash, terms, signature, and proof using the same
Lightning checks. It need not receive the private input or original request;
it trusts its admitted resource server for the expected digest, as upstream
HTTP/MCP facilitators do.

The request includes no invoice, challenge reference, proof, or future CJ event
ID. This prevents a circular hash dependency. Price ceiling, account, capability,
input, context, and expiry are pinned by the request digest; actual price also
passes invoice/requirements checks and the request ceiling. The fresh payment
hash identifies an invoice, not the purchase's retry attempt. Two fresh
challenges for the same request and terms share a binding and may accept the
original still-valid unused proof, as upstream permits. A change to the buyer,
provider, purchase nonce, request bytes, or terms cannot reuse that payment.

A refreshed outer envelope can carry the same original artifact and provenance.
Its relay URL, `created_at`, event ID, mailbox, and transport acknowledgments
are not binding inputs. They may not select a different account, operation,
input, or authority. Fresh authorization can permit retrieval, but cannot
rewrite paid request bytes.

## Payment, settlement, and execution state

Before wallet dispatch the buyer host MUST durably record purchase identity,
exact request/challenge/invoice, network/payment hash, wallet binding, current
spend grant, price and fee reservations, and wallet-attempt identity. A
checkout, model prompt, capability definition, payment demand, or retrieval
result cannot grant `spend`. POL and the admitted host binding supply it.
Reserve the maximum admitted total across concurrent tasks/devices; reserve
price and fee separately. Fees are extra, never hidden in the invoice amount.
The shared `spend_microunits` bound is not an msat bound. Keep an exact-msat
purchase ledger and an explicit conservative conversion when composing a
parent currency budget; one micro-BTC is 100,000 msats. Never round a liability
down to zero or infer a conversion rate for another currency.

The wallet binding MUST enforce the stated fee and aggregate-spend bounds
before dispatch. A requested limit unsupported by the wallet is `cannot_enforce`.
A fee reported after the transfer is accounting, not prior enforcement. A
connection's whole-wallet allowance is not automatically a per-purchase cap.
The proof, credentials, and invoice MUST stay out of model prompts, public
traces, issue comments, URLs, and ordinary logs.

After timeout, cancellation, or lost wallet acknowledgment, lookup/reconcile
that same attempt and payment hash under the admitted adapter contract. Do not
pay a fresh invoice while the original attempt is paid, in flight, or unknown.
A refusal may release a reservation only when evidence establishes no payment
was dispatched or can still complete. Payment, claim consumption, entitlement,
execution, delivery, and refund are independent states.

For native providers, the first implementation SHOULD embed facilitator and
purchase storage in one transaction. It MUST atomically:

1. Validate the still-authorized bound request and paid proof.
2. Insert the canonical replay key `network + ":" + payment_hash` exactly once.
3. Record the admitted purchase, original successful settlement, and durable
   execution intent under `(provider, buyer, purchase)`.

Only after commit may the admitted host enqueue CJ/local execution. Work
carries a provider-controlled internal reference to that purchase; do not add
an unrecognized field to a CJ wire body or trust a buyer-supplied receipt as
an execution grant. The input ArtifactRef or admitted host mapping can retain
purchase linkage within the existing interface. Pin the actual run and attempt
before execution. A provider crash between intent and dispatch must recover
under RUN's effect rules, without rerunning an unknown external mutation.
The same paid operation must not be callable through an unguarded CJ/HTTP/MCP
alias: all entrances use the provider's payment and host-admission policy.
Payment does not bypass ordinary resource permissions.

Every facilitator instance shares the authoritative replay store. Retain keys
at least until `invoice_end + skew + 3600` seconds, and longer when needed for
unresolved attempts/recovery. Backup, restore, migration, and failover must not
lose unexpired consumption or durable intent. A relay event log alone supplies
neither atomic uniqueness nor this transaction.

A second settle for a consumed proof remains `duplicate_settlement`; it is
never a fresh successful settlement. For the same authorized purchase, return
or retrieve the already recorded status/result without another settle or run.
Unknown proof consumption is not repaired by treating that error as success.
If two distinct paid invoices target one native purchase, consume at most one
into its admitted execution; refuse the second as `purchase_already_admitted`
without a second dispatch. Emit `claim_rejected` referencing the unchanged
authoritative purchase status; never overwrite that purchase as refused.
Preserve the extra known payment for manual reconciliation; no automatic
refund or additional execution follows. This
application rule does not change upstream's rule that distinct payments can
buy distinct operations.

An external facilitator is optional. Before claiming recoverability, the
provider MUST have a separately specified authenticated way to reconcile an
ambiguous settle and atomically bind its evidence to the purchase intent.
x402's standard endpoints do not provide a general settle-status lookup.
A retry that returns `duplicate_settlement` alone does not identify which
server/purchase won. Without retained authoritative evidence, leave entitlement
unknown and do not execute, repay, release liability, or switch replay stores.
Native status queries resume evidence retrieval; they do not mint another
claim, invoice, or execution. New paid attempts after a terminal unpaid refusal
require a new admitted purchase identity.

## Wallets, zaps, and related protocols

[NIP-47](../official/47.md) is the preferred optional Nostr wallet transport:
separate wallet-connection keys, NIP-44 where negotiated, signed response
correlation, and `pay_invoice` preimage validation. It remains a separately
admitted host adapter. Core NWC alone does not prove fee enforcement, durable
idempotency, or recovery. Pin any used external NWC extension and verify actual
wallet behavior. An extension advertising `max_fee` but permitting it to be
ignored cannot satisfy a hard host bound. Receiver adapters also need exact
amount invoices with caller-supplied description hashes and isolated issuance.

[NIP-57](../official/57.md) zaps are a separate optional social payment. Zap
invoices commit their description hash to the signed zap request. x402 invoices
commit it to the transport-bound operation. One invoice MUST NOT be represented
as satisfying both different commitments. A zap receipt can include a preimage;
never publish an x402 invoice/proof as a zap receipt to obtain a social badge.
A separately authorized tip is a separate payment and disclosure decision.
Recipient-issued receipts do not independently establish x402 consumption or
LAB acceptance.

[NIP-A3](../official/A3.md) and Lightning addresses can help discover payment
targets. They neither produce the required bound invoice nor replace its
receiver-key checks. Generic LNURL-pay does not guarantee an arbitrary supplied
x402 description hash. NIP-60/61 Cashu wallets/nutzaps are different proofs with
mint trust; L402/LSAT uses macaroons plus payment, not x402's payload. None is an
implicit fallback. NIP-42 authenticates a relay connection; NIP-98 authenticates
HTTP; neither pays. Protocol conversions require a separate reviewed profile.

## Bounds, receipts, and conformance

Native records obey shared body/depth/string bounds and use at most one
payment option per challenge. Any lower admitted transport bound wins. This
v1 native role requires positive amounts and both ceilings to fit the shared
safe-integer range after decimal parsing; larger values refuse rather than
round. No unbounded polling, invoice issuance, artifact fetch, or wallet retry
is permitted. Hosts pin maximum attempts, bytes, concurrency, and deadlines.

RUN evidence records requested/admitted price, actual invoice amount, actual
fees or unknown, wallet attempt identity, payment and claim states, provider
admission, execution timing, output, and recovery cause separately. General
traces contain digests and authorized references, not bearer proof. Private
wallet evidence requires its own encrypted retention and access policy; redacted
copies cannot be used as cryptographic originals. Preimage knowledge proves
possession under the receiver trust model, not an independent assertion of
actual received total, delivery, or payer identity.

Implementations MUST pass at least these cases before advertising the role:

- Upstream HTTP/MCP digest and signed-invoice vectors, malformed invoices,
  invalid signer/network/amount, missing binding, unknown profile, boundary
  expiry/skew, and actual-request recomputation. Body/credential mutation and
  same-price cross-route proofs refuse before execution.
- Native binding vectors, wrong signer/recipient/account, missing signed input,
  changed request under the same purchase, fresh envelope with stable artifact,
  same-request concurrent challenges, and distinct-purchase isolation.
- Wallet fee-cap refusal, unsupported preimage, wrong invoice/hash/amount,
  duplicate responses, ambiguous payment, revocation, and multi-task budget
  contention. No replacement payment while the first can still settle.
- Cross-process proof races, crash before/after each atomic write, restore and
  facilitator failover, duplicate settlement, response loss, and paid-result
  recovery. Payment/intent commit must not permit duplicate execution.
- Expired-but-paid refusal, failed paid handler, two paid invoices for one
  native purchase, incomplete output, unavailable recovery evidence, and
  separately admitted refunds without erasing original spending.
- Anonymous/unrelated-reader refusal on stored/live/COUNT/ID/search paths;
  no bearer proof in tags, logs, exported traces, zaps, or model context.
- Current CAP/host/worker support, no advertised unsupported native profile,
  no fabricated Lightning payer, and unchanged MKT/LAB postacceptance behavior.

The [binding vectors](../../docs/protocol/fixtures/x402-lightning-bindings-v1.json)
check canonical digest inputs only. They are not invoice-signature, wallet,
replay-store, relay-privacy, or paid-service conformance evidence. The
[integration assessment](../../docs/coder/design/x402-lightning-nostr-integration.md)
records upstream code gaps, adjacent proposals, implementation order, and
release evidence required before real spending.
