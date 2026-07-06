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
UI components. Khala Mobile's registry started as a pure-logic-only first
pass (auth discovery ordering, composer intent-builder targeting, security
validators, sort/format helpers, a Kotlin source-string pin) — none of those
oracles mounted an actual rendered React Native component tree.

That specific gap has now been closed for `ChatComposer`:
`khala_mobile.composer.rn_component_mount_coverage.v1` moved from `pending` to
`enforced` on 2026-07-05, backed by real `react-test-renderer` mounts in
`tests/chat-composer.test.tsx` — see `tests/support/rn-test-environment.ts`
for the `bun test` React Native harness that makes mounting a real production
component possible at all under `bun test` (Flow-stripping `react-native` on
the fly, stubbing the handful of native-bridge-touching leaves that have no
meaning without a device/simulator host). Still **not** covered by this: real
native rendering, gesture/touch physics, Skia canvas drawing, or Reanimated
worklet execution on an actual device/simulator — that stays under
`khala_mobile.platform.launched_app_interaction_smoke.v1` (pending), and other
screens/components beyond `ChatComposer` have not yet been given the same
component-mount treatment. Extending this harness to the next screen is the
top follow-up item for whoever picks this up next.

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

Registry version: `2026-07-06.1` (schema `openagents.behavior_contracts.v1`)

### `khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1` — RETIRED

- **Surface:** khala-mobile (auth)
- **Stated by:** owner via khala-code-session on 2026-07-04
- **Statement:** IF THERES A DEVICE ON TAILNET THATS AUTHED, USE THAT AUTOMATICALLY - NO LOGIN SCREEN. Before ever showing a manual sign-in screen, the app must look for an already-signed-in desktop Khala Code instance reachable on the same Tailnet and pull working credentials from it.
- **Enforcement tier:** unenforced
- **Verification:** Retired by docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md §0; the diagnostic pairing core remains unit-tested in clients/khala-mobile/tests/khala-mobile-pairing.test.ts but is not the default auth path.
- **Authority boundary:** Retired by owner-directed mobile-only pivot audit §0 on 2026-07-05. Kept as history only; it no longer binds the MVP launch auth path.

### `khala_mobile.auth.github_sign_in_primary_action.v1` — ENFORCED

- **Surface:** khala-mobile (auth)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Signed-out Khala Mobile users see exactly one primary action: Sign in with GitHub. The app must not probe Tailnet or require a desktop before showing that action.
- **Enforcement tier:** test-sweep
- **Oracle** `github_primary_action_only.unit` (bun-test, unit): A fresh install with no stored/dev credentials enters signed_out and exposes exactly one primary action: GitHub sign-in. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Oracle** `no_tailnet_discovery_status.unit` (bun-test, unit): The mobile-only auth machine has no Tailnet discovery status, so a cold start cannot default into desktop probing before login. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts tests/khala-auth-state-machine.test.ts tests/mobile-openauth.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the signed-out mobile MVP auth surface and auth state machine only. It does not grant repo writeback, spend, payout, or admin authority; the server-side OpenAuth and Khala Sync scope checks remain authoritative.

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
- **Authority boundary:** Binds the pure intent-builder layer only (the payload a Steer/Queue/Stop tap constructs). The composer's own React state wiring that selects which builder to call is now covered separately by khala_mobile.composer.rn_component_mount_coverage.v1.

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

### `khala_mobile.fleet.account_rows_sorted_readiness_then_ref.v1` — RETIRED

- **Surface:** khala-mobile (fleet/settings)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** Fleet account rows in Settings are always ordered by readiness (ready first) rather than raw feed/insertion order, so the most actionable accounts are never buried.
- **Enforcement tier:** unenforced
- **Verification:** RETIRED 2026-07-05 (#8487): Settings no longer renders a Fleet section at all. Historical verification: bun test tests/ux-contracts.test.ts.
- **Authority boundary:** Retired 2026-07-05 by MM-H1 (#8487, Settings rework): the desktop-oriented Fleet section this contract described has been removed from Settings entirely (acceptance criterion: "Settings contains nothing that requires a desktop"), so the statement no longer describes any rendered UI. The underlying sort helper (`sortAccountsByReadinessThenRef`) and its own unit test in `tests/khala-fleet-collections-core.test.ts` remain real, unmodified, and still pass — only this contract's claim about Settings surfacing fleet rows is retired. See `khala_mobile.settings.no_desktop_dependent_sections.v1` for the new Settings-composition contract.

### `khala_mobile.settings.no_desktop_dependent_sections.v1` — ENFORCED

- **Surface:** khala-mobile (settings)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Settings contains nothing that requires a desktop. Credits and model selection are shown honestly as coming soon until their own issues land, never as fabricated live data.
- **Enforcement tier:** test-sweep
- **Oracle** `settings_screen_excludes_fleet_desktop_copy.source` (bun-test, unit): Settings never references the old Fleet section, its entities, or desktop-only copy ("never leaves the desktop"), so a fresh mobile-only install has nothing in Settings that assumes a paired desktop. — `clients/khala-mobile/tests/settings-screen-composition.test.ts`
- **Oracle** `settings_screen_has_mobile_only_sections.source` (bun-test, unit): Settings contains the mobile-only MVP sections: Account, Credits, Models, Notifications, and About/diagnostics. — `clients/khala-mobile/tests/settings-screen-composition.test.ts`
- **Oracle** `settings_screen_stubs_are_honest.source` (bun-test, unit): The Credits and Models sections (stubbed pending #8480/#8484) state what is real (the $10 signup grant; the single default model) and say "coming soon" for the rest, never fabricating a live balance figure or a working model picker. — `clients/khala-mobile/tests/settings-screen-composition.test.ts`
- **Verification:** bun test tests/settings-screen-composition.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Source-string stopgap (explicitly labeled, same allowance as khala_mobile.android.stt_module_typed_asyncfunction_signature.v1): proves the exact shipped source text, not a mounted component tree. A real RN-mount oracle for Settings is future work under khala_mobile.platform.launched_app_interaction_smoke.v1.

### `khala_mobile.updates.ota_manifest_points_at_openagents_updates_only.v1` — ENFORCED

- **Surface:** khala-mobile (updates)
- **Stated by:** owner via khala-code-session on 2026-07-04
- **Statement:** OTA updates for Khala Mobile are served exclusively from the self-hosted OpenAgents Updates server, never Expo's hosted EAS Update service.
- **Enforcement tier:** test-sweep
- **Oracle** `ota_self_hosted_only.unit` (bun-test, unit): The OTA contract's manifest URL resolves to updates.openagents.com (never expo.dev/u.expo.dev), and the forbidden-command list names eas build/submit/update as commands this package must never invoke. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the configured manifest URL and the forbidden-command list only; it does not itself prevent a future contributor from manually invoking an `eas` CLI command out of band.

### `khala_mobile.connectivity.tailnet_health_probe_concurrent_not_serial.v1` — RETIRED

- **Surface:** khala-mobile (connectivity)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The desktop-connectivity status dot must resolve promptly on both simulator and device, without the wait time growing linearly with the number of candidate Tailnet hosts.
- **Enforcement tier:** unenforced
- **Verification:** RETIRED 2026-07-05 (#8489): the status dot is no longer rendered in AppHeader. Historical verification: bun test tests/ux-contracts.test.ts; the underlying probe function itself remains covered by tests/khala-code-connectivity.test.ts.
- **Authority boundary:** Retired 2026-07-05 by MM-H3 (#8489, mobile-only MVP pivot): the desktop-connectivity status dot this contract governed has been removed from `AppHeader` entirely (it reported whether a paired DESKTOP Khala Code instance was reachable — a permanently-red, actively-misleading signal for the post-pivot normal case of a phone-only user with no desktop at all). The underlying probe logic (`khala-code-connectivity-core.ts`) and its unit test are untouched and still pass; only its status as a rendered UI element is retired. Desktop pairing is postponed, not deleted (launch audit §6), so this stays a candidate for a future desktop-pairing return rather than dead code to delete outright.

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

### `khala_mobile.composer.rn_component_mount_coverage.v1` — ENFORCED

- **Surface:** khala-mobile (chat composer)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** ChatComposer's Steer/Queue picker, Stop button, and idle lane picker actually render the correct visible state and respond to real presses when mounted as a live React Native component tree, not just via their pure intent-builder functions.
- **Enforcement tier:** test-sweep
- **Oracle** `composer_mounts_idle_shows_send.unit` (bun-test, unit): The real ChatComposer component mounts without crashing via react-test-renderer, and the idle (no active turn) state shows exactly one Send button and zero Stop buttons. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_active_turn_shows_stop_hides_lane_picker.unit` (bun-test, unit): With an active turn, the composer shows exactly one Stop button and zero Send buttons, and the idle-only lane picker (accessibilityLabel="Provider") does not render. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_typing_updates_input_value.unit` (bun-test, unit): Calling the real TextInput's onChangeText prop updates the controlled input's value on next render, proving the component's own text state wiring, not just the pure text-merge helpers. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_press_send_calls_push_start_turn.unit` (bun-test, unit): Pressing the real Send button's onPress after typing idle text calls the injected push() exactly once with a [chat.appendMessage, runtime.startTurn] mutation pair, proving the component's own send-dispatch wiring end to end. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_press_stop_calls_push_interrupt_turn.unit` (bun-test, unit): Pressing the real Stop button's onPress while a turn is active calls the injected push() exactly once with a [runtime.interruptTurn] mutation. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_turn_status_labels_render_per_status.unit` (bun-test, unit): For each real turn-status value (queued, running, waiting_for_input), the mounted component renders the correct human status label and still shows a reachable Stop button. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Verification:** bun test tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting is enabled by the bun test React Native harness in tests/support/rn-test-environment.ts (see that file's header for how react-native itself becomes importable, and which native-bridge-touching leaves are stubbed).
- **Authority boundary:** Binds ChatComposer's own React state/render/effect wiring (button swap, lane-picker visibility, controlled-input value, push() call shape) as proven by a REAL mounted component tree via `tests/support/rn-test-environment.ts`. It does not cover real native rendering, gesture/touch physics, Skia drawing, or Reanimated worklet execution on an actual device/simulator — those stay under khala_mobile.platform.launched_app_interaction_smoke.v1, which remains pending. The Skia-drawn ArwesButton/BackgroundGradient/ActivityIndicator leaves and react-native-reanimated are test-doubled (documented in tests/chat-composer.test.tsx's header comment) because they have no meaningful non-native equivalent; everything else in the real import graph (react-native core primitives, push-to-talk-core, khala-runtime-compose-core, khala-sync-push-core, swipe-quote-core, theme/tokens) is the real, unmocked module.

### `khala_mobile.repo_picker.rn_component_mount_coverage.v1` — ENFORCED

- **Surface:** khala-mobile (repo picker)
- **Stated by:** operator-agent via khala-code-session on 2026-07-06
- **Statement:** RepoPickerScreen's loading, search-filter, repo-select, and error states actually render and respond correctly when mounted as a live React Native component tree — extending the same real-component-mount coverage ChatComposer proved out to the mobile-only MVP straight line's repo-pick step.
- **Enforcement tier:** test-sweep
- **Oracle** `repo_picker_mounts_loads_renders_repos.unit` (bun-test, unit): The real RepoPickerScreen mounts, calls through the REAL (unmocked) khala-mobile-repos-api client against a scripted globalThis.fetch, and renders both scripted repos via the REAL KhalaListItem. — `clients/khala-mobile/tests/repo-picker-screen.test.tsx`
- **Oracle** `repo_picker_search_filters_real_repo_list.unit` (bun-test, unit): Typing in the real search TextInput filters the rendered rows through the real (unmocked) khala-mobile-repo-search-core functions. — `clients/khala-mobile/tests/repo-picker-screen.test.tsx`
- **Oracle** `repo_picker_select_calls_bind_thread_repo.unit` (bun-test, unit): Pressing a real repo row's onPress calls the sync runtime's real bindThreadRepo() exactly once with the picked repo's owner/name/defaultBranch and the screen's threadId. — `clients/khala-mobile/tests/repo-picker-screen.test.tsx`
- **Oracle** `repo_picker_failed_fetch_renders_error_branch.unit` (bun-test, unit): A failed fetch renders the real client's error-mapped empty state ("Repositories unavailable"), not a silent blank screen. — `clients/khala-mobile/tests/repo-picker-screen.test.tsx`
- **Verification:** bun test tests/repo-picker-screen.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Extends tests/support/rn-test-environment.ts with a FlatList leaf stub (data.map(renderItem) inside a plain View, no virtualization) — the first contract to need it beyond ChatComposer's original primitives.
- **Authority boundary:** Binds RepoPickerScreen's own load/search/select state wiring — including the REAL (unmocked) KhalaListItem and khala-mobile-repos-api client — as proven by a mounted component tree. It does not cover real native scroll/list virtualization (FlatList's real windowing behavior is test-doubled — see tests/support/rn-test-environment.ts's FlatList leaf stub, added for this contract), real touch/gesture physics, or a live GitHub-token-backed server response; those stay under khala_mobile.platform.launched_app_interaction_smoke.v1, which remains pending.

### `khala_mobile.platform.launched_app_interaction_smoke.v1` — PENDING

- **Surface:** khala-mobile (app lifecycle)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The built app actually launches and is interactable end to end on a real Android device/emulator and a real iOS device (beyond simulator/local-build success), for at least: sign-in resolves, a thread opens, a message sends, and the composer's lane picker is visible.
- **Enforcement tier:** unenforced
- **Verification:** Launched-app receipts now exist for both platforms: docs/khala-mobile/2026-07-05-maestro-launched-app-smoke-receipt.md proves LaunchFallback.yaml passed on the iPhone 17 Pro iOS 26.5 simulator; docs/khala-mobile/2026-07-06-android-emulator-launch-smoke-receipt.md proves the same LaunchFallback.yaml flow AND the new LaunchGitHubSignInInteraction.yaml flow (real tap -> real external-browser handoff) passed on a real Android 15 (API 35) emulator against a locally built, locally installed debug APK. The broader contract remains pending on both platforms because no public-safe seeded owner/token/thread precondition was available for SignedInThreadSmoke.yaml.
- **Blockers:** `blocker.khala_mobile.needs_seeded_public_safe_test_github_account`, `blocker.khala_mobile.needs_ios_testflight_install_and_interact_pass`
- **Authority boundary:** As of 2026-07-06 both platforms have a real launched-app receipt (iOS: two independently-confirmed VALID TestFlight uploads plus a simulator Maestro pass; Android: a real emulator boot, install, launch, and Maestro pass). Neither platform yet has a signed-in thread-open/message-send receipt — that remains the shared gap, not an iOS/Android asymmetry.

### `khala_mobile.push.permission_prompt_on_first_task_dispatch.v1` — ENFORCED

- **Surface:** khala-mobile (push notifications)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Permission prompt at the right moment (first task dispatched, not first launch): the OS push-notification permission prompt only ever fires the first time a user dispatches a task (starts a brand-new turn), never on app launch, and never more than once automatically.
- **Enforcement tier:** test-sweep
- **Oracle** `push_permission_prompt_gating.unit` (bun-test, unit): The push permission prompt is only allowed to fire on a `task_dispatched` event that has never prompted before; an `app_launch` event, or any event once `hasEverPrompted` is true, must never trigger it. — `clients/khala-mobile/tests/push-registration-core.test.ts`
- **Oracle** `push_device_id_and_prompt_flag_persistence.unit` (bun-test, unit): The device id persisted for push registration is generated exactly once and reused thereafter, and the has-ever-prompted flag survives sign-out (clearPushDeviceId) since OS permission is a device-level fact, not an account-level one. — `clients/khala-mobile/tests/push-device-store.test.ts`
- **Verification:** bun test tests/push-registration-core.test.ts tests/push-device-store.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only WHEN the OS permission prompt is allowed to fire and how many times the app may trigger it automatically. It does not cover push delivery, payload content (see the server-side `push_payload_safety` oracle in apps/openagents.com/workers/api), or notification preference UI (owned by the mobile Settings lane, #8487).

### `khala_mobile.onboarding.first_task_straight_line.v1` — ENFORCED

- **Surface:** khala-mobile (onboarding)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** A new user reaches a running first task in under a minute of active interaction, with honest states at every fork: sign in with GitHub, land with the $10 grant visible, guided repo pick (or skip), a suggested first task (or a custom one), then watch the turn stream — never blocked by a fork the app can't honestly resolve.
- **Enforcement tier:** test-sweep
- **Oracle** `onboarding_never_blocks_on_undetermined_balance.unit` (bun-test, unit): The onboarding first-task 'Start' action is blocked only when the balance is CONFIRMED zero or negative; when the balance cannot be determined at all (endpoint unavailable, network error), Start is never blocked — the straight line never stalls on missing billing data. — `clients/khala-mobile/tests/onboarding-core.test.ts`
- **Verification:** bun test tests/onboarding-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only the balance-gate decision function (never block on undetermined/unavailable data, only on a confirmed non-positive balance) and the suggested-task/title-derivation content. It does not prove the full onboarding screen mounts correctly end to end on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1, which remains pending. It also does not claim a live balance check is exercisable today: the balance endpoint itself is still proposed, not built (#8480), so this gate is currently always permissive in practice until that lands.

### `khala_mobile.push.notification_tap_opens_thread.v1` — ENFORCED

- **Surface:** khala-mobile (push notifications)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Push-on-completion is not just a notification that fires (MM-G2, #8486) — tapping it must take the user straight to the thread it's about, reusing the app's own khala://thread/:threadId deep-link scheme.
- **Enforcement tier:** test-sweep
- **Oracle** `notification_tap_opens_thread_deep_link.unit` (bun-test, unit): Extracts the server-emitted khala://thread/<threadId> deep link from a notification's data payload when well-formed, and rejects a missing/non-string/wrong-scheme value — so Linking.openURL is never handed an arbitrary or malformed URL from a push payload. — `clients/khala-mobile/tests/push-notify-deep-link-core.test.ts`
- **Verification:** bun test tests/push-notify-deep-link-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only the extraction/validation of the deep-link string from a notification's data payload, and that a well-formed one is handed to Linking.openURL — it does not prove the OS actually delivers the notification, that the resulting navigation lands on the exact right screen state (that's the broader real-device claim under khala_mobile.platform.launched_app_interaction_smoke.v1, pending), or that the server always includes a threadId (MM-G2, #8486, is outside this lane's scope).

### `khala_mobile.credits.ten_dollar_grant_visible_post_signin.v1` — PENDING

- **Surface:** khala-mobile (onboarding)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Land with the $10 grant visible: a new user signing in with GitHub sees their $10 free credit balance on the onboarding welcome step, not just a promise that it was granted.
- **Enforcement tier:** unenforced
- **Verification:** No automated oracle yet — genuinely blocked on the server-side balance endpoint proposed in #8480 (GET /api/mobile/credits/balance does not exist on main). The client wiring (CreditsBalanceChip in onboarding-flow.tsx's WelcomeStep) is already built and will render the real figure automatically once that route lands; today it renders nothing (honest, not fabricated) because the endpoint is unavailable. Move to enforced with a real fetched-balance-renders oracle once #8480's server half ships.
- **Blockers:** `blocker.khala_mobile.needs_credits_balance_endpoint`
- **Authority boundary:** This is an honest 'not yet true' record, not a claim about broken code: the onboarding welcome step already renders a CreditsBalanceChip and would show the real $10 grant the instant the balance endpoint exists. It documents exactly what's missing (the server route) so this contract can move to enforced the moment #8480's proposed contract lands, rather than the expectation living only in conversation.

### `khala_mobile.credits.no_free_execution_path_claims.v1` — ENFORCED

- **Surface:** khala-mobile (credits)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Everything uses credits — there is no free execution path. The app's own copy must never imply otherwise (no "unlimited", "free forever", or "no cost" language), even informally.
- **Enforcement tier:** test-sweep
- **Oracle** `mobile_copy_never_claims_free_execution.source` (bun-test, unit): None of the onboarding, settings, or i18n copy files claim unlimited, free-forever, or no-cost-ever usage — everything-uses-credits means the mobile app's own copy never implies a free execution path exists, even informally. — `clients/khala-mobile/tests/ux-contracts.test.ts`
- **Verification:** bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Source-string stopgap (explicitly labeled, same allowance as khala_mobile.android.stt_module_typed_asyncfunction_signature.v1 and khala_mobile.settings.no_desktop_dependent_sections.v1): scans a fixed, deliberately bounded set of user-facing copy files for a small forbidden-phrase list. It cannot catch every possible free-execution implication in prose, and does not itself enforce that the SERVER actually gates every turn on a credit balance (that invariant is MM-D2/#8479's, still open) — it only binds this app's own copy to never CLAIM a free/unlimited path exists.
