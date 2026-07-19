# IDE roadmap ProductSpec and AssuranceSpec crosswalk

Date: 2026-07-19
Status: specification traceability index
Canonical implementation sequence: `docs/ide/ROADMAP.md`

## Authority and use

This document makes the IDE roadmap visible across the complete OpenAgents
specification graph. It does not duplicate product intent, sequence work,
admit an AssuranceSpec, prove a criterion, dispatch an agent, approve a
release, or authorize a public claim.

The authority order remains:

1. the owning ProductSpec states the durable outcome;
2. `docs/ide/ROADMAP.md` names the dependency-ordered IDE-00..19 delivery map
   and honest release-rung vocabulary;
3. `docs/sol/MASTER_ROADMAP.md`, live claims, accepted packets, and current
   repository state determine what may run now;
4. an exact-revision AssuranceSpec states proposed or admitted proof design;
5. observed receipts, independent review, owner disposition, release policy,
   and the promise registry determine what may be accepted or claimed.

An IDE packet row below means “this product criterion owns that outcome.” It
does not mean the packet is admitted, implemented, assured, or complete.

## ProductSpec inventory disposition

| ProductSpec                                                      | Revision after reconciliation | IDE roadmap relationship                                                                                          | Disposition                                                              |
| ---------------------------------------------------------------- | ----------------------------: | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `specs/desktop/desktop-trust-complete-workbench.product-spec.md` |                             7 | Primary Desktop intent for IDE-00..19, built-in Vim, Tokyo Night, Effect/Rust ownership, and release-rung honesty | Material intent reconciled in rev 7                                      |
| `specs/openagents/cursor-capability-parity.product-spec.md`      |                             3 | Cross-surface breadth and IDE-19 closure authority                                                                | Material intent reconciled in rev 3                                      |
| `specs/openagents/portable-coding-sessions.product-spec.md`      |                             4 | IDE-13 portable capability and IDE-14 continuation substrate                                                      | Material intent reconciled in rev 4                                      |
| `specs/openagents/managed-agent-sandboxes.product-spec.md`       |                             1 | Concrete GCP managed-sandbox dependency for IDE-13 and IDE-17; Box compatibility remains an adapter               | New owner-directed intent under epic #9023                               |
| `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`  |                             7 | IDE-14 bounded mobile review, supervision, and Desktop handoff                                                    | Material intent reconciled in rev 7                                      |
| `specs/web/openagents-com-trust-surface.product-spec.md`         |                             7 | IDE-14 authenticated review and `CodeShareBundle` publication                                                     | Material intent reconciled in rev 7                                      |
| `specs/desktop/full-auto.product-spec.md`                        |                            14 | Existing Full Auto/automation runtime consumed by IDE-17                                                          | Reused without byte changes so AssuranceSpec rev 4 remains exactly bound |
| `specs/openagents/fast-follow.product-spec.md`                   |                             4 | Existing current-evidence refresh and Cursor gap generation consumed by IDE-19                                    | Reused without changing learning intent                                  |
| `specs/openagents/authority-delegation.product-spec.md`          |                             5 | Governs implementation authority independently of IDE product scope                                               | No IDE intent change                                                     |
| `specs/openagents/sarah-owner-orchestrator.product-spec.md`      |                             4 | May create and supervise the admitted managed sandbox through its own closed broker; owns no IDE outcome           | Rev 4 adds Sarah sandbox action intent; runtime authority remains gated  |
| `specs/web/openagents-com-sales-landing.product-spec.md`         |                             2 | Sales copy cannot outrun IDE promise/release gates                                                                | No IDE intent change                                                     |

The closed MVP ProductSpecs under `docs/mvp/` remain historical exact-revision
subjects. The IDE roadmap is their follow-on product program; it does not
rewrite RC evidence, old criterion identities, owner admission, or the bytes
bound by their AssuranceSpecs.

## AssuranceSpec inventory disposition

| AssuranceSpec                                                                    | Exact subject                                            | Lifecycle                                                                                                                                                           | IDE roadmap meaning                                                                                                                                                               |
| -------------------------------------------------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `specs/desktop/desktop-trust-complete-workbench.assurance-spec.md`               | Desktop ProductSpec rev 7, AC-1..AC-52                   | `proposed`; mechanically bridged from the validated structured item list and rebound to the unchanged original-byte digest; every obligation remains `needs_design` | Creates exact criterion coverage for the primary IDE contract while disclosing the current proposer/session structured-item extraction gap; claims no proof adequacy or execution |
| `specs/openagents/cursor-capability-parity.assurance-spec.md`                    | Cursor parity ProductSpec rev 3, CP-AC-01..CP-AC-27      | `proposed`; generated coverage skeleton; every obligation remains `needs_design`                                                                                    | Creates exact criterion coverage for the parity gate without claiming any ledger row is accepted                                                                                  |
| `specs/desktop/full-auto.assurance-spec.md`                                      | Full Auto ProductSpec rev 14, FA-AC-01..FA-AC-76         | `proposed`; assurance rev 4; mixed designed/needs-design state stated in that file                                                                                  | Existing IDE-17 dependency; intentionally not rebound because Full Auto intent did not change                                                                                     |
| `specs/openagents/sarah-owner-orchestrator.assurance-spec.md`                    | Sarah ProductSpec rev 4, SARAH-AC-01..23                 | `proposed`; assurance rev 2; every obligation remains `needs_design`                                                                                               | Rebound for the sandbox action intent; SBX-00 still owns exact proof design and authority admission                                                                               |
| `docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md` and Phase-2 proposals | Exact frozen MVP subjects stated in `docs/mvp/README.md` | mixed admitted/proposed historical states                                                                                                                           | Historical proof remains byte- and revision-stable; no IDE closure is inferred                                                                                                    |

The proposed Desktop and Cursor companions are starting points for human proof
design. Neither selects a repository candidate, Environment Profile, proof
technique, oracle, falsifier, reviewer, or gate. They are not admitted or
authorized to execute and cannot be used as release evidence.

## IDE-00..19 criterion map

| Packet                                            | Product intent bindings                                                                                                         | Assurance binding                                                                                                          | Required truth before the packet can support its rung                                                                                                                                               |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| IDE-00 — project graph/baseline                   | Desktop AC-39..47, AC-50..52; Cursor CP-AC-19..23, CP-AC-26..27                                                                 | Desktop and Cursor proposed companions                                                                                     | Stable generation-fenced refs, explicit baseline, one Schema-first contract graph, and no status promotion from design prose                                                                        |
| IDE-01 — dependency/theme/Vim/native spikes       | Desktop AC-47..49, AC-52; Cursor CP-AC-23..25, CP-AC-27                                                                         | Desktop and Cursor proposed companions                                                                                     | License/package/worker/CSP/offline proof, Tokyo Night mapping, Vim engine decision/fallback, and Rust necessity/reversal evidence                                                                   |
| IDE-02 — complete Pierre path index               | Desktop AC-39..40; Cursor CP-AC-19                                                                                              | Desktop and Cursor proposed companions                                                                                     | Delivered by #9017: complete generation-fenced tree behavior, cancellation/staleness, multi-root/worktree isolation, honest partial/degraded states, typed operations, accessibility, scale/resource and packaged-journey receipts; does not close the broader criteria |
| IDE-03 — Monaco/Tokyo Night/Vim                   | Desktop AC-39..41, AC-48..51; Cursor CP-AC-19, CP-AC-24..26                                                                     | Desktop and Cursor proposed companions                                                                                     | Delivered by #9018: opaque model/view lifecycle, Effect-owned draft/revision/recovery truth, fixed Tokyo Night, first-party persistent Vim, gap/resync fences, lazy private-scheme package, and resource/Finder receipts; does not close broader criteria |
| IDE-04 — navigation/groups/settings/keymaps       | Desktop AC-11, AC-12, AC-39..41, AC-48; Cursor CP-AC-03, CP-AC-19, CP-AC-24                                                     | Desktop and Cursor proposed companions                                                                                     | Delivered by #9019: one command graph, stable navigation/groups, bounded Quick Open, typed settings/keymaps and visible Vim precedence; broader criteria remain open                                |
| IDE-05 — Pierre review classes                    | Desktop AC-5, AC-6, AC-41, AC-43; Cursor CP-AC-04, CP-AC-20                                                                     | Desktop and Cursor proposed companions                                                                                     | Delivered by #9020: eight versioned diff classes remain distinct, stale bases refuse, Pierre stays projection-only, and mutation authority remains deferred to its owning packet                     |
| IDE-06 — language/Problems                        | Desktop AC-42; Cursor CP-AC-19..20                                                                                              | Desktop and Cursor proposed companions                                                                                     | Delivered by #9021: visible local/project tiers, full first TypeScript capability corpus, document/service generations, cancellation, supervised restart, placement/evidence, shared projections, and unavailable truth; IDE-07 subsequently accepted the integrated rung |
| IDE-07 — daily-use basic IDE gate                 | Desktop AC-39..42, AC-48..52; Cursor CP-AC-18..19, CP-AC-24..27                                                                 | Desktop and Cursor proposed companions                                                                                     | Delivered by #9022 for exact macOS arm64 candidate `48c32a1d4c`: 15/15 matrix rows and 27/27 metrics pass; release says only “OpenAgents basic IDE”; every later gap and epic owner disposition stay visible |
| IDE-08 — agent context/proposals/backlinks        | Desktop AC-17, AC-43; Cursor CP-AC-20                                                                                           | Desktop and Cursor proposed companions                                                                                     | Inspectable disclosure, exact proposal base, review/apply/undo receipts, post-apply evidence, and no harness editor authority                                                                       |
| IDE-09 — AI completion/inline/multi-file editing  | Desktop AC-26, AC-28; Cursor CP-AC-04, CP-AC-12..13                                                                             | Desktop and Cursor proposed companions                                                                                     | Quality/latency corpus, exact effective model/harness/placement/data disclosure, and canonical accept/reject/undo                                                                                   |
| IDE-10 — terminal/tasks/tests/Output              | Desktop AC-42, AC-44, AC-47; Cursor CP-AC-19, CP-AC-23                                                                          | Desktop and Cursor proposed companions                                                                                     | Effect-owned identity/retention/evidence; xterm projection; narrowly admitted PTY helper; restart/gap/late-event proof                                                                              |
| IDE-11 — debug/DAP                                | Desktop AC-42, AC-47; Cursor CP-AC-19, CP-AC-23                                                                                 | Desktop and Cursor proposed companions                                                                                     | Effect-owned DAP graph, adapter supervision, late-event fencing, and no native application authority                                                                                                |
| IDE-12 — Git/worktrees/delivery                   | Desktop AC-6, AC-17, AC-43; Cursor CP-AC-06, CP-AC-20                                                                           | Desktop and Cursor proposed companions                                                                                     | Expected-version mutation, collision-safe worktrees, distinct delivery/verification/acceptance, and exact Git receipts                                                                              |
| IDE-13 — portable project capabilities            | Desktop AC-45, AC-50..51; Cursor CP-AC-09, CP-AC-21, CP-AC-26..27; Portable Sessions rev 4; Managed Sandboxes MSB-AC-01..14, AC-17..18 | Desktop and Cursor proposed companions; managed-sandbox AssuranceSpec remains SBX-00 work                                   | Same command/results and stable refs across placements; exclusive attachment; fresh destination admission; real GCP readiness, lifecycle and cleanup; no raw roots/native state/Vim authority movement |
| IDE-14 — mobile/web/share projections             | Desktop AC-46, AC-50; Cursor CP-AC-09, CP-AC-22, CP-AC-26; Mobile AC-21..27; Web AC-22..31                                      | Desktop and Cursor proposed companions provide cross-surface umbrella; target-specific proof design remains to be authored | Bounded Schema-decoded refs/content, review/handoff, Tokyo Night semantic subset, share redaction/audience/revocation, and zero editor/runtime authority                                            |
| IDE-15 — isolated extension/component ABI         | Desktop AC-30; Cursor CP-AC-11                                                                                                  | Desktop and Cursor proposed companions                                                                                     | Signed provenance, capability manifests, isolation, host-effect brokering, compatibility, rollback, inventory, and no trusted extension host                                                        |
| IDE-16 — browser/preview/design/computer use      | Desktop AC-27; Cursor CP-AC-07                                                                                                  | Desktop and Cursor proposed companions                                                                                     | Partitioned browser, explicit server/network/OS authority, secret handling, approvals, per-action receipts, and deny/ask default                                                                    |
| IDE-17 — agent platform/automations               | Desktop AC-4, AC-20..24, AC-31; Cursor CP-AC-03, CP-AC-05..08; Full Auto FA-AC-38..76; Managed Sandboxes MSB-AC-06..14, AC-17..18 | Desktop and Cursor proposed companions plus existing Full Auto AssuranceSpec rev 4; sandbox proof is separate               | Editor and Agents Window share one graph; local and managed background work has leases, budgets, intervention, reports, exact lifecycle and cleanup; Full Auto cross-machine admission remains excluded |
| IDE-18 — custody/migration/platform/accessibility | Desktop AC-7, AC-9, AC-29, AC-32, AC-49; Cursor CP-AC-13..18, CP-AC-25; Mobile AC-26; Web AC-24..30                             | Desktop and Cursor proposed companions                                                                                     | Complete data inventory/erasure convergence, safe Cursor import, enterprise/distribution evidence, localization/accessibility, and deferred light/high-contrast/system themes                       |
| IDE-19 — maintained Cursor closure                | Desktop AC-25, AC-52; Cursor CP-AC-01..02, CP-AC-16, CP-AC-27; Fast Follow FF-AC-16                                             | Cursor proposed companion is the primary parity proof-design seed                                                          | Every current ledger row has exact evidence, implementation, acceptance, assurance, placement/data posture, owner disposition, and promise gate; no required gap remains                            |

### IDE-00 implementation evidence

IDE-00 is implemented by the exact receipt in
[#9015](https://github.com/OpenAgentsInc/openagents/issues/9015). Its admitted
code evidence is the schema-first graph and scoped Effect service under
`apps/openagents-desktop/src/ide/`, the schema-derived shipped Files/recovery
contracts, the `check:ide-boundaries` architecture guard, the public-safe
p50/p95/p99 baseline receipts, and behavior contract
`openagents_desktop.ide_project_generation_fencing.v1`. The issue's closing
comment is authoritative for the final `main` SHA and verification commands.

That evidence supports only the IDE-00 foundation row. It does not change the
proposed lifecycle of either AssuranceSpec, satisfy IDE-01–07, promote the
Daily-use basic IDE rung, or close any later Cursor parity row.

### IDE-01 implementation evidence

IDE-01 is implemented by the exact receipt in
[#9016](https://github.com/OpenAgentsInc/openagents/issues/9016) and the full
decision record in
`docs/ide/2026-07-19-ide-01-package-admission.md`. Its admitted evidence is:

- exact Monaco/Pierre pins, immutable identities, license/provenance,
  dependency/cost/compatibility matrices, rollback plans, and schema-enforced
  no-authority audits;
- real development/ASAR, restrictive-CSP, offline worker and asset probes with
  repeated disposal, injected failure, virtualized 200-file diff scale, and a
  deterministic Tokyo Night visual fixture;
- an ordinary-chat startup A/B and opt-in lazy bundle/source-map/renderer
  memory receipts that keep package presence off the ordinary renderer graph;
- the 32-capability first-party Vim fallback decision after two explicit
  package rejections;
- one owned Tokyo Night semantic projection with checked provenance and
  contrast adjustment; and
- a 10,000-file TypeScript search/index/watch benchmark with explicit Rust
  rejection and reconsideration gates.

Behavior contract `openagents_desktop.ide_package_admission.v1` and the
schema-decoded receipts under `apps/openagents-desktop/benchmarks/ide/` guard
that result. This evidence supports only the IDE-01 foundation row. It does not
promote either proposed AssuranceSpec, mount Monaco/Tokyo Night/Vim in the
production workbench, satisfy IDE-02–07, or authorize a daily-use basic-IDE,
Zed-quality, or Cursor-parity claim.

### IDE-03 implementation evidence

IDE-03 is implemented by the exact receipt in
[#9018](https://github.com/OpenAgentsInc/openagents/issues/9018) and the full
delivery record in
`docs/ide/2026-07-19-ide-03-monaco-vim-tokyo-night.md`. Its observed evidence
includes:

- one lazy packaged Monaco island keyed by branded opaque document refs, with
  paths kept mutable and roots/grants/bridges withheld;
- schema-derived generation, sequence, model-version, view, selection, edit,
  Vim-status, resource, benchmark, and packaged-journey contracts;
- Effect-owned draft, disk revision, conflict, dirty, selection, and version-3
  restart recovery with stale-generation refusal and sequence-gap resync;
- fixed Tokyo Night from native-window/first-paint through React/Effect Native,
  Pierre, and Monaco, with no model recreation;
- a persistent, off-by-default app-owned Vim controller that routes save/close
  through typed authority and finalizes mappings on disable, blur, and scope
  disposal;
- p50/p95/p99 1 MB open/edit/gap and 12-tab recovery measurements, ordinary-
  boot/editor/worker byte accounting, zero active-resource delta, and an
  explicit TypeScript-over-Rust placement decision; and
- a packaged macOS LaunchServices/Finder journey proving editable Monaco,
  recovery, two split views, Vim toggle, private-scheme assets, root
  withholding, production-textarea absence, and zero stopped resources.

Behavior contract `openagents_desktop.ide_monaco_document_runtime.v1` and the
schema-decoded receipts under `apps/openagents-desktop/benchmarks/ide/` guard
that result. This evidence supports only the IDE-03 delivery row. It does not
promote either proposed AssuranceSpec, satisfy IDE-04–07, establish the daily-
use basic-IDE rung, or authorize Zed-quality or Cursor-parity language. The
ProductSpec bytes and both AssuranceSpec exact subjects remain unchanged;
therefore their revisions, digests, and `proposed` lifecycles are not rebound
by this implementation receipt.

### IDE-04 through IDE-06 implementation evidence

IDE-04, IDE-05, and IDE-06 are implemented by the exact closing receipts in
[#9019](https://github.com/OpenAgentsInc/openagents/issues/9019),
[#9020](https://github.com/OpenAgentsInc/openagents/issues/9020), and
[#9021](https://github.com/OpenAgentsInc/openagents/issues/9021). Their full
records are:

- `docs/ide/2026-07-19-ide-04-daily-workbench.md`;
- `docs/ide/2026-07-19-ide-05-versioned-pierre-review.md`; and
- `docs/ide/2026-07-19-ide-06-generation-safe-language.md`.

IDE-06 adds the enforced behavior contract
`openagents_desktop.ide_generation_safe_language.v1` and a real TypeScript
6.0.3 worker receipt covering exact version/environment, p50/p95/p99 latency,
100-request supersession, forced crash/restart generation advance, offline
operation, and zero-worker/pending teardown. Its renderer projections remain
root-redacted and apply edits only through exact canonical document authority.

This implementation evidence does not change either AssuranceSpec from
`proposed`, satisfy the complete Desktop AC-42 or Cursor parity criteria by
itself, or admit another language/LSP. IDE-07 subsequently accepted only the
narrow integrated basic-IDE rung.

### IDE-07 implementation evidence

IDE-07 is implemented by the exact closing receipt in
[#9022](https://github.com/OpenAgentsInc/openagents/issues/9022) and the full
record in `docs/ide/2026-07-19-ide-07-basic-ide-acceptance.md`. Candidate
`48c32a1d4c2f9ff84d8e92fe1c9ab074096b1fec` binds one 360-file macOS arm64
Forge app tree by SHA-256. Its schema-first, non-overridable evaluator checks
all seven closed child packets, all 15 daily-use matrix rows, all 27 frozen
p50/p95/p99 metric rows, the packaged Finder/editor/review/language/Vim/
offline/disposal journey, seven zero-IDE-cost chat launches, Effect custody,
rollback, public safety, target unavailability, and exact claim language.

That receipt admits only **OpenAgents basic IDE**. It does not promote either
AssuranceSpec from `proposed`, satisfy the full Desktop or Cursor criteria,
claim another platform, imply Zed quality, close IDE-08..19, close epic #9014,
or replace its human owner disposition.

## Cross-surface defaults and non-authorities

| Concern                | Desktop authority                                                                            | Mobile contract                                              | Web/public contract                                                     |
| ---------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------- |
| Editor                 | Complete Editor mode over Monaco behind the Effect document service                          | No general editor; bounded review/small staged commands only | No editor or mutation; bounded supervision/share only                   |
| Vim                    | Built in, packaged, persistent, off by default; app-owned controller                         | Does not store, toggle, or interpret Desktop Vim state       | Does not store, toggle, or interpret Desktop Vim state                  |
| Initial theme          | Owned Khala editor semantic projection across all default-mounted IDE/workbench adapters; Tokyo Night retained as built-in fallback | Allowlisted review-token subset only                         | Allowlisted review-token subset only; no executable theme bytes         |
| Later themes           | Light, high-contrast dark/light, and system-following remain IDE-18/full-parity requirements | May consume safe semantic projections after admission        | May consume safe semantic projections after admission                   |
| Project state          | One generation-fenced Effect graph                                                           | Schema-decoded bounded refs and evidence                     | Schema-decoded bounded refs and `CodeShareBundle`                       |
| Runtime/native helpers | Effect authority; Rust only PTY/containment or benchmark-admitted kernel                     | No Monaco/LSP/Git/PTY/Rust runtime                           | No Monaco/LSP/Git/PTY/Rust runtime                                      |
| Release claims         | Must name the exact rung and gaps                                                            | Cannot promote Desktop/system rung                           | Cannot promote Desktop/system rung; public copy also needs promise gate |

## Effect and Rust architecture obligations

These requirements are product intent, not merely implementation-style advice:

- boundary authority is one identified `Schema.Struct`,
  `Schema.TaggedStruct`, or `Schema.TaggedUnion`; TypeScript types derive from
  the schema, scalar refs are constrained/branded, and raw interfaces or
  handwritten unions cannot become parallel persisted/wire contracts;
- application capabilities are `Context.Service`s implemented with
  `Layer.effect`; public and non-trivial operations are named `Effect.fn`s;
  domain failures are `Schema.TaggedErrorClass` values; untrusted inputs decode
  at entry;
- project generations own scoped watchers, language/debug processes, terminal
  children, streams, subscriptions, adapters, and teardown; late output cannot
  outlive or regain authority after its scope closes;
- Rust owns no project, document, session, identity, policy, credential,
  database, approval, projection, or receipt. PTY/containment and a separately
  benchmark-admitted kernel are the only default native categories, each with
  generated bounded contracts, failure semantics, conformance, and reversal;
- Monaco, Pierre, xterm, LSP/tsserver/DAP/Git/harness executables, Vim engine,
  and theme adapters are replaceable mechanics. Their presence proves no
  ProductSpec criterion by itself.

## Release-rung proof labels

| Rung                  | Minimum roadmap boundary                                                                  | Permitted specification language                                         | Forbidden inference                                                                                |
| --------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| Files foundation      | Existing Files/Finder substrate                                                           | “Files mode”, “editor-first file open”                                   | Monaco, basic IDE, Zed quality, or Cursor parity                                                   |
| Daily-use basic IDE   | IDE-00..07 accepted at their required proof rung                                          | “OpenAgents basic IDE”                                                   | Agent IDE, portable platform, or parity                                                            |
| Agent IDE             | IDE-08..12 plus the basic-IDE base accepted                                               | “integrated OpenAgents agent IDE”                                        | Cross-host/platform/ecosystem or parity completion                                                 |
| Portable IDE platform | IDE-13..18 plus dependencies accepted                                                     | “Cursor-parity candidate” only                                           | “Cursor parity”, “full parity”, or “drop-in replacement”                                           |
| Full parity           | IDE-19, every required ledger row owner-accepted, ProductSpec and promise gates satisfied | “Cursor parity” or “full parity” only for the exact release/evidence set | Promotion from package presence, architecture, screenshot, fixture, proposal, or agent self-report |

## Maintenance rule

Whenever IDE product intent changes:

1. increment the owning ProductSpec revision;
2. update this crosswalk and the canonical roadmap references without copying
   sequencing into the ProductSpec;
3. explicitly rebind or supersede every exact-digest AssuranceSpec companion;
4. keep proposed, admitted, designed, executed, observed, owner-accepted,
   released, and publicly promised states separate;
5. preserve frozen MVP and historical receipt subjects rather than relabeling
   them as evidence for the new revision.
