# Unified Completion Plan for Open Issues in Omega Repository

- Status: active
- Owner: OpenAgents / Omega Team
- Date: 2026-07-25
- Target Repositories: `OpenAgentsInc/omega` and `OpenAgentsInc/openagents`
- Audience: Product, Engineering, Release, and Assurance Teams

---

## 1. Scope and Executive Summary

This document provides one unified plan to resolve all open issues in the `OpenAgentsInc/omega` repository.
It synthesizes 10 open issues across 3 technical tracks into a 4-phase execution sequence.

The execution sequence moves logically:
1. Protocol and record layer convergence between the Omega desktop host and OpenAgents mobile client.
2. Mobile Workroom user interface and community room completion.
3. Packaged macOS application candidate build, installed identity journey proof, and Full Auto incident replay proof.
4. Physical device multi-surface proof and epic closeout.

---

## 2. Open Issues Master Inventory

The 10 open issues in the `omega` repository are organized into three technical tracks:

### Track A: Identity-First Onboarding & Packaged Application Release (#9, #8, #16)

- **Issue #16: Prove the installed Omega RC brand and package**
  - **Type**: Enhancement / Release
  - **Objective**: Prove Omega desktop application packaging, Apple notarization, brand assets, update/rollback lifecycle, and network capture boundaries.
- **Issue #9: Add identity-first Omega onboarding**
  - **Type**: Enhancement / UX
  - **Objective**: Implement identity-first onboarding in GPUI where the user configures or imports an identity before editor and agent setup.
- **Issue #8: Prove the packaged Omega identity-first journey**
  - **Type**: Enhancement / Verification
  - **Objective**: Deliver candidate-bound evidence for the installed identity-first journey, including identity matrix execution, secret tripwire scanning, accessibility review, offline encrypted recovery, and independent attestation.

### Track B: Full Auto Autonomous Engine Proof (#26)

- **Issue #26: OMEGA-FA-07: Prove Omega Full Auto (incident replay and owner journey)**
  - **Type**: Enhancement / Verification
  - **Objective**: Prove the Full Auto autonomous execution engine on an installed Omega release candidate, including incident replay, multi-turn unattended owner journeys, Codex/Claude provider handoffs, process tree cleanup, and dual-role authority signoff.

### Track C: Sarah Workroom & Nostr-First Cutover (EPIC #31 & Sub-issues #45–#49)

- **Issue #31: EPIC: Sarah workroom — Nostr-first cutover, and the v2 community workroom**
  - **Type**: Epic / Master Tracking
  - **Objective**: Coordinate the Nostr-first Sarah workroom program across desktop, mobile, and relay infrastructure.
- **Issue #45: OMEGA-MOB-31-01: Connect mobile and Omega to the real Nostr record**
  - **Type**: Enhancement / Infrastructure
  - **Objective**: Prove real Nostr record convergence between a physical mobile device and an Omega desktop host without test relay fallbacks.
- **Issue #46: OMEGA-MOB-31-02: Complete the owner-private mobile room**
  - **Type**: Enhancement / Mobile UX
  - **Objective**: Render the complete signed Sarah transcript, activity ladder, send lifecycle, and host projection decryption in OpenAgents Mobile.
- **Issue #47: OMEGA-MOB-31-03: Join Full Auto, provider accounts, and evidence**
  - **Type**: Enhancement / Mobile Integration
  - **Objective**: Integrate Full Auto execution rows, provider account rosters, and public-safe evidence inspectors into the mobile Workroom.
- **Issue #48: OMEGA-MOB-31-04: Complete the community mobile room**
  - **Type**: Enhancement / Community
  - **Objective**: Connect community room projections and signed community actions (NIP-LBR, work-units, experience points, arbitration) in mobile and desktop.
- **Issue #49: OMEGA-MOB-31-05: Prove the issue #31 thin whole on physical devices**
  - **Type**: Enhancement / Verification
  - **Objective**: Execute terminal evidence-bound validation of Issue #31 on physical iOS and Android devices paired with an installed macOS Omega host.

---

## 3. Current State and Remaining Technical Gaps

### Issue #16: Installed Brand and Package Proof

- **Current Truth**: Release Candidates RC2 and RC3 were built, signed, notarized, and stapled.
- **Remaining Gap**: Recent source changes on `main` (accessibility semantics, recovery matrix, Full Auto process tree fixes) invalidate old candidate digests. A fresh Release Candidate must be built, notarized, stapled, and proven against current `main`.

### Issue #9: Identity-First Onboarding

- **Current Truth**: Source implementation landed in `efa9af40c2`. Onboarding theme tiles, Agent Setup cards, accessibility semantics, and tab focus controls pass unit tests and clippy.
- **Remaining Gap**: Requires installed proof on a fresh signed Release Candidate under Issue #8.

### Issue #8: Packaged Identity-First Journey Proof

- **Current Truth**: Source hardening landed in `cde0e33348` (#53). Euler attested 5 gates on the frozen RC3 candidate (`b50bf428614c...`).
- **Remaining Gap**:
  1. Build a fresh Release Candidate from current `main`.
  2. Execute the expanded 24-case identity matrix and 6-surface secret tripwire scanner.
  3. Execute the deny-network offline encrypted recovery matrix.
  4. Perform an 11-check manual and screen reader accessibility review.
  5. Run pristine install, removal, reinstall, and downgrade/rollback lifecycle checks.
  6. Collect named owner observation and independent attestation from `openagents.assurance_reviewer`.

### Issue #26: Full Auto Incident Replay and Owner Journey

- **Current Truth**: Process settlement, cancellation grace, and process tree orphan cleanup fixes landed in `15edb3286a` paired with `omega-effectd-v0.1.0-rc.8`. Full Auto evidence collector implemented in `e3e89fadcb`. Partial RC3 run (`run.full-auto.mrzo65ww...`) recorded.
- **Remaining Gap**:
  1. Execute incident replay and multi-turn unattended owner journeys on a fresh installed Release Candidate.
  2. Verify Codex-to-Claude provider handoffs without process tree leaks.
  3. Verify restart reconciliation and offline/Sync gap behavior.
  4. Collect complete evidence root.
  5. Obtain `openagents.assurance_reviewer` verification decision receipt.
  6. Obtain `openagents.owner` release decision receipt.

### Issue #31 & Sub-issues #45–#49: Sarah Workroom & Mobile Parity

- **Issue #44**: Completed (`5522a3457e` / `bdd3419b66`).
- **Issue #45**: Host foundation (`deecc5ab33`) and mobile foundation (`e0326cf6c3`, `65687ef65d`) landed.
  - **Remaining Gap**: Finalize v2 command record format (carrying message/read/reminder payloads), host-mediated source-bound per-record projections encrypted for paired devices, NIP-RS/ER replaceable no-`p` event shape alignment, signed `turn.interrupted` reconciliation, and joint physical phone pairing convergence proof.
- **Issue #46**: Split into mobile UI lane and production Sarah relay consumer lane (`sarah-nostr-relay-consumer`).
  - **Remaining Gap**: Complete v2 transcript rendering with pagination, activity ladder, send/pending/confirmed/refused/failed lifecycle, and mobile projection decryption.
- **Issue #47**: Blocked on #45 and #46.
  - **Remaining Gap**: Wire Full Auto run status rows, provider account roster, and evidence inspector into mobile Workroom.
- **Issue #48**: Blocked on #45 and #46.
  - **Remaining Gap**: Connect community room projection and signed community actions in mobile Workroom and Omega host.
- **Issue #49**: Terminal verification gate.
  - **Remaining Gap**: Execute end-to-end evidence run on physical iOS/Android devices paired with an installed macOS Omega host across owner-private and community rooms.

---

## 4. Four-Phase Unified Execution Plan

```
[Phase 1: Protocol & Record Layer Convergence] (#45, #46 Relay Consumer)
  │
  ▼
[Phase 2: Mobile Workroom Experience Completion] (#46, #47, #48)
  │
  ▼
[Phase 3: Packaged macOS Candidate Release & Proof] (#16, #9, #8, #26)
  │
  ▼
[Phase 4: Physical Device & Multi-Surface Final Convergence Proof] (#49, #31 Closeout)
```

### Phase 1: Protocol & Record Layer Convergence (Issues #45 & #46 Relay Consumer)

- **Task 1.1: Coordinated v2 Command Record & Projection Schema**
  - Define v2 command payload schema supporting message, read-state, and reminder operations.
  - Implement host-mediated source-bound per-record projection encrypted to the paired device's public key (NIP-44/59).
- **Task 1.2: Production Sarah Relay Consumer**
  - Complete `apps/openagents.com/workers/api/src/sarah-nostr-relay-consumer` in `openagents`.
  - Remove test signer/cipher/publisher fallbacks and enforce production relay acknowledgements.
- **Task 1.3: Protocol Alignment & Interrupt Reconciliation**
  - Align NIP-RS and NIP-ER replaceable event shapes without extraneous `p` tags.
  - Implement signed `turn.interrupted` terminal state reconciliation.
- **Phase 1 Verification Gate**:
  - `pnpm --filter @openagentsinc/sarah test` passes.
  - `pnpm --filter @openagentsinc/openagents-mobile test` passes.
  - `cargo test -p omega_effectd` passes.

### Phase 2: Mobile Workroom Experience Completion (Issues #46, #47, #48)

- **Task 2.1: Owner-Private Mobile Room (#46)**
  - Implement v2 transcript rendering with bounded pagination.
  - Render activity ladder without fake streaming.
  - Wire send lifecycle: pending, confirmed, refused, failed, and terminal states.
  - Decrypt host-mediated per-record projections using the device key.
- **Task 2.2: Full Auto, Provider Accounts, & Evidence Integration (#47)**
  - Embed Full Auto execution run rows in the mobile Workroom.
  - Render provider account roster and quota readiness.
  - Add public-safe evidence inspector drawer.
- **Task 2.3: Community Mobile Room Integration (#48)**
  - Wire community room projection skeleton to real Nostr community events (NIP-LBR, NIP-29).
  - Implement signed community actions: work-unit claim, submission, and arbitration.
- **Phase 2 Verification Gate**:
  - `pnpm --filter @openagentsinc/openagents-mobile typecheck` passes cleanly.
  - All 348+ mobile unit tests pass.

### Phase 3: Packaged macOS Candidate Release & Proof (Issues #16, #9, #8, #26)

- **Task 3.1: Build Fresh macOS Release Candidate (#16)**
  - Compile production Omega binary from current `main`.
  - Package DMG, submit for Apple notarization, and staple ticket.
  - Record candidate SHA-256 and release record.
- **Task 3.2: Packaged Identity-First Journey Proof (#9, #8)**
  - Install notarized candidate to `/Applications/Omega.app`.
  - Run automated 24-case identity matrix and 6-surface secret tripwire scanner.
  - Run deny-network offline encrypted recovery matrix and rollback lifecycle checks.
  - Perform 11-check manual and VoiceOver screen reader accessibility review.
  - Record named owner observation.
  - Obtain `openagents.assurance_reviewer` (Euler) independent attestation receipt.
- **Task 3.3: Full Auto Incident Replay & Owner Journey Proof (#26)**
  - Execute incident replay and multi-turn unattended owner journeys on installed candidate.
  - Verify Codex-to-Claude provider handoffs and clean process tree termination.
  - Collect evidence root via `collect-omega-full-auto-installed-evidence`.
  - Obtain `openagents.assurance_reviewer` verification decision.
  - Obtain `openagents.owner` release decision.
- **Phase 3 Verification Gate**:
  - `IDENTITY_PROOF.md` and `FULL_AUTO_PROOF.md` generated with `status: verified`.
  - Installed proof harnesses validate all input SHA-256 digests.

### Phase 4: Physical Device & Multi-Surface Final Convergence Proof (Issues #49 & #31 Closeout)

- **Task 4.1: Physical Device Pairing & Real-Record Convergence (#49)**
  - Pair signed physical iOS and Android builds with installed macOS Omega host over production Nostr relay.
  - Execute owner-private conversation and command loop.
  - Execute community room interaction and experience point projection.
- **Task 4.2: Roadmap Section 11.1 Matrix Validation (#49)**
  - Verify every row in section 11.1 of the adaptation audit has a real source, authorized interaction, and honest error handling.
- **Task 4.3: Sequential Closeout of Open Issues**
  - Sequentially close issues in `omega` repository upon evidence verification: #45, #46, #47, #48, #49, #31, #8, #9, #16, #26.
- **Phase 4 Verification Gate**:
  - All 10 open issues verified and closed with explicit commit references and hashed evidence receipts.

---

## 5. Cross-Repository Dependency & Ownership Matrix

| Issue ID | Owning Repository | Primary Source Paths | Key Verification Commands |
| :--- | :--- | :--- | :--- |
| **#16** | `omega` | `cargo/`, `script/bundle-mac` | `script/bundle-mac --notarize` |
| **#9** | `omega` | `crates/agent_ui/src/onboarding/` | `cargo test -p agent_ui onboarding` |
| **#8** | `omega` | `crates/omega_identity/`, `script/` | `cargo test -p omega_identity` |
| **#26** | `omega` | `crates/omega_effectd/`, `script/` | `script/collect-omega-full-auto-installed-evidence` |
| **#31** | `openagents` + `omega` | `apps/openagents-mobile/`, `crates/workroom_ui/` | Joint review & physical proof |
| **#45** | `openagents` + `omega` | `packages/sarah/`, `crates/omega_effectd/` | `pnpm --filter @openagentsinc/sarah test` |
| **#46** | `openagents` | `apps/openagents-mobile/src/workroom/` | `pnpm --filter @openagentsinc/openagents-mobile test` |
| **#47** | `openagents` + `omega` | `apps/openagents-mobile/src/full-auto/` | Mobile test suite |
| **#48** | `openagents` + `omega` | `packages/world-client/`, `crates/workroom_ui/` | Shared community tests |
| **#49** | `openagents` + `omega` | Both repositories | Physical iOS/Android + macOS proof |

---

## 6. Verification and Evidence Protocol

All claims of progress or issue completion must adhere to the OpenAgents Assurance Protocol:

1. **Candidate Binding**: All installed proof evidence must be bound to the exact candidate digest (`sha256`) of the notarized DMG and release record.
2. **Traversal Safety**: Evidence references must use non-symlink `{"path", "sha256"}` structures beneath an explicit evidence root.
3. **No Credential Copy**: Verification must use native session vaults or mock keychains without copying user credentials.
4. **Dual-Role Attestation**: Proof under #8 and #26 requires two distinct attestations:
   - `openagents.assurance_reviewer` (independent reviewer decision)
   - `openagents.owner` (final owner release decision)
   - Self-review or role substitution is strictly forbidden.
5. **No Fabricated Evidence**: Unissued or unperformed gates must remain explicitly `pending_required_gates`. No screenshot or historical evidence from prior release candidates may be promoted across source changes.
