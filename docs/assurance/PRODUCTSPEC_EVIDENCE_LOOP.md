# ProductSpec Evidence Loop boundary

Date: 2026-07-13

Status: adopted architecture boundary and implementation plan; current
OpenAgents ProductSpec code has not yet caught up to the upstream features
described here

Upstream audited: `gokulrajaram/ProductSpec` at `9ef2654` (`0.19.0`, document
format `0.1`); Agent Run addendum (below) audited at `c7250a8` (parser
`0.22.0` on main, npm still `0.19.0`) on 2026-07-13

## Decision

Adopt ProductSpec's Evidence Loop as the portable attachment layer. Do not
build a competing evidence ledger into AssuranceSpec.

The resulting system has two related loops with different jobs:

```text
ProductSpec document
  intent + stable AC/EVAL/SM IDs + Related Artifact index
        ├──> implementation / eval / analytics systems
        │       └──> durable artifacts linked back by Related Artifacts
        │                 └──> ProductSpec Decision Trace when evidence changes intent
        │
        └──> AssuranceSpec
                reviewed proof obligations, risks, oracles, falsifiers,
                environments, proof rungs, evidence and gate policy
                    └──> Assurance Manifest
                            └──> native QA tools / QA Swarm
                                    └──> exact Assurance Receipts
                                            └──> approved public-safe artifact links

OpenAgents Desktop ProductSpec workroom loop
  accepted plan -> packets/leases -> evidence refs -> independent verification
  -> owner packet disposition
        ^ may register an Assurance Receipt by reference
```

The upstream ProductSpec Evidence Loop is a portable index in the authored
document. The Desktop workroom loop is a runtime workflow. AssuranceSpec is the
pre-build verification design. They compose; none replaces the others.

## What upstream now guarantees

ProductSpec `0.19.0` adds a structured `productspec-related-artifacts` block in
the optional `Related Artifacts` section. An entry has a supported `type` and a
durable external URL or repository-relative path. It may target a section and
an exact `AC-<number>`, `EVAL-<number>`, or `SM-<number>` item.

The validator:

- rejects malformed blocks and unsupported artifact types;
- rejects an `item_id` that does not exist in the same Product Spec;
- rejects duplicate durable AC, EVAL, and SM IDs;
- warns when an artifact kind usually belongs to another item family, such as
  an `eval_run` attached to an `SM-*` item.

The separate ProductSpec graph resolver warns when a `product_spec` dependency
revision pin differs from the target Product Spec.

The MCP evidence checklist groups expected links by AC, EVAL, and SM. It marks
AC/EVAL work as release-blocking and SM outcomes as post-launch, but remains a
deterministic checklist: completion claims still report criteria as needing
verification and evals as not run by ProductSpec.

It does **not**:

- fetch or authenticate an artifact URL;
- prove that an artifact contains what its title claims;
- run a test or eval, collect a trace, or read a dashboard;
- establish source, command, target, environment, seed, or toolchain identity;
- judge oracle sensitivity, evidence sufficiency, independence, freshness, or
  release fitness;
- turn a link, a passing suite, or a dashboard movement into acceptance;
- replace GitHub, Linear, Braintrust, Langfuse, Datadog, native test systems,
  analytics, or the OpenAgents promise registry.

In other words, ProductSpec validates attachment syntax and the same-document
item target. Assurance and the evidence-producing system validate the *proof
claim*.

## Authority boundary

| Layer | Owns | Must not claim |
| --- | --- | --- |
| ProductSpec | Current product intent, stable AC/EVAL/SM IDs, Related Artifact references, portable Decision Trace | Evidence execution, adequacy, freshness, acceptance, release, or public claims |
| Upstream Related Artifacts | Typed pointers from intent IDs to durable external evidence | That the target is reachable, authentic, sufficient, current, or passing |
| OpenAgents Desktop workroom loop | Accepted implementation plan, packets, dependency/lease state, evidence envelopes, host verifier/producer ref inequality, owner packet disposition | Authenticated identity separation, pre-build oracle adequacy, full-criterion assurance, release readiness, or promise state |
| AssuranceSpec | Reviewed verification intent: risk, obligation, environment, oracle, falsifier, seam, proof rung, evidence, gate, and independence policy | Product intent, implementation scheduling, live evidence, packet state, or release authority |
| Assurance Manifest | Immutable resolved verification plan | Latest status, semantic planning, or acceptance |
| Native tools and evidence stores | Execute and retain tests, evals, traces, dashboards, artifacts, and native reports | Product or release authority by implication |
| Assurance Receipt | Exact observed result and provenance against an exact Manifest unit | Intent revision, owner acceptance, merge, deploy, or promise promotion |
| Evidence publisher | Review and add approved public-safe Related Artifact pointers | Rewrite receipts or interpret a link as a pass |
| ProductSpec Decision Trace | Explain consequential intent or product-decision changes caused by evidence | Replace run history or assurance-policy history |
| Release/owner policy | Accept, reject, waive, or release within explicit authority | Rewrite intent or evidence history |
| Product-promise registry | Public claims and claim transitions | Infer a promise from any other green state |

### Verbs stay precise

- A ProductSpec **commits** intent.
- A ProductSpec plan is **accepted**.
- A work packet is **admitted** and later receives an owner **disposition**.
- An Assurance Spec is **reviewed** and **admitted for verification**.
- A native tool **observes**; an Assurance Receipt reports `CONFIRMED`,
  `REFUTED`, or `INCONCLUSIVE`.
- A verifier **reviews evidence** under a named policy.
- Release policy **permits or blocks release**.
- The promise registry **promotes or withholds a public claim**.

`verified` in the current Desktop workroom means that every evidence receipt
ref supplied to that verification resolved and a `verifierRef` different from
the host's `evidenceProducerRef` recorded a `passed` receipt. The host currently
sets that producer ref to the active lease executor; it does not authenticate
human identity or inspect an Assurance Receipt's actual producer. It does not
mean that the full ProductSpec item is assured, that an Assurance gate is
satisfied, or that release or a public promise is authorized.

## Related Artifact vocabulary we will use

Use the upstream vocabulary without inventing local meaning:

| ProductSpec target | Normal upstream types | OpenAgents rule |
| --- | --- | --- |
| `AC-*` | `github_pr`, `code`, `release`, `engineering_spec` | Link implementation and approved test/receipt artifacts; a link is not an acceptance verdict |
| `EVAL-*` | `eval_run` | Link the durable run or human-review record; Assurance Receipt retains exact execution provenance |
| `SM-*` | `dashboard`, `analytics_snapshot`, `experiment` | Link post-launch outcome evidence; these normally do not block implementation completion |
| Any valid item | `other` when no precise type exists | Use a precise title and durable path; do not misuse `eval_run` merely to avoid `other` |

The current upstream vocabulary has no `assurance_spec`, `test_report`, or
`assurance_receipt` type. The first dogfood should use `engineering_spec`,
`code`, or `other` honestly. Propose new upstream types only after real usage
shows that `other` loses material interoperability.

## Agent Run addendum (upstream v0.21.0/v0.22.0, 2026-07-13)

Upstream added a second companion artifact: **Agent Run**
(`.agent-run.json`, `agent_run_format_version: "0.1"`,
`schema/agent-run.schema.json`) — a self-reported receipt for one agent
execution against a pinned Product Spec. `productspec init-run` drafts it with
every `AC-`/`EVAL-`/`SM-` ID at `not_checked`; the executing agent then fills
in per-item `passed`/`failed`/`not_checked`/`blocked`, evidence links, a
`drift` block, and a free-text `completion_claim`. `productspec validate-run`
and the MCP `draft_agent_run` tool shipped with it. Distribution honesty: all
of this is on upstream `main` but unpublished to npm (registry latest is
`0.19.0`).

How it relates to this boundary:

- **Attachment is directional and second-class.** The run pins the spec
  (`product_spec.path` + `spec_revision` + *optional* `content_hash`), but
  there is no `agent_run` Related Artifact type, so a Product Spec that wants
  to index its runs must use `other` with a repo-relative path. Upstream's own
  starter kit stores runs beside the spec tree (`docs/agent-runs/`) and
  validates them in CI rather than linking them from the spec.
- **Validation is shape-only.** `validate-run` does not check that the
  `checked_items` IDs exist in the referenced spec, does not recompute the
  content hash, and does not dereference evidence. A structurally valid Agent
  Run can cite a spec it never read.
- **PSEL treatment: accepted as a pointer, never a verdict (Law 13).** An
  `.agent-run.json` is admissible into our loop exactly like any Related
  Artifact target: a durable, typed *claim record* whose item statuses are the
  producing agent's self-report. It never enters the Desktop workroom's
  verification path as a receipt, never satisfies an Assurance obligation, and
  never moves any of the eight status axes past "a self-report exists." The
  proposed typed ingest (AGENT_TOOLING.md §7) maps it to the lowest proof
  rung as self-reported evidence with `producer == claimant` flagged —
  proposed, not implemented.
- **What it is genuinely good for:** a portable, machine-checkable statement
  of *what the agent believes it did*, drafted from the spec's own durable
  IDs. That is a better claim substrate than prose "done" messages, and our
  `claim` CLI / `check_completion_claim` tooling can meet it: their artifact
  records the claim; our tools report what remains unverified about it.

## Two digests, not one

Upstream explicitly permits evidence-link maintenance without a
`spec_revision` bump. That invalidates our earlier rule that *any* ProductSpec
byte change automatically stales proof design.

Assurance binding therefore needs two identities:

1. **Document digest** — SHA-256 of the exact authored UTF-8 file. It changes
   for every edit, including a new Related Artifact link. It is retained for
   provenance, cache invalidation, and recheck-based race detection.
2. **Intent digest** — a canonical digest of the ProductSpec intent projection.
   It includes intent-bearing frontmatter and sections, AC/EVAL/SM definitions,
   stable IDs, `applies_to`, `product_spec` dependency Related Artifacts, and
   all `tool_metadata` consumed by execution or policy. It excludes only
   attachments a typed classifier proves are evidence-only, plus explicitly
   non-intent provenance fields such as `created_at`/`updated_at`. Unknown
   fields are intent-bound by default.

The rules are:

- changed `spec_revision` or intent digest makes the Assurance Spec subject
  stale and requires explicit reconciliation;
- changed targeted item text or ID makes obligations against that item stale;
- changed document digest with the same revision and intent digest does not by
  itself invalidate proof design. It reports `evidence_index_changed` only
  after a typed semantic diff proves that the change is limited to classified
  evidence attachments and permitted provenance fields;
- a missing, superseded, unreachable, or content-changed evidence artifact can
  stale the evidence projection without changing product intent;
- adding evidence that changes scope, criteria, UX, metrics, or the underlying
  bet requires a ProductSpec revision and usually a ProductSpec Decision Trace.

The canonical intent projection must be specified and conformance-tested. It
cannot be an ad hoc Markdown deletion pass.

These are proposed Assurance-layer binding fields, not new ProductSpec
frontmatter and not current Desktop identity semantics. The current Desktop
`ProductSpecIdentity` pins the exact document digest; any byte edit, including
an evidence-only edit, makes the existing run `revision_mismatch`. Its prior
receipts remain historical under the old exact identity. Intent digest does
not relax that runtime contract.

Until the projection has two independent implementations and golden fixtures,
the conservative bootstrap is to add one stable Related Artifact URL for a
public-safe Assurance Evidence Index *before* admitting the Assurance Spec.
Runs update the index and immutable receipt targets, not the ProductSpec URL.
That avoids recursively invalidating an exact-byte subject after every run.
Once dual-digest semantics are real, individual reviewed artifact links may be
added without misclassifying evidence-index maintenance as intent drift.

The revision-6 legacy MVP pilot is an explicit exception to that portable
bootstrap: it keeps exact-byte binding, adds no Related Artifacts, and makes no
upstream Evidence Loop interoperability claim. PSEL-2/3 performs the later
portable migration and rerun.

## OpenAgents implementation gap

`@openagentsinc/product-spec` was built from an earlier upstream snapshot. It
does not currently parse or validate:

- structured `productspec-acceptance-criteria` items;
- upstream-standard `AC-*` criterion identity;
- current `SM-*` success-metric identity and fields;
- current `productspec-ai-evals` case/check shape and `EVAL-*` identity;
- current `applies_to` frontmatter semantics;
- `productspec-related-artifacts`;
- Related Artifact dangling-ID errors and unusual-target warnings;
- the upstream Evidence Loop checklist or MCP session behavior.

The current MVP ProductSpec is valid under our local extension but not under
upstream `0.19.0`: it uses Markdown `CW-AC-*` criteria and semantic success
metric IDs with OpenAgents-only fields. We must not call its Related Artifact
links portable until this gap is reconciled.

## Adoption sequence

### PSEL-0 — freeze current behavior

- preserve the revision-6 MVP ProductSpec and its current digest as a baseline;
- retain the existing Desktop workroom loop and `CW-AC-*` packet history;
- add upstream `0.19.0` valid/invalid fixtures without weakening local tests;
- record the exact current incompatibilities as tests, not folklore.

### PSEL-1 — catch up the local ProductSpec package

- implement current `applies_to` plus structured AC, EVAL, SM, and Related
  Artifact blocks;
- match upstream error/warning semantics for dangling IDs, duplicate IDs, and
  unusual artifact targets;
- preserve round trips and custom sections;
- add document- and intent-digest APIs with golden projections;
- add a typed, owner-confirmed evidence-attachment edit path that proves the
  intent projection is unchanged and atomically rechecks the exact bytes
  before write; do not relax the existing generic edit/revision rule;
- keep the upstream parser as a conformance reference, not a runtime
  dependency.

### PSEL-2 — reconcile the MVP ProductSpec

- propose a ProductSpec revision that converts `CW-AC-01…18` to portable
  `AC-1…18` structured items and the seven current metrics to `SM-1…7`;
- preserve criterion/metric semantics with reviewed single-line criterion
  normalization fixtures, because the upstream handwritten parser does not
  accept YAML block scalars;
- preserve each current metric's `segment` and `source` in a keyed
  `custom-success-metric-context` section because those fields are not in the
  upstream Success Metric schema;
- create a machine-readable old-to-new ID mapping artifact; use ProductSpec
  Decision Trace prose plus a link to explain and approve the migration;
- retain or supersede old-identity Desktop runs/packets, create a new accepted
  plan/run for the `AC-*` identity, and reconcile tests, issues, and Assurance
  bindings without rewriting history;
- do not silently relabel existing evidence.

This is an intent-identity migration and therefore receives a revision bump
even if the prose is otherwise unchanged.

Status (2026-07-13, #8758): the migration revision exists as a proposal.
`docs/mvp/openagents-codex-workroom-mvp.rev7-proposed.product-spec.md`
(revision 7) validates under both profiles with structured `AC-1…18`/`SM-1…7`
items, a keyed `custom-success-metric-context` section, and a Decision Trace
section; `docs/mvp/openagents-codex-workroom-mvp.id-map.json` is the
machine-readable ID map, and `packages/product-spec` tests pin map/rev-6/rev-7
agreement (pure whitespace-collapse normalization). The live subject, the
checked-in AssuranceSpec, and the MVP-01 dogfood deliberately remain bound to
revision 6; owner-gated adoption, identity freeze, rebinding, and the new
accepted `AC-*` plan/run are PSEL-3.

### PSEL-3 — close the first Assurance loop

- seed the stable public-safe Assurance Evidence Index in `no_evidence` state
  and add its approved Related Artifact link to the migrated `AC-*` target;
- freeze the migrated ProductSpec document/intent identities, then rebind and
  re-admit AssuranceSpec and create the new exact-identity Desktop plan/run;
- execute the first admitted Assurance obligation and retain native plus
  normalized receipts;
- resolve and validate the receipt through a typed bridge, register its opaque
  immutable ref in the Desktop workroom, use a distinct verifier reference,
  and leave owner disposition separate;
- validate that a nonexistent item ID fails and an unusual type only warns;
- prove that a typed evidence-attachment-only edit can change the document
  digest without changing the intent digest or `spec_revision`; the current
  Desktop run still becomes exact-identity mismatch and a new run is required;
- do not send `REFUTED`, `INCONCLUSIVE`, stale, or infrastructure-failed
  observations through the Desktop loop's current pass-only verification path.

### PSEL-4 — reconcile learning

- when a typed diff only changes classified evidence attachments, maintain
  Related Artifacts without revising intent;
- when evidence changes the bet, UX, scope, criterion, eval, or metric, append
  ProductSpec Decision Trace and revise the ProductSpec;
- when evidence changes proof policy but not product intent, revise
  AssuranceSpec and its assurance decision history instead;
- when a public claim changes, use the promise registry's own authority and
  receipts.

## Non-goals

- storing raw traces, customer data, credentials, or private run payloads in a
  Product Spec;
- mirroring every receipt into Markdown;
- auto-writing evidence links before redaction and policy review;
- treating link count as coverage, progress, quality, or confidence;
- using Related Artifacts as the live mutable status database;
- making AssuranceSpec a second ProductSpec, work planner, issue tracker, or
  evidence store;
- making the Desktop workroom loop portable AssuranceSpec semantics.

## Primary upstream references

- [ProductSpec Evidence Loop](https://github.com/gokulrajaram/ProductSpec/blob/9ef2654bdd01aef3985fef6ed5a9ab66365999e1/docs/evidence-loop.md)
- [ProductSpec format and Related Artifacts](https://github.com/gokulrajaram/ProductSpec/blob/9ef2654bdd01aef3985fef6ed5a9ab66365999e1/SPEC.md)
- [Validator semantics](https://github.com/gokulrajaram/ProductSpec/blob/9ef2654bdd01aef3985fef6ed5a9ab66365999e1/docs/validator.md)
- [Reference parser and validator](https://github.com/gokulrajaram/ProductSpec/blob/9ef2654bdd01aef3985fef6ed5a9ab66365999e1/parsers/ts/src/index.ts)
