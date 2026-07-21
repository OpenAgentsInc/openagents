# Open issues audit and ASAP implementation sequence

- Date: 2026-07-21
- Scope: every **open** GitHub issue on `OpenAgentsInc/openagents` at audit time
- Method: `gh issue list --state open` (23 issues) plus full body and comment
  export for each issue
- Snapshot base: `origin/main` at write time (audit tree started at `93a5e3ecc1`)
- Class: research / dispatch strategy
- Authority: **not** ProductSpec, AssuranceSpec, or release authority. Live
  issues, `docs/sol/MASTER_ROADMAP.md`, claims, and proofs still own execution.

## Executive summary

There are **23 open issues**. They are not 23 independent product gaps.

They collapse into **five active programs** plus **one optional remainder** and
**one correctly deferred** cloud packet:

| Program | Open issues | Real remaining work |
| --- | --- | --- |
| Full Auto productization | #8967, #8978, #8979 | Almost all implementation is closed. Remaining is **independent assurance admission** and **signed packaged release / promise admission**. |
| Release / distribution remainder | #8920, #8993 | Windows optional experimental portable. Update-feed / first Full Auto tag wiring (partially advanced, not closed). |
| Managed sandboxes | #9023, #9033, #9032 | SBX-00–08 closed. SBX-09 producer acceptance is green. Needs **independent verifier + owner live observation**. SBX-10 blocked on that. |
| Agent IDE / Cursor parity | #9035, #9041–#9047 | IDE-08–12 closed. IDE-13 is the live critical path with heavy evidence still incomplete. IDE-14 has foundation work. IDE-15–19 have almost no comment progress. |
| Apple FM full agent system | #9077, #9089 | AFS-00–10 and AFS-12 closed. AFS-11 is the release-evidence remainder (signing ceremony already recorded in comments). |
| RLM recursive recall | #9136, #9141–#9144 | RLM-01–04 + SDK adoption #9154 closed. #9141–#9143 are agent-actionable. #9144 is deferred. |

**ASAP strategy in one line:** run **three parallel agent implementation lanes**
(RLM-05, IDE-13, IDE-14 foundations) while a **fourth proof lane** prepares
Full Auto / sandbox independent-admission packages, and reserve owner minutes
only for admission, packaged observation, and cloud-boundary decisions.

Do **not** try to “implement all 23 tickets serially.” Most epic parents stay
open only as tracking shells. Closing the few real leaf gates closes the epics.

## Inventory (all 23 open issues)

| # | Title (short) | Labels / priority | Comment signal | Disposition class |
| --- | --- | --- | --- | --- |
| 8920 | DIST-07 Windows x64 experimental portable | P0 release (amended optional) | rc.25 portable published. Stable 0.1.0 intentionally excluded Windows. | Optional agent build lane |
| 8967 | EPIC Full Auto | P0 tracking | Children through #9002/#8976 closed. Open only for #8978/#8979. | Tracking shell |
| 8978 | FA-AS-01 Full Auto assurance | P1-parallel / proof | AssuranceSpec designed (rev 6, 76/76 criteria). Independent admission still open. | Independent proof + design execution |
| 8979 | FA-REL-01 Full Auto release admission | P0 proof | Dev rung green. Needs admitted assurance + signed package + owner packaged observation. | Release / owner-gated |
| 8993 | REL-FEED-01 live update feed + first Full Auto tag | unlabeled | 2026-07-19 claim + real RC update repairs landed. No closure comment. | Finish-or-close audit + residual work |
| 9023 | EPIC managed sandboxes | P1-parallel tracking | SBX-00–08 closed. Waiting SBX-09 admission. | Tracking shell |
| 9032 | SBX-10 snapshots/fork/ingress | P2-deferred | No comments. Explicitly after Phase 1. | Blocked on #9033 |
| 9033 | SBX-09 live GCP acceptance | P0 proof | Producer staging acceptance green. Independent verifier + owner observation missing. | Independent proof / owner-gated |
| 9035 | EPIC IDE-08–19 | P0 tracking | IDE-08–12 closed. 13–19 open. | Tracking shell |
| 9041 | IDE-13 portable capabilities | P0 feature/proof | Very active. Expanded real-local matrix still incomplete. Explicitly non-acceptance. | Critical path agent + proof |
| 9042 | IDE-14 safe projection mobile/web/public | P0 feature | Foundation packages committed on a branch/progress note. Not acceptance. | Parallel agent lane |
| 9043 | IDE-15 extension/component ABI | P0 feature | No comments | Agent implementation (after contracts from 13/14 where shared) |
| 9044 | IDE-16 supervised preview/browser/design/computer-use | P0 feature | No comments | Agent implementation (later in IDE chain) |
| 9045 | IDE-17 Editor + Agents Window graph | P0 feature | No comments | Agent implementation (later) |
| 9046 | IDE-18 custody, Cursor migration, six-target, enterprise, a11y | P0 feature | No comments | Late IDE + release coupling |
| 9047 | IDE-19 continuous Cursor-parity evidence | P0 tracking/proof | No comments | Continuous proof after 13–18 |
| 9077 | EPIC AFS-00–12 | roadmap feature | Children closed except #9089 | Tracking shell |
| 9089 | AFS-11 packaged/release evidence | release | Signing/notarization ceremony recorded. Honest open points remain in audit comments. | Closeout / residual evidence |
| 9136 | EPIC RLM | engine tracking | RLM-01–04 + #9154 closed. 05–07 open. 08 deferred. | Tracking shell |
| 9141 | RLM-05 Tier S admission/citations/usage | engine | Unblocked by #9154. Clear remaining deliverables. | **Immediate agent work** |
| 9142 | RLM-06 Full Auto RLM consumer | engine | Depends on foundation (done) and ideally #9141 | Agent work after/with #9141 |
| 9143 | RLM-07 dense-recall eval honesty gate | engine | Harness still open. Can parallel #9141 with care. | Agent eval lane |
| 9144 | RLM-08 managed corpus (DEFERRED) | cloud-boundary | Explicitly deferred. Separate cloud admission required. | Do not start |

## What is already done (do not re-implement)

These are closed and should not absorb ASAP capacity:

- **Full Auto implementation train:** #8968–#8977, #8987, #8991, #9000–#9002,
  plus many FA wiring/mobile helpers cited on the epic.
- **Distribution core:** #8913–#8919, #8921–#8926 (signed multi-target release
  program closed). Windows is the optional remainder only.
- **Sandbox implementation train:** #9024–#9031, #9034 (SBX-00–08).
- **IDE foundation train:** #9036–#9040 (IDE-08–12).
- **AFS implementation train:** #9078–#9088, #9090 (and related closed work).
- **RLM foundation train:** #9137–#9140, #9154.
- **VSE / FAV groundwork:** #9104–#9106, #9108–#9110 closed (inputs to assurance
  honesty, not substitutes for #8978 admission).

## Blocker taxonomy

### B0 — No code blocker (agent can start now)

| Issue | Why unblocked | Risk notes |
| --- | --- | --- |
| #9141 RLM-05 | #9154 closed. Deterministic foundation present. | Must keep Tier S default-off. Exact usage honesty required. |
| #9143 RLM-07 | Eval harness can use fixtures + scripted models. | No public quality claims until gate passes. |
| #9041 IDE-13 | Active implementation/evidence program. | Acceptance bar is harsh. Do not claim done early. |
| #9042 IDE-14 | Projection schemas/services already started. | Hot-file collision risk with IDE-13 if both touch shared IDE runtime carelessly. |
| #9043–#9045 | No comments and no explicit hard block in issue text. | Prefer sequencing after IDE-13 attachment model is stable enough to extend. |
| #8920 Windows portable | Signing requirement removed. Build is optional experimental. | Outside signed stable set. Do not block mac/Linux release work. |
| #8993 feed/tag | Prior claim advanced real update path. | First re-read acceptance vs main. Close if already satisfied. Else finish residuals only. |
| #9089 AFS-11 | Ceremony evidence already posted. | May be primarily documentation/ledger honesty + residual oracles. |

### B1 — Agent can prepare, but cannot finish alone

| Issue | What agents can do | What still needs a human/independent role |
| --- | --- | --- |
| #8978 FA assurance | Expand executable proofs, formal model, receipts, map criteria to evidence. | **Independent admission** (producer may not self-admit). |
| #8979 FA release | Prepare candidate pin checklist, packaged automation, promise draft evidence pack. | Signed release identity, owner packaged quit/relaunch observation, promise flip authority. |
| #9033 SBX-09 | Keep producer matrix green, fix regressions, assemble public-safe aggregate. | Independent verifier disposition + owner live observation. Production remains default-off. |
| #9089 AFS-11 | Repair claim-evidence harness honesty issues raised in comments. | Any residual owner ceremony / release claim language. |

### B2 — Hard blocked on another open issue

| Issue | Blocked on | Notes |
| --- | --- | --- |
| #8967 epic | #8978 and #8979 | Implementation children closed. |
| #9023 epic | #9033 (then optionally #9032) | SBX-00–08 closed. |
| #9032 SBX-10 | #9033 | Correctly P2/deferred until Phase 1 admission. |
| #9035 epic | #9041–#9047 chain | 08–12 closed. |
| #9077 epic | #9089 | Implementation children closed. |
| #9136 epic | #9141–#9143 | #9144 does not block epic productization if kept deferred. |
| #9142 | ideally #9141 for semantic path | Deterministic-only path can start earlier, but full acceptance wants Tier S policy. |
| #9144 | separate cloud admission | Do not pull into ASAP desktop/engine lanes. |

### B3 — Owner / policy / spend gates (not pure coding)

- Independent assurance admission (#8978).
- Packaged Full Auto owner observation and promise admission (#8979).
- Sandbox owner live observation (#9033).
- Any new cloud placement / managed corpus (#9144).
- Signing secrets for residual release ceremonies (partially already used for
  AFS-11 / Desktop RC paths).

## One strategy: ASAP sequence

### Principle

Maximize parallel agent throughput **without** hot-file collisions and without
faking admission. Prefer:

1. Close real leaf gates that unlock epics.
2. Keep proof gates honest (designed ≠ observed ≠ admitted).
3. Do not open deferred cloud work to look busy.
4. Prefer finish-or-close audits on half-done release issues over greenfield.

### Wave 0 — immediate (start today, parallel)

Run **up to four lanes** at once if fleet capacity exists.

#### Lane A — RLM Tier S (highest pure-engineering ROI)

1. **#9141 RLM-05** first.
   - Semantic mode admission (user/application only).
   - Exact usage ledger `rlm:<runRef>:<callRef>`.
   - Citations, partial honesty, provider Layer injection.
2. In parallel if second agent available: **#9143 RLM-07** hermetic eval
   harness (fixtures, scoring, no live spend by default).
3. After #9141 lands enough: **#9142 RLM-06** Full Auto consumer with
   fail-soft recall and no authority transfer.

**Success:** #9136 can close when #9141–#9143 close (#9144 stays deferred).

#### Lane B — IDE critical path

1. **#9041 IDE-13** continues as the longest pole of product surface work.
   - Fill remaining real-local matrix rows.
   - Keep simulator rows labeled non-acceptance.
   - Only claim acceptance when exclusive attachment + checkpoint/failback bar
     is fully met.
2. **#9042 IDE-14** parallel only on non-colliding packages
   (`packages/ide-runtime` projection/cache work already started). Coordinate
   claims on shared schemas.

**Do not** start #9046/#9047 early. They need earlier IDE identity and release
truth.

#### Lane C — Finish dangling release paperwork

1. **#8993** audit against current `main` and current Desktop stable/RC truth.
   - If acceptance already observed, close with receipt links.
   - If not, finish only the residual feed/tag obligations.
2. **#9089** convert ceremony comments into the exact remaining acceptance
   checklist, fix harness honesty issues from the independent audit comment,
   then close or list the irreducible owner residual.
3. **#8920** only if spare capacity: produce/attach current Windows experimental
   portable for the latest RC/stable line **without** treating it as signed-set
   work.

#### Lane D — Proof packaging (not self-admission)

1. **#8978** evidence packaging: map every highest-risk FA-AC criterion to an
   executable oracle/receipt path. Produce a review packet for an independent
   reviewer.
2. **#9033** keep producer green and write the independent-verifier checklist
   so a second agent/owner can admit without re-running folklore.

**These lanes must not close themselves.** They prepare close conditions.

### Wave 1 — once Wave 0 leaves open the human gates

1. Independent reviewer admits **#8978** (or returns exact residual list).
2. With #8978 admitted, execute **#8979** on a pinned signed candidate:
   packaged quit/relaunch, six-test matrix on package, promise evidence.
3. Independent verifier + owner live observation close **#9033**.
4. Only then consider **#9032** (snapshots/fork/ingress) if still desired.

**Success:** #8967 and #9023 close. Full Auto and managed sandboxes stop being
open epics.

### Wave 2 — IDE completion train (serializing where necessary)

After IDE-13 attachment/checkpoint semantics are stable enough:

1. #9043 extension/component ABI
2. #9044 supervised preview/browser/design/computer-use
3. #9045 Editor + Agents Window graph completion
4. #9046 custody / Cursor migration / six-target / enterprise / a11y
5. #9047 continuous parity evidence program

Parallelism rule: schema/contract drafting for later IDE issues can start early
as **read-only design notes**, but implementation claims must stay collision-safe
with #9041/#9042.

### Wave 3 — deliberately later or never-from-this-queue

- **#9144** only after separate cloud admission.
- Any new epic not in the open set.
- Reopening closed FA implementation children without a new defect.

## Recommended fleet assignment (concrete)

Assume a multi-agent fleet with claim protocol and worktrees:

| Agent slot | Issue | Mode | Exit condition |
| --- | --- | --- | --- |
| A1 | #9141 | implement + test | Tier S admitted path + exact usage + green focused tests |
| A2 | #9143 | implement eval | Hermetic suite reproducible + honesty gate doc |
| A3 | #9041 | implement + evidence | Matrix progress with non-acceptance honesty until full bar |
| A4 | #9042 | implement | Safe projection schemas/services on main with tests |
| A5 | #8993 | finish-or-close | Closed or exact residual list ≤1 day of work |
| A6 | #9089 | finish-or-close | Closed or exact residual list |
| A7 | #8978 prep | proof packaging only | Independent-reviewer packet ready |
| A8 | #9033 prep | proof packaging only | Independent-verifier packet ready |
| A9 (optional) | #8920 | experimental build | Artifact attached, no signed-set claims |
| Owner / independent | #8978 admit, #8979 observe, #9033 observe | human gates | Written admission/observation |

If only **three** agents exist, keep A1, A3, A5 (RLM-05, IDE-13, release
finish-or-close). Those three remove the most open-issue mass per hour.

If only **one** agent exists, order is:

1. #9141 (fast closed leaf, unblocks RLM epic and FA consumer)
2. #8993 / #9089 finish-or-close (cheap epic closures)
3. #9041 (long pole)
4. #9142 / #9143
5. proof packets for #8978/#9033

## Dependency graph (open set only)

```text
#9141 ──► #9142
  │
  └──► #9143 (soft parallel)
         │
         └──► #9136 closes (with 9141-9143; 9144 stays deferred)

#9041 ──► #9043 ──► #9044 ──► #9045 ──► #9046 ──► #9047 ──► #9035 closes
   │
   └── soft parallel ► #9042

#8978 ──► #8979 ──► #8967 closes
            ▲
            └── also needs signed package machinery (mostly closed #8913 train)
                and #8993-style feed/tag truth if release claims require it

#9033 ──► #9032
   │
   └──► #9023 closes (after 9033; 9032 optional/P2)

#9089 ──► #9077 closes

#8920  (independent optional)
#9144  (out of band; cloud admission)
```

## What agents must not do while “implementing ASAP”

- Self-admit #8978 or #9033.
- Flip product promises from development receipts alone.
- Treat simulator rows as physical/device or signed-package proof.
- Start #9144 or other cloud-boundary work without a new admission.
- Duplicate closed Full Auto implementation issues under new titles.
- Claim Windows experimental portable as supported/signed auto-update.
- Expand IDE scope into #9046 enterprise/migration before #9041 attachment
  truth is solid.

## Comment-derived status nuances (easy to misread)

1. **#8978 body still mentions old “37 needs_design” language.** Comments
   reconcile current truth: AssuranceSpec rev 6, 76/76 designed, still
   `proposed` / not independently admitted.
2. **#9033 is not “failing.”** Producer acceptance is green. The issue is open
   for independence and owner observation.
3. **#9041 is not close.** Multiple “expanded evidence” comments explicitly say
   non-acceptance.
4. **#8920 is no longer blocked on Authenticode.** Owner amendment removed that
   requirement. Remaining is optional experimental portable production.
5. **#9141 engine substrate changed.** Comments override any leftover Python
   leaf-executor language: Effect-native recursive recall only.
6. **#8993 and #9089 look “almost done.”** They are high-value finish-or-close
   targets. Do not re-architect them.

## Mapping to roadmap truth

This audit agrees with `docs/sol/MASTER_ROADMAP.md` revision-class priorities:

- Full Auto remains the flagship product program, but its open work is now
  **assurance + release admission**, not greenfield feature coding.
- Managed sandboxes remain a P1 parallel program at the **proof/admission**
  boundary.
- IDE remains a large open implementation surface after IDE-12.
- RLM is an active engine program with immediate unblocked leaves.
- Distribution’s signed multi-target train is largely closed. Open residue is
  optional Windows and feed/tag/Full Auto packaging truth.

## Suggested close-order scoreboard (target)

Work this scoreboard top-down. Each row is a “celebration unit” that reduces
open-issue count honestly.

1. Close #8993 and/or #9089 (cheap epic pressure relief).
2. Close #9141, then #9142, then #9143 → close #9136.
3. Keep #9041/#9042 moving every day (long pole).
4. Land independent admission for #8978 → close #8979 → close #8967.
5. Land independent + owner observation for #9033 → close #9023 (and only then
   consider #9032).
6. Burn IDE-15→19 after 13/14 stabilize → close #9035.
7. Produce #8920 when convenient.
8. Leave #9144 deferred until cloud admission.

## Evidence of this audit

- Issue count: **23 open** via `gh issue list --state open`.
- Full bodies and comments exported under a local temp readable set during the
  audit session (not vendored into the repo).
- Related closed-child states checked with `gh issue view` for Full Auto,
  DIST, SBX, IDE-08–12, AFS-00–10/12, RLM-01–04, #9154, and VSE/FAV groundwork.

## Bottom line

ASAP does **not** mean “open 23 PRs for 23 issues.”

It means:

- **Implement now:** #9141, #9041, #9042, #9143, then #9142, then IDE-15+.
- **Finish or close now:** #8993, #9089, optional #8920.
- **Package proof now, admit later:** #8978, #9033.
- **Owner/independent gates next:** #8978 admit, #8979 packaged observation,
  #9033 observation.
- **Do not start:** #9144 and any cloud self-expansion.

That sequence is the fastest path from 23 open issues to a short list of true
remaining gates without lying about assurance or release.
