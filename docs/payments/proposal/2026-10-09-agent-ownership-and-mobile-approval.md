# Agent ownership & mobile purchase approval — design (draft, 2026-10-09)

**Decision (owner, 2026-10-09):** adopted as the merchant-settlement profile for third-party sellers, coexisting with the custodial receiver for our own sales. OpenAgents supports every agent payment protocol; see [Agent payments: pay any way](../agent-payments.md).

**Status:** Design for discussion. The identity/approval *primitives* exist in the
NIPs; the *buyer-facing experience* described here is largely to-build.
**Answers two buyer questions:** (1) how is an agent provably *mine*? (2) how do I
authorize/approve its purchases, from a phone?
**Companions:** [non-custodial rail proposal](2026-10-09-non-custodial-agent-commerce-rail.md) ·
[merchant-receiver profile](2026-10-09-x402-merchant-receiver-profile.md) ·
[merchant-facilitator API sketch](2026-10-09-x402-merchant-facilitator-api-sketch.md).
**Grounding:** [NIP-SOV](../../../nips/openagents/NIP-SOV.md),
[NIP-HOST](../../../nips/openagents/NIP-HOST.md), [NIP-POL](../../../nips/openagents/NIP-POL.md),
[NIP-CAP](../../../nips/openagents/NIP-CAP.md), [NIP-REACH](../../../nips/openagents/NIP-REACH.md),
[NIP-CTRL](../../../nips/openagents/NIP-CTRL.md), [NIP-X402](../../../nips/openagents/NIP-X402.md).

---

## 0. TL;DR

- **"Is this agent mine?"** is a cryptographic fact, not a login: the agent's
  NIP-SOV profile names *your owner key* as its `authority`, and your host admitted
  that profile. You verify ownership; you don't assert it.
- **Approving a purchase** is NIP-POL: an over-budget spend emits a single-use
  `approval-request`; your enrolled phone shows it; your tap is an atomically-consumed
  `approval-decision`. Under budget → the agent pays autonomously, no prompt.
- **No new identity or approval protocol is needed.** SOV (ownership) + HOST (device
  enrollment) + CAP/SOV (budget) + POL/guardian (approval) already compose into the
  flow. The work is the **mobile app surface** and the **x402↔POL wiring** (§8).

---

## 1. Why "log in to your agent" is the wrong model

A buyer naturally thinks "my agent is the one I'm logged into." That is not how this
works, and the difference is the whole security story. An agent is an independent
Nostr principal with its own key; *ownership is a signed relationship between your
key and the agent's*, rooted in an act your device performed. You can hand the agent
to another device, it can run while your phone is off, and it still can't spend
beyond what you signed for — because the authority is cryptographic, not a session.

---

## 2. How an agent is provably yours (the ownership binding)

### 2.1 The chain
- The agent has its **own durable Nostr key** (NIP-SOV), distinct from yours.
- It is governed by an immutable **`sovereign-profile.v1`** carrying:
  `agent` (the agent key), **`authority`** (the key allowed to admit this profile —
  *your owner key*), `policy` (NIP-POL governance), `custody` (NIP-CAP), `treasury`
  (spend policy).
- "This agent is mine" ≡ **the profile's `authority` is my owner key, and my host
  admitted that profile.**

### 2.2 Why that actually means something
Ownership is not a self-signed claim. The spec is explicit:

> "The admitted authority authenticates the exact profile bytes." … "the host must
> independently establish the initial authority through its authenticated admission
> process. **Self-publication cannot establish that trust.**" — NIP-SOV

And signatures alone are weak evidence of permission:

> "Signatures establish attribution, not truth, permission, statistical validity, or
> remote attestation." — shared contracts

So ownership is rooted in **your host admitting the agent**, where your owner key
lives — not in the agent waving a signature around. An attacker who copies the
agent's public key cannot make it "theirs"; they would need to be the admitting
authority on a profile your host accepted.

### 2.3 Where your owner key comes from
Locally, on a device you control (NIP-HOST): "a desktop app that creates an owner key
in the machine's keychain on first run," or an operator command / local operator
socket. "The host is the only issuer of access … a relay, an invitation, an SSH
login, or network membership can introduce a device; none of them is a login."

### 2.4 Verifying it (the check the merchant also runs)
To confirm an agent is yours, resolve its sovereign profile and check:
1. `profile.authority == my owner pubkey`;
2. the profile was admitted by my host (not self-published);
3. the active `treasury`/spend grant is the one I issued.

This is exactly what the merchant's `BuyerAttestation` hook verifies before issuing
an invoice (API sketch §2): the attestation carries the `sovereign-profile` (whose
`authority` is you) plus the `spend` grant. If an agent isn't yours, it can't present
your authority, so it can't present *your* budget.

> **Key rotation caveat (NIP-SOV):** changing the agent's identity key "creates a new
> identity and explicit, authorized lineage link. Existing contracts and grants do
> not automatically transfer." Rotation is a deliberate, re-authorized act — not a
> silent transfer of ownership or budget.

---

## 3. Your identity and the phone as approver

### 3.1 The phone's two possible roles
- **As (a holder of) the owner key** — the owner key lives in the phone keychain; the
  phone *is* the authority.
- **As an enrolled device with delegated rights** — more common and safer. The owner
  key lives elsewhere (desktop/host); the phone is enrolled and granted specific
  rights (notably the right to review/approve), so a lost phone ≠ lost ownership.

The repo already ships the clients for this: iOS (Swift) and Android (Kotlin), with
`crates/openagents-mobile` (Breez/Spark wallet) and `crates/spark-wallet`; phone and
desktop can share one Spark balance from the same seed. (Note: "A Spark wallet can
pay x402 sellers but cannot receive x402 payments" — fine, the buyer only pays.)

### 3.2 Enrolling the phone (NIP-HOST)
Four admission paths exist; for a consumer phone the natural two are:
- **QR / connect code** — scan a `coder-host:` invitation or an `openagents-connect:`
  (iroh) code. One-time-use; "Another device already redeemed it (`forbidden`)."
- **Short code** — an 8-char Crockford base32 code (40 random bits); the host stores
  only its SHA-256 hash and "closes after 5 mismatches."

Enrollment yields a **`host-grant.v1`** carrying `host`/`owner`/`device` keys,
`relay`, a **`rights`** list, an `epoch` (for revocation), `origin`
(`invitation`/`approval`), and `issued_at`/`expires_at` (max 30 days). Rights include
`observe`, `operate`, `review`, `terminal`, `access_read`, `access_admin`, `world`.
Approval authority maps to the right to `review`/approve. Delegation is bounded: a
device with `access_admin` "can issue invitations and approvals only for rights it
holds, and only with a grant expiry no later than its own."

---

## 4. The budget that decides *when* you're asked

Whether a purchase prompts you at all is set by the budget you signed — this is what
makes autonomy safe and the phone quiet:

- **NIP-CAP `spend` grant** — `spend` is granted *separately* from other effects, with
  `bounds`, `scope`, `expires_at`. Critically it is "Host/operator policy, never
  supplied by the component itself" — the agent cannot widen its own budget.
- **NIP-SOV `treasury`** — names the wallet adapter, authorizing principal, permitted
  payment profiles/destinations, **per-operation ceilings**, **aggregate allowance**,
  accounting period, fees, guardian requirements, and reconciliation authority. Child/
  delegated work "cannot evade the ancestor's budget or guardian gates."

Tiers map naturally:

| Spend vs. policy | What happens |
| --- | --- |
| ≤ per-op ceiling, within aggregate allowance | **Auto-pay.** No prompt. |
| > ceiling, or sensitive category | **NIP-POL approval** on your phone (§5). |
| high value / listed operation | **NIP-SOV guardian** threshold (M-of-N) (§5.3). |

---

## 5. The purchase-approval flow (end to end)

### 5.1 Sequence
```
Agent                         Merchant            Owner's phone (enrolled, `review` right)
  │  request goods                 │                         │
  │────────────────────────────────▶                        │
  │  402 + challenge (payTo=merchant, amount msat)           │
  │◀────────────────────────────────                        │
  │                                                          │
  │  check amount vs CAP spend grant + SOV treasury ceiling  │
  │                                                          │
  │  (A) ≤ ceiling → pay BOLT11 directly, retry, done. no prompt.
  │                                                          │
  │  (B) > ceiling → build POL action.v1 (op+merchant+amount+nonce)
  │       publish approval-request.v1 (requester=agent, approver=owner) ─────────▶ push
  │                                                          │   "Acme-Shopper wants to
  │                                                          │    pay 12,000 sats to
  │                                                          │    Merchant X for Y"
  │                                                          │   [Approve] [Deny]
  │       ◀───────────── approval-decision.v1 (single-use, consumed atomically) ──┘
  │  if approve → pay BOLT11, retry, settle, execute
  │  if deny    → do not pay
```

### 5.2 Why the approval is trustworthy (NIP-POL)
- The decision is bound to the **exact action**: an `action.v1` binds operation,
  binding, input, context, effects, bounds, and a random `nonce`; its digest is the
  approval subject. You approve *that* purchase, not a blank cheque.
- Approvals are "**single-use admissions consumed atomically by the host, not
  transferable bearer grants**," and "a later denial cancels still-unconsumed
  approval." "Silence is not an approval."
- So a captured approval can't be replayed for a second purchase, and you can revoke
  an approval you haven't spent yet.

### 5.3 High-value: guardian threshold (NIP-SOV)
For listed operations, a `sovereign-guardian-policy.v1` (`members` 1–32, `threshold`,
`operations` as CAP refs) requires M-of-N approvals — e.g. you **and** a co-signer, or
you on two devices. "Any authenticated denial for that action and policy received
before consumption blocks admission, including a conflicting decision by an approving
member."

---

## 6. The "is this agent mine?" view (consumer feature, to build)

A buyer-facing screen that renders §2's verification:
- the agent's name/key, with a clear **"Owned by you ✓"** derived from
  `profile.authority == your owner key` and host-admission evidence;
- the agent's **current spend budget** (per-op ceiling, remaining aggregate
  allowance, expiry) from the CAP grant / SOV treasury;
- recent purchases (from the agent's private `3188` x402 records / your sales feed);
- **Revoke** / **Edit budget** actions (issue a narrower CAP grant; "removing
  membership, closing a session, or revoking an API key denies subsequent reads and
  destination changes," and a new `epoch` on a host grant revokes a device).

---

## 7. Getting the prompt to the phone in seconds (reachability)

The approval request travels over relays; for a snappy UX you need the phone to be
reachable immediately:
- **NIP-REACH** provides the owner host directory, host presence, reachability hints,
  and authenticated direct channels ("a direct channel binds to one HOST grant ID and
  epoch").
- **NIP-CTRL** provides client pairing and task-scoped control (`observe`/`steer`/
  `cancel`) with revocation and bounded catch-up — the same channel that would carry
  "approve this purchase."
- **Gap:** reliable mobile **push** (APNs/FCM) so the prompt arrives in seconds rather
  than by relay polling is an implementation piece, not a protocol one.

---

## 8. What exists vs. what to build

| Capability | Status | Source |
| --- | --- | --- |
| Agent identity (durable key) | ✅ designed | NIP-SOV |
| Ownership binding (authority admits profile) | ✅ designed | NIP-SOV |
| Owner key creation (local keychain) | ✅ designed | NIP-HOST |
| Phone enrollment + rights | ✅ designed | NIP-HOST |
| Spend budget (ceilings/allowance) | ✅ designed | NIP-CAP + NIP-SOV |
| Single-use purchase approval | ✅ designed | NIP-POL |
| M-of-N guardian approval | ✅ designed (draft) | NIP-SOV |
| Reachability / pairing channels | ✅ designed | NIP-REACH, NIP-CTRL |
| Mobile clients + wallet | ✅ present | iOS/Android, `crates/openagents-mobile` |
| **x402-purchase → POL-approval wiring** | 🔴 **build** | x402 is "Designed, not implemented" |
| **"Owned by you ✓" verification view** | 🔴 **build** | §6 |
| **Budget / grant management on phone** | 🔴 **build** | §6 (owner console) |
| **Mobile push for approvals (APNs/FCM)** | 🔴 **build** | §7 |
| **Attestation header on HTTP x402** | 🔴 **build** | API sketch §2, open #1 |

Takeaway: **no new protocol work** for identity or approval — the four 🔴 items are
app surface + the glue that fires a POL approval when an agent hits a 402 over its
ceiling, and carries the ownership attestation to the merchant.

---

## 9. Security properties and edge cases

- **A copied agent key doesn't transfer ownership.** Ownership needs your authority on
  an admitted profile; a stranger can read the agent's pubkey but can't be its
  authority.
- **A compromised agent is bounded by its budget.** Worst case is the CAP spend
  grant's ceilings/allowance — never your wallet. Narrow or revoke the grant to
  contain it.
- **A lost/stolen phone:** if the phone was an *enrolled device* (not the owner key),
  bump the host grant `epoch` to revoke it; ownership and budget authority survive. If
  the phone *held the owner key*, this is key loss — handle via the SOV lineage/
  re-authorization path, and prefer the enrolled-device model to avoid it.
- **Replay/forgery of approvals:** prevented by single-use atomic consumption and the
  action-digest binding (§5.2).
- **Remote signers (NIP-46):** usable for signing, but "connecting to a signer does
  not give unrestricted application authority" — a signer is not an approval.

---

## 10. Open questions

1. **Owner key on phone vs. enrolled device** — which do we make the default consumer
   onboarding? (Recommend enrolled-device, for lost-phone safety.)
2. **Approval UX latency** — commit to APNs/FCM push, or lean on NIP-REACH direct
   channels with the app foregrounded?
3. **Default spend tiers** — what ceiling/allowance ship by default so most purchases
   are frictionless but surprises always prompt?
4. **Attestation transport on HTTP** — a dedicated request header for the
   sovereign-profile + spend grant (API sketch open #1), so merchants can verify
   "this is an owned, funded agent" on the volume HTTP path.
5. **Guardian defaults for consumers** — is M-of-N overkill for a single user, or do
   we offer "approve on two of my devices" as an opt-in for large spends?
