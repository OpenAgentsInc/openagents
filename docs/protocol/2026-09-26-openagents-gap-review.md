# OpenAgents NIP coverage review

Status: specification review, 2026-09-26. This review compares all twelve
existing [OpenAgents NIPs](../../nips/openagents/README.md), their shared
contracts, the fifteen [Block specifications](../../nips/block/README.md),
and the [official NIP index](../../nips/official/README.md) with the current
[Coder suite](../coder/design/typesafe-product-suite.md) and
[agent labor plan](../agents/market-infrastructure.md). It adds three v1
drafts. It does not implement their validators, host behavior, or relay roles.

## Missing contracts added

| New contract | Missing agreement between independent participants | Existing foundation reused |
| --- | --- | --- |
| [NIP-CTRL](../../nips/openagents/NIP-CTRL.md) | Which client device may observe, steer, or cancel which task; pairing, revocation, command acknowledgment, and catch-up. | POL authority and exact approvals; CTX task frames; CAP/CJ operations; RUN history and fencing; private artifacts. |
| [NIP-MKT](../../nips/openagents/NIP-MKT.md) | What a provider offers, what both parties accepted, what an order owes, and what payment evidence establishes. | CAP descriptions; private artifacts; host reservations and RUN evidence; an explicit Bitcoin settlement profile. |
| [NIP-LAB](../../nips/openagents/NIP-LAB.md) | Contracted work, submission, verification, acceptance, rework, and disputes for agent labor. | MKT agreements; CJ execution; CTX/POL scope; COORD claims; RUN results; EVAL evidence. |

MKT separates commercial agreement from execution and wallet authority. LAB
supplies service-specific delivery rules. CTRL addresses the missing
client-to-task relationship without creating another execution engine.
Signatures establish attribution, not successful effects.

These are new specifications authored for this repository. They do not claim
wire compatibility with the historical Immortal NIP-MKT mentioned in episodes
266–267. A future import or compatibility adapter needs the original public
revision, license review, and explicit conformance tests.

## Why the other lanes do not already fill these gaps

| Existing specifications | Reuse | Boundary that requires an OpenAgents contract |
| --- | --- | --- |
| Official [01](../../nips/official/01.md), [42](../../nips/official/42.md), [44](../../nips/official/44.md) | Signed events, relay authentication, recipient encryption. | Connection authentication is not task, market, or wallet authorization. Encryption does not hide tags or enforce retention. |
| Block [OA](../../nips/block/NIP-OA.md), [AA](../../nips/block/NIP-AA.md), [AP](../../nips/block/NIP-AP.md) | Owner-agent provenance, membership admission, personas. | Attestation does not grant arbitrary task control, spending, or contractual authority. AA does not enforce attestation kind conditions during connection admission. |
| Block [AO](../../nips/block/NIP-AO.md), [AM](../../nips/block/NIP-AM.md) | Live telemetry and durable attributed turn metrics. | AO is ephemeral; its cancellation command does not prove a durable exact-attempt stop. AM's estimated costs are not invoices or payment evidence. |
| Block [AE](../../nips/block/NIP-AE.md), [RS](../../nips/block/NIP-RS.md) | Private memory and device read-position synchronization. | Mutable memory and read markers do not synchronize authoritative task state or grant another device control. |
| Block [PL](../../nips/block/NIP-PL.md), [ER](../../nips/block/NIP-ER.md) | Mobile wakeups and reminders. | A push lease is not an execution lease; a reminder is not an admitted recurring job. Coder needs its own wake profile rather than assuming the Buzz APNs profile applies. |
| Block [CW](../../nips/block/NIP-CW.md), [DV](../../nips/block/NIP-DV.md), [WP](../../nips/block/NIP-WP.md), [IA](../../nips/block/NIP-IA.md) | Channel windows, DM visibility, workspace appearance, and identity archival. | UI projections and relay membership do not define task ownership or commercial acceptance. Archival is not permission revocation. |
| Block [MP](../../nips/block/NIP-MP.md), [GS](../../nips/block/NIP-GS.md); official [34](../../nips/official/34.md) | Repository projects, attributable Git objects, issues, and patches. | Project membership and signed patches do not authorize writes or prove buyer acceptance. |
| Official [46](../../nips/official/46.md), [07](../../nips/official/07.md), [55](../../nips/official/55.md) | Remote, browser, and Android signing. | Signing capability is distinct from scoped task control. Preserve the distinction between client, remote-signer, and user keys. Do not copy an owner secret to every device. |
| Official [99](../../nips/official/99.md), [89](../../nips/official/89.md) | Classified listings and handler discovery. | Neither defines immutable typed offers, private accepted terms, order transitions, or settlement. A listing can point to an exact MKT offering. |
| Official [15](../../nips/official/15.md), [90](../../nips/official/90.md) | Historical marketplace and compute-job compatibility. | Both are marked unrecommended in the pinned index. Their legacy encryption and cancellation conventions are not the basis for new durable labor jobs; use CJ execution. |
| Official [47](../../nips/official/47.md), [57](../../nips/official/57.md), [60](../../nips/official/60.md), [61](../../nips/official/61.md) | Wallet access, zaps, Cashu wallet state, and nutzaps. | A zap receipt is not proof of payment. Core NWC `pay_invoice` specifies no max-fee argument. Wallet state does not establish labor acceptance; unsupported fee or settlement enforcement must refuse. |
| Official [09](../../nips/official/09.md), [40](../../nips/official/40.md), [62](../../nips/official/62.md) | Deletion and expiry requests. | Deletion or expiry cannot stop a process, erase a liability, revoke consumed approval, or prove global erasure. |
| Official [65](../../nips/official/65.md), [66](../../nips/official/66.md), [67](../../nips/official/67.md), [77](../../nips/official/77.md) | Relay discovery, liveness, completeness hints, and synchronization. | Synchronizing events cannot establish a current grant or resolve a fork. |
| Official [94](../../nips/official/94.md), [B7](../../nips/official/B7.md), [98](../../nips/official/98.md) | Artifact locators, blob storage, and HTTP authentication. | Bytes still need exact digest, size, disclosure, fetch limits, and retained provenance under the shared contracts. |

The remaining official social, media, group, identity, and application formats
retain their own meanings. None needs a duplicate OpenAgents event family just
to appear in Coder. NIP-MV remains the separate world-state contract, not a
physical-control or commercial-settlement authority.

## Existing OpenAgents coverage to preserve

| Need | Existing contract; no new NIP needed |
| --- | --- |
| Dynamic context, summaries, variables, source expansion, hierarchical history | CTX and shared evidence/observation contracts. Retrieval algorithms and indexes are host work. |
| Progressive tool schemas/manuals, packages, and skills | CAP and EXT; native bindings and installed resolution remain implementation work. |
| Typed workflows, bounded composition, and Wasm tools | PRG; do not invent another executable wire language. |
| Mandatory instructions, script-bound approval, disclosure, cache economics | POL, CAP, and shared enforcement records. CTRL does not replace POL approval. |
| Durable execution, restart, unknown effects, ownership transfer | CJ execution and RUN. A device view does not take over the controller by connecting. |
| Claims, resources, exact deduplication, background findings | COORD. Resource-aware scheduling and actual enforcement remain host work. |
| Independent measurement and replaceable semantic policies | EVAL and OPT. A paid order does not buy access to protected evaluation labels. |
| Consented reusable knowledge and withdrawals | KB. Labor payment grants no implicit training or publication rights over traces. |

## Allocation and source checks

The official and Block sources remain pinned at the commits in
[`nips/manifest.json`](../../nips/manifest.json): official
`c53877571f96eb423661fc23c620d629d37b8f19` and Block
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79`. No upstream specification or
manifest is changed by this review.

Two statements in the pinned Block text need reconciliation when implementing
encryption. AO describes NIP-44 v2 as XChaCha20-Poly1305, but official NIP-44
specifies ChaCha20 with HMAC-SHA256; follow the official algorithm and test
vectors. AE calls 65,535 bytes the universal NIP-44 plaintext limit, while the
pinned official version supports an extended length prefix. Preserve the
Block profiles' explicit 65,535-byte limits as their own bounds, rather than
expanding them implicitly or treating them as a limit on all NIP-44 use. The
repository's NIP-44 primitive already handles the extended prefix. These
source discrepancies do not justify rewriting the synced specifications.

MKT assigns draft kinds `3192` (immutable offering) and `30192` (discovery
head). Neither appears as an allocation in the pinned official/Block lanes,
the earlier OpenAgents set, or the
[kind registry at `5cf2b84`](https://github.com/nostr-protocol/registry-of-kinds/blob/5cf2b84eaa4871fd2071cc6dc18975db53e7fc33/schema.yaml),
checked on 2026-09-26. The fetched registry file has SHA-256
`a47f201d48fde7776decd8dea221a6490806f1857f6ce30293d99571b96e2623`.
This check is not an upstream reservation or proof that no application uses
those numbers. Recheck before public interoperation. CTRL and LAB allocate no
kinds; private records reuse `3188` and execution uses the existing CJ family.

## What remains implementation work or a later profile

Follow-up: the [teardown review](2026-09-26-teardown-coverage.md) now specifies
finite scheduled/source-triggered continuation in AUTO and image/time anchors
in LIVE, together with SESS, WS, WORK, and ENV. The paragraph below records
the gaps at this earlier review; those draft additions resolve part of its
specification work, not their implementation. Calendar/DST schedules, general
PRG suspension, threshold approval, and arbitrary domain anchors remain later
profiles. See the current [implementation plan](implementation-plan.md).

These drafts do not complete the [general architecture backlog](../agents/roadmap.md).
First-class suspended PRG continuations, external-event/scheduled triggers,
organizational threshold approval, multimodal anchors, additional domain
quantities, and device-specific guarantees still need concrete consumer
contracts. CTRL and commercial orders do not silently supply those behaviors.

The initial profiles avoid those undefined dependencies. Persist an order
while awaiting a reply, then admit a new bounded operation. Do not keep an
expired CJ execution alive or pretend an unimplemented PRG wait exists. A
schedule, threshold rule, financial product, or physical device needs its own
supported profile rather than fields hidden in `meta`.

No swap, credit, broad compute exchange, data resale, or royalty protocol is
added speculatively. Labor supplies the immediate buyer/provider workflow.
MKT can support further profiles once their acceptance, rights, payment, and
recovery rules are specified and tested.

Implementation starts with strict parsers and cross-record fixtures, then
local host state and a no-spend two-operator flow. Add relay privacy/recovery,
authenticated device control, and payment-adapter tests before reporting
support. A README entry, valid signature, or schema check alone does not
establish a deployed market or safe task control.
