# Repo docs/direction cleanup audit — retire Tassadar/Psionic, focus Khala Code + business

Date: 2026-07-08
Status: **audit + execution prescription** (owner mandate). This document
inventories the stale-docs / old-decision surface of this repository and
prescribes exactly what gets retired, superseded, postponed, or kept. It
flips no promise state, changes no runtime authority, and broadens no
public copy. The execution issue filed from this audit is the handoff
contract; this document is its content authority.

## 1. The mandate (owner, 2026-07-08)

- **Focus everything on Khala Code and business-facing efforts** — the
  mobile MVP, Sarah/outbound sales, credits/payments, the sales landing,
  and the Effect Native conversion program (MASTER_ROADMAP rev 6,
  commit `5ea343c583`).
- **Deprioritize/deprecate the Tassadar/Psionic program** — the
  LLM-computer research, executor/training/gym/inference lanes, and the
  compute-market framing built around them. **Retired for now, revived
  only by explicit owner decision after the company is cashflow-positive.**
- **Postpone everything else that is neither Khala Code nor
  business-facing** — parked as direction, not deleted.
- The docs tree carries months of point-in-time audits and superseded
  decisions (pre-GCP Cloudflare era, pre-Effect-Native ONE-UI/TanStack
  decisions, Foldkit migration plans, training-era episode audits). These
  must stop masquerading as current direction.

## 2. The status taxonomy (the only four labels)

Every touched doc gets exactly one banner at the top of the file, below
the title. **Nothing is deleted. Nothing is moved.** (Moving breaks
hundreds of relative links and issue references; history stays where it
is.) A central ledger `docs/RETIRED.md` indexes everything labeled
retired/postponed.

1. **RETIRED FOR NOW** — programs the owner has shelved (Tassadar/
   Psionic and their satellites). Banner:

   > **STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
   > OpenAgents is focused on Khala Code and business-facing work
   > (`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
   > until an explicit owner decision revives it (earliest
   > reconsideration: after cashflow-positive). Preserved for history;
   > do not route new work, issues, or copy from this document.

2. **SUPERSEDED** — decision documents replaced by a newer decision.
   Banner names the successor explicitly, e.g.:

   > **STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
   > §EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
   > the historical record of the earlier decision; do not implement
   > from this document.

3. **POSTPONED** — still intended direction, parked behind the focus
   (e.g. Reactor, labor-market, collective-intelligence, Verse/game,
   Nostr lanes). Banner:

   > **STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
   > business focus (MASTER_ROADMAP rev 6).** Direction retained;
   > implementation resumes only when MASTER_ROADMAP sequences it or
   > the owner pulls it forward. Do not route new work from it now.

4. **HISTORICAL** — point-in-time audits/receipts that were never
   direction (dated audit docs, episode records). Banner:

   > **STATUS: HISTORICAL — point-in-time record (accurate as of its
   > date). Not current direction; consult MASTER_ROADMAP.**

Rules: a banner is ≤6 lines, sits directly under the H1, and never
edits the body below it. Docs already carrying an equivalent status
note (e.g. the four ROADMAP_* sequencing notes) get the stronger banner
only if their existing note misstates current direction.

## 3. What is current (DO NOT banner these)

- `docs/fable/MASTER_ROADMAP.md` (rev 6 — the single sequencing
  authority), `EXECUTION.md`, `README.md` (gets an index update, §6).
- The 2026-07-07/08 strategy set in `docs/fable/` (Sarah spec, beyond-MVP
  Codex doc, overarching roadmap, product suite, sovereignty analysis,
  what-openagents-is essay, Apollo outbound plan, mobile-MVP audit,
  seam-testing audit).
- `docs/effect-native/` (all seven files), `docs/khala-code/`,
  `docs/khala-mobile/`, `docs/khala-sync/`, `docs/khala/`,
  `docs/khala-cli/`, `docs/mobile/`, `docs/codex/`, `docs/cloud/`,
  `docs/ops/`, `docs/qa/`, `docs/promises/` (registry evidence — never
  edited by cleanup), `docs/crm/`, `docs/business/`, `docs/blitz/`,
  `docs/stripe/`, `docs/payments/`, `docs/mpp/`, `docs/auth/`,
  `docs/ota/`, `docs/apple-fm/`, `docs/desktop/`, `docs/incidents/`,
  `docs/legal/`, `docs/handoff/`, `docs/cleanup/`, `docs/design/`,
  `docs/perf/`, `docs/adr/`, `docs/refactor/`, `docs/reference/`,
  `docs/artanis/` (fleet-operations tooling — actively used to run the
  delegation fleet), `docs/agents/`, `docs/sandboxes/`, `docs/mcp/`,
  `docs/metrics/`, `docs/feature-requests/`, `docs/DEPLOYMENT.md`.
- `docs/transcripts/` — **preserved by standing repo contract; never
  touched by any cleanup.**
- `docs/agenticsociety/` — P3–P6 content authority under MASTER_ROADMAP;
  already sequenced later, needs no banner.
- Anything under `apps/*/docs/` that is a runbook or proof bundle for a
  live surface (e.g. `apps/pylon/docs/proofs/`, `apps/oa-updates/docs/`).

## 4. RETIRED FOR NOW — the Tassadar/Psionic program surface

Docs directories (banner every `.md` in each; add one line per dir to
`docs/RETIRED.md`):

| Directory | Files | What it is |
|---|---:|---|
| `docs/tassadar/` | 21 | LLM-computer/Percepta research, executor specs, plugin marketplace, capability envelopes |
| `docs/training/` | 20 | Training-run lanes, cockpits, episode training receipts |
| `docs/gym/` | 8 | Proof-gym / training-gym lanes |
| `docs/inference/` | 36 | Inference-engineering book-loop implementation notes and serving-lane docs |
| `docs/gepa/` | 1 | GEPA optimization notes |
| `docs/sakana/` | 12 | Sakana-inspired research lanes |
| `docs/benchmarks/` | 1 | Benchmark lanes tied to the training program |
| `docs/stress-testing/` | 1 | Load lanes for the compute market |
| `docs/confidental-compute/` | 1 | Confidential-compute research (sic — do not rename; link-stability) |
| `docs/agi/`, `docs/asi/` | 3+3 | Speculative capability essays |
| `docs/tokens/` | 2 | Served-token compute-market framing (the *counter itself* stays live product) |
| `docs/traces/` | 5 | ATIF trace-corpus research (the trace *runtime* stays; research direction retired) |
| `docs/unit/`, `docs/systems/`, `docs/verification/`, `docs/proof/`, `docs/apm/` | ~7 | Verification-by-replay / systems research satellites |

`docs/fable/` members of the same program (banner RETIRED FOR NOW):
`2026-06-10-tassadar-executor-pylon-v03-readiness-audit.md`,
`2026-06-10-cs336-distributed-homework-continuation-audit.md`,
`2026-06-12-episode-236-training-launch-gap-audit.md`,
`2026-07-04-ts-6-start-khala-tassadar-route-slice.md` (also superseded
— use the RETIRED banner and mention §EN in its line).

**Code footprint — explicitly OUT of cleanup scope:** Tassadar/Psionic
identifiers appear in ~563 TS files, overwhelmingly `apps/pylon` (src +
tests: tassadar-assignment, psionic-connector, training-cockpit, etc.).
That code stays compiled and its tests stay green — deleting it is a
separate, owner-gated engineering lane (the cleanup agent files ONE
follow-up issue enumerating those surfaces; it does not touch them).
Same for any `tassadar`/`gym` panes still mounted in shipping UIs: they
convert or retire under the Effect Native program (EN-5 #8574), not
under this cleanup.

## 5. SUPERSEDED and POSTPONED and HISTORICAL — the rest

**SUPERSEDED (banner names the successor):**

- Pre-Effect-Native web/UI decisions → superseded by MASTER_ROADMAP §EN
  rev 6: `docs/fable/2026-07-04-ui-react-edition.md`,
  `2026-07-04-tanstack-start-sites-and-web-app-evaluation.md`,
  `2026-07-04-tanstack-start-funnel-routes.md`,
  `2026-07-04-tanstack-start-parity-contract.md`,
  `2026-07-04-ts-4-start-sites-template-build-lane.md`,
  `2026-07-04-ts-5-sites-tanstack-rules-and-contracts.md`,
  `2026-07-04-ts-8-expo-mobile-scaffold.md` (Expo host stays true; the
  UI-authoring half is superseded — say so in the banner),
  `2026-07-04-khala-code-react-sidebar.md`,
  `2026-07-01-khala-code-desktop-qa-framework-design.md` **only if** it
  prescribes the React shell (verify; else HISTORICAL).
- `docs/convex/` (1 file) → superseded by the GCP/Cloud SQL + Khala Sync
  stack.
- Any doc whose body directs work at Cloudflare Workers/D1 as the
  target platform → superseded by the GCP evacuation (epic #8515); the
  sweep list comes from `docs/cleanup/2026-07-05-d1-zero-reference-sweep.md`
  (do not re-derive it).

**POSTPONED (direction kept, parked):**

- `docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md`
  and the ten `2026-07-04-rx-*.md` receipts — Reactor is P7, sales-led;
  banner POSTPONED (owner may pull forward for a real enterprise deal).
- `docs/game/` (26 — Verse/game surfaces), `docs/collective-intelligence/`
  (4 — the bare-"Khala" product), `docs/labor/` (12 — labor-market rails),
  `docs/nostr/` (3) + `docs/nips/` (7), `docs/tether/`, `docs/pro/`,
  `docs/raise/` (owner-timed), `docs/ads/`, `docs/forge/` (10 — points at
  the separate private repo; banner notes the repo boundary).
- `docs/fable/ROADMAP_AFTER.md` — already marked speculative; upgrade its
  note to the POSTPONED banner.

**HISTORICAL (point-in-time; banner only, no judgment):**

- `docs/autopilot-coder/` (124) — the pre-Khala-Code Autopilot Coder era.
- `docs/tui/` (5, June 10), `docs/pylon/` (1, June 10),
  `docs/flue/` (2), `docs/opencode/` (14 — third-party analyses),
  `docs/afteraction/` (6), `docs/audits/` (3), `docs/qa-demo/`,
  `docs/ade/` (1), `docs/stats/` (1).
- `docs/fable/` June-era point-in-time audits (all 15 files dated
  2026-06-09 … 2026-06-16 not already listed in §4): ark-mdk payments,
  agent-pause, agent-work-12h, open-issues attack order, always-on-fleet,
  oldest-open-issues, stacker-news, focus-sweep, effect-three-fiber,
  help-flip-the-green-gates, homepage-pylon-stats,
  `2026-07-05-promise-all-cron-landmine-audit.md` stays un-bannered
  (recent, still-true engineering note) — judgment call recorded here.
- `docs/research/` (47) — mixed bag: banner HISTORICAL per file **unless**
  the file is cited by MASTER_ROADMAP's document map (none are, as of
  rev 6 — verify with grep before bannering).
- `docs/launch/` (93) — launch-era receipts; leave un-bannered (receipts
  are self-dating and referenced from promise evidence). Recorded here
  as a deliberate skip.

**Lane-family roadmaps (`docs/fable/ROADMAP*.md`):** all four already
carry the 2026-07-07 sequencing note deferring to MASTER_ROADMAP; that
stays. Add to `ROADMAP.md` (the 2026-07-01 unified roadmap) one
paragraph under its existing note: its Foldkit-migration workstream is
superseded by §EN rev 6, and any Tassadar/training/gym workstreams are
retired per this audit — the rest of its Khala Code desktop/harness
content remains the content authority.

## 6. Index + ledger deliverables

1. **`docs/RETIRED.md`** (new): one table — program/dir, status
   (retired/postponed), date, revival condition ("owner decision;
   earliest after cashflow-positive" for §4; "MASTER_ROADMAP sequencing"
   for postponed), and the authority link to this audit. One row per §4
   directory and §5-postponed directory.
2. **`docs/fable/README.md`**: add this audit + `docs/RETIRED.md` to the
   index; correct the "Start Here" framing so MASTER_ROADMAP rev 6 is
   named first (it currently leads with the 2026-07-01 ROADMAP.md).
3. **`docs/fable/MASTER_ROADMAP.md`**: append a short "Retired programs
   (rev 6.1)" subsection under the status snapshot: Tassadar/Psionic
   retired for now (revival = owner decision, earliest cashflow-positive);
   pointer to this audit and `docs/RETIRED.md`. Do not renumber phases.

## 7. Hard guardrails for the executing agent

- **Never delete or move a file.** Banners + the three deliverables in
  §6 are the entire write surface, plus the one follow-up issue in §4.
- **Never touch:** `docs/transcripts/`, `docs/promises/` bodies, promise
  registry records/evidence, INVARIANTS files, `apps/*/docs/proofs/`,
  anything under `apps/`, `packages/`, `clients/` (no code, no tests, no
  UI), public marketing/user-facing copy, and GitHub issue states
  (comment-only if at all).
- Banner text is exactly the §2 templates (fill the successor name for
  SUPERSEDED). No editorializing inside old docs.
- Commit in reviewable slices (one commit per §4/§5 group), push to
  `main` per repo completion discipline; every push green under the
  pre-push guard. `bun run check:docs`-class gates (if present) must
  pass; broken relative links introduced by the ledger/index edits are
  bugs.
- Where this audit says "verify before bannering" (README-cited files,
  the QA-framework doc, research/), do the grep, then act; if a file
  turns out to be load-bearing for a live surface, skip it and record
  the skip in the PR/commit message and on the issue.
- Anything ambiguous: skip, list it in a final "unclassified" comment on
  the issue rather than guessing.

## 8. Explicit non-goals

- No code retirement (see §4 — one follow-up issue, no edits).
- No GitHub-issue backlog rewriting (the 25 open issues are all current
  post-rev-6 lanes).
- No renaming of misspelled dirs (`confidental-compute`) — link
  stability wins.
- No changes to the standing lane families' content beyond the §5 note
  to `ROADMAP.md`.
- No public-copy or promise-state changes of any kind.
