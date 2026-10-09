# `crates/x402` merchant-facilitator — API sketch (draft, 2026-10-09)

**Decision (owner, 2026-10-09):** adopted as the merchant-settlement profile for third-party sellers, coexisting with the custodial receiver for our own sales. OpenAgents supports every agent payment protocol; see [Agent payments: pay any way](../agent-payments.md).

**Status:** Design sketch for discussion. Not implemented. Rust below is
illustrative, not compiled.
**Purpose:** Show how a third-party merchant runs its own X402 receiver + facilitator
so agents settle **directly to the merchant**, by (a) *reusing* the existing
`crates/x402` surface and (b) adding a small, clearly-marked set of new pieces.
**Companions:** [deployment profile](2026-10-09-x402-merchant-receiver-profile.md) ·
[non-custodial rail proposal](2026-10-09-non-custodial-agent-commerce-rail.md).
**Grounding:** every reused type below is real, with `crates/x402/src/...` anchors.

---

## 0. Design constraints (match the crate's house style)

The sketch deliberately conforms to what `crates/x402` already is:

- **Synchronous.** No `tokio`, no `async fn`. The server is one OS thread per
  connection (`front::serve`, `crates/x402/src/front.rs:1587`). Every new trait is
  sync too.
- **Generic on the store, erased elsewhere.** `ReplayStore` is a generic parameter
  (`Facilitator<S>`, `Front<S>`) so the hot settle path monomorphises;
  `Receiver`, `SettlementSink`, `RouteExecutor` are `Arc<dyn …>`. New collaborators
  follow the same rule: hot/uniqueness path = generic, policy hooks = trait objects.
- **Errors:** `thiserror` enums with `#[from]` for structured cases; `Result<_, String>`
  for trait/domain methods; `&'static str` for the upstream `errorReason` vocabulary.
  No `anyhow`, no `Result` aliases.
- **Reuse, don't fork.** The merchant path is an *assembly* of existing pieces plus a
  thin policy layer — not a second facilitator.

### What is reused vs. new

| Piece | Status | Where |
| --- | --- | --- |
| `Receiver` (issue exact invoice on `payTo`) | ♻️ reuse | `server.rs:25` |
| `ReplayStore` / `FileReplayStore` / `ReplayEntry` | ♻️ reuse | `replay.rs` |
| `Facilitator<S>` / `verify` / `settle` / `Admission` | ♻️ reuse | `facilitator.rs` |
| `Front<S>` / `Config` / `Route` / `RouteExecutor` / `serve` | ♻️ reuse | `front.rs` |
| `SettlementSink` | ♻️ reuse (merchant impl) | `front.rs:251` |
| Wire types `PaymentRequired`/`PaymentPayload`/`SettlementResponse` | ♻️ reuse | `wire.rs` |
| `MerchantReceiver` scope wrapper | 🆕 new | `merchant.rs` (proposed) |
| `BuyerAttestation` + `Attestation` (the moat hook) | 🆕 new | `merchant.rs` |
| `MerchantSink` (record sale, no split/payout) | 🆕 new | `merchant.rs` |
| Platform fee as a *second* requirement (`FeePolicy`, `FeeReceiver`) | 🆕 new (needs spec review) | `merchant.rs` |
| `DelegatedReceiver` (topology 4B, managed facilitator) | 🆕 new | `merchant.rs` |

Everything 🆕 is proposed to live in a new `crates/x402/src/merchant.rs` module so the
core stays untouched.

---

## 1. Topology 4A — self-hosted merchant, (almost) pure reuse

A merchant that runs its own Lightning node needs *no new types* for the base case —
it assembles the existing crate. The only genuinely new thing here is the sink
(records its own sale; there is no central split ledger because it was paid directly).

```rust
use std::sync::{Arc, atomic::AtomicBool};
use x402::{
    facilitator::Facilitator,
    front::{self, Config, Front, Route, Settlement, SettlementSink},
    replay::FileReplayStore,
    server::Receiver,
};

// (a) The receiver is the MERCHANT's own node. `crates/wallet`'s `--lsp mdk`
//     implementation of `Receiver` (receive_exact, crates/wallet/src/ldk.rs:315)
//     runs here, at the merchant — not centrally.
let receiver: Arc<dyn Receiver> = Arc::new(merchant_wallet_receiver());

// (b) One replay store for THIS receiver's scope (deployment profile §3.2/§3.4).
let store = FileReplayStore::open(std::path::Path::new("/var/lib/merchant/replay"))?;

// (c) Facilitator over that store. HTTP skew default 60s.
let facilitator = Facilitator::new(store, nostr::x402::DEFAULT_CLOCK_SKEW);

// (d) The merchant sink: record the sale locally; NO split, NO payout worker.
let sink: Arc<dyn SettlementSink> = Arc::new(MerchantSink::open("/var/lib/merchant/sales.ndjson")?);

// (e) Routes: the goods this merchant sells, priced in msat.
let routes = vec![ /* Route { id, method, path, price: Price::Fixed(5_000), executor, .. } */ ];

let front = Front::new(
    Config {
        base_url: "https://api.acme-merchant.com".into(),
        network: nostr::x402::MAINNET,
        realm: "acme".into(),
        challenge_key: load_challenge_key(),   // >= 32 bytes, shared across this merchant's processes
        timeout_secs: 120,
    },
    receiver,
    facilitator,
    sink,
    routes,
)?;

let stop = Arc::new(AtomicBool::new(false));
let listener = std::net::TcpListener::bind("0.0.0.0:8402")?;
front::serve(listener, Arc::new(front), stop, |ev| log_event(ev))?;
```

That is the whole base case: the central `pay serve` front, pointed at the
merchant's own node and a local sink. The `payTo` in every challenge is the
merchant's key (`front.pay_to()`), funds settle at the merchant, and the fixed
ordering the crate already guarantees — **insert replay key → `received_msat` →
`on_settled` → execute**, with release-on-sink-failure (`front.rs`) — is inherited
unchanged.

### 1.1 `MerchantSink` (new, but just a `SettlementSink`)

```rust
/// Records the merchant's own completed sale. No shares, no payout worker —
/// the merchant was paid directly to its node at settlement.
pub struct MerchantSink {
    log: NdjsonSettlements,                 // reuse the built-in idempotent file sink
    notify: Option<Arc<dyn SaleObserver>>,  // optional webhook / private 3188 record
}

pub trait SaleObserver: Send + Sync {
    /// Best-effort side effect (webhook, NIP-44 `3188` purchase record). MUST NOT
    /// block admission: errors are logged, not propagated, to avoid refusing a
    /// settlement the merchant already received.
    fn observe(&self, settlement: &Settlement);
}

impl SettlementSink for MerchantSink {
    fn on_settled(&self, settlement: &Settlement) -> Result<(), String> {
        self.log.on_settled(settlement)?;           // durable, idempotent per payment_hash
        if let Some(n) = &self.notify { n.observe(settlement); }
        Ok(())
    }
}
```

The contrast with custody is entirely in this type: `pay-ledger`'s split/accrual/
payout sink is replaced by a sink that just records the sale. Nothing is held.

---

## 2. The moat: `BuyerAttestation` (new)

The rail's differentiator (proposal §1) is that the buyer is a *verified,
owner-authorized* agent. The HTTP path today does not verify a Nostr buyer (that is
native-only). We add an **optional** policy hook the merchant can require, evaluated
*before* an invoice is issued, so unauthorized buyers never even get a challenge.

```rust
/// Proof, carried in a request header, that the caller is an owner-attested agent
/// with a signed spend budget. Opaque to the crate; interpreted by the verifier.
pub struct AttestationHeader<'a> {
    pub sovereign_profile: &'a str,  // NIP-SOV sovereign-profile.v1 (agent + authority)
    pub spend_grant: &'a str,        // NIP-CAP grant.v1 with the `spend` effect
    pub signature: &'a str,          // NIP-98-style signature over the bound request
}

#[derive(Debug, Clone)]
pub struct Attestation {
    pub buyer: [u8; 32],             // verified Nostr x-only pubkey
    pub owner: [u8; 32],             // attesting authority (NIP-SOV `authority`)
    pub max_per_tx_msat: Option<u64>,// from the CAP spend grant bounds
    pub reputation: Option<u32>,     // optional, from a reputation feed (NIP-EVAL)
}

pub trait BuyerAttestation: Send + Sync {
    /// Verify the attestation against this merchant's requirement at `now`.
    /// `price_msat` lets the verifier reject when the buyer's grant ceiling is
    /// below the quoted price (fail fast, before issuing an invoice).
    fn verify(
        &self,
        header: &AttestationHeader<'_>,
        price_msat: u64,
        now: u64,
    ) -> Result<Attestation, AttestationError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AttestationError {
    #[error("no attestation presented")]
    Missing,
    #[error("signature invalid")]
    Signature,
    #[error("sovereign profile not admitted by a trusted authority")]
    Authority,
    #[error("spend grant does not cover {price_msat} msat (ceiling {ceiling_msat})")]
    OverCeiling { price_msat: u64, ceiling_msat: u64 },
    #[error("reputation {got} below required {required}")]
    Reputation { got: u32, required: u32 },
    #[error("attestation expired")]
    Expired,
}

/// Per-route (or per-merchant) requirement, mirroring the discovery descriptor's
/// `require_attestation` (deployment profile §5).
#[derive(Clone, Default)]
pub struct AttestationRequirement {
    pub required: bool,            // false => open, like a normal x402 endpoint
    pub min_reputation: Option<u32>,
}
```

House-style notes: `BuyerAttestation` is a trait object (policy, not hot path);
`AttestationError` is a `thiserror` enum. The verifier itself (parsing NIP-SOV/NIP-CAP
artifacts, checking the authority is admitted) lives behind this trait, in a
`nostr`-backed impl — the crate stays transport-only.

---

## 3. Platform fee as a second requirement (new — needs spec review)

Proposal §7 / profile §7: the platform fee is a **separate** payment, never folded
into the merchant's invoice ("Fees are extra, never hidden in the invoice amount" —
NIP-X402). This does **not** fit `PaymentRequired.accepts`, whose entries are
*alternatives* (any one satisfies), not *both-required*. So this is a genuine
protocol extension and must go through the X402 compatibility review the spec
requires (profile open item #4). Sketch of the shape:

```rust
/// A platform fee owed to a DIFFERENT receiver (an OpenAgents node) alongside the
/// merchant's invoice. Carried in `PaymentRequired.extensions["openagents/fee"]`.
#[derive(Clone)]
pub struct FeePolicy {
    pub fee_receiver: Arc<dyn Receiver>,  // the fee node (not the merchant)
    pub fee_store: Arc<dyn ReplayStore>,  // the fee receiver's OWN replay store
    pub schedule: FeeSchedule,
}

#[derive(Clone)]
pub enum FeeSchedule {
    Bps(u32),          // basis points of the merchant price
    FlatMsat(u64),
}

impl FeePolicy {
    pub fn fee_msat(&self, price_msat: u64) -> u64 { /* checked exact arithmetic */ }
}
```

Settlement with a fee requires **two** admissions before execution. Because the two
invoices settle to two different nodes, this is all-or-nothing only at the
application layer (not atomic on Lightning). The honest rule: settle the fee first;
if the merchant settle then fails as `duplicate_settlement`/store error, the fee is
already consumed and must be reconciled, not silently refunded.

```rust
// inside the merchant handle path, after verifying both preimages:
let fee = fee_policy.settle_fee(&fee_requirements, &fee_payload, purchase, now)?; // Admission
match facilitator.settle(&merchant_requirements, &merchant_payload, purchase, now) {
    Ok(admission) => { /* both paid → execute */ }
    Err(resp) => {
        // merchant leg failed AFTER fee consumed: do NOT execute, do NOT auto-refund.
        // Emit a reconciliation record; the fee stays recorded for manual handling.
        return refuse_with_reconciliation(fee, resp);
    }
}
```

**Recommendation (profile §7):** ship single-payee first (no `FeePolicy`), and gate
this behind the X402 review. Monetize via LSPS4 liquidity (topology 4C) until the fee
extension is specced.

---

## 4. Topology 4B — managed facilitator, `DelegatedReceiver` (new)

For zero-infra merchants, an operator runs a *stateless* facilitator but the invoice
must still be on the **merchant's** key (profile §3.1: a shared operator key for
untrusted tenants is non-compliant). `DelegatedReceiver` issues on a merchant LN
address while pinning the association.

```rust
/// Issues invoices that settle to the merchant's own LN address / node, under a
/// pinned delegation. NOT a shared custodial key: each instance serves exactly one
/// merchant scope, and `pay_to()` returns that merchant's node key.
pub struct DelegatedReceiver {
    merchant_node_key: String,          // 33-byte compressed, hex — the pinned payTo
    lnurl: LnurlPayResolver,            // resolves the merchant's LN address to invoices
    delegation: PinnedDelegation,       // signed grant of exclusive scope (profile open #1)
}

impl Receiver for DelegatedReceiver {
    fn pay_to(&self) -> String { self.merchant_node_key.clone() }

    fn invoice(&self, amount_msat: u64, request_hash: [u8; 32], expiry_secs: u32)
        -> Result<String, String>
    {
        // LNURL-pay generally does NOT honor a caller-supplied description hash;
        // the spec requires an exact description-hash invoice (NIP-X402 wire rules).
        // So this path is only valid where the merchant endpoint supports
        // description-hash invoices; otherwise fall back to topology 4A.
        let inv = self.lnurl.invoice_with_description_hash(amount_msat, request_hash, expiry_secs)?;
        let decoded = nostr::x402::decode_invoice(&inv).map_err(|_| "invoice invalid")?;
        if hex::encode(decoded.payee()) != self.merchant_node_key {
            return Err("delegated invoice payee is not the pinned merchant key".into());
        }
        Ok(inv)
    }
}
```

The caveat is real and in the code: generic LNURL-pay "does not guarantee an
arbitrary supplied x402 description hash" (NIP-X402, *Wallets*). Where it can't,
4B is not usable and the merchant must self-host (4A).

---

## 5. Request lifecycle with the new hooks

Where the new pieces fire relative to the crate's fixed, already-implemented order:

```
request ─▶ match route (front.rs)
         ├─ [NEW] if AttestationRequirement.required:
         │        BuyerAttestation::verify(header, price_msat, now)
         │        └─ Err → 402/403 WITHOUT issuing an invoice (fail fast)
         ├─ price the route (Price::Fixed | Of | Quote)         ── reuse
         ├─ Receiver::invoice(amount, request_hash, expiry)     ── reuse (merchant payTo)
         │   [NEW] if FeePolicy: also FeeReceiver::invoice(fee, …) in extensions
         └─ respond 402 PAYMENT-REQUIRED (wire.rs)              ── reuse

paid retry ─▶ decode PAYMENT-SIGNATURE (wire.rs)               ── reuse
           ├─ [NEW] if FeePolicy: fee_policy.settle_fee(...)    (fee store insert)
           ├─ Facilitator::settle(req, payload, purchase, now)  ── reuse
           │   └─ verify → ReplayStore::insert (atomic consume) ── reuse
           ├─ Receiver::received_msat(payment_hash)             ── reuse
           ├─ SettlementSink::on_settled(&Settlement)           ── NEW MerchantSink (no split)
           │   └─ on failure: replay key released, 503          ── reuse
           └─ RouteExecutor::execute(&Call) ─▶ 200 + result     ── reuse
```

The only insertions are the attestation check (before invoice issuance) and the
optional fee leg (alongside settle). The uniqueness/atomicity guarantees are
untouched because they live in `Facilitator::settle` + `ReplayStore::insert`.

---

## 6. New crate surface, summarized

Proposed additions, all in `crates/x402/src/merchant.rs`, re-exported from `lib.rs`:

```rust
// Policy hooks (trait objects — not the hot path)
pub trait BuyerAttestation: Send + Sync { /* §2 */ }
pub trait SaleObserver:    Send + Sync { /* §1.1 */ }

// Values
pub struct Attestation { /* §2 */ }
pub struct AttestationRequirement { /* §2 */ }
pub struct AttestationHeader<'a> { /* §2 */ }
pub struct MerchantSink { /* §1.1 */ }     // impl SettlementSink
pub struct DelegatedReceiver { /* §4 */ }  // impl Receiver
pub struct FeePolicy { /* §3 — gated on X402 review */ }
pub enum   FeeSchedule { /* §3 */ }

// Errors (thiserror)
pub enum AttestationError { /* §2 */ }

// Assembly helper (optional convenience over Front::new)
pub struct MerchantServer<S: ReplayStore> { /* wraps Front<S> + attestation + fee */ }
impl<S: ReplayStore + Send + Sync + 'static> MerchantServer<S> {
    pub fn builder(config: Config, receiver: Arc<dyn Receiver>, store: S) -> MerchantBuilder<S>;
    pub fn serve(self, listener: std::net::TcpListener, stop: Arc<AtomicBool>) -> std::io::Result<()>;
}
```

### Core-crate changes required (as opposed to pure additions)
Two, both small, both needed only for the policy hooks — the money path is untouched:

1. **An attestation hook on the request path.** Either `Front` gains an optional
   `Arc<dyn BuyerAttestation>` + per-`Route` `AttestationRequirement`, or
   `MerchantServer` wraps `Front::handle`. Prefer the wrapper to keep `Front`
   unaware of attestation.
2. **Fee extension plumbing** (only if §3 is adopted): emit/read
   `PaymentRequired.extensions["openagents/fee"]` and sequence two settles. This is
   the one change that needs an X402 compatibility review before implementation.

---

## 7. Conformance

A merchant deployment reuses the crate's facilitator, so it inherits its conformance
vectors. The new pieces add their own required cases:

- **Attestation:** missing / bad-signature / untrusted-authority / over-ceiling /
  below-reputation / expired — each refuses **before** invoice issuance; an open
  route (not required) is unaffected.
- **Fee (if adopted):** fee-leg settled but merchant-leg fails → no execution, no
  auto-refund, reconciliation record emitted; replayed fee proof →
  `duplicate_settlement`; fee amount matches `FeePolicy::fee_msat` exactly.
- **Delegated receiver:** `invoice()` rejects any invoice whose decoded payee ≠ the
  pinned merchant key; refuses when the LN endpoint can't bind the description hash.
- Plus the existing NIP-X402 cases (profile §8): cross-process proof races,
  crash before/after the atomic write, `duplicate_settlement`, expired-but-paid
  refusal, no bearer proof in logs/traces.

---

## 8. Open questions

1. **Attestation on HTTP.** The clean place to carry `AttestationHeader` — a new
   request header vs. reusing the native `3188` records only for the native binding.
   HTTP merchants are the volume case, so an HTTP header is likely necessary.
2. **Fee extension (§3).** Adopt the second-requirement model, or defer all fees to
   LSPS4 liquidity (4C) for v1? Recommend defer.
3. **`MerchantServer` vs. `Front` change.** Wrap (keeps core clean) or extend `Front`
   (fewer types)? Recommend wrap.
4. **Reputation source.** Where `Attestation.reputation` comes from (NIP-EVAL feed)
   and how fresh it must be — out of scope for the crate, but the trait needs a
   provider.
