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

| ProductSpec | Revision after reconciliation | IDE roadmap relationship | Disposition |
| --- | ---: | --- | --- |
| `specs/desktop/desktop-trust-complete-workbench.product-spec.md` | 7 | Primary Desktop intent for IDE-00..19, built-in Vim, Tokyo Night, Effect/Rust ownership, and release-rung honesty | Material intent reconciled in rev 7 |
| `specs/openagents/cursor-capability-parity.product-spec.md` | 3 | Cross-surface breadth and IDE-19 closure authority | Material intent reconciled in rev 3 |
| `specs/openagents/portable-coding-sessions.product-spec.md` | 4 | IDE-13 portable capability and IDE-14 continuation substrate | Material intent reconciled in rev 4 |
| `specs/mobile/mobile-any-host-fleet-controller.product-spec.md` | 7 | IDE-14 bounded mobile review, supervision, and Desktop handoff | Material intent reconciled in rev 7 |
| `specs/web/openagents-com-trust-surface.product-spec.md` | 7 | IDE-14 authenticated review and `CodeShareBundle` publication | Material intent reconciled in rev 7 |
| `specs/desktop/full-auto.product-spec.md` | 14 | Existing Full Auto/automation runtime consumed by IDE-17 | Reused without byte changes so AssuranceSpec rev 4 remains exactly bound |
| `specs/openagents/fast-follow.product-spec.md` | 4 | Existing current-evidence refresh and Cursor gap generation consumed by IDE-19 | Reused without changing learning intent |
| `specs/openagents/authority-delegation.product-spec.md` | 5 | Governs implementation authority independently of IDE product scope | No IDE intent change |
| `specs/openagents/sarah-owner-orchestrator.product-spec.md` | 3 | May supervise admitted IDE work through existing capabilities; owns no IDE outcome | No IDE intent change |
| `specs/web/openagents-com-sales-landing.product-spec.md` | 2 | Sales copy cannot outrun IDE promise/release gates | No IDE intent change |

The closed MVP ProductSpecs under `docs/mvp/` remain historical exact-revision
subjects. The IDE roadmap is their follow-on product program; it does not
rewrite RC evidence, old criterion identities, owner admission, or the bytes
bound by their AssuranceSpecs.

## AssuranceSpec inventory disposition

| AssuranceSpec | Exact subject | Lifecycle | IDE roadmap meaning |
| --- | --- | --- | --- |
| `specs/desktop/desktop-trust-complete-workbench.assurance-spec.md` | Desktop ProductSpec rev 7, AC-1..AC-52 | `proposed`; mechanically bridged from the validated structured item list and rebound to the unchanged original-byte digest; every obligation remains `needs_design` | Creates exact criterion coverage for the primary IDE contract while disclosing the current proposer/session structured-item extraction gap; claims no proof adequacy or execution |
| `specs/openagents/cursor-capability-parity.assurance-spec.md` | Cursor parity ProductSpec rev 3, CP-AC-01..CP-AC-27 | `proposed`; generated coverage skeleton; every obligation remains `needs_design` | Creates exact criterion coverage for the parity gate without claiming any ledger row is accepted |
| `specs/desktop/full-auto.assurance-spec.md` | Full Auto ProductSpec rev 14, FA-AC-01..FA-AC-76 | `proposed`; assurance rev 4; mixed designed/needs-design state stated in that file | Existing IDE-17 dependency; intentionally not rebound because Full Auto intent did not change |
| `specs/openagents/sarah-owner-orchestrator.assurance-spec.md` | Sarah ProductSpec rev 3, SARAH-AC-01..20 | `proposed` | No IDE proof ownership; retained unchanged |
| `docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md` and Phase-2 proposals | Exact frozen MVP subjects stated in `docs/mvp/README.md` | mixed admitted/proposed historical states | Historical proof remains byte- and revision-stable; no IDE closure is inferred |

The proposed Desktop and Cursor companions are starting points for human proof
design. Neither selects a repository candidate, Environment Profile, proof
technique, oracle, falsifier, reviewer, or gate. They are not admitted or
authorized to execute and cannot be used as release evidence.

## IDE-00..19 criterion map

| Packet | Product intent bindings | Assurance binding | Required truth before the packet can support its rung |
| --- | --- | --- | --- |
| IDE-00 — project graph/baseline | Desktop AC-39..47, AC-50..52; Cursor CP-AC-19..23, CP-AC-26..27 | Desktop and Cursor proposed companions | Stable generation-fenced refs, explicit baseline, one Schema-first contract graph, and no status promotion from design prose |
| IDE-01 — dependency/theme/Vim/native spikes | Desktop AC-47..49, AC-52; Cursor CP-AC-23..25, CP-AC-27 | Desktop and Cursor proposed companions | License/package/worker/CSP/offline proof, Tokyo Night mapping, Vim engine decision/fallback, and Rust necessity/reversal evidence |
| IDE-02 — complete Pierre path index | Desktop AC-39..40; Cursor CP-AC-19 | Desktop and Cursor proposed companions | Complete generation-fenced tree behavior, cancellation/staleness, multi-root/worktree isolation, and honest partial/degraded states |
| IDE-03 — Monaco/Tokyo Night/Vim | Desktop AC-39..41, AC-48..51; Cursor CP-AC-19, CP-AC-24..26 | Desktop and Cursor proposed companions | Canonical document authority outside Monaco, one theme projection, first-party Vim, derived contracts, scoped teardown, and packaged/offline proof |
| IDE-04 — navigation/groups/settings/keymaps | Desktop AC-11, AC-12, AC-39..41, AC-48; Cursor CP-AC-03, CP-AC-19, CP-AC-24 | Desktop and Cursor proposed companions | One command graph, stable navigation and group state, typed setting precedence, and visible keybinding/Vim conflicts |
| IDE-05 — Pierre review classes | Desktop AC-5, AC-6, AC-41, AC-43; Cursor CP-AC-04, CP-AC-20 | Desktop and Cursor proposed companions | Versioned diff sources remain distinct; stale bases refuse; review/apply/undo uses canonical authority |
| IDE-06 — language/Problems | Desktop AC-42; Cursor CP-AC-19..20 | Desktop and Cursor proposed companions | Document/service generations, cancellation, restart, placement/evidence tier, and unavailable truth are observable |
| IDE-07 — daily-use basic IDE gate | Desktop AC-39..42, AC-48..52; Cursor CP-AC-18..19, CP-AC-24..27 | Desktop and Cursor proposed companions | Packaged integrated corpus passes; release says only “daily-use basic IDE”; all later gaps stay visible |
| IDE-08 — agent context/proposals/backlinks | Desktop AC-17, AC-43; Cursor CP-AC-20 | Desktop and Cursor proposed companions | Inspectable disclosure, exact proposal base, review/apply/undo receipts, post-apply evidence, and no harness editor authority |
| IDE-09 — AI completion/inline/multi-file editing | Desktop AC-26, AC-28; Cursor CP-AC-04, CP-AC-12..13 | Desktop and Cursor proposed companions | Quality/latency corpus, exact effective model/harness/placement/data disclosure, and canonical accept/reject/undo |
| IDE-10 — terminal/tasks/tests/Output | Desktop AC-42, AC-44, AC-47; Cursor CP-AC-19, CP-AC-23 | Desktop and Cursor proposed companions | Effect-owned identity/retention/evidence; xterm projection; narrowly admitted PTY helper; restart/gap/late-event proof |
| IDE-11 — debug/DAP | Desktop AC-42, AC-47; Cursor CP-AC-19, CP-AC-23 | Desktop and Cursor proposed companions | Effect-owned DAP graph, adapter supervision, late-event fencing, and no native application authority |
| IDE-12 — Git/worktrees/delivery | Desktop AC-6, AC-17, AC-43; Cursor CP-AC-06, CP-AC-20 | Desktop and Cursor proposed companions | Expected-version mutation, collision-safe worktrees, distinct delivery/verification/acceptance, and exact Git receipts |
| IDE-13 — portable project capabilities | Desktop AC-45, AC-50..51; Cursor CP-AC-09, CP-AC-21, CP-AC-26..27; Portable Sessions rev 4 acceptance set | Desktop and Cursor proposed companions; no separate portable proposal yet | Same command/results and stable refs across placements; exclusive attachment; fresh destination admission; no raw roots/native state/Vim authority movement |
| IDE-14 — mobile/web/share projections | Desktop AC-46, AC-50; Cursor CP-AC-09, CP-AC-22, CP-AC-26; Mobile AC-21..27; Web AC-22..31 | Desktop and Cursor proposed companions provide cross-surface umbrella; target-specific proof design remains to be authored | Bounded Schema-decoded refs/content, review/handoff, Tokyo Night semantic subset, share redaction/audience/revocation, and zero editor/runtime authority |
| IDE-15 — isolated extension/component ABI | Desktop AC-30; Cursor CP-AC-11 | Desktop and Cursor proposed companions | Signed provenance, capability manifests, isolation, host-effect brokering, compatibility, rollback, inventory, and no trusted extension host |
| IDE-16 — browser/preview/design/computer use | Desktop AC-27; Cursor CP-AC-07 | Desktop and Cursor proposed companions | Partitioned browser, explicit server/network/OS authority, secret handling, approvals, per-action receipts, and deny/ask default |
| IDE-17 — agent platform/automations | Desktop AC-4, AC-20..24, AC-31; Cursor CP-AC-03, CP-AC-05..08; Full Auto FA-AC-38..76, especially FA-AC-54..55 and FA-AC-67..68 | Desktop and Cursor proposed companions plus existing Full Auto AssuranceSpec rev 4 | Editor and Agents Window share one graph; parallel/background/triggered work has leases, budgets, intervention, reports, and honest effective identity; Full Auto proof remains independently bound |
| IDE-18 — custody/migration/platform/accessibility | Desktop AC-7, AC-9, AC-29, AC-32, AC-49; Cursor CP-AC-13..18, CP-AC-25; Mobile AC-26; Web AC-24..30 | Desktop and Cursor proposed companions | Complete data inventory/erasure convergence, safe Cursor import, enterprise/distribution evidence, localization/accessibility, and deferred light/high-contrast/system themes |
| IDE-19 — maintained Cursor closure | Desktop AC-25, AC-52; Cursor CP-AC-01..02, CP-AC-16, CP-AC-27; Fast Follow FF-AC-16 | Cursor proposed companion is the primary parity proof-design seed | Every current ledger row has exact evidence, implementation, acceptance, assurance, placement/data posture, owner disposition, and promise gate; no required gap remains |

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

## Cross-surface defaults and non-authorities

| Concern | Desktop authority | Mobile contract | Web/public contract |
| --- | --- | --- | --- |
| Editor | Complete Editor mode over Monaco behind the Effect document service | No general editor; bounded review/small staged commands only | No editor or mutation; bounded supervision/share only |
| Vim | Built in, packaged, persistent, off by default; app-owned controller | Does not store, toggle, or interpret Desktop Vim state | Does not store, toggle, or interpret Desktop Vim state |
| Initial theme | One owned Tokyo Night semantic projection across all mounted IDE/workbench adapters | Allowlisted review-token subset only | Allowlisted review-token subset only; no executable theme bytes |
| Later themes | Light, high-contrast dark/light, and system-following remain IDE-18/full-parity requirements | May consume safe semantic projections after admission | May consume safe semantic projections after admission |
| Project state | One generation-fenced Effect graph | Schema-decoded bounded refs and evidence | Schema-decoded bounded refs and `CodeShareBundle` |
| Runtime/native helpers | Effect authority; Rust only PTY/containment or benchmark-admitted kernel | No Monaco/LSP/Git/PTY/Rust runtime | No Monaco/LSP/Git/PTY/Rust runtime |
| Release claims | Must name the exact rung and gaps | Cannot promote Desktop/system rung | Cannot promote Desktop/system rung; public copy also needs promise gate |

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

| Rung | Minimum roadmap boundary | Permitted specification language | Forbidden inference |
| --- | --- | --- | --- |
| Files foundation | Existing Files/Finder substrate | “Files mode”, “editor-first file open” | Monaco, basic IDE, Zed quality, or Cursor parity |
| Daily-use basic IDE | IDE-00..07 accepted at their required proof rung | “OpenAgents basic IDE” | Agent IDE, portable platform, or parity |
| Agent IDE | IDE-08..12 plus the basic-IDE base accepted | “integrated OpenAgents agent IDE” | Cross-host/platform/ecosystem or parity completion |
| Portable IDE platform | IDE-13..18 plus dependencies accepted | “Cursor-parity candidate” only | “Cursor parity”, “full parity”, or “drop-in replacement” |
| Full parity | IDE-19, every required ledger row owner-accepted, ProductSpec and promise gates satisfied | “Cursor parity” or “full parity” only for the exact release/evidence set | Promotion from package presence, architecture, screenshot, fixture, proposal, or agent self-report |

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
