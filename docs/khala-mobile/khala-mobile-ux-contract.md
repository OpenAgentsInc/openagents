# Khala Mobile UX Behavior Contract

This is the durable home for stated UX expectations for Khala Mobile (the Expo
React Native companion app) — the mobile equivalent of
`docs/khala-code/khala-code-ux-contract.md`, and the mechanism this repo uses
to answer "where is correct mobile behavior defined and how is it tested
against?"

The machine source of truth is the typed registry at
`clients/khala-mobile/src/contracts/ux-contracts.ts`
(schema: `packages/behavior-contracts`, `@openagentsinc/behavior-contracts` —
the SAME shared package the desktop registry uses, not a parallel copy). This
document is the human rendering; the test
`clients/khala-mobile/tests/ux-contracts.test.ts` fails the normal test sweep
if this doc, the registry, or the oracle tests drift apart.

This registry was seeded on 2026-07-05 from a full, honest audit of the app's
current real state (`docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md`).
Every `enforced` entry below cites an oracle that was written and run as part
of that audit pass — nothing here is aspirational. Real, important behavior
that this environment genuinely cannot verify yet (physical-device native
capture, a full React Native component-mount test harness) is recorded
`pending` with an honest blocker ref instead of silently assumed or skipped.

## Rules

Identical to the desktop contract's rules (owner mandate, 2026-07-03/07-04):

- When the owner (or later, a customer) states a mobile UX expectation in any
  session, the receiving agent must land it in this registry in the same
  change: statement verbatim, source recorded, oracle test written or the
  entry marked `pending` with a blocker ref.
- `enforced` requires: at least one oracle, an automated enforcement tier
  (`test-sweep` or `nightly`), and zero blocker refs — the same mechanical
  green-gate discipline as the product-promise registry
  (`docs/promises/registry.md`) and the desktop UX contract.
- Oracles must assert on real behavior (unit-tested pure logic, mounted
  component trees, RPC results, on-device scenarios). Source-string assertions
  are acceptable only as an explicitly labeled stopgap and should carry a
  follow-up — see `khala_mobile.android.stt_module_typed_asyncfunction_signature.v1`
  for the one case that currently uses this allowance.
- Bump `version` (`YYYY-MM-DD.N`) on every registry change and regenerate the
  registry section below with `renderBehaviorContractMarkdown(khalaMobileUxContractRegistry)`.
- Contract deviations found in the wild are strict bugs: file them with the
  contract id in the title.

## Known Structural Gap vs. Desktop (honest, as of 2026-07-05)

Khala Code desktop's registry has ~35 enforced contracts built up over weeks
of dogfooding, with real DOM-mounted oracles (`happy-dom`) exercising actual
UI components. Khala Mobile's registry is a first pass: every enforced
contract here is a **pure-logic unit-test oracle** (auth discovery ordering,
composer intent-builder targeting, security validators, sort/format helpers,
a Kotlin source-string pin). None of them mount and assert on an actual
rendered React Native component tree yet — `react-test-renderer` is a listed
devDependency, but no test currently uses it. That gap is itself recorded as
`khala_mobile.composer.rn_component_mount_coverage.v1` (pending) rather than
silently accepted. Closing it is the top follow-up item for whoever picks this
up next.

## How this runs in the normal sweep

- `bun test` in `clients/khala-mobile` includes `tests/ux-contracts.test.ts`
  (the oracle + coverage + doc-sync tests), so it runs in the package test
  glob and the repo-root `test:khala-mobile` step, which is itself part of the
  root `bun run test` sweep before pushes to `main`.
- The registry validation mirrors the desktop and product-promise transition
  checks (`validateBehaviorContractRegistry`), and the coverage check
  (`checkBehaviorContractCoverage`) proves every enforced `bun-test` oracle
  file exists and references its contract id.
- QA Swarm integration: see
  `docs/khala-mobile/2026-07-05-qa-swarm-mobile-adaptation.md` for how this
  registry's enforced/pending split feeds the mobile QA Swarm's coverage
  frontier and findings ledger, mirroring
  `docs/qa/qa-swarm-khala-code-standing-engagement.md` for desktop.

## Registry

Registry version: `2026-07-05.1` (schema `openagents.behavior_contracts.v1`)

### `khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1` — ENFORCED

- **Surface:** khala-mobile (auth)
- **Stated by:** owner via khala-code-session on 2026-07-04
- **Statement:** IF THERES A DEVICE ON TAILNET THATS AUTHED, USE THAT AUTOMATICALLY - NO LOGIN SCREEN. Before ever showing a manual sign-in screen, the app must look for an already-signed-in desktop Khala Code instance reachable on the same Tailnet and pull working credentials from it.
- **Enforcement tier:** test-sweep
- **Oracle** `tailnet_discovery_concurrent_priority.unit` (bun-test, unit): Probes multiple Tailnet candidate hosts concurrently (not serially) and returns a real credential pair when any host reports a signed-in desktop, so the app never blocks on a per-host timeout multiplied by candidate count. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Oracle** `tailnet_discovery_outcome_priority.unit` (bun-test, unit): A paired outcome always wins over a merely-reachable-but-signed-out host, which always wins over unreachable; ties resolve to the first candidate in the documented host-priority list. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds discovery ordering and priority only. It does not authorize a second auth layer beyond Tailscale's own network ACL (the desktop pairing endpoint trusts reachability on the tailnet), and it does not promise discovery succeeds off-tailnet.

### `khala_mobile.composer.pushtotalk_disabled_when_unavailable.v1` — ENFORCED

- **Surface:** khala-mobile (chat composer)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** Push-to-talk dictation never attempts a native recognition call the availability probe already predicted would fail, and a finished dictation always merges into (never replaces) whatever the user had already typed.
- **Enforcement tier:** test-sweep
- **Oracle** `pushtotalk_pressable_gating.unit` (bun-test, unit): The composer's mic button is only pressable while the phase is idle or recording; denied/unavailable/checking/error phases are never pressable, so a doomed native startRecognitionAsync() call never fires from a stray tap. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Oracle** `dictation_merge_preserves_draft.unit` (bun-test, unit): Merging a finished dictation transcript into the composer draft appends (with a separating space) rather than overwrites existing typed text, and an empty/cancelled transcript is a no-op that never clobbers an in-progress draft. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** This binds only the mic button's own gating logic (whether a tap is allowed to attempt native recognition) and draft-merge semantics. It does not cover whether the underlying native call actually captures audio on a device — see khala_mobile.stt.real_device_capture_proof.v1 for that.

### `khala_mobile.composer.steer_targets_active_turn_lane_not_idle_picker.v1` — ENFORCED

- **Surface:** khala-mobile (chat composer)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** A running turn's provider (Codex vs Claude) is fixed. Steering a follow-up or queuing behind it must always target that turn's own lane, never whatever the idle composer's lane picker happens to show.
- **Enforcement tier:** test-sweep
- **Oracle** `steer_and_queue_use_active_turn_lane.unit` (bun-test, unit): Steering a follow-up while a turn is active builds a runtime.appendUserMessage intent targeting the ACTIVE turn's own lane, and queuing a new turn behind an active one inherits that same lane — never the idle lane picker's current (possibly stale) selection. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the pure intent-builder layer only (the payload a Steer/Queue/Stop tap constructs). It does not cover the composer's own React state wiring that selects which builder to call — see khala_mobile.composer.rn_component_mount_coverage.v1 for that gap.

### `khala_mobile.security.delegation_prompt_rejects_secrets_and_local_paths.v1` — ENFORCED

- **Surface:** khala-mobile (security)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** A coding-delegation prompt built from the mobile app must never carry local paths, Codex auth material, bearer/API tokens, provider secrets, mnemonics/passwords, emails, or high-entropy strings into a typed codex_agent_task request.
- **Enforcement tier:** test-sweep
- **Oracle** `delegation_prompt_blocklist.unit` (bun-test, unit): Rejects a coding-delegation prompt containing a local filesystem path, a .codex/auth.json reference, a Bearer token, an oa_agent_ API key, a named provider secret env var, mnemonic/password-shaped phrases, an email address, or a high-entropy string, and reports the specific blocker ref(s); a public-safe prompt with none of those validates ok. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** This is not a filter applied to ordinary chat messages; it binds only the typed codex_agent_task delegation-prompt path documented in the repo-root Khala -> Pylon -> Codex runbook. It also binds only pattern-detectable secret shapes, not a guarantee that no private material can ever pass validation.

### `khala_mobile.android.stt_module_typed_asyncfunction_signature.v1` — ENFORCED

- **Surface:** khala-mobile (native modules (Android))
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The Android build must not regress the 2026-07-05 Kotlin reified-generic fix that unblocked a total clean-build failure in the push-to-talk STT module.
- **Enforcement tier:** test-sweep
- **Oracle** `stt_asyncfunction_pinned_type.source` (bun-test, unit): The Kotlin STT module's startRecognitionAsync declares an explicit AsyncFunction<Map<String, Any>, String?> signature rather than leaving R to reified-generic inference, which is the exact fix for the 2026-07-05 clean-build failure ('Cannot use Nothing as reified type parameter') caused by the always-throwing shell inferring a Nothing return type. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real build evidence: `bun run build:android:local` -> BUILD SUCCESSFUL, recorded in clients/khala-mobile/README.md and the 2026-07-05 audit doc.
- **Authority boundary:** Source-string assertion, explicitly labeled per the coverage-checker's allowance for stopgap oracles (packages/behavior-contracts docs). Confirms the FIX is present in source; the accompanying real-build evidence (docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md) is what proves it actually compiles clean on Android today. A follow-up should replace this with an automated Gradle-build CI oracle.

### `khala_mobile.fleet.account_rows_sorted_readiness_then_ref.v1` — ENFORCED

- **Surface:** khala-mobile (fleet/settings)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** Fleet account rows in Settings are always ordered by readiness (ready first) rather than raw feed/insertion order, so the most actionable accounts are never buried.
- **Enforcement tier:** test-sweep
- **Oracle** `fleet_account_readiness_sort.unit` (bun-test, unit): Fleet account rows sort ready before cooldown before unavailable before unknown, tie-broken by account ref hash, so a user always sees actionable (ready) accounts first regardless of feed order. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds display sort/format helpers only; does not claim the underlying Khala Sync fleet collection itself is verified live-correct on device (that is a broader claim tracked separately).

### `khala_mobile.updates.ota_manifest_points_at_openagents_updates_only.v1` — ENFORCED

- **Surface:** khala-mobile (updates)
- **Stated by:** owner via khala-code-session on 2026-07-04
- **Statement:** OTA updates for Khala Mobile are served exclusively from the self-hosted OpenAgents Updates server, never Expo's hosted EAS Update service.
- **Enforcement tier:** test-sweep
- **Oracle** `ota_self_hosted_only.unit` (bun-test, unit): The OTA contract's manifest URL resolves to updates.openagents.com (never expo.dev/u.expo.dev), and the forbidden-command list names eas build/submit/update as commands this package must never invoke. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the configured manifest URL and the forbidden-command list only; it does not itself prevent a future contributor from manually invoking an `eas` CLI command out of band.

### `khala_mobile.connectivity.tailnet_health_probe_concurrent_not_serial.v1` — ENFORCED

- **Surface:** khala-mobile (connectivity)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The desktop-connectivity status dot must resolve promptly on both simulator and device, without the wait time growing linearly with the number of candidate Tailnet hosts.
- **Enforcement tier:** test-sweep
- **Oracle** `connectivity_profile_resolution.unit` (bun-test, unit): Resolving Khala Code connectivity against multiple Tailnet candidate hosts returns the first reachable host's profile without waiting a full serial multiple of the per-host timeout, and simulator/device target selection (loopback vs tailnet) matches the caller's isDevice flag. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds probe concurrency/latency shape only; does not itself prove any particular Tailnet host is reachable from any given real device network.

### `khala_mobile.security.api_key_only_via_secure_store.v1` — ENFORCED

- **Surface:** khala-mobile (security)
- **Stated by:** owner via khala-code-session on 2026-07-04
- **Statement:** API keys and bearer material are stored only through the secure-store/keychain adapter, never persisted in SQLite, AsyncStorage, source files, or bundled config.
- **Enforcement tier:** test-sweep
- **Oracle** `api_key_secure_store_round_trip.unit` (bun-test, unit): Saving, loading, and deleting the Khala API key route through expo-secure-store's keychain-backed setItemAsync/getItemAsync/deleteItemAsync with the app's dedicated keychain service, and a blank/whitespace-only key is rejected before any store write. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds this package's own SecureStore call sites (keychain.ts) only. It does not audit every module in the tree for an accidental console.log or crash-report leak, which stays a manual-review responsibility until a dedicated scanner oracle exists.

### `khala_mobile.stt.real_device_capture_proof.v1` — PENDING

- **Surface:** khala-mobile (native modules)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** Push-to-talk dictation actually captures microphone audio and returns a real transcript on a physical iOS device (Speech framework) and a physical Android device (SpeechRecognizer), not just an availability probe.
- **Enforcement tier:** unenforced
- **Verification:** No automated oracle yet. Both native module shells currently report a runtime-pending state and reject on startRecognitionAsync by design (see module source); needs a physical-device manual-check receipt or an on-device XCTest/Espresso-style capture proof before this can move to enforced.
- **Blockers:** `blocker.khala_mobile.needs_physical_ios_device_for_speech_capture`, `blocker.khala_mobile.needs_physical_android_device_for_speechrecognizer_capture`
- **Authority boundary:** Blocked on hardware this environment cannot provide. Do not describe push-to-talk as working end to end until this contract moves to enforced with a real device-capture oracle or dated manual-check receipt.

### `khala_mobile.applefm.real_device_bridge_proof.v1` — PENDING

- **Surface:** khala-mobile (native modules)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The Apple Foundation Models bridge actually returns real on-device model output on a physical iOS device with Apple Intelligence available, not just a readiness/availability probe.
- **Enforcement tier:** unenforced
- **Verification:** No automated oracle yet. The module reports a local-helper-proof blocker on iOS and explicit unavailability on Android by design; needs a physical iOS device plus the local helper referenced in the README before this can move to enforced.
- **Blockers:** `blocker.khala_mobile.needs_physical_ios_device_with_apple_intelligence`, `blocker.khala_mobile.needs_local_fm_helper_proof`
- **Authority boundary:** Blocked on hardware and the local helper referenced in the mobile README's 'Owner-Gated Proof Still Needed' section.

### `khala_mobile.composer.rn_component_mount_coverage.v1` — PENDING

- **Surface:** khala-mobile (chat composer)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** ChatComposer's Steer/Queue picker, Stop button, and idle lane picker actually render the correct visible state and respond to real presses when mounted as a live React Native component tree, not just via their pure intent-builder functions.
- **Enforcement tier:** unenforced
- **Verification:** No automated oracle yet. react-test-renderer is a devDependency but no test in this package currently mounts ChatComposer or any routed screen; needs a component-render test harness (react-test-renderer or an RN Testing Library equivalent wired into `bun test`) before this can move to enforced.
- **Blockers:** `blocker.khala_mobile.no_rn_component_render_harness_in_bun_test`
- **Authority boundary:** This is a test-infrastructure gap, not a product defect: the underlying pure builder functions ARE unit-tested (see the enforced contracts above). The gap is specifically that no test mounts the actual React Native component tree.

### `khala_mobile.platform.launched_app_interaction_smoke.v1` — PENDING

- **Surface:** khala-mobile (app lifecycle)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The built app actually launches and is interactable end to end on a real Android device/emulator and a real iOS device (beyond simulator/local-build success), for at least: sign-in resolves, a thread opens, a message sends, and the composer's lane picker is visible.
- **Enforcement tier:** unenforced
- **Verification:** No automated oracle yet. Current evidence is source-level scaffold, unit tests, local typecheck, iOS simulator build success, Android clean Gradle assemble, and two TestFlight uploads confirmed VALID via the App Store Connect API — none of that is a launched-and-interacted device pass. Needs an owner/device manual-check receipt per platform, or a Maestro/Detox-style scripted device flow, before this can move to enforced.
- **Blockers:** `blocker.khala_mobile.needs_physical_android_device_or_emulator_launch`, `blocker.khala_mobile.needs_ios_testflight_install_and_interact_pass`
- **Authority boundary:** iOS has stronger automated/proof-adjacent evidence today (two independently-confirmed VALID TestFlight uploads) than Android (clean local Gradle assemble only, no launched APK). This contract exists specifically to keep that asymmetry visible rather than implying platform parity.
