# Omega master delegation plan

- Status: owner-directed delegation instruction, 2026-07-25
- Owner: OpenAgents
- Audience: coding agents that implement the open omega issues
- Issue basis: all 19 open omega issues at 2026-07-25
- Companion documents: [Omega roadmap](./ROADMAP.md),
  [Omega Agent roadmap](./2026-07-25-omega-agent-roadmap.md),
  [open issues unified completion plan](./2026-07-25-omega-open-issues-unified-completion-plan.md)

This plan tells coding agents what to do, in what order, and what can run in
parallel.
The live GitHub issue is the claim ledger for each unit of work.
This plan orders the work. It does not replace any issue's own acceptance
criteria.

## 1. Read before you claim

Every agent reads these sources before its first mutation:

1. `AGENTS.md` in the omega repository (Rust and GPUI rules, `./script/clippy`,
   PR `Release Notes:` block).
2. `OMEGA_DELTAS.md` and `crates/omega_deltas` (the divergence ledger and its
   checks).
3. This plan, the claimed issue, and every document the issue cites.
4. For `OMEGA-AGENT` packets: the
   [Omega Agent roadmap](./2026-07-25-omega-agent-roadmap.md) and the
   [Omega Agent analysis](../fable/2026-07-25-omega-agent-analysis.md).
5. For proof packets: the evidence protocol in the
   [unified completion plan](./2026-07-25-omega-open-issues-unified-completion-plan.md).

## 2. Standing rules for every lane

1. Claim the issue before mutation. One agent per issue. Post progress and
   commits on the issue.
2. Work in a fresh worktree from current `origin/main` of the owning
   repository. Never move another agent's uncommitted work.
3. Omega code changes pass `./script/clippy` and
   `cargo test -p omega_deltas` plus the owned tests of the touched crates.
4. Every default change, removal, or keymap change lands as a numbered delta
   in `OMEGA_DELTAS.md` with a mechanical check. A change a rebase can
   silently revert is not landed.
5. Do not commit to the omega repository while a release-candidate build is
   running. The bundle binds evidence to the exact source commit.
6. Never mutate an external agent's home, credentials, provider account, or
   tool configuration. Never copy tokens between credential stores.
7. Proof evidence follows the five-rule protocol: candidate binding to the
   exact artifact digest, traversal-safe evidence refs, no credential copy,
   dual-role attestation with no self-review, no fabricated or promoted
   evidence. Unperformed gates stay `pending_required_gates`.
8. Documentation for this program lands in `docs/omega/` in the openagents
   repository. These internal strategy records can use normal technical
   language and do not run the public STE checks.
9. When a lane blocks on an owner act, write the exact needed act on the
   issue, mark it `NEEDS-OWNER`, and pull the next non-blocked lane. Never
   idle.

## 3. Track map

| Track | Issues | State at 2026-07-25 |
| --- | --- | --- |
| A. Release and identity proof | #16, #9, #8 | `0.2.0-rc6` is signed, notarized, installed. The rc6 evidence chain is complete and independently reviewed. Installed observations are partial. #9 source is landed and waits on installed proof. #8 waits on owner observation and admission. |
| B. Full Auto proof | #26 | Engine fixes landed with `omega-effectd-v0.1.0-rc.8`. The evidence collector exists. Remaining: replay and journey on a current candidate, an owner-real unattended run, and independent assurance on the exact candidate. |
| C. Omega Agent program | #73, #74–#82 | Roadmap accepted as direction. #74 (contract admission) gates every other packet. |
| D. Workroom and mobile | #31, #45–#49 | `relay.openagents.com` is live with authenticated round trips. #45 foundations landed. #46 renders live on simulator and waits on the physical phone. #47 has its producer. #48 closed a replay revocation hole and continues. #49 is not started. |
| E. Small UX corrections | #68, #69, #70 | Scoped one-file to few-file fixes with owner direction recorded on each issue. |

## 4. Order and parallel lanes

### Wave 0 — start now, all lanes parallel

| Lane | Work | Note |
| --- | --- | --- |
| E1 | #68 recovery button | Apply option 2 from the issue: keep the action and relabel by state (`Protect recovery` versus `Replace recovery file`). Change the pinning test so the two states present different actions. The owner can veto the option choice on the issue. |
| E2 | #69 workroom keybinding | Add the three keymap entries with no context predicate. Record the Save As shadow trade on the issue. Add the resolvable-action test. |
| E3 | #70 Aiur dark-only | Rename `Aiur Dark` to `Aiur`, delete `Aiur Light`. Set `DEFAULT_LIGHT_THEME` to `Ayu Light` as the plan default, subject to owner veto on the issue. Keep the onboarding family arrays coherent. |
| C0 | #74 contract and traces | Write the Omega Agent ProductSpec, the shape record, and the cloud-coupling severability trace. Resolve the duplicate 0010 and 0011 delta identifiers. Ends at an owner admission gate. |
| A1 | #16 installed observations | Finish the partial installed-observation set against the installed rc6. Do not land omega commits from this lane. |
| D1 | #45 remainder | Finish the host and mobile foundation exits against the live relay. |
| D2 | #47 remainder | Finish the Full Auto adjunct joins and evidence fail-closed paths. |
| B1 | #26 preparation | Keep collector, replay, and journey scripts current against `omega-effectd-v0.1.0-rc.8`. Do not claim the owner-real run. |

Rules for Wave 0: E1, E2, and E3 serialize with each other only on shared
files (see section 5). E2 lands before any #76 keymap work. A1 runs against
the installed rc6 and is not invalidated by new `main` commits.

### Wave 1 — after #74 admission and E-lane landings

| Lane | Work | Depends on |
| --- | --- | --- |
| C1 | #75 identity rename | #74 admitted |
| C2 | #76 chat-first front door | #74 admitted, E2 landed |
| A2 | Cut `0.2.0-rc7` | E1, E2, E3, and any onboarding fixes landed. Freeze omega pushes during the bake. |
| A3 | #9 installed proof, #8 gate matrix on rc7 | A2. The automated matrices are scripted and cheap to re-run. |
| D3 | #46 and #48 continuation | ongoing |

### Wave 2 — router and accountability

| Lane | Work | Depends on |
| --- | --- | --- |
| C3 | #77 disclosure v0 | #75 |
| C4 | #80 receipts and run links | #77 |
| C5 | #81 harness maintenance | #75, loose |
| B2 | #26 replay and packaged journey on rc7 | A2 |

### Wave 3 — turn semantics and reach

| Lane | Work | Depends on |
| --- | --- | --- |
| C6 | #78 router v1 | #76, #77 |
| C7 | #79 steer and queue | #78 |
| C8 | #82 served over ACP | #78 |
| D4 | #49 physical-device proof | #46, #47, #48, owner devices |

### Owner gates, stated once

These acts are reserved to the owner. Route around them and continue other
lanes:

1. #74 ProductSpec admission (opens C1 through C8).
2. #8 owner observation of the packaged journey, and AssuranceSpec admission.
3. #26 owner-real multi-turn unattended run on an exact signed candidate.
4. #49 physical iPhone and Android sessions.
5. Vetoes on the recorded plan defaults in E1 and E3.

The independent reviewer is designated and functioning (omega#67 closed, the
rc6 review is signed). Route new review requests through that identity, never
through the producing agent.

## 5. Hot files and serialization

One agent owns a hot file at a time. Claim order is the issue order below.

| Hot surface | Issues that touch it | Rule |
| --- | --- | --- |
| `assets/keymaps/default-*.json` | #69, then #76 | #69 lands first. #76 owns the later cleanup. |
| `OMEGA_DELTAS.md` + `crates/omega_deltas` | #74, then every delta-bearing packet | #74 resolves the duplicate identifiers and reserves numbers. After that, allocate one delta number per packet through the issue thread. |
| `assets/settings/default.json` | #70, #76 | #70 first. |
| `crates/onboarding` | #68, #70, #9 follow-ups | Serialize in that order. |
| `crates/theme`, `crates/settings_content` | #70 | Single owner. |
| `crates/agent_ui` | #77, #78, #79, #80 | Serialize in dependency order. |
| `script/bundle-*`, release records | Track A only | Release lanes own packaging. |
| `crates/workroom_ui`, mobile workroom | D lanes | D lanes own them. |

## 6. Per-issue instruction summary

| Issue | Do next | Done means |
| --- | --- | --- |
| #8 | Assemble the remaining gate matrix on the freshest candidate. Request owner observation. | All gates green on one exact candidate, dual-role attestation recorded, admission accepted. |
| #9 | Run the installed 360px, large-font, and screen-reader proof on the next candidate. | Installed proof bound to the candidate digest. |
| #16 | Finish installed observations on rc6. | Observation set complete and accepted on the exact candidate. |
| #26 | Keep proofs current, then run replay and journey on rc7. Owner run stays reserved. | All ten gates green on one candidate with independent assurance. |
| #31 | Epic. Tracks #45 through #49. | All five children closed. |
| #45 | Close the remaining foundation exits against the live relay. | Both clients hold every exit on `relay.openagents.com`. |
| #46 | Keep simulator truth honest. Prepare the physical-phone checklist. | Physical-phone exits confirmed. |
| #47 | Finish adjunct joins and fail-closed evidence. | Phone and panels cannot disagree about host records. |
| #48 | Continue exits on the community room. | Community-room exits hold, including replay revocation. |
| #49 | Start after #46 through #48. Script everything that does not need the owner's hands. | Thin whole proven on physical devices. |
| #68 | Option 2 relabel with a state-splitting test. | Protected state shows no control that implies unprotected. |
| #69 | Three keymap entries, shadow note, resolvable-action test. | Binding opens the workroom from any focus context. |
| #70 | Rename, delete light variant, new light default, coherent arrays. | One `Aiur` theme, dark, with a valid light fallback. |
| #73 | Epic. Keep the checklist current. | #74 through #82 closed. |
| #74 | ProductSpec, shape record, severability trace, delta cleanup. | Owner admits the spec. No rename landed. |
| #75 | Rename identity surfaces with a delta test. | No reachable "Zed Agent" as the first-party identity. |
| #76 | New Agent Thread surface as welcome and keybinding target. | Fresh launch lands on the surface. Typing starts a thread. |
| #77 | Executor line by extension trait. | Every thread names its executor after restart. |
| #78 | `OmegaAgentConnection` deterministic router. | Pins honored, engine-down fails closed to native, decision recorded. |
| #79 | Distinct steer and enqueue with durable admission. | Send-during-turn has declared visible behavior on every executor class. |
| #80 | Typed dispatch with linked runs and receipts. | Runs render receipt chains. Engine stays sole run authority. |
| #81 | Maintenance actions with pinning and provenance receipts. | One-click update produces a receipt. Pins block unwanted updates. |
| #82 | Loopback ACP server behind `omega-effectd`, default off. | External host attaches read-only with correct disclosure. |

## 7. Capacity and coordination

Run at most six implementation lanes at once, plus one coordinating agent.
The coordinator owns integration against `origin/main`, the hot-file claim
order, epic checklists, release-candidate timing, and updates to this plan.
A spawned agent or a passing child test is not the integration receipt.
The coordinator reconciles every child result against current `origin/main`
before reporting a wave complete.

Khala integration mints no lanes in this plan.
The shape is fixed in the
[Omega Agent roadmap](./2026-07-25-omega-agent-roadmap.md) section 5, and the
reserved `OMEGA-AGENT-K1` through `K3` packets wait for owner admission.

## 8. Completion discipline

An issue closes only when its change is merged to the owning repository's
`main`, its verification is green from that integrated state, and any claimed
proof binds to an exact artifact.
Branch work and fixture passes are progress evidence, not completion.
Report each closure on the issue with commits, evidence paths, and the exact
candidate digest where one applies.
