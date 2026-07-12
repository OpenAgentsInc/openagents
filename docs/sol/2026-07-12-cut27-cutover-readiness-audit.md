# CUT-27 cutover-readiness audit — 2026-07-12

- Issue: #8707 "CUT-27 — Prove and declare the Codex/Claude-to-OpenAgents
  coding cutover" (capstone of #8566 / #8574 / #8597)
- Audit base: `origin/main` @ `375a8997ff` (all cited SHAs verified as
  ancestors of that tip)
- Status: **readiness reconciliation only.** This document does NOT declare
  the cutover. It states, per CUT leaf and per #8707 completion criterion,
  what is landed, its proof rung, and the exact remaining gate.
- Rung vocabulary: the six proof rungs of `MASTER_ROADMAP.md` §"Proof status
  is six distinct rungs" — code-landed → fixture-proven → deployed →
  live-proven → owner-accepted → closed. No rung implies the next.
- **Amendment (owner decision, 2026-07-12, verbatim: "make nothing at all
  gated on physical android. i dont care. emulator is fine"):** every
  Android requirement in this audit and in the CUT program is satisfiable
  with Android **emulator** evidence. No gate anywhere blocks on physical
  Android hardware. Physical iOS gates are unchanged. Android references
  below are annotated accordingly.

## Method

Read the live issue ledger for #8681–#8707 plus parents/proofs #8640, #8676,
#8677, #8678; the `docs/sol/2026-07-11-cut-*` and `2026-07-12-cut-23-*`
receipts; `MASTER_ROADMAP.md` CUT sections; and the capability registry and
eval scoreboard (`apps/openagents-desktop/src/capability-registry.ts`,
`apps/openagents-desktop/tests/capability-evals.test.ts`). Every commit SHA
cited below was checked with `git merge-base --is-ancestor` against
`origin/main` @ `375a8997ff`.

## Closed foundation (CUT-01 – CUT-08)

| Leaf | Issue | Rung | Evidence |
| --- | --- | --- | --- |
| CUT-01 composer-island removal / EN surface authority | #8681 | closed | roadmap Rev. entries; physical installed-product acceptance explicitly transferred to CUT-27 |
| CUT-02 truthful green Desktop verification baseline | #8682 | closed | deterministic package verification baseline |
| CUT-03 source-coupled topology oracle, ambient-authority denial | #8683 | closed | completes #8678 with CUT-04 |
| CUT-04 service replaceability, disposal, correlation | #8684 | closed | built-host receipt under #8678 (closed) |
| CUT-05 Claude owner-local permission mode | #8685 | closed | `eb030eb748` local permission posture line is on main |
| CUT-06 supervisor-scope leaks, verifier/publication ordering | #8686 | closed | closed by production run `fleet_run.sarah.666432631ce5e88a47a5` with #8640 (stack `54934f05f5`, migration `0060`, production revision `openagents-monolith-00084-tnv`) — live-proven and owner-accepted |
| CUT-07 command convergence (lost ACK, duplicates, offline expiry) | #8687 | closed | [receipt](./2026-07-11-cut-07-command-convergence-receipt.md); migration `0061`; Runtime Gateway v7 |
| CUT-08 event ordering / cursor gaps / store compatibility | #8688 | closed | [receipt](./2026-07-11-cut-08-event-store-convergence-receipt.md); local-store schema v1 across Bun/Desktop/Expo/Web |

Closed here means the deterministic scope those leaves owned; the live fault
rung they feed (#8677) remains open and is accounted under CUT-09.

## Leaf-by-leaf status (CUT-09 – CUT-27)

### CUT-09 #8689 — lifecycle convergence (restart, stale generation, revocation, interrupted finalization) — OPEN

- Landed on main: deterministic fault rows 7–9 at `976e6b5b26` +
  `e941fb0e25`; real Desktop/Expo SQLite restart reconstruction; built
  Desktop Runtime Gateway v7 smoke green.
  [Receipt](./2026-07-11-cut-09-lifecycle-convergence-receipt.md).
- Rung: **fixture-proven**, with a built-Electron receipt (deployed for the
  dev-built artifact). On 2026-07-12 the signed development build was
  **installed on the paired physical iPhone** (`5205b4fa8c` records the
  rung); launch stopped at the device lock screen only.
- Remaining gate: the live physical acceptance row — on-device unlock,
  sign-in, and the 4-step built-Desktop/physical-mobile network-gap/restart
  journey. Owner-gated (physical touch). Closing this also closes #8677.

### CUT-10 #8690 — no-poll Runtime Gateway live events — OPEN

- Landed on main: shared Sync cursor-aware generation-fenced live events;
  mobile no-poll; Desktop renderer adapter; final Desktop consumer at
  `5e20b033a6` with a source oracle forbidding `pollAttempts` /
  `sleep(100)` / `setInterval`. Full Desktop verify green (311 tests /
  1,581 expectations at landing). A later live run exposed a separate
  boot-time lifetime race: catalog admission could arrive before verified
  Sync finished catching up and pin the renderer local. `1afe5328e2` adds
  per-operation authoritative convergence without an interval and preserves
  local thread ownership; full Desktop verification is 1,019 pass / 2 skips.
  [Receipt](./2026-07-11-cut-10-live-event-convergence-receipt.md).
- Rung: **fixture-proven** + built-Electron receipt. Non-device work is
  complete.
- Remaining gate: physical-mobile continuation receipt (owner-deferred while
  the recording phone was occupied). Owner-gated (physical).

### CUT-11 #8691 — canonical live Codex/Claude agent graph — OPEN

- Landed on main: `openagents.live_agent_graph.v1` schema + exact-cursor
  reducer; typed Codex app-server and Claude Agent SDK observation adapters;
  Sync server projector with green real-Postgres receipt; Runtime Gateway v8
  graph refs/post-images; confirmed-only client read model; desktop-local
  assembler folding fable-local/codex-local streams through the shared
  reducer at `0f475ce89b`.
  [Receipt](./2026-07-11-cut-11-live-agent-graph-receipt.md).
- Rung: **fixture-proven** for the schema/reducer/projection; **partial
  live probes** — redacted named-account probes prove the live Claude child
  lifecycle and the Codex `subAgentActivity` typed source.
- Remaining gates (code): (1) the Codex child-activity source gap — the
  SDK-bundled binary fails before a frame and the PATH binary's
  `--experimental-json` encoder omits the child activity record; Pylon must
  converge that typed app-server source (tools/history remain forbidden
  parentage sources); (2) desktop main-process live emission wiring
  (explicit residual — main.ts was hot); (3) a named confirmed-reconnect
  live trace.

### CUT-12 #8692 — equivalent Desktop/mobile live-agent supervision UI — OPEN

- Landed on main: shared presentation model + Khala Mobile surface at
  `21e9740ed9`; Desktop hierarchy/inspector over Gateway v8 at
  `c4ca3b86e2` (pointer/keyboard/screen-reader typed actions, historical
  authority labeled and control-disabled, 200-row bound, no poller). Full
  suites green at landing.
  [Receipt](./2026-07-11-cut-12-live-agent-supervision-ui-receipt.md).
- Rung: **fixture-proven** + built-Electron smoke.
- Remaining gate: physical iOS interaction receipts (owner-gated) and the
  Android interaction receipts, which per the 2026-07-12 owner decision are
  satisfiable on **emulator** (no physical Android gate). No Android
  evidence of either kind exists in the CUT program yet.

### CUT-13 #8693 — canonical project/repository/session navigation — CLOSED

- Rung: **closed** at `0c49648217`. `openagents.coding_catalog.v1`,
  64-state restart-restore model, real-Postgres server projection.
  [Receipt](./2026-07-11-cut-13-canonical-coding-session-catalog-receipt.md).

### CUT-14 #8694 — mobile bound to authenticated repos/sessions/threads — OPEN

- Landed on main: mobile binding to the live personal-scope CUT-13 catalog
  ([receipt](./2026-07-11-cut-14-mobile-authenticated-catalog-receipt.md));
  loss-accounted offline cache accounting at `ff8cc0699b` (contract
  `openagents_mobile.seam.coding_offline_cache_accounting.v1`).
- Rung: **fixture-proven** (real-SQLite oracles; simulator-level evidence).
- Remaining gate: physical-iOS plus Android-**emulator** (owner decision
  2026-07-12) process-death/reconnect and deep-link/notification receipts
  required by the close rule. The iOS half stays owner-gated (physical);
  the Android half is automatable.

### CUT-15 #8695 — typed commands/keybindings/menus/deep links — CLOSED

- Rung: **closed** at `5d36b73ad2`. Signed release packaging explicitly
  deferred to CUT-26.
  [Receipt](./2026-07-11-cut-15-canonical-command-registry-receipt.md).

### CUT-16 #8696 — Effect Native coding composer, questions, approvals, runtime controls — OPEN

- Landed on main: shared composer kernel reuse
  ([receipt](./2026-07-11-cut-16-composer-runtime-interactions-receipt.md));
  mobile native multi-file/image attachment acquisition at `c3ad8bee34`
  (bounded, hashed, sandbox-copied, `attachment.native-local.sha256` refs;
  simulator native-link/render smoke only).
- Rung: **fixture-proven**; the attachment tranche is code-landed +
  fixture-proven, explicitly *not* a picker-tap or physical receipt.
- Remaining gates: (code) attachment-bearing runtime submission/delivery
  end-to-end; Desktop file attachments/mentions (capability **I4 is still
  `missing`** in the registry); session fork (**H2 `missing`**) if it stays
  in scope rather than the loss register; (live) real provider/model/account
  selection exercised in a live turn (couples to CUT-21); (physical) the
  named cross-client receipts.

### CUT-17 #8697 — grant-scoped workspace tree/watch/cache/search — CLOSED

- Rung: **closed**; WorkContext capability core at `4bbf0c7758`.
  [Receipt](./2026-07-11-cut-17-workspace-capability-receipt.md).

### CUT-18 #8698 — editor host + conflict-safe document lifecycle — CLOSED

- Rung: **closed** on the stack ending `091574d5bc`.
  [Receipt](./2026-07-11-cut-18-conflict-safe-editor-receipt.md).

### CUT-19 #8699 — typed Git review and composer-context loop — CLOSED

- Rung: **closed** at `6ff137e5c0`.
  [Receipt](./2026-07-11-cut-19-git-review-context-receipt.md).

### CUT-20 #8700 — workspace-bounded PTY terminals + local preview — CLOSED

- Rung: **closed** at `b57bf71fac` (2026-07-12). Full adversarial suite,
  built-host + dev-preview receipts, process-tree disposal, capability D3
  flipped to `ui_available` at the **live** rung.

### CUT-21 #8701 — provider-neutral named Codex/Claude accounts, models, runtimes — CLOSED

- Landed on main: fail-closed Codex 0.144.1 / Claude Agent SDK 0.3.172
  compatibility contract `ff222f367b`; bundled runtime resolution + host
  probes + redacted runtime catalog `c0e8362cc5`; exact
  provider/model/account target boundary (named account never silently
  rotates) `655a0b772b`; Effect Native composer account selector +
  per-conversation exact target delivery `25b35b4d24`. A follow-up P0 fix
  `e7fb41623e` restored fable+codex **fixture** streaming under exact
  targets.
- Rung: **live-proven for both named providers**. The initial Claude
  0-character receipt was a driver false
  negative: the authoritative built-app thread store contained the completed
  assistant note while the driver treated the queue-enabled textarea as an
  idle signal. `716955d5ac` made provider steps exit-driving; `5e701a93b7`
  added exact named-account selection and Stop-authoritative settle logic. A
  built-app run then selected `claude-pylon-3`, reported effective model
  `claude-fable-5`, captured midstream/final PNGs, and completed with visible
  assistant text. The owner then completed the isolated device authorization
  for `codex-2`; the real preflight observed exactly one verified account.
  One fail-closed built-app run selected `claude-pylon-3` / effective
  `claude-fable-5` and `codex-2` / requested `gpt-5.6-sol`, captured
  midstream and final PNGs for both, observed streaming for both, and exited
  0 with no required failures. Other revoked Codex homes remain visibly
  `reconnect_required` and never substitute for the exact selected account.

### CUT-22 #8702 — Claude Code history import, loss-accounted — CLOSED

- Rung: **closed** (2026-07-12); criterion-3 sub-case addendum at
  `083f3f3654` (malformed / huge / account-removal / duplicate / truncated /
  schema-drift each a named oracle; 67 pass / 0 fail on the CUT-22 surface).

### CUT-23 #8703 — MCP, skills, plugins, permissions, settings typed lifecycle — CLOSED

- Landed on main: MCP/plugin/skill registries with modeled `/skill` grammar
  (never keyword-routed prose); unified declare→validate→enable→run→revoke
  lifecycle projection at `2f062a29e8` (skill grants scoped under parent
  plugin, duplicate detection, honest `partial` audits, explicit provider
  disagreement — Claude lane supported, Codex explicitly unsupported); R3
  permission-mode control (`Full tools` / `Plan only`, `bypassPermissions`
  rejected). Capabilities I3/J2/J3 now `ui_available`.
  [Receipt](./2026-07-12-cut-23-plugin-registry-receipt.md).
- Rung: **live-proven for the Claude plugin/skill lane**. On 2026-07-12 the
  typed registry admitted the local Anthropic `skill-creator` plugin, resolved
  its explicit skill grant, and named account `claude-pylon-3` streamed a real
  `claude-fable-5` response with exact usage (330 input, 15,663 cached, 735
  output, 16,728 total); no tools, files, or secrets were used.
- Close interpretation: "each provider" means each provider that declares the
  lifecycle supported. Claude passed the live workflow; Codex declares the
  plugin/skill lifecycle unsupported through the same typed disagreement
  projection and is recorded in the cutover exception register. It is never
  silently treated as a supported-but-untested lane.

### CUT-24 #8704 — preferences, accessibility, notifications, diagnostics, recovery — CLOSED

- Rung: **closed** (2026-07-12). Desktop half `64dcbb52e9`; mobile a11y half
  `06c3115280` (contract
  `openagents_mobile.seam.accessibility_core_flows.v1`).
- Residual (explicitly out of code scope, feeds CUT-27 criterion 4):
  physical VoiceOver/TalkBack live-device QA — owner-gated manual
  acceptance.

### CUT-25 #8705 — Fleet as authoritative cockpit + mobile attention surface — OPEN

- Landed on main: authoritative work cockpit `cc9cead0e1`; confirmed
  generation-bound interrupt/continue/retry/close controls over Runtime
  Gateway protocol v10 with no optimistic flips `bb170226b6`; exact-ref
  Approve/Deny pending-interaction controls `83efc87477`. Mobile derives
  from the same shared closed action table (pause deliberately filtered on
  mobile pending a distinct transport — a loss-register candidate).
- Rung: **fixture-proven** (Desktop 912 pass / 3 documented skips at
  landing; mobile 96 pass).
- Remaining gate: live/physical operator acceptance including a successful
  named-provider receipt — blocked behind CUT-21's live receipts and the
  physical-device set. Its issue criterion "named simultaneous Codex+Claude
  work with mobile attention/approval and Desktop acknowledgement" is the
  same journey CUT-27 criterion 2 needs.

### CUT-26 #8706 — hardened distribution, updates, rollback, legacy lockout — OPEN

- Landed on main: typed update/rollback contract + release-preflight oracles
  `f511c6a0ae` (`openagents.desktop.update_manifest.v1`, ed25519
  fail-closed verification, rollback-only sanctioned downgrade,
  loss-free interruption); hardened macOS arm64 Forge artifact
  `4315500bff` (frozen identity, strict Electron fuses, hardened-runtime
  entitlements, provider runtimes outside ASAR, real unsigned package
  receipt at 582 MB); API-key notarization wiring `844220af45`; native DMG
  maker preparation `dce7c749c3`. A real Developer ID signed and Apple-
  notarized app, DMG, and ZIP were then produced locally: deep strict
  `codesign`, stapler, and Gatekeeper all passed; Apple submission
  `7f06b92a-99ae-4bb1-8c87-5d16eb728326` was accepted. The unpublished
  artifacts were removed after verification. Independent signed-feed and
  asset-preserving deploy seams landed through `a0419ffd13`; Cloud Build
  `f8c2c6e5-f362-4b67-9f9d-4816215504d8` passed and Cloud Run revision
  `oa-updates-00093-g22` serves the fail-closed feed boundary while
  preserving the existing mobile release. A repo guard gates main pushes
  touching the Desktop app on the built Electron smoke (`375a8997ff`).
- Rung: **deployed** for the update-service seam and **live-proven** for local
  signing/notarization verification; no Desktop artifact is published.
- Remaining gates: DMG/ZIP publish; clean-machine
  install → update → interruption → rollback → uninstall acceptance
  transcript/video; and **legacy lockout** (marking direct Codex/Claude
  Code UI unsupported for the proven scope) has no landed evidence yet —
  it is coupled to the CUT-27 declaration itself.

### CUT-27 #8707 — prove and declare the cutover — OPEN (this audit)

No feature implementation may hide here; this document is the R7
reconciliation input only.

## Related proof parents

- **#8640** (simultaneous named Codex + Claude Phase A): CLOSED —
  **live-proven and owner-accepted** via production run
  `fleet_run.sarah.666432631ce5e88a47a5`.
- **#8678** (Desktop service scopes/boundary oracles): CLOSED.
- **#8677** (command/event fault convergence): OPEN — closes with CUT-09's
  live physical row.
- **#8676** (real streamed Desktop conversation with physical mobile
  continuation): OPEN — the accepted handoff run that CUT-27 criterion 3
  requires; the signed build is now installed on the paired iPhone, so this
  is runnable at owner convenience.

## Capability scoreboard snapshot (registry @ `375a8997ff`)

41 rows in `apps/openagents-desktop/src/capability-registry.ts`, enforced by
`tests/capability-evals.test.ts`: **24 `ui_available` / 14 `partial` /
2 `missing` (H2 session fork, I4 file attachments-mentions)**; rungs:
9 `live`, 27 `fixture`, 4 `pending` (H1 resume picker, H2, H5 context
compaction, I4). A declaration must either close these or name them in the
loss/exception register explicitly.

## #8707 completion criteria, one by one

1. **Clean installed build, one real non-trivial task each with named Codex
   and named Claude** — NOT met. A signed/notarized installer has been
   verified locally but is not published or clean-machine accepted. Blocked
   by: CUT-26 clean-install publication/acceptance and CUT-16 residuals for
   the full composer loop. All in-app workbench legs (files/editor/Git/PTY/preview) are
   closed (CUT-17–CUT-20).
2. **Physical iOS and Android reconnect/continuation/attention/interruption
   convergence during each task** — NOT met. iOS: build installed, journeys
   pending (CUT-09/10/12/14 physical rows, #8676). Android: no evidence
   now has fresh-install, cold-launch, offline restart, and fail-closed deep-link
   emulator evidence; authenticated restoration/deep-link legs remain.
3. **Run accepted #8676 handoff and #8677/#8678/#8640 parent receipts;
   every cutover leaf closed; no waived P0** — NOT met. #8678/#8640 closed;
   #8676/#8677 open; 8 CUT leaves open (09, 10, 11, 12, 14, 16, 25,
   26). The owner's 2026-07-11 phone deferral explicitly did NOT waive
   any acceptance gate.
4. **Publish loss/exception register, accessibility/privacy/security
   results, artifact provenance, rollback result, remote-work boundary** —
   PARTIAL inputs exist: a11y code + contracts landed (CUT-24) but physical
   VoiceOver/TalkBack QA pending; rollback contract fixture-proven but no
   clean-machine rollback result; artifact provenance pending the signed
   run; the later-remote-work boundary is already drafted in
   [`2026-07-11-remote-first-portable-coding-sessions-pathway.md`](./2026-07-11-remote-first-portable-coding-sessions-pathway.md)
   and the roadmap's Revision 31 non-goals. The register itself is
   declaration-time authoring (candidates already visible: H2, I4, H1/H5
   pendings, mobile pause transport, Codex plan-mode/extension
   unsupported facts, Codex child-activity encoder gap if still open).
5. **Docs/runbook/roadmap flip: Desktop default local coding surface,
   direct Codex/Claude Code UI fallback unsupported for the proven scope** —
   NOT started (correctly — it must not precede acceptance). Couples to
   CUT-26's legacy lockout.

## Ordered remaining blockers to an honest declaration

### (a) Code work

1. CUT-11: converge the Codex typed app-server child-activity source
   (bundled binary fails pre-frame; PATH `--experimental-json` omits child
   records), wire desktop main-process live graph emission, and support the
   named confirmed-reconnect trace.
2. CUT-16: attachment-bearing runtime submission/delivery end-to-end;
   Desktop attachments/mentions (I4); decide H2 session fork (build it or
   register it as an explicit exception).
3. CUT-26: publish the verified DMG/ZIP through the deployed signed-feed seam
   and enforce legacy lockout for the proven scope.
4. CUT-27 declaration artifacts: loss/exception register,
   provenance/rollback/a11y/privacy/security results bundle, and the
   default-surface docs flip (authorable only after (b)/(c) pass).

### (b) Live-proof runs (no owner hardware, but some need (c)-item 1 first)

1. CUT-26: publish the locally verified signed/notarized artifact, then run
   install/update/interruption/rollback/uninstall acceptance.
2. CUT-25: live operator acceptance of the Fleet cockpit with named
   simultaneous Codex+Claude work.
3. #8676: the full streamed Desktop conversation with mobile continuation
   run end-to-end (also the CUT-27 criterion-1/2 spine).

### (c) Owner-gated physical/manual acceptance

1. Reconnect at least one named isolated Codex account in Desktop Settings
   (preflight found 0/7 verified, typed `reconnect_required`).
2. Paired iPhone: unlock, launch, sign in (build already installed), then
   run the CUT-09 network-gap/restart journey — closes #8689 and #8677.
3. Paired iPhone: CUT-10 continuation, CUT-12 supervision interaction, and
   CUT-14 process-death/reconnect + deep-link receipts.
4. ~~Physical Android device~~ **Reclassified (owner decision 2026-07-12):
   Android journeys run on emulator and are NOT owner-gated** — the
   CUT-12/CUT-14 and criterion-2 Android legs move to bucket (b) as
   automatable live-proof runs. Still the program's largest unstarted
   surface, but no hardware acquisition is required.
5. CUT-24 residual: VoiceOver/TalkBack live-device accessibility QA.
6. CUT-26: clean-machine acceptance transcript/video (the owner-held
   Developer ID signing/notarization gate passed locally on 2026-07-12).
7. Final owner review/acceptance of the #8676 handoff and the CUT-27
   evidence bundle; only then close #8574's ordinary local-coding scope
   (never #8566/#8597, whose remote-first outcomes stay open).

## Honest summary

18 of 27 CUT leaves are closed (01–08, 13, 15, 17, 18, 19, 20–24);
8 are open (09, 10, 11, 12, 14, 16, 25, 26) plus CUT-27 itself.
The deterministic/fixture spine of
the cutover is substantially complete and enforced by ~900+ Desktop tests,
mobile suites, real-Postgres projections, and built-Electron smokes wired
into a main-push guard. What separates today's state from an honest
declaration is concentrated, not diffuse: one signed installer journey, one
iPhone journey set on an already-installed
build, the authenticated Android emulator leg, and the small
CUT-11/CUT-16 code residuals. Nothing reviewed here justifies declaring the cutover now, and
nothing suggests the remaining gates require new architecture.

## End-of-day addendum — 2026-07-12 (post-audit landings)

Everything below is verified as an ancestor of `origin/main` @ `77c4673dfe`.
The audit body above reflects the morning base (`375a8997ff`) and is retained
unedited except for the Android-emulator amendments; this addendum is the
current blocker state.

### Landed since the audit base

- Android policy: `1eb9f1a95f` — owner decision, no physical-Android gate
  anywhere; emulator satisfies every Android leg.
- **First Android evidence in the program**: `53d928e6cb` — emulator receipts
  for cold launch, PKCE sign-in entry, deep-link fail-closed (warm+cold),
  process-death reconvergence, offline OTA fallback
  (`2026-07-12-android-emulator-receipts.md`, 9 screenshots).
- CUT-11 residuals CLOSED as code: `f1d7029554` (main-process live graph
  wiring, push+snapshot IPC), `d6a906187a` (Codex app-server child-activity
  source convergence, app-server-first dispatch), `42e2698058` (docs).
  Residual (3) live trace: the **Claude leg ran live** — a real named-account
  runtime turn end-to-end through durable production Sync with exact usage
  and single-winner claim admission. Confirmed-graph reconnect legs are
  **deploy-gated**: the deployed production API predates the server graph
  binding, so live turns emit no `live_agent_graph` rows until the next API
  deploy.
- CUT-12 residuals: `55fea7e4f5` — greenfield `apps/openagents-mobile`
  supervision stack (the gap the Android lane exposed) + shared exact-token
  attribution presentation. #8692 residual is physical-iOS interaction only;
  its Android leg is now emulator-runnable.
- CUT-16: `82b2682544` — durable-turn Stop and queue-until-idle now real
  (both previously no-oped), control-intent lane fence fixed, contract
  `durable_runtime_turn_controls.v1`.
- CUT-26 code residuals CLOSED: `8a6a5b5167` — scripted signed publish path
  converged on the deployed oa-updates desktop feed seam + legacy desktop
  lockout (typed 410, armed by default, fail-closed). Only owner ceremony
  remains.
- #8636: `5df9aa6a88` — typed `khala.fleet_execution_target_decision.v1`
  routing with honest provenance in the intake projection.
- Live-dispatch defect found and fixed during the physical-iPhone lane:
  `9f8a76333c` — truncated claim `stableId` meant only the FIRST
  `turn.desktop.*` run in history could ever dispatch; later runs were
  silently rejected. SHA-256 ids + regression test.
- Physical iPhone: signed build launched and running on-device; live Desktop
  rungs receipted (hosted streamed turn, host-restart identity, mid-stream
  renderer reload). Remaining: 4 owner taps (see `NEEDS_OWNER.md`, now
  push-visible on GitHub per the new workspace AGENTS.md rule).
- #8704 (CUT-24) closed earlier today with desktop + mobile receipts.

### Current blocker list (supersedes the one above)

(a) Code: CUT-16 attachment-bearing runtime delivery + capability H2/I4
residuals (H1/H2 under an active #8712 claim by a concurrent session);
CUT-25 remainder per its own ledger. CUT-11/CUT-12/CUT-26 code lanes are
done.

(b) Live proofs (automatable): deploy the API so live turns emit
`live_agent_graph` rows, then the CUT-11 per-provider confirmed reconnect
traces; CUT-21 named-Codex turn (after owner reconnect) — the named-Claude
leg is now live-proven; CUT-23 live plugin/skill receipt; Android-emulator
authenticated CUT-14 legs + CUT-12 interaction receipts (scriptable after
the one-time emulator GitHub sign-in); #8676 end-to-end journey.

(c) Owner-gated (all queued in `NEEDS_OWNER.md`, pushed): 4 iPhone taps;
one-time Android-emulator GitHub sign-in; Codex account reconnect in
Desktop Settings; VoiceOver/TalkBack QA; Developer ID signing +
clean-machine acceptance; final #8676/CUT-27 acceptance review.
