# The OpenAgents Protocol: one specification and plan for every project

Status: **Designed**. Written 2026-10-27. This is a specification and build
plan. No schema in it has been committed under `proto/` yet, except the
existing `openagents.engine.v1` package it builds on. The `.proto` sketches
show shape, naming, ownership and boundaries. Their field numbers are
provisional until each package's first descriptor snapshot is published.
After that they are frozen.

This document defines **the OpenAgents Protocol**: one versioned, public set
of Protobuf definitions for everything OpenAgents builds and intends to
build. It covers:

- every product: the web app, mobile, desktop, Coder, the Verse, Pylon,
  Forum, Gym, the Decisions API, OpenAgents Cloud and Computers, Labs and
  Agent Studio;
- every agent concept: identity, sessions, control, context, policy,
  capabilities, programs, plugins, skills, knowledge, evaluation,
  optimization, traces, coordination, automation, work, workspaces,
  environments, hosts, terminals, live media and sovereign agents;
- every money concept: markets, labor, x402, Lightning, payment evidence,
  ledgers, billing, earnings, compute balance and Pylon settlement, on
  Bitcoin and Bitcoin-based stablecoins only;
- the whole Nostr protocol as typed mirrors, the way `nips/official/`
  vendors the NIP text;
- Lightning Labs' protocols, vendored byte-exact the way
  `crates/lnd-sdk/proto/` already does.

It includes designs that are not built yet. If the glossary, a NIP or a
design document says how something should work, it has a package here,
marked with its status. The aim is that a stranger can build a compatible
client, host, worker, relay, wallet, game or agent from `proto/` and this
document alone, without reading our Rust.

The 3D subtree, `openagents.verse.*`, has its own chapter: Chapter V,
[`docs/verse/openagents-verse-protocol.md`](../verse/openagents-verse-protocol.md).
This document states the rules that chapter follows and places it in the
tree. It does not repeat the chapter's 28 package sketches.

## Contents

**Part I. Frame**

1. [Why one protocol](#1-why-one-protocol)
2. [What exists today](#2-what-exists-today)
3. [Principles](#3-principles)
4. [The tree](#4-the-tree)
5. [Naming, versioning and stability](#5-naming-versioning-and-stability)
6. [Data classes, descriptor options and authority](#6-data-classes-descriptor-options-and-authority)
7. [Limits, numbers, canonical bytes and JSON](#7-limits-numbers-canonical-bytes-and-json)
8. [One message, three carriages: Nostr, HTTP and RPC](#8-one-message-three-carriages-nostr-http-and-rpc)

**Part II. Foundations**

9. [`openagents.type.v1`: shared primitives](#9-openagentstypev1-shared-primitives)
10. [`openagents.api.v1`: the HTTP envelope](#10-openagentsapiv1-the-http-envelope)
11. [`nostr.*`: the whole Nostr protocol as typed mirrors](#11-nostr-the-whole-nostr-protocol-as-typed-mirrors)
12. [OpenAgents NIPs: every kind and every body, mapped](#12-openagents-nips-every-kind-and-every-body-mapped)
13. [`third_party.*`: Lightning Labs and other vendored protocols](#13-third_party-lightning-labs-and-other-vendored-protocols)

**Part III. Domains**

14. [Domain catalog and import graph](#14-domain-catalog-and-import-graph)
15. [Identity, accounts and secrets](#15-identity-accounts-and-secrets)
16. [Decisions, inference and receipts](#16-decisions-inference-and-receipts)
17. [Conversation and execution jobs](#17-conversation-and-execution-jobs)
18. [Agents, sessions and control](#18-agents-sessions-and-control)
19. [Context, policy and capability](#19-context-policy-and-capability)
20. [Programs, extensions, skills and knowledge](#20-programs-extensions-skills-and-knowledge)
21. [Evaluation, Gym, optimization and traces](#21-evaluation-gym-optimization-and-traces)
22. [Durable work: runs, coordination, automation, work items](#22-durable-work-runs-coordination-automation-work-items)
23. [Places to run: workspaces, environments, hosts, reach, terminals, live](#23-places-to-run-workspaces-environments-hosts-reach-terminals-live)
24. [Cloud, Computers and compute](#24-cloud-computers-and-compute)
25. [Coder](#25-coder)
26. [Money: markets, labor, payments, ledgers, billing](#26-money-markets-labor-payments-ledgers-billing)
27. [Pylon, attestation and sovereign agents](#27-pylon-attestation-and-sovereign-agents)
28. [Community: XP, Forum, deploy status, referrals](#28-community-xp-forum-deploy-status-referrals)
29. [Interface: UI view trees, engine, Verse](#29-interface-ui-view-trees-engine-verse)
30. [Telemetry, research records and evidence](#30-telemetry-research-records-and-evidence)

**Part IV. Making it real**

31. [Services and transports](#31-services-and-transports)
32. [Migration map: 2,368 schema IDs, 33 JSON Schemas, 5 OpenAPI documents](#32-migration-map)
33. [Tooling and generation](#33-tooling-and-generation)
34. [Publishing the protocol](#34-publishing-the-protocol)
35. [Conformance and verification](#35-conformance-and-verification)
36. [The build plan](#36-the-build-plan)
37. [Risks](#37-risks)
38. [Decisions this document makes](#38-decisions-this-document-makes)
39. [Open questions for the owner](#39-open-questions-for-the-owner)
40. [Sources](#40-sources)

---

# Part I. Frame

## 1. Why one protocol

OpenAgents already speaks dozens of protocols. Each was written well for its
own product, and they do not share a schema language:

- **36 OpenAgents NIPs** with JSON bodies, 10 JSON Schema bundles, and 23
  single-record schemas under `nips/openagents/schemas/`. Most NIP records
  have no machine-readable schema at all (§2.3).
- **Five OpenAPI documents** for the HTTP surfaces: first-party, App,
  Computers, Labs, System One, plus the Decisions API's YAML.
- **2,368 distinct `openagents.*.vN` identifiers** in the repository. Some
  are wire contracts. Many are evidence files, research manifests,
  qualification reports and fixtures (§30, §32).
- **One real Protobuf package**, `openagents.engine.v1`, with descriptor
  snapshots, Nanopb options, Rust, C++, Python and Swift consumers, and a
  hash-pinned bundle that `aethyr-cpp` imports.
- **One more Protobuf package**, `space.credential.provider.v2`, in
  `crates/space-key-broker/proto/`.
- **Vendored Lightning Labs protos** in `crates/lnd-sdk/proto/`: LND
  v0.21.1-beta, Taproot Assets v0.8.0, Loop v0.34.0-beta, Lightning Terminal
  v0.17.0-alpha. That is 44 `.proto` files.
- **Hand-written binary formats**: REACH frames, TERM record streams, AESV
  world containers, kernel ABI slabs, Loro updates, HOST invitation strings.

This works inside one team. It does not work as a public protocol, for four
reasons:

1. **A stranger cannot generate a client.** JSON prose in a NIP is precise
   but not executable. Without a schema, every implementer re-derives field
   types, limits and enum spellings, and gets some wrong.
2. **The same idea is spelled many ways.** "Outcome" is
   `completed|refused|failed|cancelled|unknown` in `contracts.md`,
   `answered|refused|unavailable|unattempted|unknown` in execution receipts,
   `accepted|rejected|failed|timeout` in Pylon receipts, and
   `pass|fail|inconclusive` in eval reports. Some differences are real. Some
   are drift. One tree forces each to be one named type or an explicitly
   different one.
3. **There is no compatibility gate.** A JSON field rename breaks a reader
   in another repo silently. Protobuf descriptors plus frozen snapshots turn
   that into a CI failure, as `engine.v1` already does.
4. **Language reach.** Rust, TypeScript, Swift, Kotlin, C++, Python and Go
   all consume these surfaces today or will soon. Generated code from one
   tree is cheaper and safer than seven hand ports.

The protocol does **not** replace the authoritative bytes where signatures
or digests cover them. Nostr events stay NIP-01 JSON. JCS statements stay
JCS. Content-addressed blobs stay as stored. Proto describes and carries
those bytes; it never re-encodes them (§3, principle 4).

## 2. What exists today

### 2.1 Protobuf

| Package | Where | Consumers | Status |
| --- | --- | --- | --- |
| `openagents.engine.v1` | `proto/openagents/engine/v1/` (scene, agent, hud, control, state, assets) | `crates/engine-proto` (Prost 0.13.5), Nanopb 0.4.9.1, Python protobuf 5.29.3, Swift via C ABI, `aethyr-cpp` via the `openagents.engine-proto.bundle.v1` export | Implemented; descriptor snapshots in `compatibility/`; epic aethyr-cpp#113 closed |
| `space.credential.provider.v2` | `crates/space-key-broker/proto/` | Space key broker | Implemented; outside the `openagents.` tree |
| `lnrpc`, `taprpc`, `looprpc`, `litrpc`, … | `crates/lnd-sdk/proto/<project>/<tag>/` with `sources.json` | `crates/lnd-sdk` | Vendored byte-exact; 44 files |

Generation runs on protoc 29.3. `profiles.json` beside each engine package
lists the language profiles. The `.options` files are hand-written Nanopb
limits.

### 2.2 NIPs

`nips/` has three lanes, described in `nips/README.md` and
`nips/manifest.json`:

- **`official/`**: 99 NIP specs plus the index, pinned to
  nostr-protocol/nips at `b82211e9…`.
- **`block/`**: 17 specs pinned to block/buzz at `781d3951…`: AA, AE, AM,
  AO, AP, CW, DV, ER, FI, GS, IA, MP, OA, PL, PMA, RS, WP.
- **`openagents/`**: 36 `NIP-*.md` drafts written here, plus
  `contracts.md`, the kind registry `README.md`, and `schemas/`.

If an identifier exists in two lanes, `official` wins.

### 2.3 JSON Schemas and schema IDs

`nips/openagents/schemas/` holds 33 files, about 475 KB:

- **23 single-record schemas**: `artifact-ref`, `atif-chunk`,
  `atif-manifest`, `cj-card`, `eval-case`, `eval-draft`, `ext-eval`,
  `host-answer`, `host-call`, `kb-entry`, `kb-head`, `kb-withdrawal`,
  `payment-receipt`, `registry`, and nine `xp-*` schemas.
- **10 bundles with `$defs`**: automation (18 defs), coordination (17),
  environment (30), live (32), session (54), sovereign custody (11),
  sovereign lifecycle (13), sovereign treasury (18), work (25) and
  workspace (29).

These NIPs have **no schema file**: CAP, PRG, EXT core, RUN, CJ execute,
most of CTX, POL, the EVAL report, suite and admission, OPT, CTRL, MKT,
LAB, X402, HOST (except call and answer), REACH, TERM, VAULT, ATT, PYLON,
DEC, DEPLOY, AGENT, SKILL and MV. This spec gives every one of them a
package.

Repository-wide there are 2,368 distinct `openagents.*.vN` identifiers.
The largest families are `coder` (216, including 109 `coder-one`), `gym`
(119), `verse` (102), `sales` (88), `delegation` (53), `terminal` (52),
`environment` (52), `briefing` (50), `work` (47), `pylon` (45), `web` (44),
`tbench` (43), `computers` (43), `cloud` (37) and `coordination` (35).
Section 32 sorts them into protocol records, product API records, and
evidence or research records that stay JSON.

### 2.4 HTTP surfaces

- `https://api.openagents.com/v1`, served by the Rust Axum
  `crates/openagents-web` and `crates/gateway`.
- OpenAPI: `docs/api/openapi.first-party.json`,
  `docs/sdk/openapi.app.json`, `openapi.computers.json`,
  `openapi.labs.json`, `openapi.systemone.json`, and
  `docs/decision-models/api/openapi.yaml`.
- Shared conventions:
  - Error shape `{error:{type, code, message, param, request_id}}`.
  - Pagination `limit` and `after`, returning `{data, next}`.
  - `Idempotency-Key` header; `X-Workspace-Id` for workspace selection.
  - ID prefixes: `oak_ sess_ acct_ ws_ key_ th_ msg_ run_ tr_ pl_ resp_
    batch_ pr_ agent_ env_ envk_ sesn_ sevt_ vlt_ depl_ drun_ memstore_
    mem_ memver_ file_ skill_ wrun_ work_ whk_ whe_ outc_ cmk_ dec_ prj_
    upl_ inv_ turn_ lab_ job_ oa_sk_ ogb_`.

### 2.5 Other wire formats

| Format | Owner | Shape |
| --- | --- | --- |
| REACH frames | NIP-REACH, `coder-reach` | 4-byte length, 1-byte kind, 8-byte sequence, body; ≤65,536 B |
| TERM record streams | NIP-TERM, `coder-pty` | 10-byte header (tag u16, length u32, crc32c); ≤16 MiB per stream |
| HOST invitation | NIP-HOST | `coder-host:` + base64url of packed bytes; ≤640 B |
| Gym connect | NIP-EVAL | `gym-connect:` + base64url |
| AESV world container | Aethyr | magic, version, layer, schema, world, payload, CRC32 |
| Kernel ABI (`AFB1`), behavior bytecode (`AFBCv1`) | Aethyrforge | Fixed-stride binary |
| Loro updates and chat2 frames | `coder-sync` | Loro's own format plus framed JSON |
| ATIF trajectories | Harbor ATIF-v1.8 | JSON, sorted-key digest rule |

## 3. Principles

These bind every package in the tree. Each restates something the
repository already requires. None is new policy.

1. **A schema describes; it never authorizes.** Decoding a message grants
   nothing. A capability list, a grant body, a signature, a digest or a
   profile ID each states something. Authority is always a separate check
   by the host that owns the effect: tenant, workspace, grant, scope,
   expiry, revocation, epoch.
2. **Data classes never share a writable field.** Authored documents,
   runtime state, presence, releases, intents, receipts, views and
   telemetry are different classes (§6). A message declares its class, and
   a field never moves between classes by copying.
3. **Clients send intent; authorities send results.** Requests carry what
   the caller wants. Positions, damage, prices, rewards, XP, payment
   success and verification verdicts come only from the authority that
   owns them.
4. **Signed and content-addressed bytes keep their exact form.** NIP-01
   event IDs cover NIP-01 JSON. JCS digests cover JCS bytes. Blob digests
   cover stored bytes. Proto mirrors or carries these and never re-encodes
   them in a way that changes what was signed or hashed (§8).
5. **Closed vocabularies; no escape hatch.** No `google.protobuf.Any`,
   `Struct`, `Value` or free JSON maps for semantic data. An unmodeled
   format travels as a named `Opaque{contract, media_type, bytes}` with a
   declared contract ID, and that is migration debt. Unknown versions,
   features, kinds and enum values on authority paths fail closed with
   `unsupported_version` or `unsupported_feature`.
6. **Bounded everything.** Every string, bytes and repeated field has a
   maximum. Every decoder enforces it before allocating, including Nanopb
   and Wasm. The NIP limits are copied exactly, not rounded.
7. **One concept, one type.** A digest, an event reference, an outcome, a
   refusal, an amount, a time and a cursor each have one type in
   `openagents.type.v1`. A domain that means something different defines a
   differently named type and says why.
8. **Revisions fence mutations; request IDs deduplicate retries.** A
   mutating request carries an expected revision or epoch and an
   idempotency key. Revisions, epochs and generations never decrease. Undo
   is a new compensating command.
9. **Effects attenuate.** A child's `Effects` must be a subset of its
   parent's. Grants narrow and never widen. That holds across
   `capability`, `program`, `session`, `host`, `vault` and `payment`.
10. **Bitcoin-only money.** Amounts are millisatoshis or a Taproot Assets
    amount of a Bitcoin-based stablecoin. No EVM, Solana, Base, USDC-on-EVM,
    Tempo or Cashu rails in new schemas. Upstream-vendored formats that
    name other rails are mirrored as-is and never imported by OpenAgents
    packages (§26.1).
11. **Public schemas, private data.** Every schema is public. Personal
    data, secrets, licensed bytes and private evidence never appear in a
    public record. Fields that can carry them are annotated, and generated
    formatters redact them (§6.3).
12. **Owner direction is not re-litigated by schema.** Where the owner has
    decided a shape (DEC keeps `openagents.systemone.v1` forever; live
    sessions move to Loro rooms under HOST authority; one conversation, no
    Chat/Code modes), the schema follows the decision.

## 4. The tree

### 4.1 Top level

```
proto/
  README.md                  index, status per package, generation, publishing
  buf.yaml                   module, lint and breaking rules
  openagents/                everything OpenAgents owns
  nostr/                     typed mirrors of pinned Nostr NIPs (official + block)
  third_party/               byte-exact vendored protocols we do not own
  compatibility/             frozen descriptor sets per published package version
```

`openagents.*` is ours. `nostr.*` mirrors a protocol we follow but do not
own. `third_party.*` keeps upstream package names unchanged. Nothing under
`nostr/` or `third_party/` imports `openagents/`.

### 4.2 `openagents/`, the whole tree

```
proto/openagents/
  type/v1/              Digest, Pubkey, EventRef, ArtifactRef, DefinitionRef, Lock,
                        Effects, Bounds, Evidence, Observation, Outcome, Verification,
                        Integration, Refusal, Amount, Time, Cursor, Usage, Opaque
  options/v1/           descriptor options: data class, limits, sensitivity, units,
                        nip binding, json contract id, http mapping hints
  api/v1/               HTTP envelope: ApiError, Page, idempotency, webhooks envelope

  # Identity, accounts and secrets (§15)
  account/v1/           accounts, principals, workspaces, memberships, invitations,
                        tenant registry, admission snapshot, quotas, reservations
  credential/v1/        oak_ keys, scopes, device sign-in, linked identities, BYOK,
                        customer-managed keys (cmk_), OAuth connections
  vault/v1/             NIP-VAULT objects, wraps, slots, indexes, leases
  identity/v1/          Nostr key identity, NIP-05, owner attestation, key links

  # Decisions, inference, receipts (§16)
  decision/v1/          System One (noul/choice/score), Decisions API, NIP-DEC, doors,
                        classify, jobs, model cards, calibration records
  inference/v1/         Responses and chat-completion shapes, models, usage, tools
  receipt/v1/           execution receipts, receipt chains, receipt references

  # Jobs (§17)
  chat/v1/              NIP-CJ conversation family: request, result, feedback, cards,
                        offers, judgments, router, answer bank, threads, messages
  exec/v1/              NIP-CJ execution family: execute, accepted, progress, result,
                        status, replay, cancel

  # Agents (§18)
  agent/v1/             NIP-AGENT definitions, exports, gateway receipts, autonomy
  agent/managed/v1/     managed agents API: sessions, events, deployments, memory
                        stores, webhooks, environments, vault bindings
  session/v1/           NIP-SESS adapters, features, commands, intents, turns, history
  control/v1/           NIP-CTRL pairing, grants, commands, reads (revision pending)

  # Context, policy, capability (§19)
  context/v1/           NIP-CTX frames, snapshots, representations, selection, expansion
  policy/v1/            NIP-POL recipients, disclosure, instructions, preferences,
                        approvals, routes, route usage
  capability/v1/        NIP-CAP definitions, bindings, grants, presence, preferences,
                        service contracts

  # Programs, extensions, skills, knowledge (§20)
  program/v1/           NIP-PRG definitions, steps, bindings, modules, plugin packet ABI
  extension/v1/         NIP-EXT packages, manifests, releases, listings, revocations,
                        migrations, component sets; NIP-REG registries
  skill/v1/             NIP-SKILL listings, skill directory, skill bodies and hooks
  knowledge/v1/         NIP-KB entries, heads, withdrawals

  # Evaluation, Gym, optimization, traces (§21)
  eval/v1/              NIP-EVAL suites, cases, reports, publications, admissions,
                        extension evaluation, gates, verdicts
  gym/v1/               Gym boards, leaderboards, suites, grants, connections, recipes,
                        eval rows, results publications
  optimize/v1/          NIP-OPT signatures, implementations, studies, trials, results
  trace/v1/             ATIF trajectories (mirror of Harbor ATIF-v1.8), NIP-ATIF
                        manifests and chunks

  # Durable work (§22)
  run/v1/               NIP-RUN records, heads, fencing, handoff
  coordination/v1/      NIP-COORD coordinators, proposals, claims, findings, plans
  automation/v1/        NIP-AUTO plans, triggers, occurrences, evaluations, controls
  work/v1/              NIP-WORK items, commands, revisions, delegation, dispositions

  # Places to run (§23)
  workspace/v1/         NIP-WS bindings, resources, checkpoints, projections, activity
  environment/v1/       NIP-ENV definitions, leases, materialization, attachment, cleanup
  host/v1/              NIP-HOST grants, requests, replies, operations, connect codes
  reach/v1/             NIP-REACH directory, presence, hints, channel frames, placement
  terminal/v1/          NIP-TERM open, attach, input, frames, record streams, extensions
  live/v1/              NIP-LIVE plans, tracks, participants, consent, floors, captures

  # Cloud and compute (§24)
  cloud/v1/             retail cloud, pools, quotes, budgets, enrollment, host bindings
  computers/v1/         Computers API: computers, commands, files, credentials, usage
  compute/v1/           compute balance, placement, route contracts, leases

  # Coder (§25)
  coder/task/v1/        task store, task records, execution grants, requirements
  coder/host/v1/        host descriptor, host runtime, SSH-managed hosts, push leases
  coder/sync/v1/        coder-sync rooms, Loro frontiers, chat2 frames
  coder/router/v1/      chat router versions, tiers, routes, offers
  coder/foreman/v1/     Foreman dispatch, claims, chat joins, delegations
  coder/lab/v1/         Agent Lab and Labs runs, templates

  # Money (§26)
  market/v1/            NIP-MKT offerings, heads, records, terms, OrderRef
  labor/v1/             NIP-LAB terms, deliverables, verification, review, disputes
  payment/v1/           payment evidence, spend grants and receipts, Lightning
                        address, wallet views, payment receipts
  x402/v1/              NIP-X402 discovery, native records, status
  ledger/v1/            msat ledger, earnings, statements, compute balance entries
  billing/v1/           plans, subscriptions, entitlements, price terms, invoices,
                        provider-event journal

  # Pylon, attestation, sovereign agents (§27)
  pylon/v1/             NIP-PYLON beacons, receipts, pools, checks, model groups
  attest/v1/            NIP-ATT releases, heads, rebuilds, endpoints, sealed requests
  sovereign/v1/         NIP-SOV profile, activation, custody, lifecycle, treasury

  # Community (§28)
  xp/v1/                NIP-XP quests, awards, revocations, playtests, profiles, cards
  forum/v1/             Forum boards, threads, posts, moderation (Nostr-backed)
  deploy/v1/            NIP-DEPLOY phase events and lane heads
  referral/v1/          referral sources, attribution, consent, commission terms

  # Interface (§29)
  ui/v1/                Aethyr view tree (aethyr.ui.v1), intents, activation, tokens
  engine/v1/            (exists) engine-neutral runtime contracts
  verse/...             Chapter V: everything 3D

  # Telemetry and evidence (§30)
  telemetry/v1/         product events, playtest events, consent, automation marking
  evidence/v1/          the generic evidence envelope for research and qualification
                        records that stay JSON
  discovery/v1/         capability manifest, API index, docs corpus entries
```

That is 64 OpenAgents packages outside the Verse, plus the 28 Verse
packages in Chapter V.

### 4.3 `nostr/` and `third_party/`

```
proto/nostr/
  event/v1/             Event, Tag, Filter, relay messages (REQ, EVENT, OK, EOSE,
                        CLOSED, NOTICE, AUTH, COUNT, NEG-*)
  nip<NN>/v1/           one package per official NIP with a structured body (§11.3)
  block/<id>/v1/        one package per Block NIP with a structured body (§11.4)
  MANIFEST.json         pins: nips/manifest.json commits, per-package source digest

proto/third_party/
  lightninglabs/
    lnd/v0.21.1-beta/              lnrpc, routerrpc, invoicesrpc, walletrpc, signrpc,
                                   chainrpc, peersrpc, …
    taproot-assets/v0.8.0/         taprpc, assetwalletrpc, mintrpc, universerpc,
                                   priceoraclerpc, rfqrpc, tapchannelrpc, …
    loop/v0.34.0-beta/             looprpc, swapserverrpc
    lightning-terminal/v0.17.0-alpha/  litrpc
    sources.json                   upstream commit, path, SHA-256, license per file
  harbor/atif/v1.8/                (if Harbor publishes protos; else the JSON Schema
                                   copied byte-exact, §13.3)
```

### 4.4 Ownership rules for the tree

- **Imports flow downward.** `type` and `options` import nothing of ours.
  `api` imports `type`. Every domain imports `type` and `options`. Domains
  import each other only along the graph in §14.2, which has no cycles.
- **Reference other owners; never embed them.** A Verse quest references an
  XP award by `EventRef`. It does not embed `xp.v1.Award`. A labor order
  references a payment by `PaymentRef`. It does not embed the BOLT11
  invoice's decoded form. This keeps each owner free to version
  independently.
- **NIP bodies live with their owner.** NIP-MV bodies live in
  `openagents.verse.presence.v1`, NIP-PYLON bodies in `openagents.pylon.v1`,
  NIP-DEC bodies in `openagents.decision.v1`. The event envelope is always
  `nostr.event.v1.Event`.
- **Vendored packages keep their names.** We never rename `lnrpc` to
  `openagents.lightning`. A wrapper that adds OpenAgents semantics, such as
  payment evidence about an LND invoice, lives in `openagents.payment.v1`
  and references the upstream object by hash and type.

## 5. Naming, versioning and stability

### 5.1 Names

- **Packages:** `openagents.<domain>[.<subdomain>].v<N>`, lowercase, no
  underscores. Subdomains only where a domain has distinct owners, as in
  `coder.task`, `coder.sync` and `agent.managed`.
- **Files:** named by subject (`grant.proto`, `frame.proto`), not by
  message. Every package has a `<domain>.proto` holding its shared types.
- **Messages:** `PascalCase` nouns. Requests and replies are
  `VerbNounRequest` and `VerbNounReply`, as in `engine.v1`. Events and facts
  are past-tense or noun phrases (`GrantRevoked`, `ClaimRenewed`).
- **Fields:** `snake_case`. Units go in the name: `_ms`, `_s`, `_bytes`,
  `_msat`, `_m`, `_pct`, `_microunits`. The NIP JSON key and the proto
  field name are the same wherever the JSON key is a readable word. They
  differ only for single-letter keys (NIP-MV `p`, `q`) and for keys that
  collide with proto reserved words. Every difference is in the package's
  mapping table.
- **Enums:** `ENUM_NAME_VALUE`. Zero is `*_UNSPECIFIED` and invalid on any
  durable, intent, release or receipt message. An enum value's JSON
  spelling is the NIP spelling: `OUTCOME_COMPLETED` maps to `"completed"`.
  The mapping is generated from an `(openagents.options.v1.wire)` value
  option on each enum value, never derived by lowercasing.
- **Open vocabularies** that third parties extend are `string` slugs with a
  documented registry, not enums. Examples: capability profiles' transport
  names, gesture names, HOST operation kinds from extensions, component
  kinds from new extension types, feature IDs.
- **Schema IDs.** Every durable top-level message carries the existing JSON
  schema ID as a message option, `(openagents.options.v1.contract) =
  "openagents.grant.v1"`. The proto message and the JSON contract are two
  encodings of one contract and share one version.

### 5.2 Version axes

| Axis | Example | Changes when |
| --- | --- | --- |
| Package | `openagents.host.v1` | A field's meaning or type changes, or a required field is added |
| Contract ID | `openagents.host-grant.v1` | That record's JSON contract changes; usually tracks the package |
| Profile or feature ID | `pylon.deadline.v1`, `openagents.labor-binding.v1`, `oa-x402-v1` | An opt-in behavior changes |
| Wire or container | REACH frame kinds, TERM record header, AESV v2, NIP-44 v2 | Framing changes |
| Upstream pin | NIP commit `b82211e9`, LND `v0.21.1-beta` | We re-vendor |

Rules:

- **Additive change stays in the package**: new optional fields, new
  messages, new enum values on non-authority messages.
- **Each published package version is frozen** as a descriptor set in
  `proto/compatibility/<package>/<version>.binpb`. CI fails on a breaking
  change against any frozen set (`buf breaking` plus the existing
  `engine-proto` checker).
- **Retired field numbers and names are reserved forever.**
- **`v2` is a new directory.** Its messages are new types. The owning crate
  ships a `v1 → v2` converter, and readers keep `v1` for as long as any
  durable `v1` record exists.
- **A contract that existed before its package can start at its existing
  version.** `openagents.pylon-receipt.v2` becomes a message in
  `openagents.pylon.v1` with contract option `openagents.pylon-receipt.v2`.
  Package version and contract version are separate axes, and we do not
  rename shipped contract IDs to make them line up.
- **`openagents.systemone.v1` never becomes v2.** This is an owner
  decision recorded in NIP-DEC. New decision shapes get new messages under
  new contract IDs.

### 5.3 Stability labels

| Label | Meaning | Who may depend on it |
| --- | --- | --- |
| `v1alpha1` | Shape under design; can break between commits | Tests and prototypes; never a durable store |
| `v1beta1` | Shape frozen for review; breaking changes need a migration note | Product code behind a flag |
| `v1` | Frozen; breaking changes forbidden | Everyone, including external implementers |

Packages that mirror a contract that is already implemented and stable
start **direct v1**: `type`, `api`, `decision`, `exec`, `chat`, `run`,
`trace`, `deploy`, `pylon`, `knowledge`, `xp`, `host`, `reach`, `terminal`
and the `nostr.*` mirrors. Everything else starts `v1alpha1` and graduates
through the §35 gate. The status table in §36 tracks each one.

## 6. Data classes, descriptor options and authority

### 6.1 `openagents.options.v1`

Every top-level message declares its data class and its contract. Fields
declare limits, sensitivity and unit. These options are the single source
for generated validators, Nanopb `.options`, C++ field tables, TypeScript
guards, redaction helpers and the JSON mapping.

```proto
// proto/openagents/options/v1/options.proto
syntax = "proto3";
package openagents.options.v1;
import "google/protobuf/descriptor.proto";

enum DataClass {
  DATA_CLASS_UNSPECIFIED = 0;
  DATA_CLASS_DOCUMENT = 1;   // authored source; durable; strict
  DATA_CLASS_RUNTIME = 2;    // authoritative state; strict
  DATA_CLASS_PRESENCE = 3;   // self-asserted, non-authoritative; lenient
  DATA_CLASS_RELEASE = 4;    // immutable identity of a published thing; strict
  DATA_CLASS_INTENT = 5;     // a request to an authority; strict; idempotent
  DATA_CLASS_RECEIPT = 6;    // an authority's attributed answer or outcome; strict
  DATA_CLASS_VIEW = 7;       // projection of another owner's records; lenient
  DATA_CLASS_TELEMETRY = 8;  // measurement; append-only; lenient; privacy-annotated
  DATA_CLASS_GRANT = 9;      // a bounded delegation of authority; strict; expiring
  DATA_CLASS_EVIDENCE = 10;  // a captured observation or report others may cite; strict
}

enum Sensitivity {
  SENSITIVITY_UNSPECIFIED = 0;
  SENSITIVITY_PUBLIC = 1;
  SENSITIVITY_PERSONAL = 2;  // free text, email, location, display names tied to accounts
  SENSITIVITY_CONFIDENTIAL = 3; // tenant data: code, prompts, transcripts, documents
  SENSITIVITY_SECRET = 4;    // keys, tokens, mnemonics: only inside an encrypted carriage
}

enum Carriage {
  CARRIAGE_UNSPECIFIED = 0;
  CARRIAGE_NOSTR_PUBLIC = 1;     // signed public event content
  CARRIAGE_NOSTR_PRIVATE = 2;    // kind 3188 envelope, NIP-44 to one recipient
  CARRIAGE_NOSTR_EPHEMERAL = 3;  // CJ / MV ephemeral kinds, NIP-44 where the NIP says so
  CARRIAGE_HTTP = 4;             // api.openagents.com and other HTTP surfaces
  CARRIAGE_RPC = 5;              // gRPC / Connect services
  CARRIAGE_FRAME = 6;            // REACH, TERM and realm framed channels
  CARRIAGE_FILE = 7;             // stored artifacts, bundles, snapshots
}

message FieldLimits {
  uint32 max_bytes = 1;      // strings and bytes, UTF-8 bytes
  uint32 min_bytes = 2;
  uint32 max_count = 3;      // repeated
  uint32 min_count = 4;
  string min = 5;            // decimal; numeric bound, inclusive
  string max = 6;
  bool finite = 7;           // refuse NaN and ±Inf
  string grammar = 8;        // a named grammar from type.v1 (slug, hex64, digest, ...)
  uint32 max_depth = 9;      // nested EntryType-like trees
}

message NipBinding {
  string nip = 1;            // "NIP-PYLON", "NIP-01", "block:OA"
  repeated uint32 kinds = 2; // event kinds whose content is this message
  string topic_tag = 3;      // "oa:pylon:beacon:v1" etc.
}

message HttpBinding {
  string method = 1;         // "POST"
  string path = 2;           // "/v1/decisions"
  string id_prefix = 3;      // "dec_"
}

extend google.protobuf.MessageOptions {
  DataClass data_class = 50100;
  uint32 max_encoded_bytes = 50101;
  string contract = 50102;          // JSON contract ID, e.g. "openagents.grant.v1"
  NipBinding nip = 50103;
  repeated Carriage carriage = 50104;
  HttpBinding http = 50105;
  string status = 50106;            // "implemented" | "partial" | "designed"
}
extend google.protobuf.FieldOptions {
  FieldLimits limits = 50110;
  Sensitivity sensitivity = 50111;
  string unit = 50112;
  string json_key = 50113;          // NIP JSON key when it differs from the field name
  bool nullable = 50114;            // JSON null is a distinct value (§7.5)
}
extend google.protobuf.EnumValueOptions {
  string wire = 50120;              // NIP / JSON spelling of this value
}
```

The extension range 50100–50199 comes from the `50000–99999` in-house
range. Before publication we register a block in protobuf's global
extension registry (§34.4).

### 6.2 What each class means for readers

| Class | Unknown fields | Missing required | Who may write | Kept by |
| --- | --- | --- | --- | --- |
| Document | Reject on read-for-edit; preserve only where a package declares a preservation block | Reject | The authoring service after admission | Project stores |
| Runtime | Reject | Reject | The owning authority | Checkpoints, journals |
| Presence | Ignore | Drop the record | Anyone, about themselves | Relays, last-known |
| Release | Reject | Reject | The publisher, through a publish transaction | Release stores, relays |
| Intent | Reject | Refuse `malformed` | A caller with a session or grant | Idempotency log |
| Receipt | Reject | Reject | The authority that performed the effect | Authority ledger, relays |
| View | Ignore | Show what exists | Any projector | Not durable |
| Telemetry | Ignore | Drop the field | Instrumented clients | Append-only stores |
| Grant | Reject | Reject | The issuer, within its own authority | Issuer and holder |
| Evidence | Reject | Reject | The observer named in the record | Artifact stores |

A reader that rewrites a durable record never silently drops unknown
fields. Strict readers refuse the record. Preserving readers use an
explicit block the package declares. Protobuf's default unknown-field
behavior is never relied on, because it differs between runtimes and
between binary and JSON.

This matches `contracts.md`: an unknown version, feature, enum value or
semantic field on a body refuses with `unsupported_version` or
`unsupported_feature`. Only the `meta` object is inert, and proto models
it as a bounded `Meta` message (§9.11), not as a free map.

### 6.3 Sensitivity in practice

- `PERSONAL` and `CONFIDENTIAL` fields are redacted from `Debug`, logs,
  traces and crash reports by generated formatters. Telemetry carries
  them only with a consent field set.
- `SECRET` fields may exist only inside a message whose carriage is
  `NOSTR_PRIVATE`, `RPC` over an authenticated channel, or a sealed vault
  object. The generator refuses a `SECRET` field in a message whose
  carriage list includes `NOSTR_PUBLIC`, `HTTP` responses, or `FILE`
  without encryption. Today that covers provider keys in BYOK flows,
  NIP-46 custody requests, NWC secrets and vault DEKs.
- Owner-local secrets named in the workspace `AGENTS.md` (treasury
  mnemonic, CRM tokens, service account keys) have **no** schema field
  anywhere. They are never carried by this protocol.

### 6.4 Authority by domain

| Domain | Authority | Proto consequence |
| --- | --- | --- |
| Accounts, keys, workspaces | `openagents-web` tenancy store | `account.v1` intents in; views out; keys are `GRANT` |
| Decisions | Decision worker and doors | `decision.v1` requests are `INTENT`; results carry a `RECEIPT` |
| Chat jobs | The addressed worker key | `chat.v1` requests `INTENT`; feedback `VIEW`; result `RECEIPT` |
| Execution jobs | The addressed worker key | `exec.v1`; idempotency key `(worker, principal, request, attempt)` |
| Run journals | The controller holding the current generation | `run.v1` records `RUNTIME`; forks halt the run |
| Capabilities and grants | Each host for its own bindings | `capability.v1` grants are `GRANT`; presence is `PRESENCE` |
| Hosts and devices | The host key | `host.v1` grants `GRANT`, requests `INTENT`, replies `RECEIPT` |
| Markets and labor | Buyer and provider jointly, through sequenced records | `market.v1` record chain with `seq` and `prev` |
| Payments | The payment observer and the wallet | `payment.v1` evidence is `EVIDENCE`; no client-asserted settlement |
| Pylon pools | The aggregator, recomputable from inputs | `pylon.v1` pools are `RECEIPT` with input digests |
| XP | The referee key | `xp.v1` awards `RECEIPT`; clients only display |
| Evaluation | The evaluator named in the report | `eval.v1` reports `EVIDENCE`; admissions `RECEIPT` |
| Verse | Per Chapter V §6.3 | Unchanged |

## 7. Limits, numbers, canonical bytes and JSON

### 7.1 Limits copied from `contracts.md`

These apply to every OpenAgents NIP body and therefore to every proto
message that mirrors one:

- Body ceiling 1,048,576 bytes unless a NIP sets a lower one. Lower ones
  are common: DEC decrypted payload 256 KiB, HOST bodies 128 KiB, AGENT
  content 262,144 B, SKILL content 16,384 B, activity summaries 1,024 B.
- JSON nesting depth ≤64. DEC `state` ≤32 and entry types ≤8.
- Integers within ±(2^53−1). Proto `int64` and `uint64` fields that cross
  JSON carry a `limits.max` of `9007199254740991`, and the validator
  enforces it on both encodings.
- Duplicate JSON keys and non-finite numbers are refused before mapping.
- Global ceilings for proto-native messages follow `engine.v1`: 16 MiB per
  message, 64 nesting levels, 250,000 visited values.

### 7.2 Numbers

- **Money** is never a float. `Amount` (§9.8) holds millisatoshis as
  `uint64` with the 2^53 bound when it crosses JSON, matching the
  `*_msat` decimal strings the NIPs use. The JSON mapping writes a
  decimal string where the NIP does, and a number where the NIP does.
  Each field's mapping says which.
- **Microunits.** POL usage and the bounds vocabulary use
  `cost_microunits`/`spend_microunits` paired with `currency`. Proto keeps
  that pairing as `MicroAmount{microunits, currency}` and restricts
  `currency` to `BTC` and registered Taproot asset IDs (§26.1).
- **Probabilities** are `double`, finite, in `[0, 1]`. Sums are checked to
  within 1e-6, as NIP-DEC requires.
- **Time.** Wall-clock instants are `int64` with an explicit unit suffix.
  Nostr fields are seconds (`created_at`, `expires_at`, `issued_at` in
  most NIPs). HTTP and Verse fields are milliseconds. A field never changes
  unit; a new field is added instead. `google.protobuf.Timestamp` is not
  used, because Nanopb, the C ABI and the NIPs are simpler with integers.
- **Durations** are `uint32` or `uint64` with `_ms` or `_s`.

### 7.3 IDs and grammars

`type.v1` defines named grammars that `FieldLimits.grammar` refers to:

| Grammar | Pattern | Used by |
| --- | --- | --- |
| `slug` | `^[a-z0-9][a-z0-9_-]{0,63}$` | package and component slugs, set names |
| `hex64` | 64 lowercase hex | pubkeys, event IDs, request IDs, run IDs |
| `digest` | `sha256:<64 hex>` | every `Digest` in text form |
| `qualified_id` | `<hex64>:<slug>/<slug>` | EXT and KB component IDs |
| `kb_id` | `^[a-z][a-z0-9.-]{0,127}$` | KB entry IDs |
| `prefixed_id` | `^<prefix>_[A-Za-z0-9]{8,64}$` | HTTP resource IDs (§10.3) |
| `semver` | SemVer 2.0 | SKILL versions, package versions |
| `relay_url` | `wss://` or `ws://` URL ≤512 B | relay lists |
| `bolt11` | BOLT11, ≤8,192 B | MKT and X402 invoices |
| `lnbtc_network` | `lnbtc:<32 hex>` | X402 networks |

### 7.4 Canonical bytes and hashing

Protobuf binary is not canonical. Where a record is hashed, the owning
package states one of three rules, and no package invents a fourth:

1. **Hash the stored bytes.** Blobs, artifacts, packs, chunk text and
   ciphertext. The digest covers exactly the bytes as stored.
2. **Hash the JCS of the JSON form.** This is the rule for almost every
   OpenAgents NIP record today: lock digests, run record digests, ATT
   bindings, vault cores, receipts and fingerprints. Proto messages that
   mirror these records are hashed by mapping to JSON and applying RFC 8785
   JCS. The proto bytes are never hashed. Domain-separated prefixes stay
   exactly as written, such as `"openagents.run-record.v1\n"` and
   `"openagents.att.v1\0"`.
3. **Hash a canonical proto encoding.** Only for proto-native records that
   have no JSON form and need multi-producer digests (Verse documents,
   replay inputs). Fields go in number order, with no unknown fields, no
   maps, packed scalars, no default-valued scalars, and presence
   preserved. `openagents-proto-canon` is tested byte-identical in Rust,
   C++ and Python.

ATIF has its own rule: sorted keys, no whitespace, SHA-256. That differs
from JCS, and the two are never compared (NIP-ATIF). `trace.v1` records
which rule each digest field uses with a `DigestRule` enum.

Maps are banned in hashed messages and discouraged elsewhere. Repeated
keyed messages keep order deterministic.

### 7.5 JSON, null and omission

`contracts.md` makes omitted and `null` different. Proto3 cannot express
that with a plain field, so the protocol uses these patterns:

- **Optional value:** `optional T x` means omitted or present.
- **Nullable value:** a field with `(nullable) = true` and a wrapper
  `oneof x_state { Null x_null = N; T x = M; }`. `Null` is
  `openagents.type.v1.Null`, an empty message. Omitted, `null` and a value
  are all distinct.
- **`meta`** is `Meta`, a bounded list of `{key, opaque_json}` pairs that
  readers must not interpret (§9.11). It survives round trips byte-exact.
- **Integer `v` versus string `v`.** Some NIP bodies use `v: 1` and others
  use `v: "openagents.x.v1"`. The mapping table for each message states
  which and what value. Proto has no `v` field; the version is the
  contract option, and the mapper writes `v`.

The protobuf JSON mapping (proto3 JSON) is supported for tools and logs.
It is **never** the NIP canonical form. NIP JSON comes from the generated
NIP mapper only (§8.2).

## 8. One message, three carriages: Nostr, HTTP and RPC

Many OpenAgents records travel more than one way. A decision request goes
over HTTP (`POST /v1/systemone`) and over Nostr (kind 25910). A host
request goes over a 3188 envelope or a REACH direct channel. An execution
receipt is an HTTP response field, a NIP-AGENT gateway receipt and a Pylon
receipt input. The protocol defines each record once and maps it to every
carriage.

### 8.1 The rule

```
            one proto message (the contract)
           /              |                 \
   NIP JSON mapper   HTTP JSON mapper     binary proto
   (signed content,   (OpenAPI bodies,     (RPC, frames,
    3188 envelopes)    webhook bodies)      files, C ABI)
```

- The **NIP JSON mapper** produces exactly the JSON the NIP specifies:
  keys, `v`, enum spellings, decimal-string amounts, `null` handling and
  `meta`. Its output is what gets signed or encrypted. It is generated from
  descriptor options and tested against every example in the NIP text and
  every fixture in `nips/openagents/schemas/` and `docs/protocol/fixtures/`.
- The **HTTP JSON mapper** produces the OpenAPI body. For records that are
  also NIP bodies, it is the NIP mapper. For HTTP-only records, it follows
  the `api.v1` conventions (§10): `snake_case`, `object` discriminator,
  prefixed IDs, `created_at` in seconds.
- **Binary proto** is used where both ends are ours or are generated
  clients: RPC services, framed channels, stored bundles, the engine's C
  ABI.

### 8.2 Carriage table for the major records

| Record | NIP carriage | HTTP carriage | RPC or frame |
| --- | --- | --- | --- |
| `decision.v1.SystemOneRequest` | 25910, NIP-44 | `POST /v1/systemone` | `DecisionService.Decide` |
| `decision.v1.DecisionsRequest` | via a door's CJ lane | `POST /v1/decisions` | `DecisionService.Decisions` |
| `chat.v1.ConversationRequest` | 25900, NIP-44 | app chat routes | coder-sync chat2 frame |
| `exec.v1.Execute` | 25920, NIP-44 | `POST /v1/jobs` (worker gateway) | `ExecService.Execute` |
| `host.v1.HostRequest` | 3188 or REACH data frame | — | REACH channel |
| `terminal.v1.TerminalFrame` | 3188 or REACH | — | REACH channel, TERM record stream |
| `run.v1.RunRecord` | 3187, NIP-44 | — | `RunService.Append` |
| `pylon.v1.Beacon` | 30200 public | `GET /v1/pylons` | — |
| `receipt.v1.ExecutionReceipt` | inside 3204, 3201 | `X-Receipt`, response field | response field |
| `agent.v1.AgentDefinition` | 30187 public, 3188 export | `/v1/agents` | — |
| `x402.v1.PaymentRequired` | native record on 3188 | HTTP 402 body (upstream-exact) | MCP binding (upstream-exact) |
| `deploy.v1.DeployStatus` | 3205, 30205 | Grid billboard feed | — |

### 8.3 Encrypted carriages

- **Kind 3188 private artifact envelope.** Exactly one `p`, one random
  64-hex `h` mailbox, `t: oa:artifact:v1`. Content is NIP-44 v2 to `p`.
  The decrypted plaintext is `openagents.artifact-envelope.v1`:
  `{artifact: ArtifactRef, inline, issued_at, retain_until}`. Proto models
  the plaintext as `type.v1.ArtifactEnvelope`, and the `inline` body as a
  `oneof` over the registered private record types, never as free JSON.
  Relays restrict reads to author and recipient and advertise
  `oa-private-artifacts-v1` in NIP-11.
- **NIP-44 v2** payloads are modeled in `nostr.nip44.v1` as their binary
  layout (version byte, nonce, ciphertext, MAC) for implementers. The
  protocol never defines its own encryption.
- **NIP-59 gift wraps**, **HPKE sealed requests** (ATT) and **vault wraps**
  (VAULT) each have a typed outer message and a typed decrypted inner
  message. Code that decrypts produces the inner type directly.

### 8.4 Registered private record types

Every record that can travel in a 3188 envelope is registered in one
`oneof` in `type.v1.PrivateRecord`. Field numbers are allocated in blocks
of 100 per owning package, so packages can add records without touching
each other's ranges:

| Block | Owner | Examples |
| --- | --- | --- |
| 100–199 | `agent` | definition export |
| 200–299 | `atif`/`trace` | private trajectory copies |
| 300–399 | `automation` | plans, occurrences, controls |
| 400–499 | `capability` | grants, private preferences |
| 500–599 | `context` | frames, selections |
| 600–699 | `control` | invitations, grants, commands |
| 700–799 | `coordination` | proposals, claims, findings |
| 800–899 | `environment` | requests, leases, attachments |
| 900–999 | `host` | grants, requests, replies |
| 1000–1099 | `labor` | terms, submissions, reviews |
| 1100–1199 | `live` | plans, captures, transcripts |
| 1200–1299 | `market` | the 18 record types |
| 1300–1399 | `policy` | approvals, preferences |
| 1400–1499 | `reach` | directory, presence, hints |
| 1500–1599 | `session` | commands, intents, history |
| 1600–1699 | `sovereign` | custody, lifecycle, treasury |
| 1700–1799 | `terminal` | open, attach, frames |
| 1800–1899 | `vault` | slots, indexes, requests, leases |
| 1900–1999 | `work` | items, commands, revisions |
| 2000–2099 | `workspace` | bindings, checkpoints, projections |
| 2100–2199 | `x402` | native records |
| 2200–2299 | `attest` | approval sets |
| 2300–2399 | `eval`/`gym` | private boards, grants |
| 2400–2499 | `verse` | private placements |
| 2500–2999 | reserved | future owners |


---

# Part II. Foundations

## 9. `openagents.type.v1`: shared primitives

Status: **direct v1**. Every type here mirrors `nips/openagents/contracts.md`
or an existing product convention. Nothing imports anything but
`options.v1`.

### 9.1 References

```proto
syntax = "proto3";
package openagents.type.v1;
import "openagents/options/v1/options.proto";

// sha256:<64 hex> in text; 32 raw bytes in binary.
message Digest {
  bytes sha256 = 1 [(openagents.options.v1.limits) = {min_bytes: 32, max_bytes: 32}];
}

// 32-byte x-only secp256k1 public key (Nostr identity).
message Pubkey { bytes key = 1 [(openagents.options.v1.limits) = {min_bytes: 32, max_bytes: 32}]; }

// contracts.md EventRef: {id, pubkey, kind, coordinate?}
message EventRef {
  bytes id = 1;                 // 32 bytes
  Pubkey pubkey = 2;
  uint32 kind = 3;
  optional string coordinate = 4; // "<kind>:<pubkey hex>:<d>" for addressable events
}

message ArtifactSource {
  oneof source {
    string url = 1 [(openagents.options.v1.limits) = {max_bytes: 2048}];
    EventRef event = 2;
  }
}

// contracts.md ArtifactRef: {digest, size, media_type, schema?, event?, sources?}
message ArtifactRef {
  Digest digest = 1;
  uint64 size = 2 [(openagents.options.v1.unit) = "bytes"];
  string media_type = 3 [(openagents.options.v1.limits) = {max_bytes: 255}];
  optional ArtifactRef schema = 4;   // SchemaRef: JSON Schema 2020-12 artifact
  optional EventRef event = 5;
  repeated ArtifactSource sources = 6 [(openagents.options.v1.limits) = {max_count: 16}];
}

// contracts.md DefinitionRef: {id, artifact, event?}
message DefinitionRef {
  string id = 1;                     // grammar: qualified_id
  ArtifactRef artifact = 2;
  optional EventRef event = 3;
}

// <64hex-pubkey>:<package-slug>/<component-slug>
message QualifiedId { Pubkey publisher = 1; string package = 2; string component = 3; }

// Address of an HTTP resource: the prefixed id and its kind.
message ResourceId { string id = 1; string object = 2; } // {"id":"run_…","object":"run"}
```

### 9.2 Locks

```proto
// openagents.lock.v1; lock_digest = SHA256(JCS(lock))
message Lock {
  option (openagents.options.v1.contract) = "openagents.lock.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_RELEASE;
  repeated string requires = 1;
  DefinitionRef root = 2;
  repeated LockEntry entries = 3 [(openagents.options.v1.limits) = {max_count: 4096}];
}
message LockEntry { string id = 1; DefinitionRef definition = 2; repeated string dependencies = 3; }
```

### 9.3 Effects and bounds

```proto
// contracts.md Effects. Attenuation: child ⊆ parent on every field.
message Effects {
  repeated string reads = 1;      // scope IDs
  repeated string writes = 2;
  repeated string network = 3;
  bool process = 4;
  bool delegates = 5;
  bool spend = 6;
}

// The closure of effective authority, recorded as `reach`.
message Reach { repeated ArtifactRef bindings = 1; }

// contracts.md bounds vocabulary. Every field optional: absent = not bounded here.
message Bounds {
  optional uint64 wall_ms = 1;
  optional uint64 memory_bytes = 2;
  optional uint64 input_bytes = 3;
  optional uint64 output_bytes = 4;
  optional uint64 read_bytes = 5;
  optional uint64 storage_bytes = 6;
  optional uint32 calls = 7;
  optional uint32 attempts = 8;
  optional uint32 concurrency = 9;
  optional uint32 depth = 10;
  optional uint64 fuel = 11;
  optional MicroAmount spend = 12;   // spend_microunits + currency
}

enum BoundSupport {
  BOUND_SUPPORT_UNSPECIFIED = 0;
  BOUND_SUPPORT_ENFORCED = 1 [(openagents.options.v1.wire) = "enforced"];
  BOUND_SUPPORT_NOT_ENFORCED = 2 [(openagents.options.v1.wire) = "not_enforced"];
  BOUND_SUPPORT_UNKNOWN = 3 [(openagents.options.v1.wire) = "unknown"];
}
```

### 9.4 Evidence, observation and context manifests

```proto
// openagents.evidence.v1
message Evidence {
  option (openagents.options.v1.contract) = "openagents.evidence.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_EVIDENCE;
  string id = 1;
  ArtifactRef content = 2;
  EvidenceSource source = 3;
  Capture capture = 4;
  repeated string derived_from = 5;
  EvidenceScope scope = 6;
  optional ArtifactRef transform = 7;
  optional ArtifactRef parameters = 8;
}
message EvidenceSource { string kind = 1; string identity = 2; string version = 3;
                         optional string adapter = 4; optional string observation = 5; }
message Capture { bool complete = 1; uint64 omitted_bytes = 2; string reason = 3; }
message EvidenceScope { string task = 1; repeated string recipients = 2; string classification = 3; }

// openagents.observation.v1
message Observation {
  option (openagents.options.v1.contract) = "openagents.observation.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_EVIDENCE;
  ResourceScope resource = 1;
  string adapter = 2;
  int64 captured_at = 3 [(openagents.options.v1.unit) = "s"];
  string provider_revision = 4;
  ArtifactRef content = 5;
  Consistency consistency = 6;
  optional int64 valid_until = 7;
}
message ResourceScope { string scope = 1; string id = 2; }
enum Consistency {
  CONSISTENCY_UNSPECIFIED = 0;
  CONSISTENCY_IMMUTABLE = 1;
  CONSISTENCY_CONDITIONAL = 2;
  CONSISTENCY_OBSERVATIONAL = 3;
}

// openagents.context.v1, the manifest of what a recipient was shown.
message ContextManifest {
  option (openagents.options.v1.contract) = "openagents.context.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_EVIDENCE;
  string task = 1;
  string recipient = 2;
  ArtifactRef policy = 3;
  repeated ContextEntry entries = 4;
  repeated Omission omissions = 5;
  Coverage coverage = 6;
  optional ArtifactRef selection = 7;
}
message ContextEntry { string evidence = 1; string representation = 2; bool mandatory = 3; }
message Omission { string evidence = 1; string reason = 2; }
message Coverage { uint32 included = 1; uint32 omitted = 2; bool complete = 3; }
```

### 9.5 Outcomes

The repository has several outcome vocabularies. The protocol keeps the
ones that mean different things and names each one:

```proto
// contracts.md: what happened to an operation.
enum Outcome {
  OUTCOME_UNSPECIFIED = 0;
  OUTCOME_COMPLETED = 1 [(openagents.options.v1.wire) = "completed"];
  OUTCOME_REFUSED = 2   [(openagents.options.v1.wire) = "refused"];
  OUTCOME_FAILED = 3    [(openagents.options.v1.wire) = "failed"];
  OUTCOME_CANCELLED = 4 [(openagents.options.v1.wire) = "cancelled"];
  OUTCOME_UNKNOWN = 5   [(openagents.options.v1.wire) = "unknown"];
}
// Recorded separately from Outcome: was the work handed to an executor at all?
message Dispatch { bool dispatched = 1; }

enum Verification {
  VERIFICATION_UNSPECIFIED = 0;
  VERIFICATION_PASSED = 1; VERIFICATION_FAILED = 2;
  VERIFICATION_UNVERIFIABLE = 3; VERIFICATION_NOT_RUN = 4;
}
enum Integration {
  INTEGRATION_UNSPECIFIED = 0;
  INTEGRATION_ACCEPTED = 1; INTEGRATION_REJECTED = 2;
  INTEGRATION_PENDING = 3; INTEGRATION_NOT_REQUESTED = 4;
}

// openagents.receipt.execution.v1: what a service did with a request.
enum ServiceOutcome {
  SERVICE_OUTCOME_UNSPECIFIED = 0;
  SERVICE_OUTCOME_ANSWERED = 1; SERVICE_OUTCOME_REFUSED = 2;
  SERVICE_OUTCOME_UNAVAILABLE = 3; SERVICE_OUTCOME_UNATTEMPTED = 4;
  SERVICE_OUTCOME_UNKNOWN = 5;
}

// Shared result status of admission-style operations
// (COORD, AUTO, ENV, WORK, X402 native records).
enum AdmissionStatus {
  ADMISSION_STATUS_UNSPECIFIED = 0;
  ADMISSION_STATUS_ACCEPTED = 1; ADMISSION_STATUS_DUPLICATE = 2;
  ADMISSION_STATUS_CONFLICT = 3; ADMISSION_STATUS_REFUSED = 4;
  ADMISSION_STATUS_UNKNOWN = 5;
}

// Eval verdicts: pass|fail|inconclusive.
enum Verdict { VERDICT_UNSPECIFIED = 0; VERDICT_PASS = 1; VERDICT_FAIL = 2; VERDICT_INCONCLUSIVE = 3; }
```

Pylon receipts use `accepted|rejected|failed|timeout`. That vocabulary
describes buyer acceptance of a unit of work, not operation outcome, so it
stays in `pylon.v1.ReceiptOutcome` under that name.

### 9.6 Refusals

```proto
// contracts.md's 14 shared codes. Domains extend with their own enum, never by
// reusing a shared number for a new meaning.
enum RefusalCode {
  REFUSAL_CODE_UNSPECIFIED = 0;
  REFUSAL_CODE_MALFORMED = 1;
  REFUSAL_CODE_UNSUPPORTED_VERSION = 2;
  REFUSAL_CODE_UNSUPPORTED_FEATURE = 3;
  REFUSAL_CODE_NOT_ADMITTED = 4;
  REFUSAL_CODE_UNAVAILABLE = 5;
  REFUSAL_CODE_CONTENT_UNAVAILABLE = 6;
  REFUSAL_CODE_IDENTITY_MISMATCH = 7;
  REFUSAL_CODE_INCOMPATIBLE = 8;
  REFUSAL_CODE_REVOKED = 9;
  REFUSAL_CODE_STALE = 10;
  REFUSAL_CODE_CANNOT_ENFORCE = 11;
  REFUSAL_CODE_LIMIT_EXCEEDED = 12;
  REFUSAL_CODE_IDEMPOTENCY_CONFLICT = 13;
  REFUSAL_CODE_CONFLICT = 14;
}

message Refusal {
  RefusalCode code = 1;
  string domain_code = 2;     // e.g. "quote_expired" (MKT), "missing_right" (HOST)
  string message = 3 [(openagents.options.v1.limits) = {max_bytes: 1024}];
  repeated string missing = 4; // HOST missing rights, required features, ...
  optional uint64 retry_after_ms = 5;
}
```

Domain refusal codes registered by NIPs:

| Domain | Extra codes |
| --- | --- |
| HOST | `unsupported`, `forbidden`, `missing_right`, `expired`, `revoked`, `stale`, `conflict`, `bounds`, `unavailable`, `rate_limited`, `wrong_code`, `denied` |
| MKT | `unsupported_profile`, `quote_expired`, `quote_consumed`, `confirmation_expired`, `price_exceeded` |
| TERM | `lost`, `closed`, `not_typist` |
| HTTP | `api.v1.ErrorType` values (§10.2) |

### 9.7 Time, cursors and pages

```proto
message Window { int64 from = 1; int64 to = 2; }      // seconds unless the field says _ms
message Cursor { string opaque = 1 [(openagents.options.v1.limits) = {max_bytes: 1024}]; }
message PageRequest { uint32 limit = 1; optional Cursor after = 2; }
```

### 9.8 Amounts

```proto
// Bitcoin-only money (principle 10).
message Amount {
  oneof unit {
    uint64 msat = 1 [(openagents.options.v1.unit) = "msat"];
    AssetAmount asset = 2;         // Taproot Assets stablecoin, by asset id
  }
}
message AssetAmount {
  bytes asset_id = 1 [(openagents.options.v1.limits) = {min_bytes: 32, max_bytes: 32}];
  uint64 units = 2;                // smallest asset unit
  optional bytes group_key = 3;    // Taproot Assets group key, 33 bytes
}
message MicroAmount { uint64 microunits = 1; string currency = 2; } // "BTC" or "tap:<asset id hex>"

enum BitcoinNetwork {
  BITCOIN_NETWORK_UNSPECIFIED = 0;
  BITCOIN_NETWORK_MAINNET = 1 [(openagents.options.v1.wire) = "bitcoin"];
  BITCOIN_NETWORK_TESTNET = 2 [(openagents.options.v1.wire) = "testnet"];
  BITCOIN_NETWORK_SIGNET = 3  [(openagents.options.v1.wire) = "signet"];
  BITCOIN_NETWORK_REGTEST = 4 [(openagents.options.v1.wire) = "regtest"];
}
```

`BitcoinNetwork` is one enum for MKT, PYLON and EXT. Today MKT allows
`bitcoin|testnet|regtest` and PYLON allows all four. Each record's
validator states which subset it accepts. X402 keeps its CAIP-style
`lnbtc:<genesis prefix>` strings because x402 v2 requires them. The
mapping between the two is fixed in `x402.v1` (§26.4).

BIP177 display (₿ and sats) is presentation and not part of the type.

### 9.9 Usage

```proto
// NIP-POL route usage, also used by inference and decisions.
message Usage {
  optional uint64 input_tokens = 1;
  optional uint64 cached_input_tokens = 2;
  optional uint64 output_tokens = 3;
  optional uint64 latency_ms = 4;
  optional MicroAmount cost = 5;
  UsageBasis basis = 6;
}
enum UsageBasis { USAGE_BASIS_UNSPECIFIED = 0; USAGE_BASIS_OBSERVED = 1; USAGE_BASIS_ESTIMATED = 2; USAGE_BASIS_UNKNOWN = 3; }
```

### 9.10 Envelopes and opaque data

```proto
// Decrypted content of kind 3188 (openagents.artifact-envelope.v1).
message ArtifactEnvelope {
  option (openagents.options.v1.contract) = "openagents.artifact-envelope.v1";
  repeated string requires = 1;
  ArtifactRef artifact = 2;
  oneof inline_state { Null inline_null = 3; PrivateRecord inline = 4; }
  int64 issued_at = 5;
  int64 retain_until = 6;
}

// Registered private records (§8.4). Each owner fills its block.
message PrivateRecord {
  oneof record {
    // 100–199 agent, 200–299 trace, ... filled in by each package via
    // a generated aggregation file, proto/openagents/type/v1/private_records.proto
  }
}

// Migration debt: an unmodeled format with a declared contract.
message Opaque {
  string contract = 1;       // e.g. "openagents.verse-world-tree.v1"
  string media_type = 2;
  bytes bytes = 3;
}

message Null {}
```

`PrivateRecord` lives in its own generated file that imports every owning
package. It is the one place the import graph fans in, so it sits outside
the `type.v1` core and nothing else imports it (§14.2).

### 9.11 `meta` and feature lists

```proto
// contracts.md: optional inert `meta`. Readers must not interpret it.
message Meta { repeated MetaEntry entries = 1 [(openagents.options.v1.limits) = {max_count: 64}]; }
message MetaEntry { string key = 1; bytes json = 2; }   // exact JSON bytes of the value

// contracts.md `requires`: feature IDs the reader must support.
message Requires { repeated string features = 1; }
```

## 10. `openagents.api.v1`: the HTTP envelope

Status: **direct v1**. It mirrors what `crates/openagents-web` and the
OpenAPI documents already do.

### 10.1 Errors

```proto
package openagents.api.v1;

message ErrorBody { ApiError error = 1; }
message ApiError {
  ErrorType type = 1;
  string code = 2;          // stable machine code, e.g. "insufficient_balance"
  string message = 3;
  optional string param = 4;
  string request_id = 5;
  optional openagents.type.v1.Refusal refusal = 6; // when the cause is a protocol refusal
}
enum ErrorType {
  ERROR_TYPE_UNSPECIFIED = 0;
  ERROR_TYPE_INVALID_REQUEST = 1;   // 400
  ERROR_TYPE_AUTHENTICATION = 2;    // 401
  ERROR_TYPE_PERMISSION = 3;        // 403
  ERROR_TYPE_NOT_FOUND = 4;         // 404
  ERROR_TYPE_CONFLICT = 5;          // 409
  ERROR_TYPE_PAYMENT_REQUIRED = 6;  // 402
  ERROR_TYPE_RATE_LIMIT = 7;        // 429
  ERROR_TYPE_API = 8;               // 5xx
  ERROR_TYPE_OVERLOADED = 9;        // 529/503
}
```

The exact `type` strings in today's responses are kept by the `wire`
option. The table above is the target set, and §32.3 lists any current
spelling that differs.

### 10.2 Pages, idempotency and headers

```proto
message ListRequest { uint32 limit = 1; optional string after = 2; }
// Every list response is {data: [...], next: string|null}. Each list RPC
// defines its own typed Page message with `repeated T data` and `next`.

message RequestHeaders {
  optional string idempotency_key = 1;  // Idempotency-Key
  optional string workspace_id = 2;     // X-Workspace-Id
  optional uint32 attempt = 3;          // X-Attempt (decisions)
}
```

Idempotency semantics everywhere: the same key and the same body give the
stored response. The same key and a different body give a 409 `conflict`
with code `idempotency_conflict`. Keys are retained for the window each
endpoint documents.

### 10.3 Prefixed IDs

`type.v1.ResourceId` and the `prefixed_id` grammar cover every HTTP
resource. The registry of prefixes is a table in
`proto/openagents/api/v1/prefixes.md` that the generator checks against
each message's `(http).id_prefix`:

| Prefix | Object | Package |
| --- | --- | --- |
| `acct_` | account | `account.v1` |
| `ws_` | workspace | `account.v1` |
| `inv_` | invitation | `account.v1` |
| `sess_` | sign-in session | `credential.v1` |
| `oak_` | API key secret (shown once) | `credential.v1` |
| `key_` | API key record | `credential.v1` |
| `cmk_` | customer-managed key | `credential.v1` |
| `oa_sk_` | service key | `credential.v1` |
| `vlt_` | vault | `vault.v1` / `agent.managed.v1` |
| `dec_` | decision | `decision.v1` |
| `job_` | durable job | `decision.v1` |
| `resp_` | response | `inference.v1` |
| `batch_` | batch | `inference.v1` |
| `th_` | thread | `chat.v1` |
| `msg_` | message | `chat.v1` |
| `turn_` | turn | `chat.v1` |
| `run_` | run | `run.v1` / `coder.task.v1` |
| `tr_` | trace | `trace.v1` |
| `agent_` | agent | `agent.v1` |
| `sesn_` | managed session | `agent.managed.v1` |
| `sevt_` | session event | `agent.managed.v1` |
| `depl_` | deployment | `agent.managed.v1` |
| `drun_` | deployment run | `agent.managed.v1` |
| `memstore_`, `mem_`, `memver_` | memory store, memory, memory version | `agent.managed.v1` |
| `env_`, `envk_` | environment, environment key | `environment.v1` |
| `file_`, `upl_` | file, upload | `api.v1` files |
| `skill_` | skill | `skill.v1` |
| `wrun_` | workflow run | `program.v1` |
| `work_` | work item | `work.v1` |
| `whk_`, `whe_` | webhook endpoint, webhook event | `api.v1` webhooks |
| `outc_` | outcome | `receipt.v1` |
| `prj_` | project | `account.v1` |
| `pl_` | pylon | `pylon.v1` |
| `pr_` | pull request | `coder.task.v1` |
| `lab_` | lab | `coder.lab.v1` |
| `ogb_` | gym board | `gym.v1` |

### 10.4 Webhooks

```proto
message WebhookEndpoint { string id = 1; string url = 2; repeated string events = 3; bool enabled = 4; int64 created_at = 5; }
message WebhookEvent {
  string id = 1;            // whe_
  string type = 2;          // "session.status_idle", "deployment.run.completed", ...
  int64 created_at = 3;
  WebhookPayload data = 4;  // oneof over the registered event payloads
}
// Signature: header "OpenAgents-Signature: t=<s>,v1=<hex hmac-sha256>" over "<t>.<raw body>".
```

The event type list is owned by each package and aggregated the same way
as `PrivateRecord`.

### 10.5 Files and uploads

```proto
message File { string id = 1; string filename = 2; uint64 bytes = 3; string media_type = 4;
               openagents.type.v1.Digest digest = 5; string purpose = 6; int64 created_at = 7; }
message Upload { string id = 1; uint64 bytes = 2; uint64 received = 3; string status = 4; }
```

## 11. `nostr.*`: the whole Nostr protocol as typed mirrors

The owner wants the full Nostr protocol represented as proto definitions,
the way `nips/` vendors the specs. `proto/nostr/` does that. It is a
mirror. The NIP text stays authoritative, and the mirror is pinned to the
same upstream commit.

### 11.1 Rules

1. **The event is NIP-01 JSON.** `nostr.event.v1.Event` holds `content` as
   the exact string. IDs and signatures are checked against the NIP-01
   serialization, never against re-encoded proto.
2. **Typed content is parsed content.** Each NIP package defines the parsed
   form of `content` and the typed form of its tags. A mapper converts
   both ways and is tested against the NIP's examples.
3. **Tags stay ordered string arrays** in the envelope. Typed tag messages
   (`nostr.nip10.v1.ThreadTags`) are a view parsed from them.
4. **One package per NIP** with structured data, named `nostr.nip<NN>.v1`,
   where `<NN>` is the NIP identifier in lowercase (`nip01`, `nip5a`,
   `nipb7`). Behaviour-only NIPs get no package; the README lists them.
5. **Deprecated NIPs** (marked `unrecommended` upstream) get packages
   marked `deprecated = true` at file level. They exist for reading old
   data and never become the base for new OpenAgents design.
6. **Pins.** `proto/nostr/MANIFEST.json` records, for each package, the
   NIP file, the `nips/manifest.json` commit and the SHA-256 of the NIP
   text it was written from. `scripts/nips-sync` flags any package whose
   source text changed.

### 11.2 `nostr.event.v1`

```proto
syntax = "proto3";
package nostr.event.v1;

message Tag { repeated string values = 1; }

message Event {
  bytes id = 1;          // 32 bytes; sha256 of [0,pubkey,created_at,kind,tags,content]
  bytes pubkey = 2;      // 32 bytes x-only
  int64 created_at = 3;  // unix seconds
  uint32 kind = 4;
  repeated Tag tags = 5;
  string content = 6;    // exact string
  bytes sig = 7;         // 64-byte BIP-340 Schnorr
}

// Unsigned event (NIP-59 rumors, NIP-37 drafts, templates before signing).
message UnsignedEvent { bytes pubkey = 1; int64 created_at = 2; uint32 kind = 3; repeated Tag tags = 4; string content = 5; }

message TagFilter { string letter = 1; repeated string values = 2; }
message Filter {
  repeated bytes ids = 1;
  repeated bytes authors = 2;
  repeated uint32 kinds = 3;
  repeated TagFilter tags = 4;     // "#e", "#p", "#d", ...
  optional int64 since = 5;
  optional int64 until = 6;
  optional uint32 limit = 7;
  optional string search = 8;      // NIP-50
}

enum KindClass {                   // NIP-01 ranges
  KIND_CLASS_UNSPECIFIED = 0;
  KIND_CLASS_REGULAR = 1;          // 1000–9999, and 1, 2, 4–44
  KIND_CLASS_REPLACEABLE = 2;      // 0, 3, 10000–19999
  KIND_CLASS_EPHEMERAL = 3;        // 20000–29999
  KIND_CLASS_ADDRESSABLE = 4;      // 30000–39999
}

// Relay protocol messages, client → relay and relay → client.
message ClientMessage {
  oneof msg {
    Event event = 1;                              // ["EVENT", event]
    Req req = 2;                                  // ["REQ", sub, filters...]
    string close = 3;                             // ["CLOSE", sub]
    Event auth = 4;                               // ["AUTH", event] (NIP-42)
    Count count = 5;                              // ["COUNT", sub, filters...] (NIP-45)
    NegOpen neg_open = 6;                         // NIP-77
    NegMsg neg_msg = 7;
    string neg_close = 8;
  }
}
message RelayMessage {
  oneof msg {
    SubEvent event = 1;        // ["EVENT", sub, event]
    Ok ok = 2;                 // ["OK", id, accepted, message]
    string eose = 3;           // ["EOSE", sub]
    Closed closed = 4;         // ["CLOSED", sub, message]
    string notice = 5;         // ["NOTICE", message]
    string auth = 6;           // ["AUTH", challenge]
    CountResult count = 7;
    NegMsg neg_msg = 8;
    NegErr neg_err = 9;
  }
}
message Req { string subscription = 1; repeated Filter filters = 2; }
message SubEvent { string subscription = 1; Event event = 2; }
message Ok { bytes event_id = 1; bool accepted = 2; string message = 3; }
message Closed { string subscription = 1; string message = 2; }
message Count { string subscription = 1; repeated Filter filters = 2; }
message CountResult { string subscription = 1; uint64 count = 2; optional bool approximate = 3; optional string hll = 4; }
message NegOpen { string subscription = 1; Filter filter = 2; bytes initial = 3; }
message NegMsg { string subscription = 1; bytes message = 2; }
message NegErr { string subscription = 1; string reason = 2; }

// Machine-readable prefixes in OK/CLOSED messages.
enum ReasonPrefix {
  REASON_PREFIX_UNSPECIFIED = 0; REASON_PREFIX_DUPLICATE = 1; REASON_PREFIX_POW = 2;
  REASON_PREFIX_BLOCKED = 3; REASON_PREFIX_RATE_LIMITED = 4; REASON_PREFIX_INVALID = 5;
  REASON_PREFIX_RESTRICTED = 6; REASON_PREFIX_MUTE = 7; REASON_PREFIX_ERROR = 8;
  REASON_PREFIX_AUTH_REQUIRED = 9;
}
```

The relay wire stays JSON arrays over WebSocket. These messages are the
typed view a client library exposes and the shape fixtures use.

### 11.3 Official NIP packages

All 99 official NIPs are listed. "Package" means a `nostr.nip<NN>.v1`
package exists in the mirror. "Behaviour" means the NIP defines rules, not
data, and gets a README entry instead. "OA use" marks the NIPs our specs
build on; those packages are built first (phase P2, §36).

| NIP | Title | Mirror | Main messages | OA use |
| --- | --- | --- | --- | --- |
| 01 | Basic protocol | `nip01` (+ `event.v1`) | `Metadata` (kind 0 JSON) | ● |
| 02 | Follow list | `nip02` | `FollowList`, `Follow{pubkey, relay, petname}` | ● |
| 03 | OpenTimestamps | `nip03` | `OtsAttestation` (kind 1040) | |
| 04 | Encrypted DM ⚠ | `nip04` deprecated | `EncryptedDirectMessage` | |
| 05 | DNS identifiers | `nip05` | `NostrJson{names, relays}` | ● |
| 06 | Key derivation ⚠ | Behaviour | — | |
| 07 | `window.nostr` | Behaviour (API surface listed) | — | |
| 08 | Mentions ⚠ | Behaviour | — | |
| 09 | Deletion request | `nip09` | `DeletionRequest` (kind 5, `e`/`a`/`k`) | ● |
| 10 | Text notes, threads | `nip10` | `ThreadTags{root, reply, mentions}` | ● |
| 11 | Relay information | `nip11` | `RelayInformation{name, limitation, fees, supported_nips, …}` | ● |
| 12, 16, 20, 33 | Folded into NIP-01 | Behaviour | — | |
| 13 | Proof of work | `nip13` | `Nonce{nonce, target}` | |
| 14 | Subject tag | `nip14` | `Subject` | |
| 15 | Marketplace ⚠ | `nip15` deprecated | `Stall`, `Product`, `Order` | compat |
| 17 | Private DMs | `nip17` | `ChatMessage` (14), `FileMessage` (15), `DmRelayList` (10050) | ● |
| 18 | Reposts | `nip18` | `Repost` (6), `GenericRepost` (16) | |
| 19 | bech32 entities | `nip19` | `Npub`, `Nsec`, `Note`, `Nprofile`, `Nevent`, `Naddr` as TLV | ● |
| 21 | `nostr:` URI | Behaviour | — | ● |
| 22 | Comment | `nip22` | `Comment` (1111) with root/parent scope tags | |
| 23 | Long-form | `nip23` | `Article` (30023/30024) | |
| 24 | Extra metadata | `nip24` | metadata extensions (`bot`, `display_name`, `website`, `banner`, `birthday`) | ● |
| 25 | Reactions | `nip25` | `Reaction` (7, 17) | |
| 26 | Delegation ⚠ | `nip26` deprecated | `Delegation` tag | |
| 27 | Text references | Behaviour | — | |
| 28 | Public chat ⚠ | `nip28` deprecated | `ChannelCreate` (40) … `MuteUser` (44) | |
| 29 | Relay-based groups | `nip29` | `GroupMetadata` (39000), `GroupAdmins` (39001), `GroupMembers` (39002), `GroupRoles` (39003), moderation 9000–9020, `JoinRequest` (9021), `LeaveRequest` (9022) | ● |
| 30 | Custom emoji | `nip30` | `Emoji{shortcode, url}` | |
| 31 | `alt` tag ⚠ | `nip31` | `Alt` | ● |
| 32 | Labeling | `nip32` | `Label` (1985) with `L`/`l` | ● |
| 34 | git | `nip34` | `RepositoryAnnouncement` (30617), `RepositoryState` (30618), `Patch` (1617), `PullRequest` (1618), `Issue` (1621), status 1630–1633 | ● |
| 35 | Torrents | `nip35` | `Torrent` (2003), `TorrentComment` (2004) | |
| 36 | Content warning | `nip36` | `ContentWarning` | |
| 37 | Draft wraps | `nip37` | `DraftWrap` (31234), `PrivateRelayList` (10013) | |
| 38 | User statuses | `nip38` | `UserStatus` (30315) | |
| 39 | External identities | `nip39` | `ExternalIdentity{platform, identity, proof}` | ● |
| 40 | Expiration | `nip40` | `Expiration` | ● |
| 42 | Client auth | `nip42` | `AuthEvent` (22242) | ● |
| 43 | Relay access | `nip43` | `RelayMembers` (13534), join and leave requests (28934–28936) | |
| 44 | Encrypted payloads | `nip44` | `PayloadV2{version, nonce, ciphertext, mac}` | ● |
| 45 | COUNT | `nip45` (+ `event.v1`) | `Count`, `CountResult` | |
| 46 | Remote signing | `nip46` | `SignerRequest{id, method, params}`, `SignerResponse{id, result, error}`, `BunkerUri`, `NostrConnectUri` (24133) | ● |
| 47 | Wallet Connect | `nip47` | `WalletInfo` (13194), `WalletRequest` (23194), `WalletResponse` (23195), `WalletNotification` (23196/23197), methods `pay_invoice`, `multi_pay_invoice`, `pay_keysend`, `make_invoice`, `lookup_invoice`, `list_transactions`, `get_balance`, `get_info` | ● |
| 48 | Proxy tags | `nip48` | `Proxy{id, protocol}` | |
| 49 | `ncryptsec` | `nip49` | `EncryptedPrivateKey{version, log_n, salt, nonce, security, ciphertext}` | |
| 50 | Search | `nip50` (+ `Filter.search`) | search extensions | |
| 51 | Lists | `nip51` | standard lists (10000–10102) and sets (30000–30267), with public and encrypted items | ● |
| 52 | Calendar | `nip52` | `DateEvent` (31922), `TimeEvent` (31923), `Calendar` (31924), `Rsvp` (31925) | |
| 53 | Live activities | `nip53` | `LiveEvent` (30311), `LiveChat` (1311), `MeetingSpace` (30312), `MeetingRoom` (30313), `RoomPresence` (10312) | ● |
| 54 | Wiki | `nip54` | `WikiArticle` (30818), merge requests (818) | ● |
| 55 | Android signer | Behaviour | — | |
| 56 | Reporting | `nip56` | `Report` (1984) with report types | |
| 57 | Zaps | `nip57` | `ZapRequest` (9734), `ZapReceipt` (9735), LNURL fields | ● |
| 58 | Badges | `nip58` | `BadgeDefinition` (30009), `BadgeAward` (8), `ProfileBadges` (30008) | |
| 59 | Gift wrap | `nip59` | `Rumor`, `Seal` (13), `GiftWrap` (1059) | ● |
| 5A | Static websites | `nip5a` | `SiteManifest` (nsite path mappings) | |
| 60 | Cashu wallet | `nip60` | `Wallet` (17375), `Token` (7375), `History` (7376) | compat only |
| 61 | Nutzaps | `nip61` | `NutzapInfo` (10019), `Nutzap` (9321) | compat only |
| 62 | Request to vanish | `nip62` | `VanishRequest` (62) | |
| 64 | Chess PGN | `nip64` | `ChessGame` (64) | |
| 65 | Relay list | `nip65` | `RelayList` (10002) with read/write markers | ● |
| 66 | Relay monitoring | `nip66` | `RelayDiscovery` (30166), `MonitorAnnouncement` (10166) | |
| 67 | EOSE completeness | `nip67` | completeness hint | ● |
| 68 | Picture feeds | `nip68` | `Picture` (20) | |
| 69 | P2P orders | `nip69` | `P2pOrder` (38383) | |
| 70 | Protected events | Behaviour (`-` tag) | — | |
| 71 | Video | `nip71` | `Video` (21, 22, 34235, 34236) | |
| 72 | Communities ⚠ | `nip72` deprecated | `Community` (34550), `Approval` (4550) | |
| 73 | External content IDs | `nip73` | `ExternalId{kind, value}` (`i`/`k` tags) | |
| 75 | Zap goals | `nip75` | `ZapGoal` (9041) | |
| 77 | Negentropy | `nip77` (+ `event.v1`) | NEG-* messages | ● |
| 78 | App data | `nip78` | `AppData` (30078) with opaque content | ● (Block RS) |
| 7D | Forum threads | `nip7d` | `Thread` (11) and replies via NIP-22 | ● (Forum) |
| 84 | Highlights | `nip84` | `Highlight` (9802) | |
| 85 | Trusted assertions | `nip85` | assertion kinds 30382–30385, provider list 10040 | ● |
| 86 | Relay management | `nip86` | `ManagementRequest{method, params}`, `ManagementResponse` | ● |
| 87 | Mint discovery | `nip87` | `MintRecommendation` (38000), Cashu and Fedimint announcements | compat only |
| 88 | Polls | `nip88` | `Poll` (1068), `PollResponse` (1018) | |
| 89 | App handlers | `nip89` | `HandlerRecommendation` (31989), `HandlerInformation` (31990) | ● |
| 90 | DVMs ⚠ | `nip90` deprecated | `JobRequest` (5000–5999), `JobResult` (6000–6999), `JobFeedback` (7000) | compat (Pylon `nip90-5050` lane) |
| 92 | `imeta` | `nip92` | `MediaAttachment` | ● |
| 94 | File metadata | `nip94` | `FileMetadata` (1063) | ● |
| 96 | HTTP file storage ⚠ | `nip96` deprecated | upload and server info JSON | |
| 98 | HTTP auth | `nip98` | `HttpAuth` (27235) | ● |
| 99 | Classifieds | `nip99` | `ClassifiedListing` (30402, 30403) | ● (MKT compat) |
| A0 | Voice messages | `nipa0` | `VoiceMessage` (1222, 1244) | |
| A3 | `payto:` targets | `nipa3` | `PaymentTarget` | |
| A4 | Public messages | `nipa4` | `PublicMessage` (24) | |
| B0 | Web bookmarks | `nipb0` | `WebBookmark` (39701) | |
| B7 | Blossom | `nipb7` | `BlobDescriptor`, `BlossomServerList` (10063), auth (24242) | ● |
| BE | BLE transport ⚠ | `nipbe` deprecated | BLE frame layout | |
| C0 | Code snippets | `nipc0` | `CodeSnippet` (1337) | |
| C7 | Chats | `nipc7` | `Chat` (9) | ● (Verse chat) |
| CC | Geocaching | `nipcc` | geocache kinds | |
| EE | MLS ⚠ | `nipee` deprecated | MLS key packages and messages; superseded by Marmot | |
| F4 | Podcasts | `nipf4` | podcast kinds | |

Packages marked "compat only" mirror rails the owner has excluded for new
OpenAgents payment design (Cashu, Fedimint). They exist so readers can
recognise and refuse or display those records, not so OpenAgents packages
can import them.

### 11.4 Block NIP packages

| NIP | Title | Kinds | Mirror | OA use |
| --- | --- | --- | --- | --- |
| AA | Agent authentication | 22242 (+13534) | `nostr.block.aa.v1` | CTRL, HOST |
| AE | Agent engrams | 30174 | `nostr.block.ae.v1` | ATIF, RUN, SESS |
| AM | Agent turn metrics | 44200 | `nostr.block.am.v1` | ATIF, RUN |
| AO | Agent observability | 24200 | `nostr.block.ao.v1` | ATIF, RUN, CTRL |
| AP | Agent personas | 30175, 30178 | `nostr.block.ap.v1` | Verse |
| CW | Channel and thread windows | 39005–39007 | `nostr.block.cw.v1` | |
| DV | DM visibility | 30622 | `nostr.block.dv.v1` | |
| ER | Event reminders | 30300 | `nostr.block.er.v1` | COORD boundary |
| FI | Federated identity | none | Behaviour | |
| GS | Git object signing | none (git envelope) | `nostr.block.gs.v1` (`GitSignature` envelope) | Coder |
| IA | Identity archival | 9035/9036, 8002/8003, 13535 | `nostr.block.ia.v1` | Verse |
| MP | Multi-repo projects | 30621 | `nostr.block.mp.v1` | WS |
| OA | Owner attestation | `auth` tag | `nostr.block.oa.v1` (`OwnerAttestation{owner, conditions, sig}`) | AGENT, PYLON, HOST |
| PL | Push leases | 30350 | `nostr.block.pl.v1` | Coder host push leases |
| PMA | Private managed-agent aggregate | 30179 reserved | `nostr.block.pma.v1` (reserved; readers reject kind 30179) | |
| RS | Cross-device read state | 30078 | `nostr.block.rs.v1` | CTRL, WS |
| WP | Workspace profile | 9033 | `nostr.block.wp.v1` | WS |

### 11.5 Nostr sync

`scripts/nips-sync` already refreshes `nips/official/` and `nips/block/`.
It gains one step: for every changed NIP that has a mirror package, it
opens a tracking issue and marks the package `stale` in
`proto/nostr/MANIFEST.json`. CI refuses to publish a `nostr.*` package
whose manifest entry is stale.

## 12. OpenAgents NIPs: every kind and every body, mapped

### 12.1 Kind registry → package

Every OpenAgents event kind has exactly one owning package. The registry
in `nips/openagents/README.md` and `crates/nostr/src/kinds.rs` stays the
source of truth for kind numbers. The generator checks that each kind
appears in exactly one `(nip).kinds` option.

| Kind | Class | NIP | Record | Package and message |
| --- | --- | --- | --- | --- |
| 3184 | regular | EXT | release | `extension.v1.Release` |
| 3185 | regular | EXT | revocation | `extension.v1.Revocation` |
| 3186 | regular | EXT | migration | `extension.v1.Migration` |
| 30184 | addressable | EXT | listing | `extension.v1.Listing` |
| 30185 | addressable | EXT | revocation checkpoint | `extension.v1.Checkpoint` |
| 3187 | regular | RUN | encrypted run record | `run.v1.RunRecordEnvelope` |
| 30186 | addressable | RUN | encrypted run head | `run.v1.RunHead` |
| 3188 | regular | contracts | private artifact envelope | `type.v1.ArtifactEnvelope` |
| 3189 | regular | EVAL | evaluation declaration | `eval.v1.Publication` |
| 3195 | regular | EVAL | Gym results publication | `gym.v1.ResultsPublication` |
| 3190 | regular | KB | entry version | `knowledge.v1.Entry` |
| 30190 | addressable | KB | head | `knowledge.v1.Head` |
| 3191 | regular | KB | withdrawal | `knowledge.v1.Withdrawal` |
| 3192 | regular | MKT | offering | `market.v1.Offering` |
| 30192 | addressable | MKT | offering head | `market.v1.OfferingHead` |
| 30193 | addressable | XP | quest | `xp.v1.Quest` |
| 3193 | regular | XP | award | `xp.v1.Award` |
| 3194 | regular | XP | revocation | `xp.v1.Revocation` |
| 3196 | regular | XP | playtest session | `xp.v1.PlaytestSession` |
| 3197 | regular | XP | playtest report | `xp.v1.PlaytestReport` |
| 13193 | replaceable | XP | trainer profile | `xp.v1.TrainerProfile` |
| 13195 | replaceable | XP | key link | `xp.v1.KeyLink` |
| 30194 | addressable | XP | trainer card | `xp.v1.TrainerCard` |
| 3198 | regular | ATIF | trajectory manifest | `trace.v1.Manifest` |
| 3199 | regular | ATIF | trajectory chunk | `trace.v1.Chunk` |
| 3201 | regular | PYLON | service receipt (and nested payment ack) | `pylon.v1.Receipt`, `pylon.v1.PaymentAck` |
| 30200 | addressable | PYLON | beacon | `pylon.v1.Beacon` |
| 30201 | addressable | PYLON | pool aggregate | `pylon.v1.Pool` |
| 3202 | regular | ATT | release | `attest.v1.Release` |
| 3203 | regular | ATT | rebuild | `attest.v1.Rebuild` |
| 30202 | addressable | ATT | release head | `attest.v1.Head` |
| 30203 | addressable | ATT | endpoint | `attest.v1.Endpoint` |
| 3204 | regular | AGENT | gateway receipt | `agent.v1.GatewayReceipt` |
| 30187 | addressable | AGENT | public agent definition | `agent.v1.PublicDefinition` |
| 3205 | regular | DEPLOY | phase event | `deploy.v1.DeployStatus` |
| 30205 | addressable | DEPLOY | lane head | `deploy.v1.LaneHead` |
| 25900 / 26900 / 27000 | ephemeral | CJ | conversation request / result / feedback | `chat.v1.ConversationRequest` / `ConversationResult` / `ConversationFeedback` |
| 25910 / 26910 / 27010 | ephemeral | DEC | decision request or cancel / result / status | `decision.v1.SystemOneRequest` / `SystemOneResult` / `SystemOneStatus` |
| 25920 / 26920 / 27020 | ephemeral | CJ | execute and controls / result / accepted, progress, sealed chunks | `exec.v1.ExecRequest` / `ExecResult` / `ExecFeedback` |
| 30180 | addressable | CAP | capability discovery head | `capability.v1.DiscoveryHead` |
| 30181 | addressable | CAP | operator preference head | `capability.v1.PreferenceHead` |
| 30182 | addressable | PRG | program head | `program.v1.ProgramHead` |
| 30183 | addressable | PRG | module announcement | `program.v1.ModuleAnnouncement` |
| 30188 | addressable | SKILL | skill listing | `skill.v1.Listing` |
| 23300 / 23301 / 23302 | ephemeral | MV | pose frame / gesture / zone command | `verse.presence.v1` (Chapter V) |
| 33300 / 33301 | addressable | MV | world definition / entity state | `verse.world.v1`, `verse.presence.v1` (Chapter V) |

### 12.2 Kinds that are used but not registered

An inventory of the NIPs found three gaps. This spec closes each one by stating
where the record lives. Registering the kinds is a NIP edit, tracked in
§36 phase P1:

- **30440, livehost.** NIP-DEPLOY mentions it, and 18 `openagents.livehost-*.v1`
  contracts exist. The package is `live.v1`, under a `livehost` file. The
  kind needs a registry row.
- **`oa:kiln-runner-beacon:v1`** is a marker on kind 30200 beacons, not a
  new kind. `pylon.v1.Beacon` gets a `runner: KilnRunner` oneof arm.
- **Nested payment-ack events** reuse kind 3201 with tag
  `oa:pylon-payment-ack:v1`. `pylon.v1.PaymentAck` is a separate message
  with the same kind and a different topic tag.

### 12.3 Bodies with no kind

Fourteen NIPs define no kinds and carry everything in 3188 envelopes, CJ
jobs, CAP operations or REACH frames: AUTO, COORD, CTRL, CTX, ENV, HOST,
LAB, LIVE, OPT, POL, REACH, SESS, SOV, TERM, VAULT, WORK, WS, X402. Each
has a package. Its records register in `type.v1.PrivateRecord` (§8.4) and,
where they are CAP or CJ operations, in the operation registry
(§19.3).

### 12.4 Retired kinds

These stay out of every package. Readers that meet them refuse, except
where a compat reader is named:

| Kinds | Was | Replaced by |
| --- | --- | --- |
| 39200–39260 | historical sovereign-agent (SA) kinds | `sovereign.v1` on 3188 |
| 39230 / 39231 | historical SA trajectory kinds | `trace.v1` on 3198 / 3199 |
| 380xx | historical agent kinds | `sovereign.v1` |
| 5934 / 6934 / 7000 (NIP-LBR over NIP-90) | historical labor | `labor.v1`; no compatibility claimed |
| 30179 | Block PMA reserved | must be rejected; relay rejection is an open fix |

## 13. `third_party.*`: Lightning Labs and other vendored protocols

### 13.1 Lightning Labs

`crates/lnd-sdk/proto/` already vendors Lightning Labs protos byte-exact.
The protocol tree adopts the same files at
`proto/third_party/lightninglabs/`, and `crates/lnd-sdk` builds from there.
That gives one copy, not two.

| Project | Tag | Packages | Why we carry it |
| --- | --- | --- | --- |
| LND | v0.21.1-beta | `lnrpc`, `routerrpc`, `invoicesrpc`, `walletrpc`, `signrpc`, `chainrpc`, `peersrpc`, `wtclientrpc`, `verrpc`, `autopilotrpc` | Invoices, payments, channels, signing |
| Taproot Assets | v0.8.0 | `taprpc`, `assetwalletrpc`, `mintrpc`, `universerpc`, `priceoraclerpc`, `rfqrpc`, `tapchannelrpc`, `tapdevrpc` | Bitcoin-based stablecoins over Taproot Assets |
| Loop | v0.34.0-beta | `looprpc`, `swapserverrpc` | Liquidity swaps |
| Lightning Terminal | v0.17.0-alpha | `litrpc` | Accounts, sessions, autopilot |

Rules:

1. **Byte-exact.** Files are copied unchanged. `sources.json` records the
   upstream repo, commit, path, SHA-256 and license for each file. CI
   re-hashes them.
2. **Package names unchanged.** `lnrpc` stays `lnrpc`. Generated code may
   use a language-level prefix (Rust module `third_party::lnrpc`), but the
   proto package never changes. That keeps wire compatibility with real
   LND.
3. **Not part of the OpenAgents API.** No `openagents.*` message embeds an
   `lnrpc` message. OpenAgents records reference upstream objects by
   identity: `payment_hash`, `asset_id`, `group_key`, channel point,
   outpoint, universe proof digest. `payment.v1` converts at the edge.
4. **Re-vendoring** is a reviewed change that updates the tag directory,
   `sources.json` and any adapters, with the upstream release notes
   linked.
5. **Reference clones** live at `projects/lightninglabs/repos/`. The
   sync manifest lists them, but they are not cloned on this machine
   today. `crates/lnd-sdk/proto/sources.json` remains the evidence.

### 13.2 LDK, Spark and Breez

LDK (rust-lightning, ldk-node) and Spark/Breez SDKs expose Rust or
UniFFI APIs, not protos. The protocol does not vendor them. `payment.v1`
defines the OpenAgents-side records (payment evidence, wallet views) that
any of these backends produce. Our owned forks (`OpenAgentsInc/ldk-node`,
`tap-ldk`) keep their own interfaces.

### 13.3 Other external protocols we follow

| Protocol | Upstream | How the tree holds it |
| --- | --- | --- |
| x402 v2 | coinbase/x402 at `4fcf836c…` | `x402.v1` mirrors `PaymentRequired`, `PaymentPayload`, `SettleResponse` field-for-field for the `exact` scheme on `lnbtc`; HTTP and MCP bindings stay upstream-exact |
| ATIF v1.8 | Harbor | `trace.v1` mirrors trajectory, step, tool call and observation; JSON stays authoritative because of the ATIF digest rule |
| OpenAI-shaped Responses and chat completions | OpenAI / Open Responses | `inference.v1` mirrors the subset `api.openagents.com` serves; we follow their field names |
| MCP | modelcontextprotocol | not mirrored; CAP `adapter` bindings reference MCP servers by URL and tool name |
| ACP | Agent Client Protocol | not mirrored; CAP `executor` transport `acp` |
| Codex app-server, Claude Code, Cursor protocols | vendors | not mirrored; adapters in `session.v1` name them by adapter ID |
| Loro | loro-dev | not mirrored; `coder.sync.v1` records frontiers and digests |
| HPKE (RFC 9180), JCS (RFC 8785), BIP-340, BOLT11, BOLT12, LNURL | standards | referenced; fields carry their encoded forms as bytes or strings |

Mirrors of other vendors' formats go under `proto/third_party/<vendor>/`
only if the vendor publishes `.proto` files. Otherwise the OpenAgents
package mirrors the fields it uses and names the upstream version in a
file-level comment.

---

# Part III. Domains

Each domain section gives the package's purpose, the sources it is built
from, its status, its key messages as a sketch, and its services. Sketches
show shape. They omit `option` lines except where an option matters, and
they use `T` as shorthand for `openagents.type.v1`.

Status words follow the glossary: **Implemented** means code serves it in
production or behind a shipped flag. **Partial** means some records are
implemented. **Designed** means a NIP or design document specifies it and
no code serves it.

## 14. Domain catalog and import graph

### 14.1 Catalog

| # | Package | NIPs and sources | Status | Start label |
| --- | --- | --- | --- | --- |
| 1 | `type.v1` | `contracts.md` | Implemented as JSON | direct v1 |
| 2 | `options.v1` | this spec | Designed | direct v1 |
| 3 | `api.v1` | `openagents-web`, OpenAPI docs | Implemented as JSON | direct v1 |
| 4 | `account.v1` | tenancy, glossary "Tenancy" | Implemented | v1alpha1 |
| 5 | `credential.v1` | oak_ keys, CMK, device sign-in, BYOK | Implemented | v1alpha1 |
| 6 | `vault.v1` | NIP-VAULT | Partial | v1alpha1 |
| 7 | `identity.v1` | NIP-05, Block OA, XP key links | Partial | v1alpha1 |
| 8 | `decision.v1` | NIP-DEC, `/v1/decisions`, `/v1/systemone`, `/v1/classify`, `/v1/jobs` | Implemented | direct v1 |
| 9 | `inference.v1` | Responses, chat completions, models | Implemented | v1alpha1 |
| 10 | `receipt.v1` | `openagents.receipt.execution.v1`, `crates/receipts` | Implemented | direct v1 |
| 11 | `chat.v1` | NIP-CJ conversation, chat router, threads | Implemented | direct v1 |
| 12 | `exec.v1` | NIP-CJ execution | Implemented | direct v1 |
| 13 | `agent.v1` | NIP-AGENT | Implemented | v1alpha1 |
| 14 | `agent.managed.v1` | Managed Agents API | Designed | v1alpha1 |
| 15 | `session.v1` | NIP-SESS | Partial; revision pending | v1alpha1 |
| 16 | `control.v1` | NIP-CTRL | Partial; revision pending | v1alpha1 |
| 17 | `context.v1` | NIP-CTX | Designed | v1alpha1 |
| 18 | `policy.v1` | NIP-POL | Designed | v1alpha1 |
| 19 | `capability.v1` | NIP-CAP | Partial | v1alpha1 |
| 20 | `program.v1` | NIP-PRG, plugin packet ABI | Partial | v1alpha1 |
| 21 | `extension.v1` | NIP-EXT, NIP-REG | Partial | v1alpha1 |
| 22 | `skill.v1` | NIP-SKILL, skill directory | Implemented | v1alpha1 |
| 23 | `knowledge.v1` | NIP-KB | Implemented | direct v1 |
| 24 | `eval.v1` | NIP-EVAL | Partial | v1alpha1 |
| 25 | `gym.v1` | NIP-EVAL Gym profile, Gym product | Implemented | v1alpha1 |
| 26 | `optimize.v1` | NIP-OPT | Designed | v1alpha1 |
| 27 | `trace.v1` | NIP-ATIF, Harbor ATIF-v1.8 | Implemented | direct v1 |
| 28 | `run.v1` | NIP-RUN | Implemented | direct v1 |
| 29 | `coordination.v1` | NIP-COORD | Partial | v1alpha1 |
| 30 | `automation.v1` | NIP-AUTO | Partial | v1alpha1 |
| 31 | `work.v1` | NIP-WORK | Partial | v1alpha1 |
| 32 | `workspace.v1` | NIP-WS | Partial | v1alpha1 |
| 33 | `environment.v1` | NIP-ENV, `env_` API | Partial | v1alpha1 |
| 34 | `host.v1` | NIP-HOST | Implemented | direct v1 |
| 35 | `reach.v1` | NIP-REACH | Implemented | direct v1 |
| 36 | `terminal.v1` | NIP-TERM | Implemented | direct v1 |
| 37 | `live.v1` | NIP-LIVE, livehost | Partial | v1alpha1 |
| 38 | `cloud.v1` | retail cloud, GCE pool, budgets | Implemented | v1alpha1 |
| 39 | `computers.v1` | Computers API (`oa-computers`) | Implemented | v1alpha1 |
| 40 | `compute.v1` | compute balance, placement, route contracts | Partial | v1alpha1 |
| 41 | `coder.task.v1` | task store v1–v3, execution grants | Implemented | v1alpha1 |
| 42 | `coder.host.v1` | host descriptor, host runtime, SSH-managed | Implemented | v1alpha1 |
| 43 | `coder.sync.v1` | coder-sync rooms, chat2 frames | Implemented | v1alpha1 |
| 44 | `coder.router.v1` | chat router v1–v3, tiers | Implemented | v1alpha1 |
| 45 | `coder.foreman.v1` | Foreman | Implemented | v1alpha1 |
| 46 | `coder.lab.v1` | Agent Lab, Labs API | Implemented | v1alpha1 |
| 47 | `market.v1` | NIP-MKT | Partial | v1alpha1 |
| 48 | `labor.v1` | NIP-LAB | Partial | v1alpha1 |
| 49 | `payment.v1` | payment evidence, spend grants, wallet, Lightning address | Partial | v1alpha1 |
| 50 | `x402.v1` | NIP-X402, x402 v2 | Implemented | v1alpha1 |
| 51 | `ledger.v1` | pay-ledger, earnings, statements | Implemented | v1alpha1 |
| 52 | `billing.v1` | plans, subscriptions, entitlements, provider events | Implemented | v1alpha1 |
| 53 | `pylon.v1` | NIP-PYLON | Implemented | direct v1 |
| 54 | `attest.v1` | NIP-ATT | Partial | v1alpha1 |
| 55 | `sovereign.v1` | NIP-SOV | Partial | v1alpha1 |
| 56 | `xp.v1` | NIP-XP | Implemented | direct v1 |
| 57 | `forum.v1` | Forum | Implemented | v1alpha1 |
| 58 | `deploy.v1` | NIP-DEPLOY | Implemented | direct v1 |
| 59 | `referral.v1` | referral contracts | Partial | v1alpha1 |
| 60 | `ui.v1` | `aethyr.ui.v1`, Rust Native views and intents | Implemented | v1alpha1 |
| 61 | `engine.v1` | exists | Implemented | v1 (published) |
| 62 | `verse.*` (28) | Chapter V | Mixed | per Chapter V |
| 63 | `telemetry.v1` | playtest events, product events | Partial | v1alpha1 |
| 64 | `evidence.v1` | research and qualification records | Implemented as JSON | v1alpha1 |
| 65 | `discovery.v1` | capability manifest, API index, docs corpus | Implemented | v1alpha1 |

### 14.2 Import graph

Arrows mean "may import". Everything imports `type` and `options`, and
those edges are omitted. Anything not listed imports only those two. The
graph is acyclic, and `buf` plus a custom lint enforce it.

| Package | May import |
| --- | --- |
| `api` | — |
| `account`, `credential` | `api` |
| `vault` | `attest` |
| `identity` | `nostr.block.oa`, `nostr.nip05` |
| `receipt` | — |
| `decision` | `receipt`, `inference` |
| `inference` | `receipt` |
| `chat` | `decision`, `receipt`, `ui` |
| `exec` | `receipt`, `context` |
| `policy` | `context` |
| `capability` | `policy`, `decision` |
| `program` | `capability` |
| `extension` | `capability`, `program`, `skill` |
| `eval` | `extension`, `receipt` |
| `gym` | `eval` |
| `optimize` | `eval`, `extension` |
| `knowledge` | — |
| `trace` | `nostr.block.ao`, `nostr.block.am`, `nostr.block.ae` |
| `run` | `capability`, `receipt` |
| `coordination` | `run`, `context` |
| `automation` | `coordination`, `run`, `context` |
| `session` | `capability`, `run`, `context` |
| `control` | `session` |
| `workspace` | `context`, `session` |
| `environment` | `workspace` |
| `work` | `context`, `workspace` |
| `host` | `reach`, `terminal`, `session` |
| `reach`, `terminal` | — |
| `live` | `capability`, `session` |
| `agent` | `session`, `receipt`, `nostr.block.oa` |
| `agent.managed` | `agent`, `environment`, `api` |
| `market` | `capability`, `payment` |
| `labor` | `market`, `exec`, `context` |
| `payment` | — (references `third_party.lightninglabs` by identity only) |
| `x402` | `payment` |
| `ledger` | `payment` |
| `billing` | `ledger`, `account` |
| `pylon` | `payment`, `receipt`, `attest` |
| `attest` | — |
| `sovereign` | `automation`, `environment`, `payment` |
| `cloud`, `computers` | `environment`, `api` |
| `compute` | `ledger`, `cloud` |
| `coder.*` | `session`, `work`, `workspace`, `host`, `chat`, `trace`, `exec` |
| `xp` | — |
| `forum` | `nostr.nip7d`, `nostr.nip22` |
| `deploy`, `referral`, `telemetry`, `evidence`, `discovery` | — |
| `ui` | — |
| `engine` | — |
| `verse.*` | `engine`, `ui`, `nostr.*` (Chapter V §9.3) |

`type.v1/private_records.proto` and `api.v1/webhooks.proto` aggregate
every owning package. They import downward into everything and are
imported by nothing except carriage code.

## 15. Identity, accounts and secrets

### 15.1 `openagents.account.v1`

**Purpose.** Accounts, principals, workspaces, memberships, invitations,
projects, the tenant registry, admission snapshots, quotas and
reservations.

**Sources.** Glossary "Tenancy" (tenant registry, door binding, admission
snapshot, account, principal, workspace, membership, quota, reservation,
request and attempt); `openagents.tenancy.v1`, `tenancy.accounts.v1`,
`tenancy.sessions.v1`, `tenancy.keys.v1`, `tenancy.backend.v1`; the
first-party OpenAPI.

**Status.** Implemented in `crates/openagents-web` as JSON.

```proto
package openagents.account.v1;

message Account { string id = 1; /* acct_ */ string name = 2; AccountKind kind = 3; int64 created_at = 4; }
enum AccountKind { ACCOUNT_KIND_UNSPECIFIED = 0; ACCOUNT_KIND_PERSON = 1; ACCOUNT_KIND_ORGANIZATION = 2; ACCOUNT_KIND_AGENT = 3; }

// A principal is whoever a request acts as: a person, an API key, an agent key, a device.
message Principal {
  oneof who {
    string account_id = 1;
    string key_id = 2;
    T.Pubkey nostr = 3;
    string device_id = 4;
  }
}

message Workspace { string id = 1; /* ws_ */ string account_id = 2; string name = 3; string slug = 4; int64 created_at = 5; }
message Membership { string workspace_id = 1; string account_id = 2; Role role = 3; int64 joined_at = 4; }
enum Role { ROLE_UNSPECIFIED = 0; ROLE_OWNER = 1; ROLE_ADMIN = 2; ROLE_MEMBER = 3; ROLE_VIEWER = 4; ROLE_BILLING = 5; }
message Invitation { string id = 1; /* inv_ */ string workspace_id = 2; string email = 3 [(openagents.options.v1.sensitivity) = SENSITIVITY_PERSONAL];
                     Role role = 4; int64 expires_at = 5; InvitationState state = 6; }
message Project { string id = 1; /* prj_ */ string workspace_id = 2; string name = 3; repeated string repositories = 4; }

// Tenant registry: which tenant a door or route belongs to, and the snapshot
// of what was admitted when a request started.
message Tenant { string id = 1; string account_id = 2; repeated DoorBinding doors = 3; }
message DoorBinding { string door = 1; string model = 2; string lane = 3; }
message AdmissionSnapshot {
  string request = 1; uint32 attempt = 2;
  Principal principal = 3; string workspace_id = 4;
  repeated string scopes = 5; Quota quota = 6; optional Reservation reservation = 7;
  int64 admitted_at = 8;
}
message Quota { string scope = 1; uint64 limit = 2; uint64 used = 3; T.Window window = 4; }
message Reservation { string id = 1; T.Amount amount = 2; int64 expires_at = 3; ReservationState state = 4; }
```

**Services.** `AccountService` (`GetAccount`, `ListWorkspaces`,
`CreateWorkspace`, `Invite`, `AcceptInvitation`, `ListMembers`,
`SetRole`). HTTP under `/v1/workspaces`, `/v1/invitations`,
`/v1/projects`.

### 15.2 `openagents.credential.v1`

**Purpose.** API keys and scopes, sign-in sessions, device sign-in,
linked identities, provider keys (BYOK), customer-managed keys and OAuth
connections.

**Sources.** Glossary "oak_ keys", "scopes", "provider key", "BYOK",
"CMK", "linked identity", "device sign-in", "data domains"; the `oak` CLI;
`openagents.cloud.provider-key-custody.v2`; `openagents.codex_auth_grant.v1`;
`openagents.oauth.*`.

**Status.** Implemented.

```proto
package openagents.credential.v1;

message ApiKey {
  string id = 1;                      // key_
  string workspace_id = 2;
  string name = 3;
  repeated string scopes = 4;         // registry in scopes.md: "decisions:write", "computers:admin", ...
  string prefix = 5;                  // "oak_live_…" first chars, for display
  int64 created_at = 6;
  optional int64 last_used_at = 7;
  optional int64 expires_at = 8;
}
// Returned once at creation; never stored or listed again.
message ApiKeySecret { string key_id = 1; string secret = 2 [(openagents.options.v1.sensitivity) = SENSITIVITY_SECRET]; }

message SignInSession { string id = 1; /* sess_ */ string account_id = 2; int64 created_at = 3; int64 expires_at = 4; string user_agent = 5; }

// Device sign-in (CLI, desktop, TV-style): user code + device code + poll.
message DeviceAuthorization { string device_code = 1 [(openagents.options.v1.sensitivity) = SENSITIVITY_SECRET];
                              string user_code = 2; string verification_uri = 3; uint32 interval_s = 4; int64 expires_at = 5; }

message LinkedIdentity { string provider = 1; /* github, google, nostr, workos */ string subject = 2; string account_id = 3; int64 linked_at = 4; }

// BYOK: the record of a provider key, never the key in a readable form.
message ProviderKey { string id = 1; string provider = 2; string label = 3; CustodyMode custody = 4; int64 created_at = 5; }
enum CustodyMode { CUSTODY_MODE_UNSPECIFIED = 0; CUSTODY_MODE_PLATFORM_KMS = 1; CUSTODY_MODE_CUSTOMER_MANAGED = 2; CUSTODY_MODE_VAULT_SEALED = 3; }

message CustomerManagedKey { string id = 1; /* cmk_ */ string kms_uri = 2; string state = 3; int64 rotated_at = 4; }

message OAuthConnection { string id = 1; string provider = 2; repeated string scopes = 3; string account_id = 4; int64 expires_at = 5; }

// Data domains: which classes of tenant data a key or session may touch.
message DataDomain { string id = 1; repeated string classifications = 2; }
```

**Services.** `CredentialService` (`CreateKey`, `ListKeys`, `RevokeKey`,
`StartDeviceAuthorization`, `PollDeviceAuthorization`, `AddProviderKey`,
`RotateCmk`).

### 15.3 `openagents.vault.v1`

**Purpose.** Sealed personal data (NIP-VAULT). Storage holds ciphertext
only. Objects open on the person's devices (`user`), on an attested
workload with the person's share (`sealed`), or through an audited KMS
operator (`operator`).

**Status.** Partial: `user` tier implemented (`oa-vault`, browser, CLI);
`sealed` implemented in Rust (`vault-sealed`) with deployment pending;
`operator` implemented (`vault-operator`).

```proto
package openagents.vault.v1;

message VaultObject {
  option (openagents.options.v1.contract) = "openagents.vault-object.v1";
  ObjectCore core = 1;                // core_digest = SHA256(JCS(core))
  repeated Wrap wraps = 2;
}
message ObjectCore {
  string object = 1; string vault = 2; Tier tier = 3;
  ContentLayout content = 4; optional MediaInfo media = 5; int64 created_at = 6;
}
enum Tier { TIER_UNSPECIFIED = 0; TIER_USER = 1; TIER_SEALED = 2; TIER_OPERATOR = 3; }
message ContentLayout {
  string alg = 1;                     // "aes-256-gcm-chunked-v1"
  uint32 chunk_bytes = 2;             // 1,024–1,048,576
  bytes nonce_prefix = 3;
  repeated T.Digest chunks = 4;
}
message Wrap {
  oneof wrap {
    UserWrap user = 1;                // {key, nonce, dek}
    SealedWrap sealed = 2;            // HPKE mode_psk {system_key, user_key, suite, enc, dek}
    OperatorWrap operator = 3;        // {kms_key, dek}
  }
}
message Slot { string vault = 1; string slot = 2; string method = 3; /* "passkey-prf…" */ bytes params = 4; bytes nonce = 5; bytes vmk = 6; string label = 7; int64 created_at = 8; }
message Index { string vault = 1; uint64 epoch = 2; repeated IndexEntry entries = 3; }
message Lease {
  string lease = 1; string run = 2; repeated string objects = 3; string purpose = 4;
  Route route = 5;                    // private | fast | device
  LeaseEffects effects = 6;           // {read, write_derived, export}
  int64 expires_at = 7;               // ≤86,400 s; default 900 s
  uint32 max_uses = 8; string issued_by = 9; int64 issued_at = 10;
}
```

The Managed Agents `vlt_` resource (§18.2) is a view over a vault plus
the credentials an agent session may use. It never exposes DEKs.

### 15.4 `openagents.identity.v1`

**Purpose.** The Nostr-key side of identity: profiles, NIP-05 names,
owner attestations, and links between keys.

**Sources.** Kind 0 with NIP-24 `bot`; Block NIP-OA `auth` tags (used by
AGENT and PYLON); NIP-XP key links (13195); NIP-39 external identities.

```proto
package openagents.identity.v1;

message KeyIdentity { T.Pubkey pubkey = 1; optional string nip05 = 2; bool bot = 3; repeated string relays = 4; }
// Block OA: owner attests that `agent` acts for `owner` under conditions.
message OwnerAttestation { T.Pubkey owner = 1; T.Pubkey agent = 2; string conditions = 3; /* "created_at<T" */ bytes sig = 4; }
message KeyLink { T.Pubkey from = 1; T.Pubkey to = 2; string purpose = 3; int64 linked_at = 4; }
message AccountKeyBinding { string account_id = 1; T.Pubkey pubkey = 2; string proof = 3; int64 bound_at = 4; }
```

## 16. Decisions, inference and receipts

### 16.1 `openagents.decision.v1`

**Purpose.** Typed decisions. A caller sends one state and typed
questions and gets probabilities back. The same contract is served over
HTTP (`POST /v1/systemone`, `POST /v1/decisions`), over Nostr (NIP-DEC on
25910/26910/27010), and in-process to Clef. Also classification
(`/v1/classify`), durable jobs (`/v1/jobs`), doors, model cards and
calibration.

**Sources.** NIP-DEC; `openagents.systemone.v1` (never moved to v2);
`openagents.decision-call.v1`; `docs/decision-models/api/openapi.yaml`;
glossary "Decision contract", "System One", "EntryType", "door identity",
"model card", "Jev/Kev/Lev/Laya", "estimators L1–L3", "calibration
record v2", "classify.v1"; owner rule "Clef first, Jev backup" (doors in
order Clef, Pylons, Jev, Vertex).

**Status.** Implemented: `crates/nostr::decision`, the hosted
`decision-worker`, `/v1/decisions`, `/v1/systemone`.

```proto
package openagents.decision.v1;

// openagents.systemone.v1. The contract is frozen; proto mirrors it exactly.
message SystemOneRequest {
  option (openagents.options.v1.contract) = "openagents.systemone.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_INTENT;
  option (openagents.options.v1.max_encoded_bytes) = 262144;
  string request = 1 [(openagents.options.v1.limits) = {max_bytes: 128}];
  uint32 attempt = 2;
  string model = 3 [(openagents.options.v1.limits) = {min_bytes: 1, max_bytes: 128}]; // jev-1.13.0, jev-latest, typesafe/jev-1.13
  EntryType state = 4;                   // string | object | array | null; ≤128 KiB, depth ≤32
  repeated NamedQuestion questions = 5 [(openagents.options.v1.limits) = {min_count: 1, max_count: 64}];
  optional int64 deadline = 6;
}
message NamedQuestion { string id = 1; Question question = 2; }
message Question {
  QuestionType type = 1;                 // noul | choice | score
  EntryType instructions = 2;            // depth ≤8
  EntryType criteria = 3;
  repeated EntryType options = 4 [(openagents.options.v1.limits) = {max_count: 255}]; // choice
  uint32 levels = 5;                     // score: 2–10
}
enum QuestionType { QUESTION_TYPE_UNSPECIFIED = 0; QUESTION_TYPE_NOUL = 1; QUESTION_TYPE_CHOICE = 2; QUESTION_TYPE_SCORE = 3; }

// EntryType is the one place the protocol models JSON-shaped data, because the
// decision contract is defined over it. It is a closed, bounded tree, not Struct.
message EntryType {
  oneof value {
    T.Null null = 1;
    string text = 2;
    EntryObject object = 3;
    EntryArray array = 4;
    double number = 5;
    bool boolean = 6;
  }
}
message EntryObject { repeated EntryField fields = 1; }   // order preserved
message EntryField { string key = 1; EntryType value = 2; }
message EntryArray { repeated EntryType items = 1; }

message Answer {
  oneof answer {
    NoulAnswer noul = 1;                 // {noul}
    ChoiceAnswer choice = 2;             // {choice, confidence, probabilities}
    ScoreAnswer score = 3;               // {score, confidence, legend, probabilities}
  }
  optional uint32 selected = 4;
}
message NoulAnswer { double noul = 1; }
message ChoiceAnswer { uint32 choice = 1; double confidence = 2; repeated double probabilities = 3; }
message ScoreAnswer { uint32 score = 1; double confidence = 2; repeated string legend = 3; repeated double probabilities = 4; }

message SystemOneResponse {
  string model = 1; repeated NamedAnswer answers = 2; T.Usage usage = 3;
  ServiceIdentity service = 4; uint64 latency_ms = 5;
}
message NamedAnswer { string id = 1; Answer answer = 2; }
message ServiceIdentity { string door = 1; string version = 2; }

message SystemOneResult {
  string request = 1; uint32 attempt = 2;
  T.Outcome outcome = 3; bool dispatched = 4;
  oneof response_state { T.Null response_null = 5; SystemOneResponse response = 6; }
  openagents.receipt.v1.ExecutionReceipt receipt = 7;
  optional string code = 8;
}
message SystemOneStatus { StatusPhase phase = 1; optional string code = 2; optional string message = 3; optional uint64 retry_after_ms = 4; }
message SystemOneCancel { string request = 1; uint32 attempt = 2; }

// POST /v1/decisions: the OpenAI-shaped wrapper (predicate | choice | score).
message DecisionsRequest {
  option (openagents.options.v1.http) = {method: "POST", path: "/v1/decisions", id_prefix: "dec_"};
  string model = 1;
  oneof kind { Predicate predicate = 2; Choice choice = 3; Score score = 4; }
  EntryType input = 5;
  optional RoutingHints routing = 6;     // X-Decision-Order / -Lane / -Door / -Shadow
}
message Decision { string id = 1; string object = 2; /* "decision" */ string model = 3; Answer answer = 4; ServiceIdentity door = 5; T.Usage usage = 6; int64 created = 7; }
message RoutingHints { repeated Door order = 1; optional string lane = 2; optional Door door = 3; bool shadow = 4; }
enum Door { DOOR_UNSPECIFIED = 0; DOOR_CLEF = 1; DOOR_PYLON = 2; DOOR_JEV = 3; DOOR_VERTEX = 4; DOOR_COMMUNITY = 5; }

// /v1/classify (openagents.classify.v1): policy-driven labeling with review and fallback.
message ClassifyRequest { string policy = 1; EntryType input = 2; optional string idempotency_key = 3; }
message ClassifyResult { string label = 1; double confidence = 2; ClassifyRoute route = 3; optional string review_id = 4; }
enum ClassifyRoute { CLASSIFY_ROUTE_UNSPECIFIED = 0; CLASSIFY_ROUTE_AUTO = 1; CLASSIFY_ROUTE_REVIEW = 2; CLASSIFY_ROUTE_FALLBACK = 3; }

// /v1/jobs: durable batches of decisions or classifications.
message Job { string id = 1; /* job_ */ JobState state = 2; uint32 total = 3; uint32 done = 4; uint32 failed = 5; optional T.ArtifactRef results = 6; int64 created_at = 7; }

// Model cards and calibration.
message ModelCard { string model = 1; string family = 2; /* jev, kev, lev, laya */ string version = 3; repeated string doors = 4;
                    repeated CalibrationSet sets = 5; T.ArtifactRef card = 6; }
message CalibrationSet { string name = 1; T.Digest digest = 2; string calibration = 3; }
message CalibrationRecord {
  option (openagents.options.v1.contract) = "openagents.calibration.v2";
  string model = 1; string set = 2; string estimator = 3; /* L1 | L2 | L3 */ double ece = 4; uint32 n = 5; T.ArtifactRef evidence = 6;
}
// ATIF recording of one decision call inside a trajectory.
message DecisionCall { option (openagents.options.v1.contract) = "openagents.decision-call.v1";
                       SystemOneRequest request = 1; SystemOneResult result = 2; }
```

**Limits** are exactly NIP-DEC's: 256 KiB payload,
`request` ≤128 B, `model` 1–128 B, `state` ≤128 KiB and depth ≤32, 1–64
questions, ≤64 KiB per question, entry depth ≤8, 1–255 choice options,
2–10 score levels, and probabilities summing to 1 within 1e-6.

**Services.** `DecisionService` (`Decide` = SystemOne, `Decisions`,
`Classify`, `CreateJob`, `GetJob`, `ListModels`, `GetModelCard`).

### 16.2 `openagents.inference.v1`

**Purpose.** The generation side of `api.openagents.com`: Responses,
chat completions, batches, models and tool calls, in the OpenAI-compatible
shape we serve.

**Sources.** `resp_`, `batch_` routes; `openagents.inference.*.v1`;
owner rule "Google-first providers" (Vertex first, OpenRouter and Vercel
as fallbacks).

**Status.** Implemented.

```proto
package openagents.inference.v1;

message CreateResponseRequest {
  option (openagents.options.v1.http) = {method: "POST", path: "/v1/responses", id_prefix: "resp_"};
  string model = 1;
  repeated InputItem input = 2;
  optional string instructions = 3;
  repeated Tool tools = 4;
  optional ToolChoice tool_choice = 5;
  optional uint32 max_output_tokens = 6;
  optional double temperature = 7;
  bool stream = 8;
  optional string previous_response_id = 9;
}
message InputItem { oneof item { Message message = 1; FunctionCallOutput function_call_output = 2; } }
message Message { string role = 1; repeated ContentPart content = 2; }
message ContentPart { oneof part { string input_text = 1; string output_text = 2; ImageRef input_image = 3; FileRef input_file = 4; } }
message Tool { oneof tool { FunctionTool function = 1; string builtin = 2; /* web_search, file_search */ } }
message FunctionTool { string name = 1; string description = 2; T.ArtifactRef parameters = 3; /* JSON Schema */ }
message Response { string id = 1; string model = 2; repeated OutputItem output = 3; T.Usage usage = 4; ResponseStatus status = 5;
                   optional ProviderSwitch switched = 6; int64 created_at = 7; }
message ProviderSwitch { string provider = 1; string model = 2; string why = 3; }
message StreamEvent { string type = 1; uint64 sequence_number = 2; oneof data { string delta = 3; Response response = 4; OutputItem item = 5; } }
message Model { string id = 1; string owned_by = 2; repeated string capabilities = 3; uint32 context_window = 4; ModelPrice price = 5; }
message ModelPrice { T.MicroAmount input_per_mtok = 1; T.MicroAmount output_per_mtok = 2; }
```

Tool `parameters` are carried as a JSON Schema artifact or inline bytes
under a declared contract, because the OpenAI shape requires arbitrary
JSON Schema there. That is the one place a schema rides as data, and it is
marked `Opaque` with contract `json-schema-2020-12`.

### 16.3 `openagents.receipt.v1`

**Purpose.** The one record of what a service did with a request:
`openagents.receipt.execution.v1`, and the chain that links receipts across
hops.

**Sources.** `crates/receipts`; NIP-AGENT gateway receipts; `X-Receipt`;
glossary "execution receipt", "receipt chain", `locked_read.v1`.

```proto
package openagents.receipt.v1;

message ExecutionReceipt {
  option (openagents.options.v1.contract) = "openagents.receipt.execution.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_RECEIPT;
  string id = 1;                          // outc_ when served over HTTP
  string service = 2;                     // "decisions", "responses", "agent-gateway", ...
  string request = 3; uint32 attempt = 4;
  T.ServiceOutcome outcome = 5;
  T.Digest request_digest = 6;
  optional T.Digest result_digest = 7;
  ServicePath path = 8;                   // door, model, provider, version actually used
  T.Usage usage = 9;
  optional T.Amount charged = 10;
  int64 started_at_ms = 11; int64 finished_at_ms = 12;
  repeated T.Digest parents = 13;         // receipt chain: digests of upstream receipts
  optional T.Refusal refusal = 14;
  bytes signature = 15;                   // service key over JCS(receipt without signature)
}
message ServicePath { string door = 1; string model = 2; string provider = 3; string version = 4; optional T.ArtifactRef attestation = 5; }

// A read of a locked artifact, recorded so a later reader can prove what was seen.
message LockedRead { option (openagents.options.v1.contract) = "openagents.locked_read.v1";
                     T.Lock lock = 1; T.Digest lock_digest = 2; repeated T.ArtifactRef read = 3; int64 read_at = 4; }
```

## 17. Conversation and execution jobs

NIP-CJ defines three encrypted ephemeral job families. Every request has
exactly one `p`. Every response has one `p` and one `e`. Content is
NIP-44 v2. Decision jobs belong to `decision.v1`. Conversation and
execution have their own packages.

### 17.1 `openagents.chat.v1`

**Purpose.** One conversation everywhere: web, mobile, desktop and Coder.
Requests, results and feedback (judgments, offers, cards, partials,
status), the chat router, the answer bank, and the HTTP thread and message
model.

**Sources.** NIP-CJ conversation; `openagents.cj-card.v1`;
`openagents.eval-draft.v1`; chat router v1–v3 (`coder.router.v1` holds
the router's own records); glossary "NIP-CJ chat jobs", "threads ↔ ATIF",
"chat worker", "Gym-in-chat cards CARD-01..07", "answer bank
`chat-answers-v1`", "offers"; owner rule "No Chat/Code modes".

**Status.** Implemented (`crates/nostr::cj_conversation`, chat worker).

```proto
package openagents.chat.v1;

message ConversationRequest {
  option (openagents.options.v1.nip) = {nip: "NIP-CJ", kinds: [25900]};
  option (openagents.options.v1.data_class) = DATA_CLASS_INTENT;
  string task = 1;
  repeated Turn transcript = 2;
  optional string instructions = 3;
  optional ClientInfo client = 4;
  optional RouterHints router = 5;
  optional JudgeHints judge = 6;
  optional string opener = 7;
  optional openagents.eval.v1.EvalDraft draft = 8;
  optional PayerKeys payer = 9;            // {keys: ciphertext}; SECRET inside NIP-44
  optional RankRequest rank = 10;          // type: "rank", candidates 1–16
  optional ConversationContext context = 11;
}
message Turn { Role role = 1; string content = 2 [(openagents.options.v1.sensitivity) = SENSITIVITY_CONFIDENTIAL]; }
enum Role { ROLE_UNSPECIFIED = 0; ROLE_USER = 1; ROLE_ASSISTANT = 2; }
message ConversationContext {
  string surface = 1; bool computer_ready = 2; string app_build = 3;
  optional ComputerContext computer = 4;   // {place: here|paired, name, engines ≤8}
  optional ProjectContext project = 5;
  optional CoderRunContext coder_run = 6;  // ending, turn, engine, model, summary ≤4 KiB, files ≤32, commands ≤16
  repeated CoderRunContext runs = 7 [(openagents.options.v1.limits) = {max_count: 5}];
}

message ConversationFeedback {
  option (openagents.options.v1.nip) = {nip: "NIP-CJ", kinds: [27000]};
  oneof feedback {
    Judgment judgment = 1;
    Offer offer = 2;
    Card card = 3;
    Partial partial = 4;                   // {seq, delta}
    FeedbackStatus status = 5;             // queued | processing | error
  }
}
message Judgment { string verdict = 1; string set = 2; string lane = 3; double confidence = 4; string bank = 5; string answer = 6;
                   string tier = 7; string route = 8; double route_p = 9; string capability = 10; }
message Offer { OfferKind kind = 1; string label = 2 [(openagents.options.v1.limits) = {max_bytes: 80}]; OfferPayload payload = 3; }
enum OfferKind { OFFER_KIND_UNSPECIFIED = 0; OFFER_KIND_RUN_CODER = 1; OFFER_KIND_OPEN_SCREEN = 2; OFFER_KIND_CLI = 3;
                 OFFER_KIND_START_EVAL = 4; OFFER_KIND_PUBLISH_EVAL = 5; OFFER_KIND_OPEN_PRESENTATION = 6; }
// openagents.cj-card.v1
message Card { CardKind kind = 1; string title = 2; CardBody body = 3; }
enum CardKind { CARD_KIND_UNSPECIFIED = 0; CARD_KIND_TOOL = 1; CARD_KIND_DRAFT = 2; CARD_KIND_RUN = 3; CARD_KIND_RESULT = 4;
                CARD_KIND_NEWS = 5; CARD_KIND_CHECK = 6; CARD_KIND_CREDIT = 7; CARD_KIND_CAPABILITY = 8; }

message ConversationResult {
  option (openagents.options.v1.nip) = {nip: "NIP-CJ", kinds: [26900]};
  option (openagents.options.v1.data_class) = DATA_CLASS_RECEIPT;
  string text = 1; TokenUsage usage = 2; string model = 3;
  optional openagents.inference.v1.ProviderSwitch switched = 4;
  optional string reasoning = 5 [(openagents.options.v1.limits) = {max_bytes: 32768}];
  string tier = 6; string route = 7; string bank = 8; string answer = 9;
  repeated string followups = 10; repeated Citation citations = 11; optional string commit = 12;
  repeated RankedCandidate ranked = 13;
}

// HTTP thread model (th_, msg_, turn_).
message Thread { string id = 1; string workspace_id = 2; string title = 3; int64 created_at = 4; int64 updated_at = 5;
                 optional T.ArtifactRef trajectory = 6; /* threads ↔ ATIF */ }
message ThreadMessage { string id = 1; string thread_id = 2; Role role = 3; repeated openagents.inference.v1.ContentPart content = 4;
                        optional string turn_id = 5; int64 created_at = 6; }
message AnswerBankEntry { option (openagents.options.v1.contract) = "openagents.chat-answers.v1";
                          string id = 1; string question = 2; string answer = 3; repeated string citations = 4; T.Digest digest = 5; }
```

Hosted `start_eval` caps (8 cases, 3 runs, 2 arms) and offer label limits
are kept as `FieldLimits`.

### 17.2 `openagents.exec.v1`

**Purpose.** Durable, idempotent execution of one locked target on a
worker: execute, accepted, progress, result, and the status, replay and
cancel controls.

**Sources.** NIP-CJ execution (`openagents.execution.v1`); features
`openagents.labor-binding.v1`, `openagents.environment-binding.v1`,
`payer.keys`, `pylon.deadline.v1`.

**Status.** Implemented.

```proto
package openagents.exec.v1;

message Execute {
  option (openagents.options.v1.contract) = "openagents.execution.v1";
  option (openagents.options.v1.nip) = {nip: "NIP-CJ", kinds: [25920]};
  option (openagents.options.v1.data_class) = DATA_CLASS_INTENT;
  string request = 1;               // hex64
  uint32 attempt = 2;
  string run = 3;
  T.DefinitionRef target = 4;
  T.Lock lock = 5;
  T.ArtifactRef input = 6;
  optional T.ContextManifest context = 7;
  Requirements requirements = 8;
  T.Bounds bounds = 9;
  int64 deadline = 10;
  int64 retain_until = 11;
  optional ParentStep parent = 12;  // {run, step, iteration, attempt}
  repeated Binding bindings = 13;   // labor-binding, environment-binding (feature-gated)
}
// Idempotency key (worker, principal, request, attempt); fingerprint = SHA256(JCS(body)).

message ExecRequest { oneof body { Execute execute = 1; StatusQuery status = 2; Replay replay = 3; Cancel cancel = 4; } }
message Replay { uint64 after_seq = 1; uint32 max_records = 2; }
message Cancel { string reason = 1; }

message ExecFeedback {
  option (openagents.options.v1.nip) = {nip: "NIP-CJ", kinds: [27020]};
  oneof body {
    Accepted accepted = 1;          // {request, attempt, run, input_digest, lock_digest, record, mailbox, retain_until, controller}
    Progress progress = 2;          // {seq, status: queued|running|reconciling}
    openagents.attest.v1.SealedChunk sealed_chunk = 3; // NIP-ATT sealed jobs
  }
}
message ExecResult {
  option (openagents.options.v1.nip) = {nip: "NIP-CJ", kinds: [26920]};
  option (openagents.options.v1.data_class) = DATA_CLASS_RECEIPT;
  oneof body {
    Result result = 1;
    StatusResult status_result = 2;
    ReplayResult replay_result = 3;
    CancelResult cancel_result = 4;
  }
}
message Result {
  T.Outcome outcome = 1; bool dispatched = 2;
  optional T.ArtifactRef output = 3; repeated T.ArtifactRef artifacts = 4;
  repeated openagents.receipt.v1.ExecutionReceipt receipts = 5;
  T.Verification verification = 6; T.Integration integration = 7;
  optional T.EventRef record = 8;   // run.v1 record that settled this
  optional string code = 9; optional string message = 10;
}
```

**Services.** `ExecService` (`Execute`, `Status`, `Replay`, `Cancel`)
over gRPC for workers we run, and the CJ Nostr binding for everyone.

## 18. Agents, sessions and control

### 18.1 `openagents.agent.v1`

**Purpose.** Saved custom agents as inert configuration (NIP-AGENT):
private export, public agent identity, owner-published gateway receipts.

**Status.** Implemented (`agent-contract`, `nostr::agent_profile`,
openagents.com publication).

```proto
package openagents.agent.v1;

message AgentDefinition {
  option (openagents.options.v1.contract) = "openagents.agent.definition.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_DOCUMENT;
  string id = 1;                        // hex64
  uint32 revision = 2;
  AgentSpec spec = 3;
  bool deleted = 4;
  optional ForkedFrom forked_from = 5;  // {published, revision}
}
message AgentSpec {
  string name = 1 [(openagents.options.v1.limits) = {max_bytes: 80}];
  string description = 2 [(openagents.options.v1.limits) = {max_bytes: 512}];
  string instructions = 3 [(openagents.options.v1.limits) = {max_bytes: 65536}];
  string runtime = 4 [(openagents.options.v1.limits) = {min_bytes: 1, max_bytes: 128}];
  oneof model_state { T.Null model_null = 5; string model = 6; }
  Access access = 7;                    // host_default | read_only | workspace_write
  repeated Fallback fallbacks = 8 [(openagents.options.v1.limits) = {max_count: 4}];
  Autonomy autonomy = 9;
}
message Autonomy {
  bool check_ins = 1;
  uint32 every_minutes = 2;             // 5–1440
  string authority = 3;
  uint32 stall_minutes = 4;             // 5–240
  uint32 shell_timeout_secs = 5;        // 10–3600
  repeated Wake wakes = 6 [(openagents.options.v1.limits) = {max_count: 16}];
  uint32 coalesce_secs = 7;             // 0–1800
}
// Kind 30187 tags: d, auth (Block OA), p owner, r URL, status, alt, e att-release/att-endpoint.
message PublicDefinition { AgentDefinition definition = 1; openagents.identity.v1.OwnerAttestation auth = 2;
                           T.Pubkey owner = 3; string url = 4; bool revoked = 5; repeated T.EventRef attestations = 6; }
// Kind 3204: owner-published gateway receipt.
message GatewayReceipt { T.Pubkey agent = 1; string coordinate = 2; T.Digest receipt_digest = 3; openagents.receipt.v1.ExecutionReceipt receipt = 4; }
message DefinitionExport { option (openagents.options.v1.contract) = "openagents.agent-definition-export.v1"; AgentDefinition definition = 1; }
```

### 18.2 `openagents.agent.managed.v1`

**Purpose.** The Managed Agents API: hosted agent sessions with events,
vault bindings, deployments, deployment runs, memory stores and
webhooks. This is the HTTP product on top of `agent.v1`, `session.v1` and
`environment.v1`.

**Sources.** The proposed Managed Agents API (resource prefixes
`agent_ sesn_ sevt_ vlt_ depl_ drun_ memstore_ mem_ memver_ env_ envk_
whk_ whe_`); `docs/sdk/openapi.app.json`.

**Status.** Designed (proposal). Field shapes follow the proposal and are
`v1alpha1`.

```proto
package openagents.agent.managed.v1;

message ManagedSession {
  string id = 1;                       // sesn_
  string agent_id = 2;                 // agent_
  optional string environment_id = 3;  // env_
  repeated string vault_ids = 4;       // vlt_
  repeated string memory_store_ids = 5;
  SessionStatus status = 6;            // idle | running | requires_action | terminated
  int64 created_at = 7; int64 updated_at = 8;
  T.Usage usage = 9;
}
message SessionEvent {
  string id = 1;                       // sevt_
  string session_id = 2;
  EventType type = 3;                  // user.message, agent.message, agent.tool_use, agent.tool_result,
                                       // session.status_running, session.status_idle, session.status_requires_action,
                                       // session.error, user.interrupt, user.tool_confirmation ...
  oneof body { UserMessage user_message = 4; AgentMessage agent_message = 5; ToolUse tool_use = 6;
               ToolResult tool_result = 7; StatusChange status = 8; T.Refusal error = 9; }
  int64 created_at = 10;
}
message Deployment { string id = 1; /* depl_ */ string agent_id = 2; Trigger trigger = 3; string environment_id = 4; bool enabled = 5; }
message Trigger { oneof kind { Schedule schedule = 1; WebhookTrigger webhook = 2; Manual manual = 3; } }
message DeploymentRun { string id = 1; /* drun_ */ string deployment_id = 2; string session_id = 3; T.Outcome outcome = 4; int64 started_at = 5; optional int64 ended_at = 6; }
message MemoryStore { string id = 1; /* memstore_ */ string name = 2; uint64 bytes = 3; uint32 memories = 4; }
message Memory { string id = 1; /* mem_ */ string store_id = 2; string path = 3; string current_version = 4; }
message MemoryVersion { string id = 1; /* memver_ */ string memory_id = 2; T.Digest digest = 3; uint64 bytes = 4; int64 created_at = 5; }
message Vault { string id = 1; /* vlt_ */ string name = 2; repeated CredentialSlot credentials = 3; }
message CredentialSlot { string name = 1; string kind = 2; /* api_key, oauth, ssh */ bool set = 3; }
```

Deployment triggers use `automation.v1` triggers underneath; this package
names them in HTTP form.

### 18.3 `openagents.session.v1`

**Purpose.** Persistent engine sessions (NIP-SESS): adapters and their
feature matrices, open and attach, command queues, steering,
interactions, child sessions, history export and import, and the
read-only retained-history observer.

**Status.** Partial. Observer in `coder-connect`; managed sessions in
`session-host`. **Revision pending (owner decision 2026-10-16):** live
sessions move to Loro rooms on `coder-sync`, with NIP-HOST as the
authority. This package keeps the adapter, feature, turn and history
records, which the move does not change. The live command path moves to
`coder.sync.v1` and `host.v1`. Both are specified so neither blocks the
other.

```proto
package openagents.session.v1;

message Adapter {
  option (openagents.options.v1.contract) = "openagents.session-adapter.v1";
  string id = 1;                        // "codex", "claude-code", "cursor-agent", "opencode", ...
  string version = 2;
  repeated FeatureSupport features = 3; // the 14-feature matrix
  SteeringProfile steering = 4;
}
enum Feature { FEATURE_UNSPECIFIED = 0; FEATURE_CREATE = 1; FEATURE_RESUME = 2; FEATURE_FORK = 3; FEATURE_IMPORT = 4;
               FEATURE_CLOSE = 5; FEATURE_CONFIGURE = 6; FEATURE_SUBMIT = 7; FEATURE_QUEUE = 8; FEATURE_REORDER = 9;
               FEATURE_STEER = 10; FEATURE_INTERRUPT = 11; FEATURE_HISTORY = 12; FEATURE_QUESTIONS = 13; FEATURE_APPROVALS = 14; }
message FeatureSupport { Feature feature = 1; Support support = 2; /* native | emulated | unsupported */
                         string operation = 3; string semantics = 4; repeated string evidence = 5; string limitations = 6; }
message SteeringProfile { SteeringMode mode = 1; /* mid_turn | turn_boundary | unsupported */
                          optional string emulation = 2; /* cancel_and_continue */ Ack acknowledgment = 3; }

message SessionCommand {
  CommandAction action = 1;             // submit, enqueue, cancel_queued, reorder, steer, interrupt, configure, archive, close
  string session = 2; uint64 expected_revision = 3; string command_id = 4;
  oneof payload { TurnIntent submit = 5; TurnIntent enqueue = 6; string cancel_queued = 7; Reorder reorder = 8;
                  Steering steer = 9; Configuration configure = 10; }
}
message TurnIntent { option (openagents.options.v1.contract) = "openagents.turn-intent.v1";
                     string id = 1; repeated openagents.inference.v1.ContentPart input = 2; IntentState state = 3; }
message TurnResult { string turn = 1; TurnCause cause = 2; T.Usage usage = 3; optional T.ArtifactRef trajectory = 4; }
enum TurnCause { TURN_CAUSE_UNSPECIFIED = 0; TURN_CAUSE_NORMAL = 1; TURN_CAUSE_USER_INTERRUPT = 2; TURN_CAUSE_BUDGET = 3;
                 TURN_CAUSE_DEADLINE = 4; TURN_CAUSE_PROVIDER_REFUSAL = 5; TURN_CAUSE_PROVIDER_FAILURE = 6; TURN_CAUSE_HOOK_STOP = 7;
                 TURN_CAUSE_POLICY = 8; TURN_CAUSE_PROTOCOL_FAULT = 9; TURN_CAUSE_LOST_OBSERVATION = 10; }
message SessionState { string session = 1; Phase phase = 2; uint64 revision = 3; repeated TurnIntent queue = 4; optional string active_turn = 5; }
enum Phase { PHASE_UNSPECIFIED = 0; PHASE_ATTACHING = 1; PHASE_READY = 2; PHASE_RUNNING = 3; PHASE_BLOCKED = 4;
             PHASE_INTERRUPTING = 5; PHASE_CLOSING = 6; PHASE_CLOSED = 7; PHASE_UNKNOWN = 8; PHASE_FAULTED = 9; }
message Interaction { string id = 1; oneof kind { Question question = 2; ApprovalRequest approval = 3; } }
message ChildSession { string parent = 1; string child = 2; ChildPolicy policy = 3; T.Effects effects = 4; }
message History { string session = 1; T.ArtifactRef trajectory = 2; T.Digest steps_digest = 3; }
message HistoryObserverGrant { option (openagents.options.v1.contract) = "openagents.history-observer-grant.v1";
                               T.Pubkey observer = 1; string scope = 2; int64 expires_at = 3; }
```

The full 54-definition bundle in `schemas/session.v1.json` maps
one-to-one to messages in this package. §32 lists the correspondence.

### 18.4 `openagents.control.v1`

**Purpose.** Pairing extra clients to one task with separate observe,
steer and cancel rights, acknowledged commands and bounded catch-up
(NIP-CTRL).

**Status.** Partial; **revision pending** with SESS. Grants are expected
to fold into `host.v1` grants once live sessions run in Loro rooms. Until
the owner closes the revision, this package exists so existing records
have a typed home, and it stays `v1alpha1`.

```proto
package openagents.control.v1;
enum Right { RIGHT_UNSPECIFIED = 0; RIGHT_OBSERVE = 1; RIGHT_STEER = 2; RIGHT_CANCEL = 3; }
message Scope { string task = 1; T.Pubkey controller = 2; uint64 generation = 3; }
message Invitation { string invitation = 1; bytes challenge = 2; T.Pubkey owner = 3; string authority = 4; T.Pubkey client = 5;
                     Scope scope = 6; repeated Right rights = 7; T.ArtifactRef policy = 8; int64 issued_at = 9; int64 expires_at = 10; }
message Grant { string grant = 1; uint32 epoch = 2; /* 0 live, 1 revoked; terminal */ T.Pubkey owner = 3; T.Pubkey client = 4;
                Scope scope = 5; repeated Right rights = 6; string invitation = 7; string pairing = 8; int64 expires_at = 9; }
message Command { string command = 1; string grant = 2; uint32 epoch = 3; Scope scope = 4; uint64 expected_revision = 5;
                  int64 expires_at = 6; oneof action { Steer steer = 7; CancelTask cancel = 8; } }
message Steer { string message = 1; optional string replaces = 2; }
message CommandReceipt { string command = 1; Disposition disposition = 2; /* correction_recorded | cancel_requested */ uint64 revision = 3; }
message TaskRead { ViewKind view = 1; /* state | history */ optional T.Cursor after = 2; uint32 max_items = 3; /* 1–256 */ uint32 max_bytes = 4; }
message TaskView { repeated ViewItem items = 1; optional T.Cursor next = 2; }
message ViewItem { oneof item { bytes frame = 1; T.EventRef run_record = 2; T.ArtifactRef trace = 3; T.Usage usage = 4;
                                T.ArtifactRef finding = 5; T.ArtifactRef projection = 6; } }
```

## 19. Context, policy and capability

### 19.1 `openagents.context.v1`

**Purpose.** Task frames, snapshots, evidence representations, the
evidence graph, context requests and builds, selection receipts, history
indexes, and bounded expansion (NIP-CTX). The shared `ContextManifest`,
`Evidence` and `Observation` live in `type.v1`, because every domain cites
them.

**Status.** Designed. `work.v1.json` already embeds the task-frame and
snapshot definitions.

```proto
package openagents.context.v1;

message TaskFrame {
  option (openagents.options.v1.contract) = "openagents.task-frame.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_DOCUMENT;
  string task = 1; T.Pubkey owner = 2; T.Pubkey controller = 3; uint64 revision = 4; optional T.Digest previous = 5;
  string objective = 6; Origin origin = 7; /* user | program | inferred */
  repeated string constraints = 8; repeated string acceptance = 9;
  T.ArtifactRef snapshot = 10; optional T.ArtifactRef instructions = 11;
  repeated Variable variables = 12; uint32 attempts = 13; repeated string unresolved = 14;
  repeated Correction corrections = 15;
}
message Snapshot { string id = 1; repeated SnapshotSource sources = 2; int64 captured_at = 3; }
message SnapshotSource { SourceKind kind = 1; /* repository|tool|conversation|document|external */ string id = 2;
                         string revision = 3; Availability availability = 4; /* retained|deleted|unavailable */ }
message Representation { string evidence = 1; RepresentationMode mode = 2; /* original|excerpt|structured|short_summary|long_summary */
                         T.ArtifactRef content = 3; uint64 tokens = 4; string tokenizer = 5; }
message EvidenceGraph { repeated T.Evidence nodes = 1; repeated Edge edges = 2; }
message Edge { string from = 1; string to = 2; Relation relation = 3; /* derived_from|supports|contradicts|supersedes */ }
message ContextRequest { string task = 1; string recipient = 2; ContextLimits limits = 3; repeated RepresentationMode allowed_modes = 4; }
message ContextLimits { uint64 input_bytes = 1; uint64 tokens = 2; string tokenizer = 3; }
message ContextSelection { option (openagents.options.v1.contract) = "openagents.context-selection.v1";
                           T.ContextManifest manifest = 1; T.Digest request_digest = 2; string selector = 3; }
message HistoryIndex { string scope = 1; T.ArtifactRef snapshot = 2; repeated string roots = 3; repeated HistoryNode nodes = 4; }
message ExpansionRequest { string selection = 1; repeated string evidence = 2; RepresentationMode mode = 3; T.Bounds bounds = 4; }
message ExpansionResult { repeated Representation added = 1; repeated T.Omission omitted = 2; }
```

### 19.2 `openagents.policy.v1`

**Purpose.** Who may see what (recipients and disclosure), instruction
sets, learned preferences, exact-action approvals, routing and usage, and
adoption authority (NIP-POL).

**Status.** Designed.

```proto
package openagents.policy.v1;

message Recipient { string host = 1; string service = 2; Location location = 3; /* local | remote */ string principal = 4;
                    string origin = 5; string account_scope = 6; string model = 7; }
message DisclosurePolicy { option (openagents.options.v1.contract) = "openagents.disclosure-policy.v1";
                           repeated DisclosureRule rules = 1; }
message DisclosureRule { string classification = 1; repeated string recipients = 2; repeated Purpose purposes = 3; bool allow = 4; }
enum Purpose { PURPOSE_UNSPECIFIED = 0; PURPOSE_DECISION = 1; PURPOSE_GENERATION = 2; PURPOSE_REVIEW = 3; PURPOSE_EXECUTION = 4;
               PURPOSE_INDEXING = 5; PURPOSE_EVALUATION = 6; PURPOSE_PUBLICATION = 7; }
message Instructions { string id = 1; repeated T.ArtifactRef documents = 2; string scope = 3; }
message PreferenceCandidate { string id = 1; PreferenceScope scope = 2; string statement = 3; repeated string evidence = 4; }
message PreferenceDecision { string candidate = 1; PreferenceAction action = 2; /* activate|reject|suspend|supersede|withdraw */ string reason = 3; }
message PreferenceApplication { string preference = 1; string task = 2; Disposition disposition = 3;
                                /* applied|inapplicable|conflicting|expired|unavailable */ }
// An exact action, fully bound, so an approval covers this action and nothing else.
message Action { option (openagents.options.v1.contract) = "openagents.action.v1";
                 string task_frame = 1; T.ArtifactRef snapshot = 2; string operation = 3; string binding = 4; T.ArtifactRef input = 5;
                 optional T.ContextManifest context = 6; T.Effects effects = 7; T.Bounds bounds = 8; Recipient recipient = 9;
                 repeated string preconditions = 10; bytes nonce = 11; }
message ApprovalRequest { T.Digest action_digest = 1; Action action = 2; int64 expires_at = 3; }
message ApprovalDecision { T.Digest action_digest = 1; bool approve = 2; T.Pubkey approver = 3; int64 decided_at = 4; }
message Route { RoutePurpose purpose = 1; /* decision|generation|review|execution|evaluation */ Recipient recipient = 2; string model = 3; }
message RouteUsage { option (openagents.options.v1.contract) = "openagents.route-usage.v1"; Route route = 1; T.Usage usage = 2; }
```

### 19.3 `openagents.capability.v1`

**Purpose.** Portable operation descriptions, host bindings, grants,
presence, operator preferences and service contracts (NIP-CAP). This is
the package that tells a caller what a host can do and on what terms.

**Status.** Partial.

```proto
package openagents.capability.v1;

message Definition {
  string id = 1; Profile profile = 2; /* native | executor | plugin | adapter | service */
  string summary = 3; T.ArtifactRef input = 4; T.ArtifactRef output = 5;
  T.Effects effects = 6; T.Bounds minimum = 7; Support support = 8; T.ArtifactRef binding_contract = 9;
  optional Transport transport = 10; optional Remote remote = 11;
}
message Transport { string kind = 1; /* subprocess | acp | http | nostr-cj | mcp */ string endpoint = 2; }
message Remote { T.Pubkey worker = 1; repeated string relays = 2 [(openagents.options.v1.limits) = {min_count: 1, max_count: 8}];
                 string endpoint = 3; string identity = 4; }
message Support {
  repeated BoundSupportEntry bounds = 1;         // per-bound enforced | not_enforced | unknown
  Cancellation cancellation = 2;                 // before_dispatch | cooperative | host_terminated | unsupported
  Idempotency idempotency = 3;                   // none | request_attempt | pure
  repeated string evidence = 4;
}
message DiscoveryHead { T.DefinitionRef definition = 1; repeated string tags = 2; /* oa:cap:v1, oa:profile:*, oa:transport:* */ bool test = 3; }

// openagents.grant.v1
message Grant {
  option (openagents.options.v1.contract) = "openagents.grant.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_GRANT;
  T.Pubkey issuer = 1; string principal = 2; string binding = 3;
  T.Effects effects = 4; T.Bounds bounds = 5; string scope = 6; repeated string recipients = 7;
  GrantPurpose purpose = 8;                      // evaluation | operational
  optional T.ArtifactRef basis = 9;              // CTX selection receipt
  int64 expires_at = 10;
}
message Presence { string binding = 1; PresenceState state = 2; /* present|absent|unavailable|unprobed|unknown */ int64 observed_at = 3; }
message Preference { repeated string prefer = 1; repeated string deny = 2; T.Bounds ceilings = 3;
                     Assurance assurance = 4; /* host_enforced | trusted_contract */ optional openagents.policy.v1.DisclosurePolicy disclosure_policy = 5; }
message PreferenceHead { oneof body { Preference public = 1; bytes encrypted = 2; /* self-encrypted, random d */ } }

// Decision-service contract a door advertises (references openagents.systemone.v1).
message ServiceContract {
  repeated Lane lanes = 1; repeated DoorDescriptor doors = 2; ServiceLimits limits = 3; ContractVersions versions = 4;
}
message Lane { oneof lane { HttpLane http = 1; CjLane nostr_cj = 2; } }
message HttpLane { string endpoint = 1; string call = 2; repeated string models = 3; }
message CjLane { T.Pubkey worker = 1; repeated string relays = 2; uint32 request_kind = 3; uint32 result_kind = 4; uint32 feedback_kind = 5; }
message DoorDescriptor { string name = 1; string model = 2; string artifact_signature = 3; string version = 4;
                         repeated openagents.decision.v1.CalibrationSet sets = 5; }
```

**The operation registry.** Every CAP or CJ operation that a NIP defines
without its own kind (CTX `context.build`, CTX `context.expand`, COORD
`claim`, ENV `lease`, LIVE `start`, OPT `study`, and so on) is registered in
`capability.v1/operations.md` with its input and output message types.
The generator checks that each registered operation names real messages.
This is how a stranger finds out what a `nostr-cj` worker accepts without
reading every NIP.

## 20. Programs, extensions, skills and knowledge

### 20.1 `openagents.program.v1`

**Purpose.** Typed workflows with seven step kinds, a binding language,
Wasm modules, and the plugin packet ABI (NIP-PRG). Workflow runs
(`wrun_`) are the HTTP view.

**Status.** Partial.

```proto
package openagents.program.v1;

message Program {
  string id = 1; string summary = 2; T.ArtifactRef input = 3; T.ArtifactRef output = 4;
  repeated Step steps = 5 [(openagents.options.v1.limits) = {max_count: 256}];
  Binding result = 6; T.Bounds bounds = 7;
}
message Step {
  string name = 1; StepKind kind = 2; T.DefinitionRef target = 3; repeated string after = 4;
  Binding input = 5; optional T.ArtifactRef output = 6; T.Bounds bounds = 7;
  OnError on_error = 8; /* stop | continue */ optional When when = 9; optional Retry retry = 10;
}
enum StepKind { STEP_KIND_UNSPECIFIED = 0; STEP_KIND_QUERY = 1; STEP_KIND_CHECK = 2; STEP_KIND_DECIDE = 3;
                STEP_KIND_DELEGATE = 4; STEP_KIND_PROGRAM = 5; STEP_KIND_MODULE = 6; STEP_KIND_INVOKE = 7; }
// Binding language: {literal} | {from, pointer} | {object} | {array}
message Binding { oneof b { bytes literal = 1; /* exact JSON */ FromPointer from = 2; BindingObject object = 3; BindingArray array = 4; } }
message FromPointer { string from = 1; string pointer = 2; /* RFC 6901 */ }
message When { string source = 1; oneof test { bytes equals = 2; bool present = 3; } }
message Retry { uint32 max_attempts = 1; repeated string retry_on = 2; }
message StepEnvelope { oneof state { T.Outcome outcome = 1; bool skipped = 2; } optional T.ArtifactRef value = 3;
                       optional openagents.receipt.v1.ExecutionReceipt receipt = 4; T.Verification verification = 5; T.Integration integration = 6; }
message ProgramHead { T.DefinitionRef definition = 1; }                       // kind 30182
message ModuleAnnouncement { string plugin = 1; T.ArtifactRef module = 2; }   // kind 30183

// openagents.plugin-packet.v1: the Wasm guest ABI.
message PacketRequest { string invocation = 1; string operation = 2; T.ArtifactRef input = 3; repeated Handle handles = 4; }
message PacketResponse { PacketStatus status = 1; /* ok | unsupported_input | refused */ optional T.ArtifactRef value = 2; optional string reason = 3; }
// Guest exports memory, oa_alloc, oa_free, oa_handle. Host import oa_host.call(metadata|list|read);
// error codes -1..-6. The ABI is byte-level; these messages are its typed request and response.
message HostCall { HostOp op = 1; string handle = 2; uint64 offset = 3; uint32 length = 4; }

message WorkflowRun { string id = 1; /* wrun_ */ T.DefinitionRef program = 2; T.Outcome outcome = 3;
                      repeated StepEnvelope steps = 4; int64 started_at = 5; optional int64 ended_at = 6; }
message BuildReceipt { option (openagents.options.v1.contract) = "openagents.plugin-build-receipt.v1";
                       T.ArtifactRef source = 1; T.ArtifactRef module = 2; string toolchain = 3; T.Digest digest = 4; }
message InvocationReceipt { string invocation = 1; T.ArtifactRef module = 2; T.Outcome outcome = 3; uint64 fuel = 4; uint64 wall_ms = 5; }
```

### 20.2 `openagents.extension.v1`

**Purpose.** Immutable component packages with signed provenance:
releases, listings, revocations, checkpoints, namespace migrations,
foreign imports and host component sets (NIP-EXT). Curated registries
(NIP-REG) are EXT catalog packages, so they live here too.

**Status.** Partial; the registry publisher, mirror and reader are
implemented in `plugin-registry`.

```proto
package openagents.extension.v1;

message Manifest {
  string package = 1; string version = 2; string license = 3; Provenance provenance = 4;
  repeated Component components = 5; repeated PackageFile files = 6; repeated Dependency dependencies = 7;
}
message Component { string slug = 1; ComponentKind kind = 2; T.DefinitionRef definition = 3; T.ArtifactRef descriptor = 4; }
enum ComponentKind { COMPONENT_KIND_UNSPECIFIED = 0; COMPONENT_KIND_PROGRAM = 1; COMPONENT_KIND_PLUGIN = 2; COMPONENT_KIND_CAPABILITY = 3;
  COMPONENT_KIND_DECISION_FUNCTION = 4; COMPONENT_KIND_SKILL = 5; COMPONENT_KIND_SOURCE = 6; COMPONENT_KIND_CHECKER = 7;
  COMPONENT_KIND_OPERATION = 8; COMPONENT_KIND_AI_SIGNATURE = 9; COMPONENT_KIND_AI_IMPLEMENTATION = 10; COMPONENT_KIND_GUIDANCE = 11;
  COMPONENT_KIND_SCHEMA = 12; COMPONENT_KIND_EVAL_SUITE = 13; }
message PackageFile { string path = 1; T.Digest digest = 2; uint64 size = 3; string media_type = 4; }
message Release { string package = 1; string version = 2; T.ArtifactRef manifest = 3;
                  optional uint64 fee_msat = 4; optional Payout payout = 5; }           // kind 3184
message Payout { oneof target { string spark = 1; string lightning_address = 2; bytes node_key = 3; /* 33 bytes */ } }
message Listing { string package = 1; ListingState state = 2; /* published | hidden */ T.EventRef release = 3; string title = 4; string description = 5; }
message Revocation { string package = 1; T.EventRef release = 2; string reason = 3; int64 effective_at = 4; }
message Checkpoint { string package = 1; uint64 revision = 2; int64 as_of = 3; int64 valid_until = 4; repeated T.EventRef revocations = 5; }
message Migration { string from = 1; string to = 2; string migration = 3; MigrationRole role = 4; /* offer | accept */ optional T.EventRef offer = 5; }
message OperationDescriptor { string id = 1; string kind = 2; T.DefinitionRef definition = 3; string summary = 4; T.ArtifactRef input = 5;
                              T.ArtifactRef output = 6; repeated string preconditions = 7; T.Effects effects = 8; T.Bounds minimum = 9; }
message DecisionFunction { string id = 1; T.ArtifactRef input = 2; T.ArtifactRef output = 3; T.ArtifactRef state_builder = 4;
                           repeated openagents.decision.v1.NamedQuestion questions = 5; T.ArtifactRef policy = 6;
                           ModelRequirements model_requirements = 7; T.Bounds bounds = 8; }
message ComponentSet { string set = 1; T.Pubkey issuer = 2; repeated SetSlot components = 3 [(openagents.options.v1.limits) = {max_count: 256}]; T.ArtifactRef policy = 4; }
message SetSlot { string slot = 1; SlotRole role = 2; /* client|engine|adapter|helper|image|extension */ T.ArtifactRef artifact = 3; T.EventRef release = 4; }

// NIP-REG: openagents.registry.v1
message Registry { option (openagents.options.v1.contract) = "openagents.registry.v1";
                   string registry = 1; uint64 revision = 2; int64 as_of = 3; int64 valid_until = 4;
                   string title = 5; string description = 6; repeated RegistryEntry entries = 7 [(openagents.options.v1.limits) = {max_count: 1000}]; }
message RegistryEntry { string package = 1; T.EventRef release = 2; T.ArtifactRef manifest = 3; string title = 4; string summary = 5;
                        optional T.ArtifactRef configuration = 6; repeated T.EventRef evaluation = 7; }
```

Client plugin packages, coder defaults locks (`openagents.coder-defaults-lock.v1`)
and plugin build receipts all resolve to `Manifest`, `ComponentSet` and
`program.v1.BuildReceipt`.

### 20.3 `openagents.skill.v1`

**Purpose.** Directory-signed listings of reviewed `SKILL.md` documents
(NIP-SKILL), the skill directory, and skill bodies with hooks (EXT
`skill` component).

**Status.** Implemented (`profiles::skills` on openagents.com).

```proto
package openagents.skill.v1;
message Listing {                                     // kind 30188; content ≤16,384 B; no `v`
  string name = 1 [(openagents.options.v1.limits) = {min_bytes: 1, max_bytes: 64}];
  string version = 2; T.Digest digest = 3; string url = 4; string license = 5; string category = 6; repeated string tags = 7;
  optional T.Pubkey author = 8; bool withdrawn = 9;
}
message Skill {                                       // EXT skill component
  string id = 1; string description = 2; T.ArtifactRef body = 3; string applicability = 4;
  repeated string allowed_operations = 5; Lifetime lifetime = 6; /* operation | task | session */ repeated Hook hooks = 7;
}
message Hook { HookEvent event = 1; /* context.requested | operation.completed | task.closed */ string operation = 2;
               T.ArtifactRef input = 3; T.Bounds bounds = 4; string on_error = 5; string destination = 6; }
message DirectoryEntry { string id = 1; /* skill_ */ Listing listing = 2; repeated T.EventRef reviews = 3; /* NIP-32 labels */ }
```

### 20.4 `openagents.knowledge.v1`

**Purpose.** A shared knowledge base of immutable entry versions, heads
and withdrawals (NIP-KB). Evidence about entries is published through
EVAL.

**Status.** Implemented (`crates/nostr::kb`, `microcoder kb`).

```proto
package openagents.knowledge.v1;
message Entry { string id = 1; /* kb_id grammar */ uint32 version = 2; EntryKind kind = 3;
                string document = 4; /* YAML front matter + Markdown; x = SHA256 */ repeated string topics = 5; }
enum EntryKind { ENTRY_KIND_UNSPECIFIED = 0; ENTRY_KIND_METHOD = 1; ENTRY_KIND_EDGE_CASE = 2; ENTRY_KIND_SLIP = 3;
                 ENTRY_KIND_ENVIRONMENT = 4; ENTRY_KIND_TOOL = 5; }
message Head { string id = 1; uint32 version = 2; T.EventRef entry = 3; }
message Withdrawal { string id = 1; uint32 version = 2; T.EventRef entry = 3; string reason = 4 [(openagents.options.v1.limits) = {max_bytes: 1000}]; }
message Equivocation { string id = 1; uint32 version = 2; repeated T.EventRef conflicting = 3; }   // must be reported
```

## 21. Evaluation, Gym, optimization and traces

### 21.1 `openagents.eval.v1`

**Purpose.** Workload evaluation: suites, cases, reports, publications,
admissions, and the extension evaluation profile with gates (NIP-EVAL).

**Sources.** NIP-EVAL; `openagents.eval-case.v1`, `eval-draft.v1`,
`ext-eval.v1`, `ext-eval-hosted`, `eval-admission.v1`,
`coder-defaults-lock`; glossary "checks/oracles", "benchmarks, arms, run
cards", "grader kinds", "ext-eval-v2 gate", "gates", "verdicts".

**Status.** Partial (`eval_ext` wire formats implemented).

```proto
package openagents.eval.v1;

message Suite { string id = 1; SuitePurpose purpose = 2; /* decision|context|routing|operation|agent */
                string workload = 3; repeated CaseRef cases = 4; string partition = 5; repeated string labels = 6;
                repeated Metric metrics = 7; Acceptance acceptance = 8; T.ArtifactRef environment = 9; }
message Case {
  option (openagents.options.v1.contract) = "openagents.eval-case.v1";
  string id = 1; CaseKind kind = 2; /* should-fire | should-not-fire */ uint32 runs = 3; /* 1–10 */
  string prompt = 4; T.ArtifactRef config = 5;
  repeated Grader graders = 6 [(openagents.options.v1.limits) = {max_count: 64}];
  repeated T.ArtifactRef fixtures = 7 [(openagents.options.v1.limits) = {max_count: 256}];
}
message Grader { string kind = 1; /* exact, regex, json-schema, checker, decision, human, ... */ T.ArtifactRef definition = 2; }
message Report {
  string suite = 1; string partition = 2; Subject subject = 3; optional Subject baseline = 4; T.Pubkey evaluator = 5;
  int64 started_at = 6; int64 ended_at = 7; uint32 runs = 8; CoveragePair coverage = 9;
  repeated Measurement measurements = 10; T.Verdict verdict = 11; repeated string limitations = 12;
}
message Subject { T.DefinitionRef definition = 1; optional T.Lock lock = 2; optional T.ArtifactRef configuration = 3; }
message RunCoverage { uint32 planned = 1; uint32 attempted = 2; uint32 completed = 3; uint32 refused = 4; uint32 failed = 5;
                      uint32 cancelled = 6; uint32 unknown = 7; uint32 excluded = 8; }
message Measurement { string arm = 1; string metric = 2; double value = 3; uint32 denominator = 4; uint32 unknown_count = 5;
                      optional Interval uncertainty = 6; repeated string evidence = 7; }
message Publication { T.ArtifactRef report = 1; Subject subject = 2; repeated T.EventRef supersedes = 3;
                      optional ExtEvalMeta ext_eval = 4; }                                  // kind 3189
message ExtEvalMeta { string gate = 1; uint32 cases = 2; Headline headline = 3; T.Pubkey requester = 4; Reliance reliance = 5;
                      IdentityBasis identity = 6; /* content|version|endpoint|unresolved */ string distribution = 7; }
message Admission { option (openagents.options.v1.contract) = "openagents.eval-admission.v1";
                    Subject subject = 1; repeated T.EventRef reports = 2; T.ArtifactRef policy = 3; string scope = 4;
                    AdmissionDecision decision = 5; /* admit|reject|inconclusive */ T.Pubkey issuer = 6; int64 expires_at = 7;
                    Stakes stakes = 8; /* severity, authority, reversibility */ }
message EvalDraft { option (openagents.options.v1.contract) = "openagents.eval-draft.v1";
                    repeated Case cases = 1 [(openagents.options.v1.limits) = {max_count: 16}]; }
```

Evidence expiry by identity basis (content 365 days, version 90,
endpoint 14) is a validator rule on `Admission`.

### 21.2 `openagents.gym.v1`

**Purpose.** Gym: suites, boards, leaderboards and their index, results
publications (kind 3195), grants and connections for running other
people's agents, request and reply, recipes, eval rows. The Gym building
in the Verse shows these as views (Chapter V `verse.gym.v1`).

**Sources.** NIP-EVAL Gym profile; 119 `openagents.gym.*` IDs; 24
`openagents.recipe.*` IDs; `gym eval_row v3`; glossary Gym ids (109).

**Status.** Implemented.

```proto
package openagents.gym.v1;
message GymSuite { option (openagents.options.v1.contract) = "openagents.gym.suite.v1"; string id = 1; openagents.eval.v1.Suite suite = 2; string title = 3; }
message Leaderboard { option (openagents.options.v1.contract) = "openagents.gym.leaderboard.v1";
                      string board = 1; /* ogb_ over HTTP */ string suite = 2; repeated Standing standings = 3; T.Digest digest = 4; bytes signature = 5; }
message Standing { uint32 rank = 1; openagents.eval.v1.Subject subject = 2; double score = 3; uint32 runs = 4; T.EventRef report = 5; }
message LeaderboardIndex { repeated BoardSummary boards = 1; }
message ResultsPublication { T.Digest digest = 1; string commit = 2;                          // kind 3195
                             repeated T.ArtifactRef boards = 3 [(openagents.options.v1.limits) = {min_count: 1, max_count: 64}]; }
message GymGrant { T.Pubkey holder = 1; uint64 wall_ms = 2; /* 1–86.4M */ uint32 starts = 3; /* 1–32 */ int64 expires_at = 4; /* ≤30 days */ }
message GymConnection { string connect = 1; /* "gym-connect:" + base64url */ T.Pubkey runner = 2; repeated string relays = 3; }
message GymRequest { string suite = 1; openagents.eval.v1.Subject subject = 2; repeated string recipes = 3; }
message GymReply { repeated EvalRow runs = 1 [(openagents.options.v1.limits) = {max_count: 64}];
                   repeated string recipes = 2 [(openagents.options.v1.limits) = {max_count: 16}]; }   // ≤128 KiB
message EvalRow { option (openagents.options.v1.contract) = "openagents.gym.eval-row.v3";
                  string case = 1; uint32 run = 2; string arm = 3; T.Verdict verdict = 4; double score = 5; T.Usage usage = 6; optional T.ArtifactRef trajectory = 7; }
message Recipe { option (openagents.options.v1.contract) = "openagents.recipe.v1";
                 string id = 1; /* content-moderation, lead-qualification, prompt-injection, ... */ string title = 2;
                 openagents.extension.v1.DecisionFunction function = 3; repeated string suites = 4; }
```

### 21.3 `openagents.optimize.v1`

**Purpose.** Semantic AI signatures, immutable implementations, bounded
optimization studies, trials and results (NIP-OPT). Tenant training and
candidate admission use these records.

**Status.** Designed. Related implemented contracts: `tenant_training.*`,
`openagents.optimization.*`, `coder-one.study-candidate.v1`.

```proto
package openagents.optimize.v1;
message Signature { string id = 1; T.ArtifactRef input = 2; T.ArtifactRef output = 3; string task = 4; repeated string invariants = 5;
                    T.Effects effects = 6; string evaluation = 7; }
message Implementation { string id = 1; string signature = 2; EntryKind entry_kind = 3; /* decision-function|program|operation */
                         T.DefinitionRef entry = 4; Target target = 5; T.ArtifactRef build = 6; T.Effects effects = 7; T.Bounds bounds = 8; }
message Target { string runtime = 1; repeated ModelSlot models = 2; T.ArtifactRef environment = 3; }
message ModelSlot { ModelRole role = 1; /* student|judge|reflection */ string component = 2; string requested = 3; string identity = 4; string assurance = 5; }
message Study { string study = 1; T.Pubkey owner = 2; string signature = 3; string baseline = 4; Space space = 5; DataPlan data = 6;
                string suite = 7; string objective = 8; string acceptance = 9; Optimizer optimizer = 10; T.Bounds bounds = 11;
                openagents.policy.v1.DisclosurePolicy disclosure = 12; int64 retain_until = 13; Confirmation confirmation = 14; }
message Space { repeated Slot slots = 1; }
message Slot { string id = 1; string component = 2; Surface surface = 3; repeated T.ArtifactRef values = 4; }
enum Surface { SURFACE_UNSPECIFIED = 0; SURFACE_INSTRUCTIONS = 1; SURFACE_DEMONSTRATIONS = 2; SURFACE_QUESTIONS = 3; SURFACE_INFERENCE = 4;
               SURFACE_MODEL = 5; SURFACE_PARAMETERS = 6; SURFACE_COMPOSITION = 7; SURFACE_SOURCE = 8; }
message Candidate { string id = 1; string study = 2; repeated SlotValue assignment = 3; T.ArtifactRef implementation = 4; }
message Trial { string candidate = 1; Phase phase = 2; /* search|selection|confirmation */ T.EventRef report = 3; repeated CategoryUsage usage = 4; }
message Result { string study = 1; ResultStatus status = 2; optional string winner = 3; repeated T.EventRef reports = 4; }
message Promotion { string candidate = 1; T.ArtifactRef admission = 2; int64 promoted_at = 3; }
```

### 21.4 `openagents.trace.v1`

**Purpose.** Agent trajectories in Harbor ATIF (v1.8 canonical, v1.0–1.7
readable), and their NIP-ATIF carriage: manifests, chunks, parent and
child links, derivations.

**Status.** Implemented (`crates/atif-nostr`, `tr_` HTTP resources).

```proto
package openagents.trace.v1;

// ATIF mirror. JSON is authoritative; this is its typed form.
message Trajectory { string schema_version = 1; string session_id = 2; Agent agent = 3; repeated Step steps = 4; optional FinalMetrics final_metrics = 5; }
message Step { string step_id = 1; string source = 2; /* user|agent|system */ optional string message = 3; repeated ToolCall tool_calls = 4;
               optional Observation observation = 5; optional Metrics metrics = 6; int64 timestamp_ms = 7; }
message ToolCall { string tool_call_id = 1; string function_name = 2; bytes arguments = 3; /* exact JSON */ }

enum DigestRule { DIGEST_RULE_UNSPECIFIED = 0; DIGEST_RULE_JCS = 1; DIGEST_RULE_ATIF_SORTED = 2; DIGEST_RULE_BYTES = 3; }

message Manifest {                                        // kind 3198
  option (openagents.options.v1.contract) = "openagents.atif-manifest.v1";
  string trajectory_id = 1; string session_id = 2; string schema_version = 3; Form form = 4; /* log | document */
  T.ArtifactRef artifact = 5; string steps_digest = 6; /* 64 hex, ATIF rule */ uint32 step_count = 7;
  State state = 8; /* ended | interrupted */ Coverage coverage = 9; /* complete | partial */
  optional Derivation derivation = 10; /* redacted | truncated | converted, from source */
  repeated ChunkRef chunks = 11; optional TaskLink task = 12; optional string run = 13; optional Selector selector = 14;
  optional ParentLink parent = 15; repeated ChildLink children = 16; optional PreviousLink previous = 17;
}
message Chunk { option (openagents.options.v1.contract) = "openagents.atif-chunk.v1";    // kind 3199; JCS ≤65,535 B
                string trajectory_id = 1; T.ArtifactRef artifact = 2; uint32 index = 3; uint32 count = 4; /* 1–4096 */
                T.Digest digest = 5; string text = 6 [(openagents.options.v1.limits) = {min_bytes: 1, max_bytes: 32768}]; }
```

Block AO, AM, AE and PMA records and NIP-29 kind 9 messages map onto ATIF
steps. The mapping table lives in the package README.

## 22. Durable work: runs, coordination, automation, work items

### 22.1 `openagents.run.v1`

**Purpose.** Encrypted durable run journals with controller fencing,
handoff and replay (NIP-RUN).

**Status.** Implemented.

```proto
package openagents.run.v1;
message RunRecord {
  option (openagents.options.v1.contract) = "openagents.run-record.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_RUNTIME;
  string run = 1; uint64 seq = 2; optional T.Digest previous = 3; T.Pubkey controller = 4; uint64 generation = 5;
  RecordType type = 6; Subject subject = 7; /* {step, iteration, attempt} */ int64 time = 8;
  oneof data { Created created = 10; Admitted admitted = 11; Dispatched dispatched = 12; Observed observed = 13;
               Resolved resolved = 14; CancelRequested cancel_requested = 15; Unknown unknown = 16; Reconciled reconciled = 17;
               Handoff handoff = 18; Settled settled = 19; }
  repeated BlockRef block_refs = 20;                // Block AO/AM/AE refs; durable flag; kind 24200 refused as durable
}
// digest = SHA256("openagents.run-record.v1\n" || JCS(record))
message Created { openagents.capability.v1.Grant grant = 1; Baseline baseline = 2; }
message Baseline { T.Lock lock = 1; repeated string recipients = 2; optional T.ArtifactRef instructions = 3; string selector = 4;
                   T.ArtifactRef sampling = 5; string runtime = 6; repeated string unpinned = 7; }
message Admitted { T.Reach reach = 1; AdmissionKind admission = 2; /* evaluation | operational */ }
message Handoff { T.Pubkey from = 1; T.Pubkey to = 2; uint64 generation = 3; }
message RunRecordEnvelope { bytes ciphertext = 1; T.Digest digest = 2; }    // kind 3187, tags p, h, t:oa:run:v1
message RunHead { string run = 1; uint64 seq = 2; T.Digest digest = 3; uint64 generation = 4; T.EventRef record = 5;
                  uint64 retained_from = 6; int64 retain_until = 7; }       // kind 30186, d = mailbox
```

State machine: `created → admitted → dispatched → resolved → settled`,
with `cancel_requested`, `unknown`, `reconciled` and `handoff` as side
transitions. A fork at one sequence number halts the run. The bounded
model and its counterexample tests live with the run host per
`INVARIANTS.md`.

### 22.2 `openagents.coordination.v1`

**Purpose.** Task proposals, fenced resource claims, shared budgets,
background plans, findings and integration (NIP-COORD).

**Status.** Partial (`coordination-host`); schema bundle
`coordination.v1.json` (17 defs) maps one-to-one.

```proto
package openagents.coordination.v1;
message Coordinator { string scope = 1; T.Pubkey controller = 2; uint64 epoch = 3; string run = 4; string capability = 5; T.ArtifactRef policy = 6; repeated string resources = 7; }
message TaskProposal { string task = 1; string coordinator = 2; string frame = 3; optional string parent = 4; T.DefinitionRef target = 5; T.Lock lock = 6;
                       T.ArtifactRef input = 7; optional T.ContextManifest context = 8; repeated string dependencies = 9; T.Effects effects = 10;
                       T.Bounds bounds = 11; string acceptance = 12; Reuse reuse = 13; /* never | exact; key = JCS hash of the 9 fields */ }
message ClaimRequest { string proposal = 1; T.Pubkey worker = 2; repeated ResourceClaim resources = 3; uint64 expected_revision = 4;
                       int64 requested_until = 5; optional string reservation = 6; }
message ResourceClaim { string id = 1; ClaimMode mode = 2; /* read | write */ string snapshot = 3; }
message Claim { ClaimRequest request = 1; uint64 token = 2; /* monotone fencing token */ uint64 epoch = 3; int64 expires_at = 4; uint64 revision = 5; }
message Control { ControlAction action = 1; /* renew | release | cancel | status */ string claim = 2; uint64 expected_revision = 3; }
message CoordinationResult { string request = 1; string coordinator = 2; uint64 revision = 3; T.AdmissionStatus status = 4;
                             optional T.EventRef record = 5; optional T.ArtifactRef value = 6; optional T.Refusal reason = 7; }
message Finding { FindingKind kind = 1; /* explanation|review|proposal|test_proposal|patch_proposal|evaluation|fact */ string task = 2; T.ArtifactRef body = 3; }
message BackgroundPlan { Trigger trigger = 1; /* snapshot_changed|operation_completed|task_closed */ Lifetime lifetime = 2; Destination destination = 3; StalePolicy stale = 4; }
message DuplicateSuggestion { string task = 1; string duplicate_of = 2; double confidence = 3; }
```

### 22.3 `openagents.automation.v1`

**Purpose.** Durable scheduled, source-triggered and continuation
automation with bounded occurrences (NIP-AUTO). Managed-agent
deployments and sovereign activations run on these plans.

**Status.** Partial (`automation-host`; effectful session and environment
bindings are refused today).

```proto
package openagents.automation.v1;
message Plan {
  string automation = 1; string run = 2; T.Pubkey owner = 3; T.Pubkey controller = 4; string coordinator = 5; string frame = 6;
  T.ArtifactRef policy = 7; optional string reservation = 8; string task = 9; T.DefinitionRef target = 10; T.Lock lock = 11;
  T.ArtifactRef payload = 12; optional T.ContextManifest context = 13; T.ArtifactRef requirements = 14; string acceptance = 15;
  T.Effects effects = 16; T.Bounds per_occurrence = 17; T.Bounds aggregate = 18; uint32 max_occurrences = 19; uint32 max_observations = 20;
  int64 starts_at = 21; int64 expires_at = 22; int64 retain_until = 23; /* starts_at < expires_at < retain_until */
  Trigger trigger = 24; Completion completion = 25;
}
message Trigger { oneof kind { Once once = 1; Interval interval = 2; SourceTrigger source = 3; Continuation continuation = 4; } }
message Once { int64 at = 1; }
message Interval { int64 first_at = 1; uint32 every_seconds = 2; Missed missed = 3; /* skip | latest; fixed UTC; no cron */ }
message SourceTrigger { string source = 1; string resource = 2; uint32 max_age_seconds = 3; }
message Continuation { uint32 minimum_interval_seconds = 1; }
message Occurrence { string automation = 1; OccurrenceKey key = 2; int64 at = 3; string run = 4; }
message OccurrenceKey { oneof k { bool once = 1; uint64 slot = 2; SourceRevision source = 3; string predecessor = 4; } }
message Evaluation { string occurrence = 1; EvalDecision decision = 2; /* continue|complete|blocked|unknown */ }
message Control { ControlAction action = 1; /* pause|resume|cancel|status */ uint64 expected_revision = 2; }
message State { AutomationStatus status = 1; /* active|paused|blocked|exhausted|complete|cancelled|expired */ uint64 revision = 2; uint32 occurrences = 3; }
message Skipped { uint64 from_slot = 1; uint64 to_slot = 2; SkipReason reason = 3; /* missed|occupied|paused|expired|bound */ }
```

### 22.4 `openagents.work.v1`

**Purpose.** Tracked work items and planning relations, revision
admission, delegation, evidence, disposition, source imports (GitHub
issues and others) and attention (NIP-WORK). The `work_` HTTP resource is
its view. V1 project tracking, Foreman claims and Agent Lab templates read
and write these items.

**Status.** Partial: `work-host` roles are implemented. The NIP text
still says "proposed protocol, not an implemented issue tracker", which
§32.5 flags as stale.

```proto
package openagents.work.v1;
message WorkRef { string authority = 1; string scope = 2; string item = 3; }
message Item {
  ItemKind kind = 1; /* work|initiative|project|milestone|cycle|decision */
  string title = 2 [(openagents.options.v1.limits) = {max_bytes: 512}]; string body = 3; T.Pubkey accountable = 4;
  ItemState state = 5; /* open|in_progress|blocked|resolved|cancelled|superseded|archived */
  optional string frame = 6; string acceptance = 7; optional Delegation delegation = 8;
  repeated Relation relations = 9; repeated string resources = 10; repeated T.Evidence evidence = 11; optional Disposition disposition = 12;
  repeated string labels = 13 [(openagents.options.v1.limits) = {max_count: 32}]; optional int64 due_at = 14; optional Timebox timebox = 15;
}
message Relation { RelationType type = 1; /* parent|blocks|related|duplicate|supersedes */ WorkRef target = 2; repeated string evidence = 3; }
message Command { T.Pubkey issuer = 1; string command_id = 2; string scope = 3; WorkRef work = 4; CommandAction action = 5; /* create|replace */
                  uint64 expected = 6; Item snapshot = 7; optional Source source = 8; string reason = 9; int64 expires_at = 10; }
message Revision { T.Pubkey issuer = 1; WorkRef work = 2; uint64 revision = 3; optional T.Digest previous = 4; string command = 5; Item snapshot = 6; int64 recorded_at = 7; }
message Source { string system = 1; /* github, linear, nostr-nip34 */ string id = 2; string url = 3; string revision = 4; }
message Attention { WorkRef work = 1; AttentionKind kind = 2; T.Pubkey who = 3; int64 at = 4; }
message ExecutionGrant { option (openagents.options.v1.contract) = "openagents.work-execution-grant.v1";
                         WorkRef work = 1; openagents.capability.v1.Grant grant = 2; }
```

## 23. Places to run: workspaces, environments, hosts, reach, terminals, live

### 23.1 `openagents.workspace.v1`

**Purpose.** Workspace bindings, resources, conditional document changes,
worktrees, checkpoints and restore, finite projections, and activity
summaries (NIP-WS).

**Status.** Partial (`workspace-host`, the Git document owner). Bundle
`workspace.v1.json` (29 defs) maps one-to-one, including the projection
family (`projection-definition`, `-cut`, `-read`, `-result`, `-page`,
`-delta`, `-marker`, `-change-page`, `-command-visibility`).

```proto
package openagents.workspace.v1;
message ResourceRef { string authority = 1; string workspace = 2; string resource = 3; }
message Resource { ResourceRef ref = 1; ResourceKind kind = 2; /* collection|document|repository|worktree|artifact */ string revision = 3; }
message DocumentReplacement { ResourceRef document = 1; string expected_revision = 2; T.ArtifactRef content = 3; }
message Checkpoint { string id = 1; repeated ResourceRef resources = 2; CaptureMode capture = 3; /* atomic|version_vector|observational */ T.ArtifactRef manifest = 4; }
message ProjectionDefinition { string id = 1; string query = 2; repeated string fields = 3; }
message ProjectionCut { string projection = 1; Consistency consistency = 2; /* single_authority|federated */ repeated VersionEntry versions = 3; }
message ProjectionRead { string projection = 1; ReadMode mode = 2; /* snapshot|changes */ optional T.Cursor after = 3; uint32 max_items = 4; /* 1–256 */ }
message ProjectionDelta { DeltaAction action = 1; /* upsert|remove */ string key = 2; bytes row = 3; }
message ProjectionMarker { Marker marker = 1; /* caught_up | resnapshot_required */ }
message ActivitySummary {                              // ≤1,024 B, no meta, retained ≤7 days
  T.Pubkey host = 1; SubjectRef subject = 2; /* task | session */ uint64 sequence = 3; string phase = 4;
  string headline = 5 [(openagents.options.v1.limits) = {min_bytes: 1, max_bytes: 160}];
  AttentionNeed attention = 6; /* none|approval|input|completed|failed */ int64 updated_at = 7;
}
```

### 23.2 `openagents.environment.v1`

**Purpose.** Environment definitions, leases, exact materialization,
participant admission, one execution attachment per lease, and cleanup
(NIP-ENV). The `env_`/`envk_` HTTP resources and the ~52 environment IDs
in the product are this package's views.

**Status.** Partial (`env-host`; SSH hosts via `coder-ssh`; sponsored GCE
placement specified).

```proto
package openagents.environment.v1;
message Definition { repeated EnvComponent components = 1; T.ArtifactRef workspace = 2; T.ArtifactRef bootstrap = 3; }
message EnvComponent { string id = 1; ComponentRole role = 2; /* base|runtime|model|toolchain|data */ T.ArtifactRef artifact = 3; }
message Request { string request = 1; T.DefinitionRef definition = 2; Requirements requirements = 3; T.Bounds bounds = 4; optional string reservation = 5; }
message Lease { string lease = 1; string request = 2; T.Pubkey allocator = 3; int64 created_at = 4; int64 not_after = 5; int64 retain_until = 6; optional string reservation = 7; }
message Materialization { string lease = 1; T.Digest definition_digest = 2; repeated T.ArtifactRef applied = 3; Enforcement enforcement = 4; }
message State { string lease = 1; EnvStatus status = 2; /* provisioning|ready|reserved|attached|draining|released|failed|unknown */ uint64 revision = 3; }
message Attach { string lease = 1; string run = 2; T.Pubkey executor = 3; }
message Attachment { string lease = 1; string run = 2; uint64 fencing = 3; AttachmentState state = 4; }
message Cleanup { string lease = 1; CleanupAction action = 2; /* destroy|detach */ CleanupStatus status = 3; /* pending|confirmed|failed|unknown */ }
message Binding { option (openagents.options.v1.contract) = "openagents.environment-binding.v1"; string lease = 1; T.Digest materialization = 2; }
message HttpEnvironment { string id = 1; /* env_ */ string name = 2; Definition definition = 3; repeated EnvKey keys = 4; }
message EnvKey { string id = 1; /* envk_ */ string name = 2; bool set = 3; }   // values never returned
```

### 23.3 `openagents.host.v1`

**Purpose.** Host-wide device enrollment, scoped rights, revocation
epochs, delegation, and the typed operation set every Coder client uses
to drive a host (NIP-HOST). After the 2026-10-16 decision, HOST is also
the authority for live session rooms.

**Status.** Implemented (`coder-access`, `coder-host`). Designed only: QR
connect codes, iroh enrollment, the local operator socket, nearby
approval, account key link by QR.

```proto
package openagents.host.v1;

enum Right {   // canonical order
  RIGHT_UNSPECIFIED = 0; RIGHT_OBSERVE = 1; RIGHT_OPERATE = 2; RIGHT_TERMINAL = 3; RIGHT_REVIEW = 4;
  RIGHT_ACCESS_READ = 5; RIGHT_ACCESS_ADMIN = 6; RIGHT_WORLD = 7;
}
message HostGrant {
  option (openagents.options.v1.contract) = "openagents.host-grant.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_GRANT;
  string grant = 1; T.Pubkey host = 2; T.Pubkey owner = 3; T.Pubkey device = 4; string relay = 5;
  repeated Right rights = 6; uint64 epoch = 7; Origin origin = 8; /* invitation | approval */
  int64 issued_at = 9; int64 expires_at = 10;           // ≤30 days; renew when ≤¼ remains
}
message HostRequest {
  option (openagents.options.v1.max_encoded_bytes) = 131072;
  string request = 1; T.Pubkey host = 2; string grant = 3; uint64 epoch = 4; string relay = 5;
  int64 issued_at = 6; int64 expires_at = 7;           // ≤60 s
  Operation op = 8;
}
message HostReply { string request = 1; T.EventRef request_event = 2; T.Pubkey host = 3; int64 issued_at = 4; int64 expires_at = 5;
                    oneof result { OperationResult ok = 6; HostRefusal refused = 7; } }
message HostRefusal { string code = 1; repeated Right missing = 2; }

// ~60 operation kinds, grouped. Each arm has its own request and result message.
message Operation {
  oneof op {
    // enroll.*, invite.*, device.*
    EnrollStart enroll_start = 10; InviteCreate invite_create = 11; DeviceList device_list = 12; DeviceRevoke device_revoke = 13;
    // task.*
    TaskCreate task_create = 20; TaskList task_list = 21; TaskRead task_read = 22; TaskOriginal task_original = 23;
    TaskSteer task_steer = 24; TaskCancel task_cancel = 25; TaskArchive task_archive = 26; TaskCommand task_command = 27;
    TaskQueue task_queue = 28; TerminalOpen task_terminal_open = 29;
    // project.*, cloud.*, workspace.*, computer
    ProjectList project_list = 40; CloudOp cloud = 41; WorkspaceList workspace_list = 42; ComputerOp computer = 43; ComputerEnrolled computer_enrolled = 44;
    // terminals
    openagents.terminal.v1.Open terminal_open = 50;
    // spend.*, chats, threads
    SpendList spend_list = 60; SpendSettle spend_settle = 61; ChatsInvite chats_invite = 62;
    ThreadList thread_list = 63; ThreadRead thread_read = 64; ThreadSend thread_send = 65; ThreadStop thread_stop = 66; ThreadRun thread_run = 67;
    // studio.* (Agent Studio, ~15), verse.private, robot.lab, request.operation
    StudioOp studio = 70; VersePrivate verse_private = 71; RobotLab robot_lab = 72; RequestOperation request_operation = 73;
    // app.rpc: EngineInfo, ListHarnesses, ListModels, ListSkills, ListCommands, AuthorizeSync, RelayCommand, RunCommand (≤60 KiB)
    AppRpc app_rpc = 80;
  }
}

// coder-host: invitation; packed bytes, base64url; ≤640 B; expiry = issue + 300 s.
message Invitation { uint32 version = 1; T.Pubkey host = 2; bytes id = 3; bytes capability = 4; uint64 issued_at = 5; uint64 expires_at = 6; string relay = 7; }
message SpendGrant { option (openagents.options.v1.contract) = "openagents.spend-grant.v1"; string grant = 1; T.Amount ceiling = 2; int64 expires_at = 3; }
message ConnectSas { option (openagents.options.v1.contract) = "openagents.connect-sas.v1"; bytes transcript_digest = 1; string words = 2; }
```

Limits: bodies ≤128 KiB; task outcomes ≤48 KiB with steps over 16 KiB
turned into gaps; originals ≤64 MiB in 16 KiB chunks; direct-channel
messages ≤256 KiB; `app.rpc` ≤60 KiB.

### 23.4 `openagents.reach.v1`

**Purpose.** Owner host directory, host presence, reachability hints,
authenticated direct channels and deterministic placement (NIP-REACH).

**Status.** Implemented (`coder-reach`, `coder-host`, TCP and
WebSocket). The iroh amendment is designed only.

```proto
package openagents.reach.v1;
message Directory { T.Pubkey owner = 1; uint64 revision = 2; int64 issued_at = 3; repeated DirectoryHost hosts = 4 [(openagents.options.v1.limits) = {max_count: 256}]; }
message DirectoryHost { T.Pubkey host = 1; string label = 2; repeated string relays = 3; /* ≤8 */ uint32 weight = 4; /* 0–1000 */ int64 added_at = 5; repeated string worlds = 6; /* ≤16 */ }
message Presence { T.Pubkey host = 1; T.Pubkey owner = 2; uint64 generation = 3; string protocol = 4; Compatibility compatibility = 5;
                   repeated string capabilities = 6; int64 observed_at = 7; optional Telemetry telemetry = 8; }
message Telemetry { uint32 cpu_count = 1; double cpu_utilization_pct = 2; double memory_available_pct = 3; }
message Hints { T.Pubkey host = 1; uint64 generation = 2; int64 issued_at = 3; int64 expires_at = 4; /* ≤24 h */ repeated Hint hints = 5; /* ≤16 */ }
message Hint { HintClass class = 1; /* loopback|lan|tailnet|public|relay */ HintTransport transport = 2; /* tcp|websocket|nostr|iroh */
               string address = 3; string status = 4; int64 observed_at = 5; }

// Frame: u32 length | u8 kind | u64 seq | body. ≤65,536 B; data ≤16,384 B; handshake JSON ≤4,096 B.
enum FrameKind { FRAME_KIND_UNSPECIFIED = 0; FRAME_KIND_HELLO = 1; FRAME_KIND_HOST_PROOF = 2; FRAME_KIND_CLIENT_PROOF = 3;
                 FRAME_KIND_VERDICT = 4; FRAME_KIND_REFUSAL = 5; FRAME_KIND_DATA = 16; FRAME_KIND_CLOSE = 17; }
message Hello { bytes ephemeral = 1; T.Pubkey client = 2; string protocol = 3; }
message HostProof { bytes ephemeral = 1; bytes sig = 2; }
message ClientProof { bytes sig = 1; string grant = 2; }
message Verdict { bool accepted = 1; optional string code = 2; }
// Session keys from ephemeral keys + transcript; "openagents.reach-channel.v1" label.
message PlacementInput { repeated DirectoryHost hosts = 1; repeated Presence presence = 2; }
// score = weight × cpu_count × (100 − util) × mem_avail%; refuse > 90 % CPU or < 10 % memory.
```

### 23.5 `openagents.terminal.v1`

**Purpose.** Interactive terminal sessions with sequenced output, bounded
replay and gaps, and the six extensions: snapshot, blocks, sessions,
proposals, effects, typist and shares (NIP-TERM).

**Status.** Implemented (`coder-pty`, `coder-host`). 52 terminal and
native-terminal IDs map here or to `evidence.v1`.

```proto
package openagents.terminal.v1;
message Open { string workspace = 1; string dir = 2 [(openagents.options.v1.limits) = {max_bytes: 1024}];
               oneof launch { Shell shell = 3; Command command = 4; } Size size = 5; repeated EnvVar env = 6 [(openagents.options.v1.limits) = {max_count: 64}]; }
message Command { string program = 1; repeated string args = 2 [(openagents.options.v1.limits) = {max_count: 256}]; }
message Size { uint32 rows = 1; uint32 cols = 2; }          // 1–1024
message Attach { TerminalRef terminal = 1; AttachMode mode = 2; /* interact | observe */ optional uint64 after = 3; optional uint32 rate = 4; }
message TerminalRef { uint64 generation = 1; string terminal = 2; }
message Input { TerminalRef terminal = 1; bytes data = 2 [(openagents.options.v1.limits) = {min_bytes: 1, max_bytes: 4096}]; }
message Resize { TerminalRef terminal = 1; Size size = 2; }
message Signal { TerminalRef terminal = 1; SignalKind signal = 2; /* interrupt|quit|terminate|hangup|kill */ }
message Frame { oneof body { Output output = 1; Exit exit = 2; Gap gap = 3; Detached detached = 4; } }
message Output { uint64 seq = 1; bytes data = 2 [(openagents.options.v1.limits) = {max_bytes: 8192}]; }
message Exit { uint64 seq = 1; ExitCause cause = 2; /* exited|closed|idle_expired|host_shutdown */ optional int32 code = 3; optional string signal = 4; }
message Gap { uint64 from = 1; uint64 to = 2; uint64 bytes = 3; }
// Record stream: 10-byte header (u16 tag, u32 length ≤1 MiB, u32 crc32c). Tags 1 TERMINAL … 7 FINISH. ≤16 MiB.
enum RecordTag { RECORD_TAG_UNSPECIFIED = 0; RECORD_TAG_TERMINAL = 1; RECORD_TAG_STATE = 2; RECORD_TAG_ROWS = 3;
                 RECORD_TAG_CONTINUATION = 4; RECORD_TAG_READY = 5; RECORD_TAG_HISTORY = 6; RECORD_TAG_FINISH = 7; }
// Extensions: Proposal{offer, read, decide}, Take/Release (typist), Share{grant, pause, unshare, viewers, handoff}, AgentInput, BlockPage, SessionList/Read/Write/Remove.
```

### 23.6 `openagents.live.v1`

**Purpose.** Voice, camera, screen, browser and device interaction bound
to admitted participants, consent, floors, captures and transcripts
(NIP-LIVE), plus the livehost text and media mailbox records.

**Status.** Partial (native text profiles in `live-host`). Audio, video
and device adapters are out of the NIP's scope; LiveKit and other media
stacks are adapters named by ID. Bundle `live.v1.json` (32 defs) maps
one-to-one.

```proto
package openagents.live.v1;
message Plan { string session = 1; string task = 2; optional string previous = 3; uint64 expected_revision = 4; string frame = 5; string conversation = 6;
               T.Pubkey owner = 7; T.Pubkey controller = 8; uint64 generation = 9; uint64 media_epoch = 10; string transport = 11;
               repeated Track tracks = 12 [(openagents.options.v1.limits) = {max_count: 32}]; openagents.policy.v1.DisclosurePolicy disclosure = 13;
               ConsentPolicy consent_policy = 14; optional string reservation = 15; T.Bounds bounds = 16; int64 expires_at = 17; }
message Track { string id = 1; T.ResourceScope source = 2; Media media = 3; /* audio|image|video|text */ Direction direction = 4; /* capture|playback */
                repeated string recipients = 5; string retention = 6; string processing = 7; }
message Participant { T.Pubkey key = 1; repeated ParticipantRight rights = 2; /* publish|subscribe|transcribe|record|playback */ }
message Consent { T.Pubkey participant = 1; repeated string tracks = 2; int64 given_at = 3; optional int64 withdrawn_at = 4; }
message Floor { string holder = 1; uint64 epoch = 2; int64 expires_at = 3; }
message Capture { string track = 1; CapturePurpose purpose = 2; /* task_input|observation|verification|recording */ T.ArtifactRef content = 3; }
message Transcript { string track = 1; repeated Segment segments = 2; }
message InputOutcome { string input = 1; Outcome outcome = 2; /* applied|refused|unknown */ }
message LivehostOperation { option (openagents.options.v1.contract) = "openagents.livehost-operation.v1"; /* kind 30440 pending registration */ }
```

## 24. Cloud, Computers and compute

### 24.1 `openagents.cloud.v1`

**Purpose.** OpenAgents Cloud as a product: retail packages, customers,
storage, quotes, budgets, enrollment, pools (the GCE pool), host bindings,
provider-key custody and cloud terminal render receipts.

**Sources.** 37 `openagents.cloud.*` IDs; `docs/cloud/`; the
`openagents cloud up` and `cloud budget` commands (default $300 over 24
hours, $10 a run, 8 hosts, 10 idle minutes); owner rule "No hosted Boat"
(sandboxes on our own GCP behind the Boat API shape).

**Status.** Implemented.

```proto
package openagents.cloud.v1;
message RetailPackage { string id = 1; string title = 2; MachineClass class = 3; T.Amount price_per_hour = 4; repeated string regions = 5; }
message MachineClass { string family = 1; uint32 vcpu = 2; uint32 memory_gb = 3; optional string accelerator = 4; uint32 accelerator_count = 5; }
message Quote { string id = 1; RetailPackage package = 2; uint32 hours = 3; T.Amount total = 4; int64 expires_at = 5; }
message Budget { string scope = 1; /* owner or agent id */ T.Amount daily = 2; T.Amount per_run = 3; uint32 max_hosts = 4; uint32 idle_minutes = 5; T.Amount spent = 6; T.Window window = 7; }
message Pool { string id = 1; repeated PoolHost hosts = 2; Budget budget = 3; }
message PoolHost { string name = 1; string zone = 2; MachineClass class = 3; HostState state = 4; int64 idle_since = 5; optional string run = 6; }
message Enrollment { string host = 1; T.Pubkey key = 2; string node_ca = 3; int64 enrolled_at = 4; }
message HostBinding { string host = 1; string workspace_id = 2; repeated string capabilities = 3; }
message ProviderKeyCustody { option (openagents.options.v1.contract) = "openagents.cloud.provider-key-custody.v2";
                             string provider_key = 1; openagents.credential.v1.CustodyMode custody = 2; string kms_key = 3; }
```

### 24.2 `openagents.computers.v1`

**Purpose.** The Computers API (`oa-computers`): computers, commands,
files, credential requests and delivery, a credential proxy, the node CA,
capabilities, lifecycle and usage.

**Sources.** `docs/sdk/openapi.computers.json`; 43 `openagents.computers.*`
IDs, of which the `*.evidence.v1` and `*-verification.v1` ones go to
`evidence.v1`.

**Status.** Implemented.

```proto
package openagents.computers.v1;
message Computer { string id = 1; string name = 2; ComputerState state = 3; /* provisioning|running|stopped|deleting|deleted|failed */
                   openagents.cloud.v1.MachineClass class = 4; string region = 5; repeated string capabilities = 6; int64 created_at = 7; }
message CommandRequest { string computer = 1; repeated string argv = 2; optional string cwd = 3; uint32 timeout_s = 4; optional string idempotency_key = 5; }
message CommandResult { string command = 1; int32 exit_code = 2; T.ArtifactRef stdout = 3; T.ArtifactRef stderr = 4; uint64 wall_ms = 5; }
message FileEntry { string path = 1; uint64 bytes = 2; T.Digest digest = 3; int64 modified_at = 4; }
message CredentialRequest { string computer = 1; string credential = 2; string purpose = 3; int64 expires_at = 4; }
message CredentialDelivery { string request = 1; DeliveryMode mode = 2; /* proxy | env | file */ int64 delivered_at = 3; }
message Usage { string computer = 1; T.Window window = 2; uint64 cpu_seconds = 3; uint64 gb_hours = 4; T.Amount cost = 5; }
service ComputersService {
  rpc CreateComputer(CreateComputerRequest) returns (Computer);
  rpc RunCommand(CommandRequest) returns (CommandResult);
  rpc ListFiles(ListFilesRequest) returns (ListFilesReply);
  rpc RequestCredential(CredentialRequest) returns (CredentialDelivery);
  rpc DeleteComputer(DeleteComputerRequest) returns (Computer);
}
```

### 24.3 `openagents.compute.v1`

**Purpose.** Compute as money and placement: compute balance, route
contracts, placement decisions, and leases across Cloud, Computers, Pylon
and owned hosts.

**Sources.** Glossary "compute balance", "route-contract placement and
leases", "placement formula"; `openagents.compute_quota_routing.v1`;
`openagents.route.*` (21 IDs: admission snapshots, eval splits,
qualification profiles, studio results, offers, funded HTTP requests).

**Status.** Partial.

```proto
package openagents.compute.v1;
message Balance { string account_id = 1; T.Amount available = 2; T.Amount reserved = 3; int64 as_of = 4; }
message RouteContract { string id = 1; string capability = 2; repeated RouteOption options = 3; T.Bounds bounds = 4; T.Amount ceiling = 5; }
message RouteOption { oneof target { string cloud_package = 1; T.Pubkey pylon = 2; string computer = 3; T.Pubkey host = 4; } T.Amount price_hint = 5; uint32 weight = 6; }
message Placement { string contract = 1; RouteOption chosen = 2; repeated RouteOption rejected = 3; string reason = 4; int64 decided_at = 5; }
message ComputeLease { string id = 1; string contract = 2; Placement placement = 3; int64 expires_at = 4; T.Amount reserved = 5; }
message RouteRecord { option (openagents.options.v1.contract) = "openagents.route.record.v1"; string route = 1; Placement placement = 2; T.Usage usage = 3; }
```

## 25. Coder

Coder is the largest product family (216 IDs). Most of its runtime
contracts already come from NIPs (HOST, REACH, TERM, SESS, WORK, WS, ENV,
ATIF), and those packages own them. The `coder.*` packages hold what is
Coder-specific.

### 25.1 `openagents.coder.task.v1`

**Purpose.** The task store (v1 through v3), task records, frozen
requirements, execution grants, policy, permits and boundaries.

**Sources.** Glossary "task owner", "execution grant", "frozen
requirements", "engine session", "turn intent", "projection cut",
"checkpoint", "policy `openagents.coder-one.policy.v1`", "permit",
"boundary"; `openagents.coder.*`, `openagents.task.*` (23).

```proto
package openagents.coder.task.v1;
message Task { string id = 1; T.Pubkey owner = 2; string title = 3; TaskState state = 4; openagents.work.v1.WorkRef work = 5;
               FrozenRequirements requirements = 6; repeated string runs = 7; optional string pull_request = 8; /* pr_ */ int64 created_at = 9; }
message FrozenRequirements { T.Digest digest = 1; repeated string acceptance = 2; T.ArtifactRef context = 3; int64 frozen_at = 4; }
message ExecutionGrant { string task = 1; openagents.capability.v1.Grant grant = 2; string engine = 3; string model = 4; }
message Policy { option (openagents.options.v1.contract) = "openagents.coder-one.policy.v1";
                 repeated string allowed_operations = 1; T.Effects effects = 2; repeated Boundary boundaries = 3; }
message Boundary { string kind = 1; /* path, network, process, spend */ repeated string allow = 2; repeated string deny = 3; }
message Permit { string task = 1; string operation = 2; T.Digest action_digest = 3; int64 expires_at = 4; }
message Run { string id = 1; /* run_ */ string task = 2; string engine = 3; string model = 4; T.Outcome outcome = 5; T.ArtifactRef trajectory = 6; T.Usage usage = 7; }
```

### 25.2 `openagents.coder.host.v1`

Host descriptor (`openagents.coder.host-descriptor.v1`), host runtime
(`openagents.coder.host-runtime.v1`), SSH-managed hosts
(`openagents.coder.ssh-managed.v1`), Block PL push leases, and the `connect-*`
enrollment family (`connect-enroll-*`, `connect-nearby-*`, `connect-ssh*`,
`connect-sas`).

```proto
package openagents.coder.host.v1;
message HostDescriptor { T.Pubkey host = 1; string label = 2; string os = 3; string arch = 4; repeated Harness harnesses = 5;
                         repeated string models = 6; repeated string capabilities = 7; string version = 8; }
message Harness { string id = 1; /* codex, claude-code, cursor-agent, opencode */ string version = 2; bool signed_in = 3; }
message SshManagedHost { string host = 1; string address = 2; string user = 3; T.Digest host_key = 4; }
message PushLease { T.Pubkey device = 1; string token_ref = 2; int64 expires_at = 3; }   // Block PL, kind 30350
```

### 25.3 `openagents.coder.sync.v1`

**Purpose.** The `coder-sync` service (sync.openagents.com): Loro rooms
for live sessions and chats, room membership, frontiers, materialized
digests, and chat2 frames.

```proto
package openagents.coder.sync.v1;
message Room { string id = 1; RoomKind kind = 2; /* session | chat | task */ repeated Member members = 3; string authority = 4; /* host grant */ }
message Member { T.Pubkey key = 1; repeated openagents.host.v1.Right rights = 2; }
message Frontier { string room = 1; bytes loro_frontier = 2; T.Digest materialized = 3; uint64 version = 4; }
message Chat2Frame { string room = 1; uint64 seq = 2; oneof body { bytes loro_update = 3; Presence presence = 4; Ack ack = 5; } }
message AuthorizeSync { string room = 1; string grant = 2; }      // HOST app.rpc AuthorizeSync
```

### 25.4 `openagents.coder.router.v1`

Chat router v1 through v3: tiers T0–T4, routes, offers, the decision call
(kind 25910 `openagents.decision-call.v1`), the answer bank.

```proto
package openagents.coder.router.v1;
enum Tier { TIER_UNSPECIFIED = 0; TIER_T0 = 1; TIER_T1 = 2; TIER_T2 = 3; TIER_T3 = 4; TIER_T4 = 5; }
message RouteDecision { Tier tier = 1; string route = 2; double route_p = 3; string bank = 4; openagents.decision.v1.DecisionCall call = 5; }
message RouterConfig { uint32 version = 1; repeated RouteRule routes = 2; }
```

### 25.5 `openagents.coder.foreman.v1` and `openagents.coder.lab.v1`

Foreman (the overseer of issues and agents): claims, chat joins,
delegations, dispatch targets (`--on gce`, `--on archil`), model split
log. Agent Lab and the Labs API (`docs/sdk/openapi.labs.json`): labs,
templates, lab runs.

```proto
package openagents.coder.foreman.v1;
message Claim { openagents.work.v1.WorkRef work = 1; string chat = 2; T.Pubkey agent = 3; int64 claimed_at = 4; }
message Delegation { string id = 1; openagents.work.v1.WorkRef work = 2; DispatchTarget target = 3; string harness = 4; string model = 5; T.Outcome outcome = 6; }
enum DispatchTarget { DISPATCH_TARGET_UNSPECIFIED = 0; DISPATCH_TARGET_GCE = 1; DISPATCH_TARGET_AGENT_HOST = 2; DISPATCH_TARGET_ARCHIL = 3; DISPATCH_TARGET_MAC = 4; }

package openagents.coder.lab.v1;
message Lab { string id = 1; /* lab_ */ string template = 2; string title = 3; repeated string chats = 4; }
message LabRun { string id = 1; string lab = 2; T.Outcome outcome = 3; T.ArtifactRef evidence = 4; }
```

The `coder-one` research family (109 IDs: check truth, oracle labels,
study candidates, tuned tasks, briefings, executor events) is evidence,
not protocol. It goes to `evidence.v1` (§30.2), except
`coder-one.policy.v1`, which is above.

## 26. Money: markets, labor, payments, ledgers, billing

### 26.1 The money rule

Owner decision, 2026-10-09 (`docs/payments/agent-payments.md`): Bitcoin
and Bitcoin-based stablecoins only. That means Lightning, on-chain
Bitcoin, Spark, and Taproot Assets stablecoins. There is no USDC on EVM,
Base, Solana or Tempo, and no Cashu for now. The protocol enforces this in
three places:

1. `type.v1.Amount` has only `msat` and `AssetAmount` (Taproot Assets).
2. `payment.v1.Rail` and `payment.v1.Protocol` list only allowed rails.
3. The generator refuses any `openagents.*` import of `nostr.nip60`,
   `nostr.nip61` or `nostr.nip87` (Cashu and Fedimint mirrors). Those
   packages exist for reading and refusing only.

**Conflict to resolve.** `nips/openagents/schemas/payment-receipt.v1.json`
lists `rail ∈ {lightning, evm, solana, tempo, card, ecash}` and
`protocol ∈ {x402, mpp, l402, cashu, acp, ucp, ap2, stripe-checkout, zap}`.
No NIP references it. This spec does not carry those values forward:
`payment.v1.PaymentReceipt` is `lightning | onchain | spark | taproot_asset
| card`, where `card` is fiat billing through Stripe for subscriptions
(§26.6), which is not an agent payment rail. The protocol list is `x402 |
l402 | zap | bolt11 | bolt12 | stripe_checkout`. The JSON Schema file is
marked superseded in phase P1 (§36).

### 26.2 `openagents.market.v1`

**Purpose.** Immutable offerings, private negotiation, orders,
cancellation and fixed-price Bitcoin settlement (NIP-MKT).

**Status.** Partial (`crates/coder-labor`).

```proto
package openagents.market.v1;
message Offering { T.Pubkey provider = 1; string offer = 2; string capability = 3; repeated string profiles = 4; repeated PaymentProfile payment_profiles = 5;
                   repeated T.BitcoinNetwork networks = 6; /* bitcoin|testnet|regtest */ string summary = 7 [(openagents.options.v1.limits) = {max_bytes: 2048}];
                   optional uint64 price_hint_msat = 8; optional uint32 capacity_hint = 9; int64 valid_until = 10; }    // kind 3192
message OfferingHead { T.Pubkey provider = 1; string offer = 2; T.EventRef offering = 3; HeadStatus status = 4; /* active|paused|withdrawn */ int64 valid_until = 5; }
enum PaymentProfile { PAYMENT_PROFILE_UNSPECIFIED = 0; PAYMENT_PROFILE_FREE_V1 = 1; PAYMENT_PROFILE_LIGHTNING_BOLT11_FIXED_POSTACCEPTANCE_V1 = 2; }

// Sequenced private record chain on 3188.
message Record {
  RecordType type = 1; string market = 2; T.Pubkey buyer = 3; T.Pubkey provider = 4; T.Pubkey issuer = 5;
  uint64 seq = 6; optional T.Digest prev = 7; int64 issued_at = 8;
  oneof body {
    Rfq rfq = 10; Quote quote = 11; Order order = 12; OrderAck order_ack = 13; ProfileRecord profile = 14;
    StatusRequest status_request = 15; Status status = 16; CancelRecord cancel = 17; CancelAck cancel_ack = 18; Dispute dispute = 19;
    PaymentInstruction payment_instruction = 20; PaymentAttempt payment_attempt = 21; PaymentResult payment_result = 22;
    PaymentReceived payment_received = 23; RefundOffer refund_offer = 24; RefundAccept refund_accept = 25; Close close = 26; CloseAck close_ack = 27;
  }
}
message Terms { string profile = 1; T.ArtifactRef profile_terms = 2; T.Pubkey buyer = 3; T.Pubkey provider = 4; T.Pubkey worker = 5;
                uint64 price_msat = 6; uint64 fee_limit_msat = 7; PaymentProfile payment_profile = 8; T.BitcoinNetwork network = 9;
                int64 quote_expires_at = 10; int64 order_confirm_by = 11; int64 delivery_due_at = 12; int64 review_due_at = 13;
                int64 payment_due_at = 14; int64 retain_until = 15; }   // quote_expires_at ≤ order_confirm_by < delivery < review < payment < retain
message OrderRef { string market = 1; string order_id = 2; T.Pubkey buyer = 3; T.Pubkey provider = 4; T.Digest order = 5; T.Digest confirmation = 6; }
message PaymentInstruction { string obligation = 1; string bolt11 = 2 [(openagents.options.v1.limits) = {max_bytes: 8192}]; }
```

### 26.3 `openagents.labor.v1`

**Purpose.** The agent-labor profile on MKT: frozen terms, execution
linkage, deliverables, independent verification, review and rework,
disputes, acceptance and rights (NIP-LAB, `openagents.labor.v1`).

**Status.** Partial (`crates/coder-labor`). It is distinct from historical
NIP-LBR and claims no compatibility with it.

```proto
package openagents.labor.v1;
message Terms {
  string task_frame = 1; ExecutionTerms execution = 2; repeated Deliverable deliverables = 3; T.Pubkey reviewer = 4;
  AcceptancePolicy acceptance_policy = 5; T.Pubkey resolver = 6; string resolver_policy = 7; /* "labor-evidence-v1" */
  uint32 max_reworks = 8; /* 0–3 */ int64 rework_due_at = 9; int64 dispute_due_at = 10; int64 resolution_due_at = 11;
  string cancellation = 12; /* evaluate-delivered-work-v1 */ string partial_delivery = 13; /* no-partial-payment-v1 */
  string buyer_unavailable = 14; /* resolver-required-v1 */ Rights rights = 15; repeated RoleRelationship role_relationships = 16;
}
message ExecutionTerms { T.DefinitionRef target = 1; T.Lock lock = 2; T.ArtifactRef input = 3; optional T.ContextManifest context = 4; T.ArtifactRef requirements = 5; T.Bounds bounds = 6; }
message Deliverable { string id = 1; T.ArtifactRef schema = 2; uint64 max_bytes = 3; }
message AcceptancePolicy { T.DefinitionRef checker = 1; T.Lock lock = 2; repeated string criteria = 3; string rule = 4; /* all-pass-v1 */ }
message Submission { openagents.market.v1.OrderRef order = 1; repeated T.ArtifactRef deliverables = 2; T.EventRef run = 3; }
message Verification { openagents.market.v1.OrderRef order = 1; T.Verification result = 2; repeated T.Evidence evidence = 3; }
message Review { ReviewDecision decision = 1; /* accept|request_rework|reject */ string notes = 2; }
message Dispute { DisputeCause cause = 1; /* non_delivery|verification|review|cancellation|buyer_unavailable|conflict */ string statement = 2; }
message Resolution { ResolutionDecision decision = 1; /* accept|reject|unresolved */ ResolutionBasis basis = 2; }
message Acceptance { AcceptanceOutcome outcome = 1; /* accepted|rejected|unresolved */ AcceptanceBasis basis = 2; uint64 amount_due_msat = 3; }
message Rights { string input_use = 1; string output_use = 2; string publication = 3; string training = 4; string evaluation_reuse = 5; }
message LaborBinding { option (openagents.options.v1.contract) = "openagents.labor-binding.v1"; openagents.market.v1.OrderRef order = 1; }
```

### 26.4 `openagents.x402.v1`

**Purpose.** Lightning-paid single operations bought before execution:
standard x402 v2 over HTTP and MCP, plus the opt-in native Nostr binding
(NIP-X402).

**Status.** Implemented (`crates/x402`, durable proof consumption and
recovery). The upstream baseline is x402 at `4fcf836c…`.

```proto
package openagents.x402.v1;

// Upstream-exact x402 v2 objects for scheme "exact" on network lnbtc.
message PaymentRequired { uint32 x402_version = 1; repeated PaymentRequirements accepts = 2; optional string error = 3; }
message PaymentRequirements { string scheme = 1; /* exact */ string network = 2; /* lnbtc:000000000019d6689c085ae165831e93 */
                              string max_amount_required = 3; /* msat decimal */ string resource = 4; string description = 5;
                              string mime_type = 6; string pay_to = 7; uint32 max_timeout_seconds = 8; string asset = 9; /* BTC */
                              ExtraLightning extra = 10; }
message ExtraLightning { string invoice = 1; /* bolt11 */ string payment_hash = 2; }
message PaymentPayload { uint32 x402_version = 1; string scheme = 2; string network = 3; LightningPayload payload = 4; }
message LightningPayload { string preimage = 1 [(openagents.options.v1.sensitivity) = SENSITIVITY_SECRET]; string payment_hash = 2; }
message SettleResponse { bool success = 1; optional string error_reason = 2; string transaction = 3; string network = 4; string payer = 5; }

// CAP feature oa-x402-v1
message Discovery { string protocol = 1; /* x402-v2 */ string scheme = 2; string asset = 3; string method = 4; /* bolt11 */
                    string flow = 5; /* upfront */ repeated string bindings = 6; /* http:1, mcp:1, nostr:openagents:1 */
                    repeated Receiver receivers = 7; string merchant = 8; Recovery recovery = 9; optional T.ArtifactRef recovery_contract = 10; }
message Receiver { string network = 1; string pay_to = 2; }

// Native Nostr binding records (OpenAgents extension, not an upstream profile).
message NativeRecord { RecordType type = 1; string purchase = 2; T.Pubkey buyer = 3; T.Pubkey provider = 4; T.Pubkey issuer = 5; int64 issued_at = 6;
                       oneof body { Request request = 10; Challenge challenge = 11; ClaimRecord claim = 12; ClaimRejected claim_rejected = 13;
                                    StatusQuery status_query = 14; StatusRecord status = 15; } }
message Request { string capability = 1; string operation = 2; T.ArtifactRef input = 3; optional T.ContextManifest context = 4; string account = 5;
                  uint64 ceiling_msat = 6; uint64 fee_ceiling_msat = 7; int64 valid_until = 8; int64 execute_until = 9; int64 recover_until = 10; }
message StatusRecord { uint64 seq = 1; optional T.Digest prev = 2; string claim = 3; Phase phase = 4;
                       /* offered → claim_pending → admitted → running → completed | failed | refused | unknown */
                       optional SettleResponse settlement = 5; optional string run = 6; optional T.ArtifactRef output = 7; optional string cause = 8; int64 recover_until = 9; }
```

The CAIP network strings and `T.BitcoinNetwork` map one to one: mainnet
`lnbtc:000000000019d6689c085ae165831e93` = `BITCOIN_NETWORK_MAINNET`,
testnet `lnbtc:000000000933ea01ad0ee984209779ba` = `BITCOIN_NETWORK_TESTNET`,
and signet only through the research extension `openagents:signet:1`.

### 26.5 `openagents.payment.v1`

**Purpose.** OpenAgents' own payment records, which every backend (LND,
LDK, Spark/Breez, MDK, Taproot Assets) produces the same way: payment
evidence, spend grants and receipts, wallet views, Lightning addresses,
and payment receipts. It references Lightning objects by identity and
never embeds `lnrpc`.

**Sources.** NIP-MKT payment evidence; HOST `spend-grant`,
`spend-request`, `spend-receipt`, `spend-wake`; pay-host; the MDK
treasury bridge; owner rule "Bitcoin-only rails".

**Status.** Partial.

```proto
package openagents.payment.v1;

enum Rail { RAIL_UNSPECIFIED = 0; RAIL_LIGHTNING = 1; RAIL_ONCHAIN = 2; RAIL_SPARK = 3; RAIL_TAPROOT_ASSET = 4; RAIL_CARD = 5; /* Stripe subscriptions only */ }
enum Protocol { PROTOCOL_UNSPECIFIED = 0; PROTOCOL_BOLT11 = 1; PROTOCOL_BOLT12 = 2; PROTOCOL_X402 = 3; PROTOCOL_L402 = 4; PROTOCOL_ZAP = 5;
                PROTOCOL_LNURL = 6; PROTOCOL_STRIPE_CHECKOUT = 7; }

message PaymentRef { Rail rail = 1; oneof id { bytes payment_hash = 2; string outpoint = 3; string spark_transfer = 4; bytes asset_transfer = 5; string stripe = 6; } }

// NIP-MKT payment evidence; generalized to every rail.
message PaymentEvidence {
  option (openagents.options.v1.contract) = "openagents.market-payment-evidence.v1";
  option (openagents.options.v1.data_class) = DATA_CLASS_EVIDENCE;
  string order = 1; string obligation = 2; string instruction = 3; T.Pubkey observer = 4; string adapter = 5; /* lnd, ldk, spark, mdk, tapd */
  T.BitcoinNetwork network = 6; PaymentRef payment = 7; EvidenceState state = 8; /* confirmed|failed|unknown */
  T.Amount amount = 9; T.Amount fees = 10; int64 observed_at = 11; T.ArtifactRef source = 12;
}
message PaymentReceipt { option (openagents.options.v1.contract) = "openagents.payment-receipt.v2";   // supersedes v1 (§26.1)
                         PaymentRef payment = 1; Protocol protocol = 2; T.Amount amount = 3; T.Pubkey payer = 4; T.Pubkey payee = 5; int64 settled_at = 6; }
message SpendGrant { string grant = 1; T.Pubkey holder = 2; T.Amount ceiling = 3; T.Amount per_payment = 4; int64 expires_at = 5; repeated string purposes = 6; }
message SpendRequest { string grant = 1; T.Amount amount = 2; string purpose = 3; string invoice = 4; }
message SpendReceipt { string request = 1; PaymentRef payment = 2; T.Outcome outcome = 3; T.Amount spent = 4; }
message LightningAddress { string address = 1; /* name@openagents.com */ T.Pubkey nostr = 2; string lnurl_callback = 3; uint64 min_sendable_msat = 4; uint64 max_sendable_msat = 5; }
message WalletView { string wallet = 1; Rail rail = 2; T.Amount balance = 3; T.Amount pending = 4; int64 as_of = 5; }
message Invoice { string bolt11 = 1; bytes payment_hash = 2; T.Amount amount = 3; int64 expires_at = 4; string description = 5; }
```

### 26.6 `openagents.ledger.v1` and `openagents.billing.v1`

**`ledger.v1`.** The msat ledger (pay-ledger), earnings, statements and
compute balance entries. Double-entry, append-only, with every entry
pointing at the receipt that caused it.

```proto
package openagents.ledger.v1;
message Entry { string id = 1; string account = 2; T.Amount amount = 3; Direction direction = 4; /* debit | credit */
                EntryCause cause = 5; T.Digest receipt = 6; int64 posted_at = 7; string transaction = 8; }
message EntryCause { oneof cause { string usage = 1; openagents.payment.v1.PaymentRef payment = 2; string payout = 3; string refund = 4; string grant = 5; string adjustment = 6; } }
message Statement { string account = 1; T.Window window = 2; T.Amount opening = 3; T.Amount closing = 4; repeated Entry entries = 5; T.Digest digest = 6; }
message Earnings { T.Pubkey earner = 1; T.Window window = 2; T.Amount earned = 3; T.Amount paid_out = 4; repeated string sources = 5; }
```

**`billing.v1`.** Fiat subscriptions (Stripe), plans, entitlements, price
terms, invoices, settlement modes, and the provider-event journal.

```proto
package openagents.billing.v1;
message Plan { string id = 1; string name = 2; PriceTerms price = 3; repeated Entitlement entitlements = 4; }
message PriceTerms { oneof price { uint64 usd_cents_per_month = 1; T.Amount msat_per_month = 2; } repeated MeteredPrice metered = 3; }
message MeteredPrice { string meter = 1; T.MicroAmount per_unit = 2; }
message Subscription { string id = 1; string account_id = 2; string plan = 3; SubscriptionState state = 4; int64 current_period_end = 5; }
message Entitlement { string feature = 1; uint64 limit = 2; T.Window window = 3; }
message ProviderEvent { string provider = 1; /* stripe */ string event_id = 2; string type = 3; int64 received_at = 4; T.Digest body_digest = 5; ProcessingState state = 6; }
enum SettlementMode { SETTLEMENT_MODE_UNSPECIFIED = 0; SETTLEMENT_MODE_PREPAID = 1; SETTLEMENT_MODE_POSTPAID = 2; SETTLEMENT_MODE_PER_CALL = 3; }
```

Tenancy billing (Stripe) is the one place a fiat amount appears. It is in
`billing.v1` only, as cents, and never in an agent payment record.

## 27. Pylon, attestation and sovereign agents

### 27.1 `openagents.pylon.v1`

**Purpose.** Compute provider beacons, buyer service receipts,
recomputable pool aggregates, check verdicts, model groups and payment
acknowledgments (NIP-PYLON). The Pylon app and the Pylon earnings surface
read these.

**Status.** Implemented in part (`crates/nostr`, `crates/pylon`):
receipt, pool and policy v2 (2026-10-15); beacon v1 and v2 (model groups,
2026-10-20); `pylon.deadline.v1` (2026-10-21).

```proto
package openagents.pylon.v1;
message Beacon {                                                  // kind 30200
  option (openagents.options.v1.contract) = "openagents.pylon-beacon.v2";
  T.Pubkey provider = 1; string pylon = 2; string label = 3 [(openagents.options.v1.limits) = {max_bytes: 64}];
  BeaconStatus status = 4; /* online|draining|offline */ uint64 generation = 5; int64 since = 6; int64 observed_at = 7;
  int64 valid_until = 8; /* ≤ observed_at + 300 s */ MachineClass class = 9; Slots slots = 10;
  repeated Service services = 11 [(openagents.options.v1.limits) = {min_count: 1, max_count: 16}];
  string settlement = 12; repeated string pools = 13 [(openagents.options.v1.limits) = {max_count: 8}];
  optional ModelGroup group = 14; optional KilnRunner runner = 15; optional uint32 max_input_bytes = 16; /* meta; default 16,384, max 98,304 */
  optional openagents.identity.v1.OwnerAttestation auth = 17;
}
message MachineClass { Family family = 1; /* unified-memory|gpu|cpu */ SizeTier tier = 2; /* small|medium|large|xl */ uint32 memory_gb = 3; /* 8…512 */ }
message Slots { uint32 total = 1; uint32 free = 2; }               // ≤64
message Service { string capability = 1; string model = 2; repeated Lane lanes = 3; /* cj-execution|cj-conversation|cj-decision|nip90-5050 */
                  optional T.EventRef offering = 4; optional uint64 price_hint_msat = 5; }
message ModelGroup { string id = 1; GroupRole role = 2; /* head | stage */ uint64 generation = 3; T.ArtifactRef artifact = 4; string readiness = 5;
                     uint32 deadline_secs = 6; /* 1–3600 */ uint32 max_output_tokens = 7; }
message Receipt {                                                 // kind 3201
  option (openagents.options.v1.contract) = "openagents.pylon-receipt.v2";
  T.Pubkey buyer = 1; T.Pubkey provider = 2; string pylon = 3; Lane lane = 4; string capability = 5; string request = 6;
  T.Digest request_digest = 7; T.Digest result_digest = 8; int64 started_at = 9; int64 finished_at = 10;
  Units units = 11; /* tokens|seconds|jobs, count */ ReceiptOutcome outcome = 12; /* accepted|rejected|failed|timeout */
  oneof payment_state { T.Null payment_null = 13; ReceiptPayment payment = 14; }
}
message ReceiptPayment { string profile = 1; /* lightning-bolt11 | x402-exact */ T.BitcoinNetwork network = 2; uint64 amount_msat = 3; string settlement = 4; T.EventRef acknowledgment = 5; }
message PaymentAck { option (openagents.options.v1.contract) = "openagents.pylon-payment-ack.v1"; T.EventRef receipt = 1; openagents.payment.v1.PaymentRef payment = 2; }
message Pool {                                                    // kind 30201
  option (openagents.options.v1.contract) = "openagents.pylon-pool.v2";
  T.Pubkey aggregator = 1; string pool = 2; T.ArtifactRef policy = 3; T.Window window = 4; /* ≤3600 s */
  PoolInputs inputs = 5; /* beacons, receipts, checks: {count, digest} */ PoolTotals totals = 6;
  repeated double rate = 7 [(openagents.options.v1.limits) = {min_count: 1, max_count: 60}]; int64 generated_at = 8; int64 valid_until = 9;
}
// Checks: NIP-32 kind 1985 with L=openagents.pylon, l ∈ check-pass|check-fail|check-inconclusive.
message Check { T.EventRef subject = 1; CheckVerdict verdict = 2; T.ArtifactRef evidence = 3; }
```

Recomputability is the invariant. Anyone holding the input events can
recompute a pool's totals and digests, and a pool whose inputs do not
reproduce it is refused. Per window: ≤4,096 beacons, ≤65,536 receipts or
labels.

### 27.2 `openagents.attest.v1`

**Purpose.** Attested workload releases, independent rebuilds,
TEE-attested endpoints and sealed CJ/DEC jobs (NIP-ATT). The LayerTree
and Confidential Space work produces these records.

**Status.** Partial: Google Confidential Space on Intel TDX in
`crates/nostr::att`, `crates/oa-att` and Pylon `serve --attested`.
Fixture-tested: raw TDX, SEV-SNP and NRAS verification, HPKE carriage,
Rekor checks.

```proto
package openagents.attest.v1;
message Release { string workload = 1; T.Pubkey publisher = 2; Image image = 3; repeated Platform platforms = 4; repeated Measurement measurements = 5;
                  optional string gpu = 6; repeated ModelPin models = 7; repeated ComponentPin components = 8; Source source = 9;
                  repeated RebuildRef rebuilds = 10; repeated Transparency transparency = 11; string changes = 12; int64 published_at = 13; }  // kind 3202
message Platform { PlatformKind kind = 1; /* gcp-confidential-space|tdx|sev-snp|apple-app-attest */ string hwmodel = 2; string support = 3; }
message Head { string workload = 1; uint64 generation = 2; /* only grows */ uint32 notice_seconds = 3; repeated Admitted admitted = 4; bool emergency = 5; } // 30202
message Admitted { T.EventRef release = 1; int64 effective_at = 2; int64 retire_at = 3; }  // effective_at ≥ published_at + notice (≥86,400 s for emergency)
message Rebuild { T.EventRef release = 1; T.Pubkey builder = 2; T.Digest digest = 3; bool matches = 4; }   // kind 3203
message Endpoint { T.EventRef release = 1; string endpoint = 2; bytes hpke = 3; bytes binding = 4; repeated EvidenceToken evidence = 5;
                   T.Pubkey operator = 6; int64 issued_at = 7; int64 valid_until = 8; /* ≤3600 s */ }       // kind 30203
// binding = SHA-256("openagents.att.v1\0" || endpoint || hpke || release_id)
message SealedRequest { string suite = 1; string endpoint = 2; T.EventRef release = 3; T.Pubkey recipient = 4; bytes key = 5; string request = 6;
                        T.Pubkey reply_to = 7; int64 expires_at = 8; /* ≤600 s ahead */ uint64 max_output_bytes = 9; bytes enc = 10; bytes ct = 11; }
message SealedChunk { string request = 1; uint64 seq = 2; bool last = 3; bytes ct = 4; }   // plaintext {text, stop}
enum AttestationLevel { ATTESTATION_LEVEL_UNSPECIFIED = 0; ATTESTATION_LEVEL_OPEN = 1; ATTESTATION_LEVEL_HARDENED = 2; ATTESTATION_LEVEL_TEE = 3; ATTESTATION_LEVEL_TEE_CLOUD = 4; }
message ApprovalSet { repeated T.EventRef approved = 1; }   // personal; on 3188; can only narrow admission
```

### 27.3 `openagents.sovereign.v1`

**Purpose.** Sovereign agents: profile, bounded activation and
checkpoints, custody, guardians and treasury (NIP-SOV). It succeeds the
historical SA kinds.

**Status.** Partial: `openagents sov` local lifecycle, `sov-custody`
(NIP-46 custody and guardian quorum), `sov-lifecycle` (fenced transfer
and recovery), `sov-treasury`. The three schema bundles (11, 13 and 18
defs) map one-to-one.

```proto
package openagents.sovereign.v1;
message Profile { T.Pubkey agent = 1; string authority = 2; uint64 revision = 3; optional T.Digest previous = 4; T.ArtifactRef policy = 5;
                  Custody custody = 6; GuardianPolicy guardian_policy = 7; TreasuryPolicy treasury = 8; T.ArtifactRef state_schema = 9;
                  openagents.policy.v1.DisclosurePolicy disclosure = 10; repeated T.Evidence evidence = 11; }
message Activation { string activation = 1; string profile = 2; openagents.automation.v1.Plan plan = 3; T.Pubkey controller = 4; uint64 generation = 5;
                     string claim = 6; optional string reservation = 7; EnvironmentPin environment = 8; optional T.ArtifactRef checkpoint = 9; int64 admitted_at = 10; }
message Custody { string adapter = 1; /* nip46 only */ T.ArtifactRef policy = 2; }
message CustodyRequest { CustodyPurpose purpose = 1; /* sign|nip44_encrypt|nip44_decrypt */ bytes payload = 2; repeated string preconditions = 3; }
message GuardianPolicy { repeated T.Pubkey guardians = 1; uint32 quorum = 2; }
message LifecycleControl { LifecycleAction action = 1; /* stop|revoke|rollback */ uint64 expected_generation = 2; }
message Transfer { T.Pubkey from = 1; T.Pubkey to = 2; uint64 generation = 3; T.ArtifactRef state = 4; }
message TreasuryPolicy { T.Amount daily_limit = 1; T.Amount per_purchase = 2; repeated string allowed_purposes = 3; }
message PurchaseRequest { string id = 1; T.Amount amount = 2; string purpose = 3; openagents.market.v1.OrderRef order = 4; }
message EconomicStatement { option (openagents.options.v1.contract) = "openagents.sovereign-economic-statement.v2";
                            T.Window window = 1; T.Amount income = 2; T.Amount spent = 3; T.Amount balance = 4; }
```

## 28. Community: XP, Forum, deploy status, referrals

### 28.1 `openagents.xp.v1`

Non-spendable experience points: frozen quests, referee awards,
revocations, playtests, trainer profiles, key links and cards (NIP-XP).
Implemented. The nine `xp-*.v1.json` schemas map one-to-one. The quest
rule `pylon-check` has no schema definition today, and this package adds
one.

```proto
package openagents.xp.v1;
message Quest { string id = 1; string version = 2; Season season = 3; string title = 4 [(openagents.options.v1.limits) = {max_bytes: 200}];
                string objective = 5 [(openagents.options.v1.limits) = {max_bytes: 4000}]; AcceptanceRule acceptance = 6; optional string reference = 7;
                AwardSpec award = 8; Completions completions = 9; /* first | per-awardee */ optional uint32 max_awards = 10; /* 1–10,000 */ }
message AcceptanceRule { oneof rule { KbTransfer kb_transfer = 1; Reproduce reproduce = 2; Playtest playtest = 3; EvalCheck eval_check = 4;
                                      EvalAdopt eval_adopt = 5; PylonCheck pylon_check = 6; } }
message Award { T.EventRef quest = 1; string key = 2; int64 accepted_at = 3; optional T.EventRef entry = 4; optional uint32 entry_version = 5;
                repeated T.EventRef evidence = 6; repeated Awardee awardees = 7; }   // sum 1–1,000
message Awardee { string role = 1; T.Pubkey pubkey = 2; uint32 xp = 3; }
message PlaytestSession { string id = 1; string build = 2; T.Pubkey tester = 3; int64 started_at = 4; int64 ended_at = 5; }
message PlaytestReport { string session = 1; repeated string findings = 2; T.ArtifactRef survey = 3; }
message TrainerProfile { T.Pubkey key = 1; string name = 2; uint64 total_xp = 3; }
message KeyLink { T.Pubkey from = 1; T.Pubkey to = 2; bytes proof = 3; }
message TrainerCard { T.Pubkey key = 1; repeated T.EventRef achievements = 2; /* NIP-32 labels */ }
```

### 28.2 `openagents.forum.v1`

**Purpose.** Forum boards, threads, posts, reactions and moderation.
Forum is built on NIP-7D threads (kind 11), NIP-22 comments (1111),
NIP-25 reactions and NIP-32 labels for moderation, so this package is
mostly a typed view over those mirrors plus the board catalog.

**Status.** Implemented on openagents.com. The board catalog and
moderation policy shapes are `v1alpha1`. An inventory of current Forum
JSON is a phase P3 task (§36), because the survey did not find
`openagents.forum.*` contract IDs.

```proto
package openagents.forum.v1;
message Board { string id = 1; string title = 2; string description = 3; repeated string tags = 4; ModerationPolicy moderation = 5; }
message ThreadView { nostr.event.v1.Event thread = 1; /* kind 11 */ uint32 replies = 2; int64 last_activity = 3; repeated string labels = 4; }
message PostView { nostr.event.v1.Event post = 1; /* kind 1111 */ uint32 reactions = 2; bool hidden = 3; }
message ModerationAction { T.EventRef target = 1; string label = 2; /* NIP-32 */ T.Pubkey moderator = 3; string reason = 4; }
message ModerationPolicy { repeated T.Pubkey moderators = 1; repeated string blocked_labels = 2; }
```

### 28.3 `openagents.deploy.v1`

Public status of staging and production deploys from one announcer key
(NIP-DEPLOY). Implemented (`nostr::deploy`, the Grid billboard).

```proto
package openagents.deploy.v1;
message DeployStatus {                                     // kind 3205
  option (openagents.options.v1.contract) = "openagents.deploy-status.v1";
  Env env = 1; /* staging | production */ DeployTier tier = 2; /* image|module|content */ Phase phase = 3; string run = 4; /* 1–32 */
  optional string commit = 5; /* 7–40 hex */ optional string subject = 6; /* ≤72 B */ string by = 7; /* ≤24 */ optional string build = 8; int64 started = 9; optional string test = 10;
}
enum Phase { PHASE_UNSPECIFIED = 0; PHASE_BUILD_STARTED = 1; PHASE_BUILD_DONE = 2; PHASE_BUILD_FAILED = 3; PHASE_DEPLOYING = 4; PHASE_DEPLOY_FAILED = 5;
             PHASE_PROMOTED = 6; PHASE_SMOKE_RUNNING = 7; PHASE_SMOKE_PASSED = 8; PHASE_SMOKE_FAILED = 9; PHASE_TRAFFIC_SHIFTED = 10;
             PHASE_ROLLED_BACK = 11; PHASE_LIVE = 12; }
message LaneHead { string lane = 1; /* d: staging|production|test/… */ DeployStatus latest = 2; }   // kind 30205
```

Pinned announcer key `eaf77c57…ffb4`. Events more than 300 s in the
future are refused.

### 28.4 `openagents.referral.v1`

Referral sources, attribution, consent, commission terms and agreements
(six `openagents.referral.*` IDs, plus `contribution-*` and `affiliates`).
Partial. Commissions pay in msat through `payment.v1`.

```proto
package openagents.referral.v1;
message Source { string id = 1; T.Pubkey referrer = 2; string code = 3; }
message Attribution { string source = 1; string account_id = 2; int64 attributed_at = 3; bool consented = 4; }
message CommissionTerms { string id = 1; uint32 basis_points = 2; T.Window window = 3; T.Amount cap = 4; }
message CommissionAgreement { string terms = 1; T.Pubkey referrer = 2; int64 accepted_at = 3; }
```

## 29. Interface: UI view trees, engine, Verse

### 29.1 `openagents.ui.v1`

**Purpose.** The shared view language that web, mobile, desktop, Coder
Desktop and the Verse HUD render: the Aethyr view tree (`aethyr.ui.v1`),
Rust Native views, intents and activation, and token references
(`oa-tokens`). Generative UI (the OpenUI adaptation) produces these
trees.

**Sources.** `aethyr.ui.v1`; glossary "Rust Native view/intent/activation";
`packages/ui`; Effect Native; `openagents.workbench-*` (11 IDs:
directory, resource, pane, intent, session, outcome); owner rules "No
walls of text, no dead controls" and "No Chat/Code modes".

**Status.** Implemented as JSON. `v1alpha1` because Effect Native is still
being defined.

```proto
package openagents.ui.v1;
message View { string id = 1; oneof node { Stack stack = 2; Text text = 3; Button button = 4; Image image = 5; List list = 6; Field field = 7;
                                           Card card = 8; Table table = 9; Chart chart = 10; Pane pane = 11; } }
message Stack { Axis axis = 1; repeated View children = 2 [(openagents.options.v1.limits) = {max_count: 256}]; string gap = 3; /* token */ }
message Text { string value = 1; string style = 2; /* token: title, body, caption */ }
message Button { string label = 1; Intent intent = 2; bool enabled = 3; /* never shown enabled without a working intent */ }
message Intent { string name = 1; /* registry slug: open_thread, run_coder, ... */ repeated IntentArg args = 2; }
message Activation { string view = 1; Intent intent = 2; int64 at_ms = 3; }
message TokenRef { string token = 1; }   // colors, spacing, type are tokens, never literals
message Pane { string id = 1; string resource = 2; View content = 3; }
```

Rules: a `Button` whose intent no host handles is a conformance failure.
Text views carry user-facing words only. Status words such as
"retained" or "projection" are refused by a lint on fixture trees.

### 29.2 `openagents.engine.v1`

Unchanged. It is published, frozen and consumed by `aethyr-cpp`.
`openagents.options.v1` options will be added to it only through a `v1`
compatible change (new options, no field changes). The hand-written
`.options` files become generated in phase P2, and the generated output
must be byte-identical to today's before the switch.

### 29.3 `openagents.verse.*` — Chapter V

Everything 3D: worlds, documents, authoring, publishing, presence (NIP-MV
bodies), social, realm, simulation, rules, characters, NPCs, places,
agents in the world, quests, the galaxy, assets, presentation,
environment, host bridges, the Gym building, terminals, media,
telemetry, economy views, games and Space. That is 28 packages.

The full chapter is
[`docs/verse/openagents-verse-protocol.md`](../verse/openagents-verse-protocol.md).
It follows this document's principles (§3), options (§6), limits and
encodings (§7) and Nostr rules (§11). Where the chapter's §4 sketches the
whole tree, this document's §4 replaces that sketch. Chapter V's own build
phases V0–V12 run inside phase P6 here (§36).

Its cross-domain references follow §4.4. Quests reference `xp.v1` awards,
economy views reference `payment.v1` and `market.v1`, agents in the world
reference `agent.v1` and `host.v1`, and the Gym building references
`gym.v1`. None embeds those messages.

## 30. Telemetry, research records and evidence

### 30.1 `openagents.telemetry.v1`

Product events, playtest events, surveys, remarks, consent and
automation marking. Lenient readers, append-only, and every personal
field consent-bound. The owner's 2026-10-27 playtest rule (save
everything and say so in Settings) is a `Consent` record the client shows.

```proto
package openagents.telemetry.v1;
message Event { string name = 1; int64 at_ms = 2; string session = 3; string surface = 4; string build = 5; repeated Property properties = 6;
                bool automated = 7; /* bots and tests are marked, never mixed with people */ optional Consent consent = 8; }
message Property { string key = 1; oneof value { string text = 2 [(openagents.options.v1.sensitivity) = SENSITIVITY_PERSONAL]; double number = 3; bool flag = 4; } }
message Consent { string scope = 1; bool granted = 2; int64 at_ms = 3; string settings_path = 4; }
```

### 30.2 `openagents.evidence.v1`

**Purpose.** A typed home for the large number of research,
qualification, benchmark and audit records that are not wire protocol.
Examples: `coder-one.*` (109), `briefing.*` (50), `tbench.*` (43),
`deepseek_v41.*` (29), `jev*` (27), `delegation.*` (53), `sales.*` (88),
`psionic.*`, `microluna.*`, `method.*`, `metric-target*`,
`layertree.*`, `filefind.*`, `team-*`, and every `*-qualification.v1`,
`*-evidence.v1`, `*-smoke.v1` and `*-verification.v1`.

These records keep their JSON bodies. Giving each of them a proto would
freeze research shapes that change weekly, and nobody outside OpenAgents
needs to implement them. What they need is a common envelope, so tools
can index, verify and cite them:

```proto
package openagents.evidence.v1;
message Record {
  option (openagents.options.v1.data_class) = DATA_CLASS_EVIDENCE;
  string contract = 1;                 // the existing id, e.g. "openagents.tbench.matched-results.v1"
  RecordKind kind = 2;                 // qualification|benchmark|study|audit|smoke|fixture|report|registration
  string subject = 3;                  // what the record is about (issue, crate, model, release)
  string commit = 4;                   // repository commit it was produced at
  T.ArtifactRef body = 5;              // the exact JSON body
  repeated T.ArtifactRef inputs = 6;
  T.Verdict verdict = 7;
  int64 produced_at = 8;
  T.Pubkey producer = 9;
  optional bytes signature = 10;
}
```

A record graduates from `evidence.v1` to its own package only when
something outside the producing tool reads it as an interface. The
`sales.*` family is a likely candidate once the sales product's outbox,
pipeline and voice authority records stabilise. Until then they are
evidence.

### 30.3 `openagents.discovery.v1`

The capability manifest, the API index, OpenAPI document references, the
docs corpus, and NIP-89 handler information. It answers "what does this
service offer and where is its schema".

```proto
package openagents.discovery.v1;
message Manifest { string service = 1; string version = 2; repeated ApiDocument apis = 3; repeated string packages = 4; /* proto packages served */
                   repeated openagents.capability.v1.DiscoveryHead capabilities = 5; repeated string nips = 6; }
message ApiDocument { string name = 1; string url = 2; T.Digest digest = 3; string format = 4; /* openapi-3.1 | proto-descriptor */ }
message DocsEntry { string path = 1; string title = 2; T.Digest digest = 3; repeated string topics = 4; }
```

---

# Part IV. Making it real

## 31. Services and transports

### 31.1 Three transports, one schema

| Transport | Used for | Framing |
| --- | --- | --- |
| Nostr relays | Public records, private 3188 envelopes, CJ and MV ephemeral jobs | NIP-01 JSON over WebSocket; content via the NIP JSON mapper |
| HTTP/JSON | `api.openagents.com/v1`, webhooks, x402 HTTP binding | OpenAPI generated from proto (§33.3); `api.v1` envelope |
| gRPC and Connect | Service-to-service inside OpenAgents, generated SDK clients, host and worker daemons | HTTP/2 gRPC, plus Connect for browsers (JSON or binary) |
| Framed channels | REACH direct channels, TERM record streams, Verse realm sessions | The NIP-defined frame headers; bodies are binary proto messages from the owning package |

### 31.2 Service catalog

Each package that has callable operations declares one service. These are
the services the plan builds. Methods are listed by name; request and reply
messages are the ones in each section.

| Service | Package | Methods |
| --- | --- | --- |
| `AccountService` | `account.v1` | GetAccount, ListWorkspaces, CreateWorkspace, Invite, AcceptInvitation, ListMembers, SetRole |
| `CredentialService` | `credential.v1` | CreateKey, ListKeys, RevokeKey, StartDeviceAuthorization, PollDeviceAuthorization, AddProviderKey, RotateCmk |
| `VaultService` | `vault.v1` | PutObject, GetObject, ListIndex, RequestLease, RevokeLease |
| `DecisionService` | `decision.v1` | Decide, Decisions, Classify, CreateJob, GetJob, ListModels, GetModelCard |
| `ResponsesService` | `inference.v1` | CreateResponse (server stream), GetResponse, ListModels, CreateBatch |
| `ChatService` | `chat.v1` | CreateThread, ListThreads, SendMessage (server stream), ListMessages |
| `ExecService` | `exec.v1` | Execute, Status, Replay, Cancel |
| `AgentService` | `agent.v1` | CreateAgent, GetAgent, ListAgents, PublishAgent, RevokeAgent |
| `ManagedAgentService` | `agent.managed.v1` | CreateSession, SendEvent, StreamEvents, CreateDeployment, ListDeploymentRuns, CreateMemoryStore, WriteMemory |
| `SessionService` | `session.v1` | Open, Attach, Command, Read, ExportHistory, ImportHistory |
| `CapabilityService` | `capability.v1` | Discover, Describe, Grant, Probe |
| `ProgramService` | `program.v1` | Run, GetRun, ListRuns |
| `RegistryService` | `extension.v1` | GetRegistry, ResolvePackage, ListReleases |
| `EvalService` | `eval.v1` | RunSuite, GetReport, Admit |
| `GymService` | `gym.v1` | ListBoards, GetLeaderboard, Connect, Request |
| `RunService` | `run.v1` | Append, Head, Replay |
| `CoordinationService` | `coordination.v1` | Propose, Claim, Control, Finding |
| `AutomationService` | `automation.v1` | CreatePlan, Control, State |
| `WorkService` | `work.v1` | Command, Get, List, Attention |
| `WorkspaceService` | `workspace.v1` | Bind, Read, Replace, Checkpoint, Restore, Project |
| `EnvironmentService` | `environment.v1` | Request, Materialize, Attach, Control, Cleanup |
| `HostService` | `host.v1` | Call (unary, wraps `HostRequest`), Stream (bidi over REACH) |
| `ComputersService` | `computers.v1` | per §24.2 |
| `CloudService` | `cloud.v1` | Quote, Up, Budget, ListPool |
| `MarketService` | `market.v1` | Publish, Negotiate (bidi record stream), Status |
| `PaymentService` | `payment.v1` | Quote, Pay, Observe, ListEvidence |
| `LedgerService` | `ledger.v1` | Statement, Earnings, Balance |
| `PylonService` | `pylon.v1` | Beacons, Receipts, Pool |
| `AttestService` | `attest.v1` | Verify, Endpoint |
| `DeployService` | `deploy.v1` | Lane, History |
| `DiscoveryService` | `discovery.v1` | Manifest |

The HTTP mapping is declared per method with `google.api.http`
annotations, which keep the existing paths: `/v1/decisions`,
`/v1/systemone`, `/v1/classify`, `/v1/jobs`, `/v1/responses`,
`/v1/agents`, `/v1/sessions`, `/v1/environments`, `/v1/computers` and so
on. A generated OpenAPI document must match the hand-maintained one
before the hand-maintained one is retired (§32.3).

### 31.3 Streaming

- Server streams for Responses and session events use Server-Sent Events
  on HTTP and server streaming on gRPC. The SSE event names are the
  existing ones (`response.output_text.delta` and so on).
- Bidirectional streams (host, realm, market negotiation) use gRPC bidi
  or the REACH frame channel. There is no WebSocket protocol of our own
  outside Nostr relays and coder-sync.

## 32. Migration map

### 32.1 The three tiers of the 2,368 IDs

Every `openagents.*.vN` identifier in the repository falls into one tier.
The generator's migration report (`scripts/proto-inventory`) lists every
ID with its tier and target, and CI fails if a new ID appears without a
tier.

| Tier | What | Rule | Rough count |
| --- | --- | --- | --- |
| **Protocol** | Records another party reads or writes: NIP bodies, CJ jobs, HOST and REACH and TERM messages, receipts, grants | A message in a protocol package, with `(contract)` = the ID | ~700 |
| **Product API** | HTTP request and response bodies of `api.openagents.com` and app surfaces (`web.*`, `sdk.*`, `computers.*` without `evidence`, `cloud.*`, `tenancy.*`) | A message in a product package, with an HTTP binding | ~450 |
| **Evidence** | Qualification, benchmark, study, audit, smoke, fixture, report and registration records | Keeps its JSON; wrapped by `evidence.v1.Record` when published | ~1,200 |

The counts are first estimates from the family breakdown in §2.3. The
inventory script replaces them with exact numbers in phase P1.

### 32.2 Family → package

| Family (IDs) | Target |
| --- | --- |
| `coder` (216): `coder.host-*`, `coder.ssh-*`, `coder-package`, `coder.computers`, `coder.delegate-*` | `coder.*` packages (§25) |
| `coder-one.*` (109) | `evidence.v1`, except `coder-one.policy.v1` → `coder.task.v1` |
| `gym.*` (119), `recipe.*` (24) | `gym.v1` |
| `verse.*` (102), `aethyr.*`, `aethyrforge.*`, `haunt.*`, `icecream.*`, `space*` | Chapter V packages |
| `sales*` (88) | `evidence.v1` until stable; candidate future `sales.v1` |
| `delegation*` (53) | `evidence.v1`; study protocol records stay JSON |
| `terminal*` (52) | `terminal.v1`; `native-terminal.*` qualification records → `evidence.v1` |
| `environment*` (52) | `environment.v1` |
| `briefing*` (50), `tbench.*` (43), `deepseek_v41.*` (29), `jev*` (27), `microluna.*`, `method.*`, `metric-target*`, `filefind.*` | `evidence.v1` |
| `work*` (47) | `work.v1` |
| `pylon*` (45) | `pylon.v1` |
| `web.*` (44) | product packages by subject: `chat.v1` (`web.chat.v1`), `coder.task.v1` (`web.own-runs.*`), `account.v1` (`web.project-settings.v1`), `credential.v1` (`web.connections.v1`) |
| `computers.*` (43) | `computers.v1`; `*.evidence.v1` → `evidence.v1` |
| `cloud*` (37) | `cloud.v1` |
| `coordination*` (35) | `coordination.v1` |
| `agent*` (34) | `agent.v1`, `agent.managed.v1` |
| `session*`, `history-*` (32 + 10) | `session.v1` |
| `host*`, `connect-*` (32 + 13) | `host.v1`, `coder.host.v1` |
| `automation*` (31) | `automation.v1` |
| `sovereign*` (30) | `sovereign.v1` |
| `control*` (27) | `control.v1` |
| `live*`, `livehost-*` (24 + 18) | `live.v1` |
| `task*` (23) | `coder.task.v1` |
| `kb*`, `knowledge*` (23 + 7) | `knowledge.v1` |
| `route*` (21) | `compute.v1`, `policy.v1` (`route-usage`) |
| `ws*`, `workspace*`, `projection-*` (19 + 10 + 9) | `workspace.v1` |
| `labor*` (19) | `labor.v1` |
| `sdk.*` (17) | `evidence.v1` (SDK qualification records) |
| `layertree.*` (13) | `attest.v1` for releases and keys; the rest `evidence.v1` |
| `contribution-*` (13), `referral.*` (6) | `referral.v1` and `ledger.v1` |
| `ext*`, `plugin*` (16 + 10) | `extension.v1`, `program.v1` |
| `chat*` (16) | `chat.v1` |
| `vault*` (15) | `vault.v1` |
| `workbench-*` (11) | `ui.v1` |
| `reach*` (11) | `reach.v1` |
| `eval*` (10) | `eval.v1` |
| `classify*`, `systemone*`, `decision*` (10 + 7 + …) | `decision.v1` |
| `att*`, `attested*` (10 + 1) | `attest.v1` |
| `xp*` (9) | `xp.v1` |
| `x402*` (8) | `x402.v1` |
| `optimization*`, `tenant_training*` (8 + 6) | `optimize.v1` |
| `inference*` (7) | `inference.v1` |
| `tenancy*` (7) | `account.v1`, `credential.v1` |
| `nostr*` (8) | `nostr.*` mirrors |
| `team-*` (6) | `evidence.v1` |

### 32.3 OpenAPI documents

| Document | Becomes |
| --- | --- |
| `docs/api/openapi.first-party.json` | Generated from `account`, `credential`, `decision`, `inference`, `chat`, `agent`, `agent.managed`, `environment`, `computers`, `cloud`, `payment`, `ledger`, `billing` services |
| `docs/sdk/openapi.app.json` | Generated from `chat`, `agent.managed`, `coder.task` |
| `docs/sdk/openapi.computers.json` | Generated from `computers.v1` |
| `docs/sdk/openapi.labs.json` | Generated from `coder.lab.v1` |
| `docs/sdk/openapi.systemone.json`, `docs/decision-models/api/openapi.yaml` | Generated from `decision.v1` |

The migration rule: for each document, generate it from proto, diff it
against the hand-maintained file, and fix proto until the diff is empty
or every remaining difference is a documented bug fix. Only then does the
generated file replace the hand-maintained one. Error type spellings that
differ from §10.1 are listed in the first diff and either kept by `wire`
or fixed with a deprecation note.

### 32.4 JSON Schemas

The 33 files in `nips/openagents/schemas/` stay authoritative for JSON
validation until the generator emits equivalent JSON Schema from proto
(§33.3) and the two validate the same fixtures identically. Then the
schema files become generated output, checked in, with a header naming
their source package. `payment-receipt.v1.json` is superseded by
`openagents.payment-receipt.v2` (§26.1) rather than regenerated.

### 32.5 Stale text found while writing this spec

These are corrections to existing documents. They are tracked in phase P1
and fixed in the same change that lands the relevant package:

- `nips/README.md` says the OpenAgents lane has 33 NIPs. There are 36.
- `nips/README.md` still calls ATIF, REG, VAULT and ATT "Designed" and
  says "No registry client is implemented". The NIPs and
  `nips/openagents/README.md` say they are implemented.
- NIP-WORK says it is "not an implemented issue tracker", while
  `work-host` roles are implemented.
- Kind 30440 (livehost) is used but not registered.
- `xp-quest.v1.json` and `xp-award.v1.json` lack the `pylon-check` rule.
- Block PMA kind 30179 must be rejected, and the relay does not reject it
  yet.
- `payment-receipt.v1.json` lists excluded rails (§26.1).

## 33. Tooling and generation

### 33.1 Layout and build

- `proto/buf.yaml` declares one module with lint rules: `STANDARD` plus
  ours (enum zero is `_UNSPECIFIED`, every top-level message has
  `data_class` and `contract`, every string and repeated field has
  `limits`, and no `Any`, `Struct` or `Value`).
- `buf breaking` runs against every descriptor set in
  `proto/compatibility/`.
- protoc stays at 29.3 until a reviewed bump, as `engine-proto` pins it.
- `crates/oa-proto` builds all `openagents.*` and `nostr.*` packages for
  Rust with Prost 0.13.5, as `crates/engine-proto` does. `engine-proto`
  stays as is and becomes a dependency of `oa-proto`.

### 33.2 Generators

`scripts/proto.sh` drives every generator from one descriptor set. Each
generator reads the custom options:

| Generator | Output | Reads |
| --- | --- | --- |
| Rust types | `crates/oa-proto` (Prost) | all |
| Rust validators | `crates/oa-proto/src/validate/` | `limits`, `data_class`, `nullable`, grammars |
| NIP JSON mappers | Rust, TypeScript, C++ | `json_key`, `wire`, `nullable`, `contract`, `nip` |
| Nanopb `.options` | per package, checked in | `limits` |
| TypeScript | `@openagentsinc/protocol` (protobuf-es) | all |
| Swift and Kotlin | via `swift-protobuf`, `protobuf-kotlin` | all |
| C++ | protoc C++ plus Nanopb for embedded and Wasm | all |
| Python | `openagents-protocol` | all |
| Go | `buf.build/gen/go/openagents/...` | all |
| JSON Schema 2020-12 | `nips/openagents/schemas/` (generated, §32.4) | all |
| OpenAPI 3.1 | `docs/api/` and `docs/sdk/` (generated, §32.3) | services, `http` |
| Docs | `docs/protocol/reference/` | all comments and options |
| Redaction helpers | each language | `sensitivity` |

### 33.3 Fixtures

Every NIP example, every file under `docs/protocol/fixtures/`, and every
JSON Schema example becomes a golden test. The test decodes the JSON
through the NIP mapper into proto, re-encodes it, and requires
byte-identical JCS. Signed examples must verify after the round trip. The
round trip covers Rust, TypeScript, C++ and Python, and each must produce
the same bytes.

## 34. Publishing the protocol

### 34.1 What gets published

- The `proto/` tree, Apache-2.0, the repository's license. Upstream
  vendored files keep their own licenses, recorded in `sources.json` and
  beside each directory.
- Generated reference docs at `openagents.com/protocol`, built from proto
  comments and this spec.
- Descriptor sets for every published version.

### 34.2 Where

| Channel | Name | Contents |
| --- | --- | --- |
| Buf Schema Registry | `buf.build/openagents/openagents` | The `openagents` and `nostr` modules (pending owner decision, §39) |
| crates.io | `openagents-protocol` | Rust types, validators and mappers |
| npm | `@openagentsinc/protocol` | TypeScript (protobuf-es) |
| PyPI | `openagents-protocol` | Python |
| Swift Package Manager | `OpenAgentsProtocol` | Swift |
| Maven Central | `com.openagents:protocol` | Kotlin and Java |
| Go | `go.openagents.com/protocol` | Go |
| GitHub releases | `proto-vX.Y.Z` | Descriptor sets, the hash-pinned bundle (like `openagents.engine-proto.bundle.v1`) |

Each package version publishes once. Generated libraries follow SemVer
with major versions tied to package major versions.

### 34.3 The bundle

The existing `openagents.engine-proto.bundle.v1` pattern generalizes to
`openagents.protocol.bundle.v1`: a tarball of `.proto` files, descriptor
sets, generated Nanopb C, and a manifest with the SHA-256 of every file.
External C++ consumers such as `aethyr-cpp` import the bundle by hash, as
they do today.

### 34.4 Extension number registration

The options extension block (50100–50199 in this draft) is registered in
protobuf's global extension registry before the first `v1` publication,
and the draft numbers move to the assigned block in the same change.

## 35. Conformance and verification

A package graduates from `v1alpha1` to `v1beta1`, and then to `v1`, only
when every item holds:

1. **Lint and breaking checks pass**, with no exemptions.
2. **Every field is bounded**, and the generated validator refuses
   boundary-plus-one inputs in every language.
3. **NIP round trips pass**: every NIP example and fixture round-trips
   byte-identically through JCS in Rust, TypeScript, C++ and Python.
4. **Signed round trips pass**: signatures verify after round trip, and a
   test proves re-serialized content is never used for verification.
5. **Hostile inputs are refused**: duplicate JSON keys, integers above
   2^53−1, non-finite numbers, depth over the limit, oversize bodies,
   unknown enum values on authority messages, and unknown fields on strict
   classes. A fuzz target per package runs in CI on the Rust decoder and
   the Nanopb decoder.
6. **Data class is enforced**: a property test shows strict classes
   refuse unknown fields and lenient classes keep or drop them as §6.2
   says.
7. **Secrets cannot leak**: the generator test proves no `SECRET` field
   appears in a public carriage, and redaction tests cover `PERSONAL` and
   `CONFIDENTIAL`.
8. **A real consumer exists**: at least one production code path reads and
   writes the package through generated code, not a hand port.
9. **Invariants are written down**: where the package carries a state
   machine (RUN, COORD, AUTO, ENV, HOST grants, ATT admission, MKT record
   chains, X402 status), the owning crate's `INVARIANTS.md` names it, and a
   bounded model or property test covers it, per the workspace invariant
   discipline. Counterexamples become regression tests.
10. **OpenAPI or JSON Schema parity**: where a hand-maintained document
    exists, the generated one matches it (§32.3, §32.4).

## 36. The build plan

Phases are ordered by dependency. Packages inside a phase can land in
parallel. Each phase ends with its packages at the label shown.

| Phase | Scope | Exit |
| --- | --- | --- |
| **P0. Spine** | `options.v1`, `type.v1`, `api.v1`; `proto/buf.yaml`; `crates/oa-proto`; `scripts/proto.sh` with the Rust, TypeScript and validator generators; the NIP JSON mapper for `type.v1`; fixture round trips for `contracts.md` types | `type`, `options`, `api` at v1 |
| **P1. Inventory and fixes** | `scripts/proto-inventory` with the tier for all 2,368 IDs; fix the §32.5 stale text; register kind 30440; supersede `payment-receipt.v1.json`; move `crates/lnd-sdk/proto` to `proto/third_party/lightninglabs` | Inventory in CI; no untiered IDs |
| **P2. Nostr** | `nostr.event.v1` and the "OA use" official mirrors (01, 02, 05, 09, 10, 11, 17, 19, 24, 29, 31, 32, 34, 39, 40, 42, 44, 46, 47, 51, 53, 54, 57, 59, 65, 67, 77, 78, 7D, 85, 86, 89, 92, 94, 98, 99, B7, C7); Block AA, AE, AM, AO, AP, OA, PL, RS, WP, MP, IA; `MANIFEST.json` and the sync check; generated `engine.v1` `.options` byte-identical to today's | These mirrors at v1 |
| **P3. Implemented core** | Direct-v1 packages: `decision`, `receipt`, `chat`, `exec`, `run`, `trace`, `knowledge`, `xp`, `deploy`, `pylon`, `host`, `reach`, `terminal`. Each replaces its hand-written JSON structs in the owning crate with generated types plus the NIP mapper. Forum JSON inventory | These packages at v1 |
| **P4. Agent runtime** | `capability`, `policy`, `context`, `program`, `extension`, `skill`, `eval`, `gym`, `session`, `control`, `coordination`, `automation`, `work`, `workspace`, `environment`, `live`, `agent`, `attest`, `vault`, `identity` | v1beta1; v1 where §35 holds |
| **P5. Product and money** | `account`, `credential`, `inference`, `agent.managed`, `cloud`, `computers`, `compute`, `coder.*`, `market`, `labor`, `payment`, `x402`, `ledger`, `billing`, `sovereign`, `referral`, `forum`, `ui`, `telemetry`, `evidence`, `discovery`; generated OpenAPI parity for all five documents | v1beta1; OpenAPI generated |
| **P6. Verse** | Chapter V phases V0–V12 | Per Chapter V |
| **P7. Optimize and remaining mirrors** | `optimize`; all remaining official and Block mirrors, including deprecated and compat-only ones | All packages exist |
| **P8. Publish** | Extension number registration; BSR module; crates, npm, PyPI, SwiftPM, Maven, Go; reference docs site; `openagents.protocol.bundle.v1` | Public v1 release `proto-v1.0.0` |

What is direct v1 versus `v1alpha1` is in §14.1. A phase can start before
the previous one ends where the import graph allows. P3 needs only P0 and
the P2 mirrors its packages import, and P6 can run beside P3 to P5,
because Chapter V imports only `type`, `options`, `engine`, `ui` and
`nostr`.

## 37. Risks

| Risk | Mitigation |
| --- | --- |
| Proto JSON drifts from NIP JSON, and signatures break | NIP JSON comes only from the generated mapper; every NIP example is a golden test; signatures are never checked against re-serialized content (§33.3, §35) |
| The tree becomes a dumping ground for research shapes | The three tiers (§32.1); evidence stays JSON under `evidence.v1` |
| Package boundaries ossify too early | `v1alpha1` until a real consumer exists; direct v1 only for stable, implemented contracts |
| CTRL and SESS change under the Loro and HOST revision | Both stay `v1alpha1`; the live command path is specified in `coder.sync.v1` and `host.v1` |
| Two copies of Lightning Labs protos | One copy, moved to `proto/third_party/` in P1, and `lnd-sdk` builds from it |
| Nanopb and Wasm decoders allocate before checking limits | Limits generated from one source; fuzzing on the Nanopb decoder (§35, item 5) |
| Money rail drift (EVM or Cashu values creep back) | `Amount`, `Rail` and `Protocol` are closed; an import lint blocks Cashu and Fedimint mirrors |
| Generated OpenAPI changes public behaviour | Parity diff before replacement (§32.3) |
| Publication exposes private records | Every schema is public by design; `SECRET` carriage checks; no field for owner-local secrets (§6.3) |
| Upstream NIP changes silently invalidate mirrors | `MANIFEST.json` digests and the stale flag block publication (§11.5) |

## 38. Decisions this document makes

1. One tree, `proto/`, with `openagents.*`, `nostr.*` and `third_party.*`
   roots. Every OpenAgents project's protocol lives in `openagents.*`.
2. 64 OpenAgents packages outside the Verse, plus Chapter V's 28.
3. NIP JSON stays authoritative where it is signed or hashed. Proto is the
   schema, and a generated mapper produces the JSON.
4. Contract IDs are kept as they are and attached to messages by option.
   Package versions and contract versions are separate axes.
5. `openagents.systemone.v1` stays frozen.
6. Ten data classes, with reader rules per class.
7. Amounts are millisatoshis or Taproot Assets units only.
   `payment-receipt.v1.json` is superseded.
8. Lightning Labs protos are vendored byte-exact under their own package
   names and never embedded in OpenAgents messages.
9. All 99 official NIPs and 17 Block NIPs are listed, and every one with
   a structured body gets a mirror package.
10. The 2,368 schema IDs are tiered: protocol, product API, evidence.
    Evidence keeps its JSON.
11. CTRL and SESS stay `v1alpha1` until the owner closes the 2026-10-16
    revision.
12. OpenAPI documents and JSON Schemas become generated outputs only after
    a parity diff.

## 39. Open questions for the owner

1. **Buf Schema Registry.** Publish to `buf.build/openagents/openagents`,
   or only to GitHub releases and the language registries? The registry
   gives strangers `buf generate` with no clone. The cost is another
   account to run.
2. **The `openagents.` prefix.** Keep `openagents.*` (matches
   `engine.v1` and every contract ID), or use a reverse-domain
   `com.openagents.*`? This spec assumes `openagents.*`, because changing it
   would rename the frozen `engine.v1` package.
3. **Sales records.** Treat the 88 `sales*` IDs as evidence for now, as
   this spec does, or give them a `sales.v1` product package in P5?
4. **Forum.** Confirm that Forum's data is Nostr-native (NIP-7D, NIP-22),
   so `forum.v1` is a view package, or name the current Forum store if it
   is not.
5. **CTRL folding into HOST.** Once live sessions run in Loro rooms, should
   CTRL grants become HOST rights (`observe`, `operate` already overlap), and
   NIP-CTRL be retired?

## 40. Sources

- `nips/openagents/contracts.md`, `nips/openagents/README.md`, all 36
  `nips/openagents/NIP-*.md`, `nips/openagents/schemas/`
- `nips/README.md`, `nips/manifest.json`, `nips/official/` (99 specs),
  `nips/block/` (17 specs)
- `proto/README.md`, `proto/openagents/engine/v1/`, `crates/engine-proto/`
- `crates/space-key-broker/proto/`, `crates/lnd-sdk/proto/` and its
  `sources.json`
- `docs/api/openapi.first-party.json`, `docs/sdk/openapi.{app,computers,labs,systemone}.json`,
  `docs/decision-models/api/openapi.yaml`
- `docs/glossary.md`
- `docs/payments/agent-payments.md` (Bitcoin-only rule, 2026-10-09)
- `docs/cloud/`, `docs/protocol/` (NIP coverage, ledgers, fixtures)
- `docs/aethyr/merge.md` (naming: Verse, Aethyrforge, Aethyr)
- `docs/verse/openagents-verse-protocol.md` (Chapter V)
- `OpenAgentsInc/aethyr-cpp` `third_party/openagents-engine-proto`
- Repository-wide inventory of `openagents.*.vN` identifiers, 2026-10-27
  (2,368 distinct)
