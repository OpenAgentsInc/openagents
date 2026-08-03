# SpacetimeDB in OpenAgents: Full Historical Usage Audit

Date: 2026-08-03
Status: complete — historical audit, no implementation authority
Scope: every SpacetimeDB adoption, deployment, and removal in this repository
Method: Git history across all refs, GitHub issue state, live DNS and Google
Cloud inspection

## Executive Summary

SpacetimeDB was adopted three separate times in this repository between
2026-02-25 and 2026-06-22, for two different problems, and removed all three
times. It is not used anywhere today: no tracked source file outside `docs/`
references it, and no invariant, contract, or runtime path depends on it.

The three cycles were not variations on one program. They were independent
adoptions with different owners, different target problems, and different
failure modes:

| Cycle | Dates | Target problem | Reached production? | Ended by |
| --- | --- | --- | --- | --- |
| 1 | 2026-02-25 (one day) | Canonical sync/replay transport for desktop and runtime | No | Same-day repository prune |
| 2 | 2026-03-04 → 2026-06-09 | Reintegration of cycle 1 as desktop sync | No — stopped at Phase 1 (local projection) | Bun/Effect workspace rebuild |
| 3 | 2026-06-17 → 2026-06-22 | Multiplayer world state for the Verse | **Yes** — 5 days live on GCE | Owner decision to replace outright |

Only cycle 3 ever ran against a real SpacetimeDB server. It served
`spacetime.openagents.com` for five days.

The single most important finding is not historical. **A dangling DNS record
survives all three cycles**: `spacetime.openagents.com` still resolves to
`34.28.177.95`, an address that project `openagentsgemini` no longer holds. See
Finding 1.

The most useful lesson is that each cycle failed for a reason the previous
cycle had already demonstrated but had not written down in a place the next
cycle read.

## 1. Cycle 1 — SpacetimeDB as canonical sync transport (2026-02-25)

### 1.1 What was proposed

Two audits landed on 2026-02-25 arguing that OpenAgents should run its
multi-agent collaboration inside a database:

- `docs/audits/2026-02-25-spacetimedb-openagents-autopilot-audit.md`
- `docs/audits/2026-02-25-spacetimedb-first-architecture-simplification-audit.md`

The first audit is genuinely good technical work. It reviewed 18 OpenAgents
sources and 17 SpacetimeDB sources (transaction atomicity, reducers,
procedures, event tables, schedule tables, subscription semantics, the
commitlog and durability crates) and reached a defensible conclusion:

> The thesis is directionally correct for the collaborative inner loop. […] The
> core design constraint is preserving authority/replay invariants while moving
> collaboration state to a shared DB world; Git/GitHub coupling is migration
> debt, not a reason to keep git as the core primitive.

It also flagged the correct caveats: procedures re-invoke `with_tx` blocks and
so must be side-effect deterministic, and row-level security was explicitly
marked experimental upstream.

The implementation plan was `docs/plans/spacetimedb-full-integration.md`, which
specified 8 canonical tables (`sync_stream`, `sync_event`, `sync_checkpoint`,
`session_presence`, `provider_capability`, `compute_assignment`,
`bridge_outbox`, plus event tables) and 8 reducers, and preserved the existing
`(topic, seq)` monotonic apply discipline as `(stream_id, seq)`.

### 1.2 What was built

Governance:

- `docs/adr/ADR-0009-spacetime-sync-canonical-transport.md` — accepted,
  superseding ADR-0003
- `docs/adr/ADR-0010-spacetime-only-sync-transport-hard-mandate.md`
- 65 `OA-SPACETIME-*` GitHub issues (`#2231`–`#2282` and a `-TOTAL-` series),
  **all opened and closed on 2026-02-25**

Code:

| Artifact | Size |
| --- | --- |
| `crates/autopilot-spacetime/src/client.rs` | 964 lines |
| `crates/autopilot-spacetime/src/reducers.rs` | 684 lines |
| `crates/autopilot-spacetime/src/mapping.rs` | 266 lines |
| `crates/autopilot-spacetime/src/auth.rs` | 260 lines |
| `crates/autopilot-spacetime/src/schema.rs` | 177 lines |
| `crates/autopilot-spacetime/src/subscriptions.rs` | 102 lines |
| `spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs` | 384 lines |
| `apps/runtime/src/spacetime_publisher.rs` | runtime publisher |
| `scripts/spacetime/*` | 10 operator and gate scripts |

Supporting surfaces: a Grafana SLO dashboard, Prometheus alert rules, a
Laravel `SpacetimeTokenController`, canonical payload-hash test vectors, 21
documents under `docs/sync/`, and a websocket golden fixture. At the peak
commit (`9b2b978949`) the repository contained **75 SpacetimeDB-named paths**.

A signoff document was written and the program declared complete:
`docs/audits/2026-02-25-spacetime-final-migration-signoff.md`, status
`completed`, followed by `docs/audits/2026-02-26-spacetime-only-compliance-signoff.md`.

### 1.3 How cycle 1 ended

Commit timestamps tell the story precisely:

```text
09:16:09  8fedb5e9be  spacetime: add typed client crate module
11:35:08  0a0e039a81  OA-SPACETIME-037 canonicalize spacetime docs and runbooks
11:49:03  243187bdc8  Replace Khala with Spacetime across repo
14:48:29  94463c09b5  Complete Spacetime-only total convergence and signoff gates
20:58:39  17aa21b544  chore: prune repo to wgpui + vim and MVP doc only
21:00:42  31b059dacb  chore: archive scripts spacetime generated output proto
```

The entire program — audit, ADRs, 65 issues, client crate, module, publisher,
scripts, 21 sync documents, monitoring, and two signoff audits — was designed,
built, declared complete, and deleted **inside twelve hours on a single day.**

The prune commit (`17aa21b544`, 1,464 files changed, 545,457 deletions) was not
a SpacetimeDB decision. It was an unrelated repository-wide reduction to
"wgpui + vim and MVP doc only" — the same prune recorded in the workspace
contract as removing the DSPy, Adjutant, RLM, and FRLM lanes. SpacetimeDB was
collateral.

### 1.4 The naming collision

Cycle 1 contains a documentation defect worth recording because it is still
misleading to anyone reading that history.

The pre-existing in-house websocket sync transport was named **Khala**. Commit
`243187bdc8`, "Replace Khala with Spacetime across repo," performed a global
mechanical rename of the *old* transport to the *new* vendor's name, while the
new vendor's product was also being called Spacetime in the same documents. The
result is self-contradictory normative text. ADR-0009 reads, verbatim:

> Sync delivery boundary: canonical doctrine moves from Spacetime to Spacetime.

and

> Spacetime remains implemented legacy lane only until cutover gates pass

where the first "Spacetime" means Khala and the second means SpacetimeDB. The
issue titles preserve the collision from both sides: `OA-SPACETIME-033: Cutover
- Spacetime Default, Khala Emergency-Only` sits next to `OA-SPACETIME-036:
Remove Khala Client Dependencies`.

The name Khala later returned and is now load-bearing across
`packages/khala-sync*`, `packages/khala-tools`, and Khala Code. Any future
reader of Feb–Mar 2026 history must disambiguate "Spacetime" per sentence.

## 2. Cycle 2 — Reintegration (2026-03-04 → 2026-06-09)

### 2.1 The gap audit

Eight days after the prune, `docs/audits/2026-03-04-spacetime-broader-vision-gap-audit.md`
audited what actually existed and found nothing:

> Current repo does **not** have a wired Spacetime implementation today. […]
> Sync pane labels say "Spacetime", but values are local projections. […] This
> is a hard implementation gap vs product spec language.

That last sentence is the important one. The user-facing desktop panes were
labelled as if backed by a live sync substrate while rendering locally computed
values. The product claim had outlived the implementation by eight days, and it
took a dedicated audit to notice.

### 2.2 What was rebuilt

A reintegration program (`#2916`–`#2930`) ran on 2026-03-04 and was declared
complete on 2026-03-05 in `docs/SPACETIME_ROLLOUT_INDEX.md`. It restored the
client crate to the workspace, restored the `autopilot-sync` module baseline
and the publish/promote and handshake scripts, added a parity-chaos release
gate, added provider presence lifecycle and TTL policy (ADR-0001 domain
authority matrix, ADR-0002 presence cardinality), and drove desktop sync health
from lifecycle telemetry.

### 2.3 Cycle 2 never went live — and said so

To its credit, the rollout index was explicit about the honesty problem cycle 2
had just been created to fix. It defined two phases and marked which was real:

| Phase | Status | Semantics |
| --- | --- | --- |
| Phase 1: mirror/proxy sync discipline | **Current** | Desktop enforces the sync token/target contract, runs replay-safe local apply/checkpoints, and uses Spacetime-*shaped* local presence/projection state for panes |
| Phase 2: live remote Spacetime authority | **Target** | Presence/checkpoints/projections backed by live subscriptions and reducers |

Phase 2 never happened. Cycle 2 shipped a correctly-shaped local implementation
with a truthful status label, and stopped there. Acceptance criteria in that
index include "Pane source badges/labels must remain truthful to current phase
semantics" — a direct, well-designed response to the cycle 1 defect.

### 2.4 How cycle 2 ended

- 2026-06-08 (`2f1ba3abd8`): root-level `spacetime/` deprecated alongside
  `proto`, `skills`, `swift`
- 2026-06-09 (`f5919c7669`, "Rebuild openagents as Bun Effect workspace"):
  `crates/autopilot-spacetime`, `scripts/spacetime/`, and the remaining Rust
  desktop integration deleted

Again the removal was collateral: a stack-wide rebuild, not a SpacetimeDB
verdict. Cycle 2's substantive conclusion — that a shaped local projection was
sufficient and the live substrate was never needed — was never written down as
a decision. Nothing recorded "we did not need this," so nothing prevented
cycle 3.

## 3. Cycle 3 — The Verse world backend (2026-06-17 → 2026-06-22)

This is the only cycle that ran in production.

### 3.1 Problem and design

The target was multiplayer presence for the Verse — the 3D agent world behind
the Autopilot chat surface. This is a genuinely good fit for SpacetimeDB and a
much better one than sync transport: shared world state, regions, avatars,
positions, local chat, and emotes are the canonical MMORPG workload the product
is built for. The plan explicitly bounded its authority
(`docs/game/2026-06-21-spacetimedb-verse-multiplayer-audit.md`):

> Pylon/training data stays owned by the existing OpenAgents projection path;
> SpacetimeDB is the multiplayer presence and interaction layer, not the source
> of business or training truth.

That boundary held for the whole cycle and is worth reusing.

### 3.2 What shipped

Module `apps/openagents-world-spacetimedb/src/lib.rs`: **2,180 lines,
21 tables, 27 reducers.**

Presence and interaction tables: `world_region`, `pylon_station`,
`agent_avatar`, `avatar_position`, `avatar_position_near`,
`avatar_position_far`, `pylon_attention`, `local_chat_message`, `chat_bubble`,
`local_emote`, `agent_intent`.

Projection tables: `training_run`, `run_entity`, `world_edge`, `proof_ref`,
`settlement_ref`, `world_event`, `projection_cursor`, `bridge_health`.

Server-side safety was real, not stubbed: a sender could update only its own
avatar; positions were bounded by region extents; writes were interval-throttled
and speed-capped in meters per second; stale positions and ephemeral rows had
expiry windows.

Clients consumed 21 generated TypeScript binding files under
`apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings/`, plus a
desktop adapter (`apps/autopilot-desktop/src/shared/chat-world-spacetimedb.ts`)
and projection scripts that bridged Tassadar summaries, forum activity, and
timelines into the world.

Issues: `#5228`, `#5236`–`#5239`, `#5261`, `#5263`, `#5272`, `#5409`, `#5739`,
`#5825`, `#5887`–`#5893`.

### 3.3 The production deployment

Recorded in `docs/game/2026-06-17-spacetimedb-gcp-deployment-receipt.md`
(deleted with the cutover; recoverable at `3ee0785f51^`):

```text
Endpoint:      https://spacetime.openagents.com
Project:       openagentsgemini   Zone: us-central1-a
Instance:      spacetimedb-world-1 (e2-standard-4, Ubuntu 24.04)
Static IP:     34.28.177.95  (resource: spacetimedb-world-ip)
Data disk:     spacetimedb-world-data-1 (100 GB pd-balanced, /stdb)
Binary:        /stdb/bin/2.6.0/spacetimedb-standalone
Listen:        127.0.0.1:3000 behind nginx
TLS:           Let's Encrypt, SANs spacetime.openagents.com + sslip.io fallback
```

The public boundary was tight and correct — nginx exposed only
`^/v1/database/[^/]+/subscribe$` and `/v1/identity`, denying everything else,
with publish and admin work restricted to IAP SSH.

The receipt is also honest about its own rough edges: DNS initially fell back
to an `sslip.io` name because the available Cloudflare token returned 403 for
DNS mutation, and the installed `/stdb/spacetime` wrapper was flagged as
unusable under `sudo` because of a HOME-relative lookup.

### 3.4 The bug that ended it

On 2026-06-22, commit `f626fd44fa` recorded the proximate cause:

> Desktop multiplayer never delivered rows because the SpacetimeDB SDK exposes
> `connection.db` tables and `connection.reducers` by snake_case schema name
> (`world_region`, `set_avatar_position`, …) while the client read camelCase
> (`worldRegion`, `setAvatarPosition`, …). Every table read resolved to
> undefined -> empty world, no error.

A naming-convention mismatch across a generated-binding seam produced a
silently empty world with no error surface. The fix (snake-first with camelCase
fallback, plus a guarded `joinRegion` retry for rows landing a tick after
`onApplied`) worked. The conclusion drawn from it did not stop at the fix.

### 3.5 The replacement decision

`docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`
records the owner decision the same day:

> **Owner decision (2026-06-22):** No phased/compatibility-mirror approach. Rip
> out SpacetimeDB. Build our own Verse world backend with TypeScript + Effect
> and, ideally, Cloudflare's infrastructure (Durable Objects, WebSockets, D1).

The stated reasoning was ownership and stack alignment, not a defect in
SpacetimeDB. The audit is explicit that SpacetimeDB was the right first
substrate and that it delivered real multiplayer shape quickly. The complaint
was the seam count:

> getting two desktop avatars to see each other required crossing four stacks
> (Rust/WASM module, gcloud/IAP VM publish, generated TS bindings, desktop TS)
> and the failures were all silent, untyped seams […] That class of bug does
> not exist when one team owns one typed contract in one language end to end.

The audit also states plainly that the *contract* was being preserved and only
the *implementation and host* were changing — the world model, invariants,
validation rules, subscription lifetimes, and bridge semantics carried over.

Cutover ran as issues `#5959`–`#5972` (P0–P13), all closed 2026-06-22.
Deletion commit: `3ee0785f51`, "world: delete legacy world backend paths."

**Total production lifetime: 2026-06-17 to 2026-06-22 — five days.**

## 4. Aftermath: the replacement outlived SpacetimeDB by three weeks

The Cloudflare replacement shipped on schedule:

- `packages/world-contract` — Effect Schema for rows, commands, deltas
- `packages/world-client` — the single typed client for desktop and web
- `apps/openagents-world` — Worker + Region Durable Object + D1
- `docs/game/2026-06-22-cloudflare-world-production-release-receipt.md`

Then, on 2026-07-14, commit `cc0ff1e151` ("refactor: enforce Google Cloud
infrastructure authority") deleted `apps/openagents-world` entirely —
`bridge.ts` (721 lines), `commands.ts` (867 lines), `expiry.ts`, their tests,
and the README. Cloudflare Workers, Durable Objects, and D1 were retired as
infrastructure across the whole repository. `apps/openagents-world` is now a
hard-coded retired path in `scripts/google-cloud-authority-guard.mjs`.

Separately, on 2026-07-08 the replacement audit itself was banner-marked
`POSTPONED — parked behind the Khala Code + business focus`.

So the sequence is: a SpacetimeDB world backend ran for five days and was
replaced, on ownership-alignment grounds, by a Cloudflare service that ran for
about three weeks before its entire hosting platform was retired on the same
kind of grounds. Neither host survives. What survives is the contract —
`packages/world-contract` and `packages/world-client`, 5 tracked files each —
which is exactly what the replacement audit predicted would be the durable
part.

The current position (root `CLAUDE.md`) is that the world service has no active
production host, and the 2026-07-24 Omega harvest audit lists "revive
Cloudflare or SpacetimeDB world service authority" under explicit non-goals.

## 5. Current state (verified 2026-08-03)

| Check | Result |
| --- | --- |
| Tracked source outside `docs/` referencing SpacetimeDB | **0 files** |
| `INVARIANTS.md` / `apps/openagents.com/INVARIANTS.md` mentions | **0** |
| Docs mentioning SpacetimeDB | 24 files |
| GCE instance `spacetimedb-world-1` | **absent** |
| Disks `spacetimedb-world-1`, `spacetimedb-world-data-1` | **absent** |
| Static address `spacetimedb-world-ip` | **absent** |
| `spacetime.openagents.com` DNS | **still resolves to `34.28.177.95`** |
| HTTP/HTTPS on that address | no response (timeout) |
| Surviving contract packages | `packages/world-contract`, `packages/world-client` |
| Reference clones (workspace, not this repo) | 12 under `~/work/projects/spacetime/repos/` |

## 6. Findings

### Finding 1 — Dangling DNS record on a released Google Cloud address (live risk)

`spacetime.openagents.com` still resolves to `34.28.177.95`. That address is
**not** in project `openagentsgemini`'s address list, the instance and disks are
gone, nothing answers on 80 or 443, and the reverse DNS is the generic
`95.177.28.34.bc.googleusercontent.com` — meaning the address is back in Google
Cloud's pool and can be allocated to another tenant.

The record is DNS-only (it resolves to the origin address directly, with no
proxy in front), so whoever next receives that address in `us-central1` can
serve content on an `openagents.com` subdomain and obtain a certificate for it.

This was foreseen and deferred rather than missed. The P13 release receipt
states:

> Static address `spacetimedb-world-ip` remains attached to the terminated
> historical instance until a separate DNS/address cleanup decision releases
> the old name and address.

The address was subsequently released. The DNS name never was. The deferred
cleanup decision appears never to have been executed, and issue `#5972` closed
with the decommission marked done.

**Recommended action:** delete the `spacetime.openagents.com` A record in
Cloudflare DNS. Then sweep every other `openagents.com` record for the same
pattern — a record pointing at an address the project no longer reserves. This
audit checked one name; the class deserves a full pass.

### Finding 2 — Three adoptions, no decision record between them

Cycles 1 and 2 both ended by collateral deletion inside unrelated repository
restructures. Neither produced a "we evaluated this and it was not needed"
record. Cycle 2's real conclusion — that a locally-shaped projection satisfied
the requirement and Phase 2 was never justified — exists only as an unreached
"Target" row in a rollout index that was itself deleted three months later.

Cycle 3's authors were solving a different problem and were right to reassess
independently. But the *operational* lessons of cycles 1 and 2 (a hand-operated
deployment lane, a generated-binding seam, and a labels-outran-implementation
failure) were all reproduced in cycle 3, and all three appear in cycle 3's
post-mortem as if new.

**Recommended action:** when a substrate is removed, record the verdict — not
just the deletion — somewhere a future adopter will grep. This document is
intended to be that record for SpacetimeDB.

### Finding 3 — A 65-issue program was opened, closed, and deleted in one day

Every `OA-SPACETIME-*` issue in cycle 1 has `createdAt` and `closedAt` on
2026-02-25, including `OA-SPACETIME-031: Staging Canary Rollout`,
`OA-SPACETIME-032: Production Phased Rollout`, and `OA-SPACETIME-033: Cutover -
Spacetime Default`. No staging canary and no production rollout occurred; there
was no server to roll out to. Two signoff audits declared a migration complete
that had never carried traffic.

The closed-issue record and the signoff documents therefore overstate what was
achieved by a wide margin. Anyone mining this repository's issue history for
delivery evidence should treat the 2026-02-25 `OA-SPACETIME-*` set as design
artifacts, not as shipped work.

**Recommended action:** none required retroactively — the issues are closed and
the code is gone. Noted here so the history is not misread later. The general
control (do not mark a rollout issue closed without a deployment receipt) is
already covered by current completion-gate and receipt discipline.

### Finding 4 — The Khala/Spacetime rename produced self-contradictory normative text

Documented in §1.4. ADR-0009 contains the sentence "canonical doctrine moves
from Spacetime to Spacetime." A global find-and-replace was applied to
normative architecture documents, and the resulting ambiguity was signed off.

Current repository policy already prohibits this class of change: automatic
text substitution is not permitted for normative requirements or identifiers.
This is a concrete historical example of why.

**Recommended action:** none — the affected documents are deleted. Retain the
example.

### Finding 5 — 24 documents still describe SpacetimeDB, only one says it is gone

The surviving mentions split into three groups:

1. **Correctly historical** — `docs/omega/2026-07-24-omega-3d-avatar-verse-harvest-audit.md`
   lists reviving SpacetimeDB authority as a non-goal;
   `docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`
   cites it as a design reference for subscription-as-query.
2. **Superseded with a banner** — `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`
   (60 mentions), banner-marked POSTPONED on 2026-07-08.
3. **Dated snapshots with no supersession pointer** — the June `docs/launch/`
   and `docs/game/` audits, which describe a live SpacetimeDB projection in the
   present tense (for example, "Payment particles now prefer SpacetimeDB Pylon
   station and avatar coordinates").

Group 3 is legitimate as a dated record. The problem is discovery: a reader who
greps `SpacetimeDB` lands on present-tense live-system prose in most hits, and
must already know the outcome to interpret them.

**Recommended action:** link this audit from `docs/spacetime/README.md` (done)
and treat that README as the entry point. Rewriting a dozen dated snapshots is
not worth the churn; a findable index is.

### Finding 6 — Stray untracked directory at a retired path

`apps/openagents-world/` exists in the canonical checkout containing only a
`node_modules` directory, 20 days after the path was deleted and made a guarded
retired path. It is untracked and gitignored, so
`scripts/google-cloud-authority-guard.mjs` does not fail on it, but it makes the
retired path appear to still exist to anyone using `ls`.

**Recommended action:** remove the directory from the working checkout. No
repository change needed.

## 7. Assessment

Three observations that are worth more than the chronology.

**SpacetimeDB was never rejected on its merits.** Cycle 1 died in an unrelated
prune. Cycle 2 died in an unrelated stack rebuild. Cycle 3 was replaced by an
explicit owner decision that names ownership, stack alignment, and seam count —
and that same document states SpacetimeDB "was the right first substrate" and
that it "let us get real multiplayer shape quickly." No cycle produced evidence
that the database failed to do its job. Cycle 3's one real bug was in our
client's accessor naming, not in the database.

**The fit was much better for cycle 3 than for cycles 1 and 2.** Sync transport
already had a working in-house lane with settled `(topic, seq)` semantics;
replacing it was a lateral move that added a vendor, a WASM module, a token
lane, and a deployment surface to reimplement guarantees the system already
had. Multiplayer world state was the opposite: SpacetimeDB's native workload,
and it delivered 21 tables and 27 reducers with server-enforced movement limits
in about five days of work. If this substrate is ever reconsidered, cycle 3's
scoping — presence and interaction only, business and training truth stays in
the owned projection path — is the boundary that worked.

**The durable output of all three cycles was the contract, not the host.**
`packages/world-contract` and `packages/world-client` survived SpacetimeDB,
survived Cloudflare Durable Objects, and survive today with no production host
at all. Cycle 1's `(stream_id, seq)` monotonic apply discipline outlived its
own transport twice. The hosts were the disposable part every time.

## Appendix A — Commit index

Cycle 1 (2026-02-25):

| Commit | Time | Description |
| --- | --- | --- |
| `8fedb5e9be` | 09:16 | typed client crate module |
| `0a0e039a81` | 11:35 | OA-SPACETIME-037 canonicalize docs and runbooks |
| `243187bdc8` | 11:49 | Replace Khala with Spacetime across repo |
| `94463c09b5` | 14:48 | Spacetime-only total convergence and signoff gates |
| `17aa21b544` | 20:58 | prune repo to wgpui + vim and MVP doc only (removal) |
| `31b059dacb` | 21:00 | archive scripts spacetime generated output proto |

Cycle 2 (2026-03-04 → 2026-06-09):

| Commit | Date | Description |
| --- | --- | --- |
| `bd077d264e` | 03-04 | map spacetime vision, current gap, and reintroduction plan |
| `5002ec47af` | 03-04 | restore autopilot-sync module baseline |
| `59f454a6e9` | 03-04 | add autopilot-spacetime client crate to workspace |
| `340b423b45` | 03-04 | parity-chaos release gate harness |
| `deca7e4a8c` | 03-04 | record reintegration program completion |
| `b710dcaa1c` | 03-05 | wire desktop to live spacetime sync |
| `25e33e4bd4` | 03-06 | optional Spacetime chat accelerators |
| `2f1ba3abd8` | 06-08 | deprecate root-level `spacetime/` (removal) |
| `f5919c7669` | 06-09 | Rebuild openagents as Bun Effect workspace (removal) |

Cycle 3 (2026-06-17 → 2026-06-22):

| Commit | Date | Description |
| --- | --- | --- |
| `02161f482a` | 06-17 | Publish openagents world SpacetimeDB module |
| `9ef8e64e5b` | 06-17 | Record SpacetimeDB GCP deployment receipt |
| `bd2ecb8332` | 06-17 | Bridge Tassadar summary into SpacetimeDB |
| `7ecfc007cd` | 06-17 | Add Tassadar SpacetimeDB subscriptions |
| `08a0df5109` | 06-17 | Harden SpacetimeDB GCP operations |
| `1d42040655` | 06-17 | Add SpacetimeDB world interaction schema (#5261) |
| `6e182b0b08` | 06-17 | Render Tassadar pylon agents from SpacetimeDB (#5263) |
| `cdaa377263` | 06-20 | Add SpacetimeDB world region proximity contract |
| `30a62a0d1d` | 06-20 | Connect Desktop Verse to SpacetimeDB |
| `7514c62527` | 06-21 | Audit Verse SpacetimeDB multiplayer path |
| `f626fd44fa` | 06-22 | snake_case SDK accessor fix + replacement decision |
| `3ee0785f51` | 06-22 | world: delete legacy world backend paths (removal) |

Aftermath:

| Commit | Date | Description |
| --- | --- | --- |
| `cc0ff1e151` | 07-14 | enforce Google Cloud infrastructure authority — deletes `apps/openagents-world` |

Note on history: commits from Feb–Mar 2026 appear as duplicate hash pairs
across refs because the history was later rewritten. Where a pair exists, the
hash listed above is the one reachable from current `main` unless stated
otherwise.

## Appendix B — Document index

Deleted, recoverable from Git at the parent of the removal commit:

| Document | Recover at |
| --- | --- |
| `docs/audits/2026-02-25-spacetimedb-openagents-autopilot-audit.md` | `9b2b978949` |
| `docs/audits/2026-02-25-spacetimedb-first-architecture-simplification-audit.md` | `9b2b978949` |
| `docs/audits/2026-02-25-spacetime-final-migration-signoff.md` | `9b2b978949` |
| `docs/audits/2026-02-26-spacetime-only-compliance-signoff.md` | `9b2b978949` |
| `docs/plans/spacetimedb-full-integration.md` | `9b2b978949` |
| `docs/adr/ADR-0009-spacetime-sync-canonical-transport.md` | `9b2b978949` |
| `docs/adr/ADR-0010-spacetime-only-sync-transport-hard-mandate.md` | `9b2b978949` |
| `docs/sync/SPACETIME_*.md` (21 files) | `9b2b978949` |
| `docs/SPACETIME_ROLLOUT_INDEX.md` | `deca7e4a8c` |
| `docs/audits/2026-03-04-spacetime-broader-vision-gap-audit.md` | `bd077d264e` |
| `docs/game/2026-06-17-spacetimedb-openagents-mmo-database-plan.md` | `3ee0785f51^` |
| `docs/game/2026-06-17-spacetimedb-gcp-deployment-receipt.md` | `3ee0785f51^` |
| `docs/game/2026-06-17-spacetimedb-admin-runbook.md` | `3ee0785f51^` |
| `docs/game/2026-06-21-spacetimedb-verse-multiplayer-audit.md` | `3ee0785f51^` |

Retained in the working tree:

| Document | Relevance |
| --- | --- |
| `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md` | The replacement decision (POSTPONED banner) |
| `docs/game/2026-06-22-cloudflare-world-production-release-receipt.md` | Cutover and decommission receipt |
| `docs/omega/2026-07-24-omega-3d-avatar-verse-harvest-audit.md` | Current non-goal statement |
| `docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md` | Design reference |

## Appendix C — Verification commands

```sh
# Every commit that touched a SpacetimeDB-named path
git log --all --format="%ad %h %s" --date=short -- "*spacetime*" "*Spacetime*"

# Cycle 1 same-day build and prune
git log --all --diff-filter=A --format="ADD %ad %h" --date=iso \
  -- docs/sync/SPACETIME_PARITY_HARNESS.md
git log --all --diff-filter=D --format="DEL %ad %h" --date=iso \
  -- docs/sync/SPACETIME_PARITY_HARNESS.md

# Cycle 1 issue program
gh issue list --repo OpenAgentsInc/openagents --state all \
  --search "OA-SPACETIME in:title" --limit 100 \
  --json number,createdAt,closedAt

# Cycle 3 world module shape
git show 3ee0785f51^:apps/openagents-world-spacetimedb/src/lib.rs \
  | grep -c '#\[spacetimedb::table'

# Finding 1 — dangling DNS
dig +short spacetime.openagents.com
dig +short -x 34.28.177.95
CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config \
  gcloud compute addresses list --project openagentsgemini

# Current absence from code
git grep -il spacetimedb -- ':!docs'
```
