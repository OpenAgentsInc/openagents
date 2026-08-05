# Episode 267 Notes — Immortal Infrastructure

## Working title

**Immortal Infrastructure**

## One-line pitch

Episode 266 said "we're going to build the decentralized Boltz replacement."
Episode 267 shows it running: one hardened CC0 monorepo, a deployed relay
coordinating a negotiated-market protocol family, a liquidity-provider daemon
taking shape, wallets that verify before they fund — and the same market
fabric already generalizing past swaps into agent markets and work itself.

## Continuity from 266

Episode 266 ([transcript](./266.md)) was the reaction video: Boltz taken down
by AI-assisted attacks, a wave of dependent services failing with it, and the
diagnosis that the Bitcoin economy is too dependent on centralized
coordination services. It ended with a plan: harvest the tbDEX negotiation
grammar and the Boltz settlement physics, write NIP-MKT plus focused market
profiles, and implement them on our own hardened Rust Nostr relay —
"let's get a prototype up, like, tomorrow or so."

Episode 267 is the receipts episode. Since 266:

- NIP-MKT base (kinds 39600-39609) is implemented and deployed on the live
  relay, with the full profile family drafted and collision-reviewed:
  MKT-SWP (submarine/reverse/chain swaps), MKT-P2P (Mostro/NIP-69 peer
  trades), MKT-PFI (credentialed fiat ramps), MKT-MINT (Cashu/Fedimint
  gateways), MKT-LSP (LSPS1/LSPS2 channel liquidity), and MKT-INTENT
  (maker-funded covenant intents).
- Immortal converted from a single relay binary into a hardened-software
  monorepo — a recorded owner decision with a full migration analysis
  (`docs/MONOREPO.md` in the immortal repo).
- The MKT-SWP client engine, provider session library, no-spend provider
  actor, MuSig2 foundation (checked against the official BIP-327 vectors),
  pricing/quoting policy engine, and the regtest lab harness all landed.
- A public explainer page shipped at **openagents.com/infra**, and two live
  demo surfaces exist beside it: **/demo** (the market walkthrough) and
  **/work-demo** (real project work items served off the relay).

## The deployed page: openagents.com/infra

The visual companion for this episode is already live. `/infra` is a
GPUI/WebGPU document — a Rust crate compiled to wasm, rendered through
Omega's own design system, same pipeline as the market demo — with six
sections, each pairing tight prose with a mermaid diagram rendered at build
time in the Aiur theme:

1. **What went down** — the centralized failure: one operator running the
   market maker, the coordination surface, and the product UI, and what
   happens when that single point stops.
2. **The decentralized shape** — clients ⇄ N relays ⇄ M providers, each
   with their own rails; three roles that fail independently.
3. **The hardened-binary monorepo** — what ships from the immortal repo and
   the discipline every product shares.
4. **A swap, end to end** — one negotiated session over relays and rails.
5. **Inside a liquidity provider** — the operator's stack: the daemon plus
   the rail nodes it drives.
6. **Die safely** — the failure matrix; no role's failure strands another
   role's money.

The page performs exactly one live network call: a NIP-11 probe of
`relay.openagents.com`, rendered as a live badge. Everything else is static.
That is itself a talking point — a deployment claim you can check from
outside: fetch the NIP-11 document and look for the market extensions.

```sh
curl -s -H "Accept: application/nostr+json" https://relay.openagents.com \
  | jq .supported_extensions
```

Today that returns `nip-mkt`, `mkt-swp:1`, `nip-mkt-p2p:1`, `nip-mkt-mint:1`,
`nip-mkt-lsp:1`, `nip-mkt-pfi:1`, plus the full Buzz interop set
(`nip-oa`, `nip-aa`, `nip-ae`, `nip-ao`, `nip-am`, `nip-ap`, `nip-dv`,
`nip-er`, `nip-ia`, `nip-rs`, `nip-mp`). A relay whose NIP-11 lacks the
market extensions is a working relay but not a market coordinator.

## Introducing the idea

The frame to open with: **Boltz ran three roles as one operator. The
replacement splits them so they fail independently.**

| Role | What it is | What it holds |
| --- | --- | --- |
| Relay | Coordination fabric: discovery heads, gift-wrapped negotiation transport, reservation accounting, timers, bounded public evidence | Its signing key and coordination records. **Never funds, seeds, node credentials, claim/refund keys, or unreleased preimages** |
| Liquidity provider | The market maker: publishes Offerings, answers RFQs, signs Quotes, reserves capacity, executes and settles on real rails | The money — seed, hot wallet, claim/refund keys, preimages, node credentials |
| Client | Wallet or app embedding the swap engine: verifies every script, amount, hash binding, and timeout **before funding** | The user's own keys, never leaving the device |

The doomsday drill sells it: if the relay dies, in-flight swaps still
complete or refund from the client's persisted session records. If a provider
dies, its swaps refund through the client's unilateral exit path. If a client
dies, the provider's refund ladder returns its own funds. Relay acceptance is
transport evidence only — it proves a message moved, never that money did.

## The new monorepo

The repo's identity changed from "a hardened Nostr relay" to **"hardened
Rust infrastructure for the open swap network"** — small, severe,
independently deployable programs sharing one discipline, so joining the
network means *running a binary*, not integrating a stack:

- **`immortal`** (the relay, deployed): Nostr relay that is also the
  NIP-MKT coordination fabric — one binary, one Postgres database, nothing
  else. If a feature needs another running service, the feature is wrong.
- **`immortal-provider`** (the daemon, in progress): the runnable
  liquidity-provider. No-spend mode rehearses complete sessions with zero
  rail effects; funded mode reserves operator-owned Bitcoin/Lightning
  capacity, drives bitcoind and Core Lightning, and executes script-path
  claim/refund recovery. A different program run by a different party than
  the relay — that separation is the architecture.
- **The client engine** (library + wasm): the verify-before-fund engine
  wallets embed; also the source of the generated TypeScript SDK.
- **`immortal-core`**: shared audited primitives — event/tag/filter/canonical
  ID domain, NIP-44, the MKT grammar, taproot and invoice verification.
- **The regtest lab**: adversarial multi-provider, multi-relay harness
  driving both binaries against external regtest nodes.

The discipline, per product: one binary and one Postgres database; an
owner-approved dependency allowlist (the relay has exactly seven crates, and
the provider daemon launched with the *same seven*); primitives written
in-repo; prepared SQL only; fail-closed behavior; CC0 — public domain.

Why a runnable provider matters is the tbDEX lesson from 266, now sharpened:
every network that acquired independent operators shipped the operator
software as a runnable artifact — bitcoind, LND, arkd, Mostro's daemon, and
Boltz itself. tbDEX shipped a protocol and SDKs without a runnable provider
and died on the chicken-and-egg. A library-only provider story asks strangers
to build our missing component with their own capital at risk.

## The swap infrastructure

What "a swap" actually is in this system, worth walking through on the /infra
end-to-end diagram:

- **Discovery**: providers publish signed Offerings on public discovery
  heads; clients browse them relay-side, no accounts.
- **Negotiation**: private, gift-wrapped RFQ → Quote (indicative/firm, with
  reservation class) → Order → per-signer Status sequences → Close/Cancel.
  Reused verbatim from the tbDEX grammar, minus the DID/VC weight.
- **Settlement physics** (borrowed from Boltz and kept non-custodial):
  verify-before-fund as law — the client checks the lock script or Taproot
  tree, amounts, payment hash, timelocks, and claim/refund paths before any
  money moves. Hash/preimage coupling binds the legs. Cooperative MuSig2
  key-path claims optimize the happy path (now vector-checked against
  BIP-327); script-path backups guarantee unilateral exit. Reverse Quotes
  bind the exact funding transaction before either contract signs.
- **Provider reality**: a serious LP runs roughly eight things — Bitcoin
  Core, a chain indexer, a Lightning node with hold-invoice support, the
  daemon, the watchtower, wallet/reserve management, and its declared rails.
  That stack is operator infrastructure, never deployed beside the relay.
- **Migration path for the ecosystem**: a Boltz-compatible REST/WebSocket
  facade (immortal#15) and a shadow-deployment/cutover runbook (immortal#19)
  so services stranded by the shutdown can move without rewriting; a
  0-conf acceptance policy and Liquid/Ark rail extensions queued behind it.

## The Nostr relay

Ground the episode in the thing that is already live. `relay.openagents.com`
runs the immortal binary against one Postgres database. Points worth making:

- NIP set: 1, 9, 11, 17, 29, 40, 42, 45, 50, 65, 70, 94 — plus the fifteen
  Buzz NIPs, so every Immortal relay is interoperable with Block's Buzz out
  of the box, and the OpenAgents lanes on top.
- The market lane is code inside the same binary and tables inside the same
  database: MKT kind validation (39600-39699), bare-private rejection,
  recipient gating, immutable-by-contract admission for the private kinds,
  and the optional no-spend coordination handler gated behind a compiled
  conformance digest.
- It is also the **work** coordination fabric: the relay validates and
  serves the first two All Work NIPs (NIP-WK Work records, NIP-PI Issue
  projections) with a dedicated `work`-tag index — which is what feeds
  /work-demo.
- The NIPs directory mirrors three lanes — official, Block's, and ours —
  with a manifest and sync script, so "which protocol is this relay
  speaking" is a readable answer, not archaeology.

## How it all relates

This is the section that makes the episode more than a swap story. One
sentence: **the relay is the neutral coordination fabric; markets and work
are both just protocols on it; our products are just clients of it.**

- The same OpenAgents pattern at every layer: public discovery, private
  signed negotiation, explicit lifecycle, receipts — and authority stays
  external. Relay acceptance never proves execution, verification,
  acceptance, or settlement. Those need their own records.
- **openagents.com/demo** — the market walkthrough, a Rust GPUI wasm app on
  the site, probing the live relay.
- **openagents.com/work-demo** — the same move applied to work: our actual
  open project issues, signed as NIP-WK/NIP-PI events, published to the
  relay, rendered read-only in GPUI in the browser. The tracker view
  reconstructs entirely from relay data: state-grouped board, issue detail,
  append-only event timelines with explicit gap surfacing.
- Behind that, the drafted **All Work NIP program** — 25 NIPs encoding the
  Linear-class planning, delegation, coding, review, evidence, and outcome
  system on Nostr: where Buzz encoded "Slack on Nostr," these encode
  "Linear plus coding plus all work on Nostr."
- And Omega, the desktop app, gets the native panels for the same fabric —
  the negotiated-market panel speaking NIP-MKT to an Immortal relay is
  already scoped (omega#244) on the same transport-neutral client core.

Lineage worth saying out loud: tbDEX contributed the provider-neutral market
grammar; Boltz contributed the non-custodial settlement physics; Buzz
contributed relay-as-workspace and agent identity; Linear contributed the
work-management grammar. Immortal is where those get merged, hardened, and
put in the public domain.

## Future directions: agent markets beyond swaps

Swaps are the first market because Boltz made them urgent. The fabric was
never swap-specific — NIP-MKT deliberately separates the negotiation wire
from the profile that makes any given exchange correct. From 266: selling
your compute, your data, your labor, your liquidity, your risk. Concretely
already drafted in the repo:

- **Data market** (NIP-DS): dataset licensing and delivery.
- **Labor market** (NIP-LBR): bounded work engagements with content-addressed
  closeout receipts.
- **Skills registry** (NIP-SKL) and **sovereign agents** (NIP-SA): what an
  agent can do, and who attests to it — agents as first-class market
  participants, with owner attestation (NIP-OA) proving provenance.
- **Agent credit** (NIP-AC): the credit rail agent commerce needs.
- **Training** (NIP-TRN): decentralized training coordination.
- **MKT-INTENT**: maker-funded, any-filler covenant intents — the furthest
  point of "the order book is just signed events."
- **All Work as a market**: the planning graph, repository work claims,
  evidence, verification, and owner dispositions are the demand side of a
  labor market for agents. Discovery → negotiation → execution → receipts →
  accepted outcome is the same loop whether the deliverable is a swap or a
  merged pull request. Accepted outcomes are the unit; the market fabric is
  how strangers price and verify them.

Nearer-term infrastructure directions to mention: a second
operator-independent relay (immortal#31) so the coordination fabric itself
has no single operator; recruiting or operating the first funded provider
instance; the adversarial regtest lab as the standing proving ground; and
the NIP-11 extension tokens for the work lane.

## Show checklist

- [ ] openagents.com/infra — walk the six sections; point out the live
      relay badge going green.
- [ ] `curl` the NIP-11 document on stream; read the extensions list.
- [ ] openagents.com/demo — the negotiated swap session walkthrough.
- [ ] openagents.com/work-demo — real issues, live from the relay; show an
      event timeline and the explicit-gap rendering.
- [ ] Immortal repo tour: `nips/` three-lane layout, `MKT.md` and the
      profile family, `PROPOSED.md` (the 25 All Work NIPs), the four
      crates, `scripts/dev-relay.sh` + seed scripts.
- [ ] The doomsday drill, told as a story: kill the relay mid-swap on the
      whiteboard/diagram and follow the money home.
- [ ] Close on the lineage line: tbDEX's grammar, Boltz's physics, Buzz's
      relay-as-workspace, Linear's work model — merged, hardened, CC0.

## Positioning and honesty boundary

- The relay and the NIP-MKT base are deployed and checkable from outside.
- The provider daemon's funded mode, the Boltz-compatible facade, the
  shadow cutover, MuSig2 cooperative settlement end-to-end, and the second
  relay are in progress — say "underway," not "done."
- The market profiles beyond SWP are complete drafts with relay-side
  adoption, not executable end-to-end services yet.
- No custody claims: nothing in this system holds user funds except the
  provider's own operator capital, and the relay never does.
