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
- Two-sided SEAM contracts (ST-5 #8511): a contract whose id carries a
  `seam` segment (`<area>.seam.<slug>.v<N>`) binds the boundary BETWEEN a
  client artifact and a server artifact and must name both via its `seam`
  field. Its enforced `bun-test` oracle must be an e2e suite (`*.e2e.*`)
  that exercises real code from both sides — a one-sided fake-transport
  unit test never counts; the shared coverage checker in
  `packages/behavior-contracts` rejects it (`seam_oracle_not_e2e`).
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
`khala_mobile.platform.launched_app_interaction_smoke.v1` (now enforced at the launched-app-smoke tier, though real on-device gesture/touch physics stays out of its scope), and other
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

Registry version: `2026-07-07.9` (schema `openagents.behavior_contracts.v1`)

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

### `khala_mobile.auth.stored_credential_revalidated_on_launch.v1` — ENFORCED

- **Surface:** khala-mobile (auth)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** A stored/leftover credential (e.g. from a prior auth model, or a revoked session that Keychain data otherwise carries across a TestFlight build update) must never silently skip the sign-in screen. Every app launch re-validates a stored credential against the server before treating the app as signed in, exactly like a fresh sign-in does; an invalid one is cleared so the user sees the real GitHub sign-in screen. Filed after a real TestFlight build carried forward a stale pre-pivot session and skipped straight to old signed-in UI.
- **Enforcement tier:** test-sweep
- **Oracle** `resolve_verified_stored_credentials.unit` (bun-test, unit): A stored credential that fails server-side validation is cleared and treated as signed-out; a stored credential that validates is trusted unchanged; no stored credential never triggers a validation call. — `clients/khala-mobile/tests/khala-auth-resume-verify-core.test.ts`
- **Verification:** bun test tests/khala-auth-resume-verify-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only whether the app trusts a locally stored credential before showing itself as signed in. It does not change server-side session/token validation itself (Khala Sync's own bootstrap check remains authoritative), and it does not cover the initial sign-in flow (already exercised fresh at sign-in time).

### `khala_mobile.auth.stored_credential_epoch_purged_on_model_change.v1` — ENFORCED

- **Surface:** khala-mobile (auth)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** A credential predating the current auth model is force-cleared on the next launch, regardless of whether it would still authenticate against the server. Filed after a TestFlight build shipping the server-revalidation fix still skipped the GitHub sign-in screen and landed on a stale identity's UI — the leftover token was technically still valid server-side, so revalidation alone let it through.
- **Enforcement tier:** test-sweep
- **Oracle** `khala_auth_store.credential_epoch_purge.unit` (bun-test, unit): A stored ownerUserId/token pair written without a matching current credential-epoch marker (e.g. a leftover Tailnet-pairing or pre-GitHub-OpenAuth write) is unconditionally purged on load and never returned — independent of whether that token would still pass server-side validation. — `clients/khala-mobile/tests/khala-auth-store.test.ts`
- **Verification:** bun test tests/khala-auth-store.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** This binds only the local credential store's own trust decision (whether a stored token is even considered before/independent of server validation). It exists because khala_mobile.auth.stored_credential_revalidated_on_launch.v1's server re-validation was not sufficient on its own: a leftover token from a retired auth model (e.g. the old Tailnet-pairing flow) can still validate successfully server-side, which is exactly the wrong outcome — server validity is not the same as "issued by the current auth model".

### `khala_mobile.sync.must_refetch_never_stuck_loading.v1` — ENFORCED

- **Surface:** khala-mobile (sync)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** A thread-list (or any scope-entities read) that gets stuck in the sync session's must_refetch phase is never shown as an eternal, unexplained loading spinner. It surfaces as a real error with a restart hint, and the hook makes one bounded automatic retry attempt before giving up. Filed after a fresh GitHub sign-in landed on the Khala nav with a permanent 'Loading threads' spinner and no way to tell anything had gone wrong.
- **Enforcement tier:** test-sweep
- **Oracle** `resolve_scope_entities_status.must_refetch.unit` (bun-test, unit): A scope parked in the session's must_refetch phase (bootstrap retries exhausted) always maps to an error state with a clear message, regardless of item count — never silently 'loading' forever. — `clients/khala-mobile/tests/resolve-scope-entities-status.test.ts`
- **Verification:** bun test tests/resolve-scope-entities-status.test.ts tests/use-khala-sync-scope-entities.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only the thread-list/scope-entities read hook's status mapping (what the UI shows for a given sync phase). It does not change the underlying session's bootstrap retry/backoff policy itself, and it does not cover why a scope's bootstrap fails in the first place — only that a failure is never silently indistinguishable from still-loading.

### `khala_mobile.sync.reload_drains_sqlite_runtime_first.v1` — ENFORCED

- **Surface:** khala-mobile (sync)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** An OTA reload never fires while the local Khala Sync SQLite runtime still has an open connection that hasn't been given a chance to close first. Filed after a confirmed, reproducible native crash (EXC_BAD_ACCESS/SIGSEGV inside expo-sqlite's AsyncQueue, three occurrences in a row on build 11, 2026-07-06) that hit right around Updates.reloadAsync() — a known expo-sqlite race between an in-flight database request and the JS context being torn down.
- **Enforcement tier:** test-sweep
- **Oracle** `khala_mobile_sync_runtime_registry.drain_before_reload.unit` (bun-test, unit): The OTA reload path drains the active sync runtime's close() before calling Updates.reloadAsync(); a hung close() is bounded by a timeout rather than blocking the reload forever, and a missing runtime (signed out) is an instant no-op. — `clients/khala-mobile/tests/khala-mobile-sync-runtime-registry.test.ts`
- **Verification:** bun test tests/khala-mobile-sync-runtime-registry.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only the ORDERING of our own reload trigger relative to closing our own sync runtime. It does not (and cannot) fix expo-sqlite's own native concurrency bug (github.com/expo/expo #33754, #38168) — it only avoids one known way to hit it from our own reload path. A close() that hangs is bounded by a timeout so this can never turn into a stuck/unresponsive reload.

### `khala_mobile.sync.stuck_loading_watchdog.v1` — ENFORCED

- **Surface:** khala-mobile (sync)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** Loading threads (or any scope-entities read) never spins forever, even when the underlying sync phase genuinely hangs rather than rejecting or reaching the session's must_refetch give-up phase. Filed after build 13 still landed on a permanent 'Loading threads' spinner despite the earlier must_refetch fix — that fix only covers the session's own bounded-retries-exhausted phase, not a request that never settles at all.
- **Enforcement tier:** test-sweep
- **Oracle** `use_khala_sync_scope_entities.watchdog.unit` (bun-test, unit): A scope stuck in a non-terminal sync phase (e.g. bootstrapping/catching_up) with zero items force-errors with a restart hint after watchdogMs, even though it never rejects and never reaches the session's own must_refetch give-up phase. A scope that resolves to ready before the watchdog fires is never force-errored afterward. — `clients/khala-mobile/tests/use-khala-sync-scope-entities-watchdog.test.ts`
- **Verification:** bun test tests/use-khala-sync-scope-entities-watchdog.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only the client-side status the scope-entities hook reports when nothing else has resolved by the timeout. It does not diagnose or fix WHY a scope hangs (network, server, or session bug) — it only guarantees the user is never left staring at a silent, unexplained spinner forever, regardless of the cause.

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
- **Oracle** `composer_press_send_calls_push_start_turn.unit` (bun-test, unit): Pressing the real Send button's onPress after typing idle text optimistically appends the chat message through the injected appendMessage() exactly once (body/threadId/messageId), then calls push() exactly once with a [runtime.startTurn] control intent whose bodyRef references that message — proving the component's send-dispatch wiring end to end (the message goes through the optimistic overlay path, not the raw control-intent push). — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_press_stop_calls_push_interrupt_turn.unit` (bun-test, unit): Pressing the real Stop button's onPress while a turn is active calls the injected push() exactly once with a [runtime.interruptTurn] mutation. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Oracle** `composer_turn_status_labels_render_per_status.unit` (bun-test, unit): For each real turn-status value (queued, running, waiting_for_input), the mounted component renders the correct human status label and still shows a reachable Stop button. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Verification:** bun test tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting is enabled by the bun test React Native harness in tests/support/rn-test-environment.ts (see that file's header for how react-native itself becomes importable, and which native-bridge-touching leaves are stubbed).
- **Authority boundary:** Binds ChatComposer's own React state/render/effect wiring (button swap, lane-picker visibility, controlled-input value, push() call shape) as proven by a REAL mounted component tree via `tests/support/rn-test-environment.ts`. It does not cover real native rendering, gesture/touch physics, Skia drawing, or Reanimated worklet execution on an actual device/simulator — those stay under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics). The Skia-drawn ArwesButton/BackgroundGradient/ActivityIndicator leaves and react-native-reanimated are test-doubled (documented in tests/chat-composer.test.tsx's header comment) because they have no meaningful non-native equivalent; everything else in the real import graph (react-native core primitives, push-to-talk-core, khala-runtime-compose-core, khala-sync-push-core, swipe-quote-core, theme/tokens) is the real, unmocked module.

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
- **Authority boundary:** Binds RepoPickerScreen's own load/search/select state wiring — including the REAL (unmocked) KhalaListItem and khala-mobile-repos-api client — as proven by a mounted component tree. It does not cover real native scroll/list virtualization (FlatList's real windowing behavior is test-doubled — see tests/support/rn-test-environment.ts's FlatList leaf stub, added for this contract), real touch/gesture physics, or a live GitHub-token-backed server response; those stay under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics).

### `khala_mobile.platform.launched_app_interaction_smoke.v1` — ENFORCED

- **Surface:** khala-mobile (app lifecycle)
- **Stated by:** operator-agent via khala-code-session on 2026-07-05
- **Statement:** The built app launches and is interactable end to end beyond a bare local build: on an iOS Release-configuration simulator, with a seeded public-safe signed-in account, sign-in resolves, the thread list renders, a seeded thread opens, the composer's lane picker is visible, and a typed message sends and appears in the transcript; the same launch and GitHub sign-in handoff are independently proven on a real Android emulator.
- **Enforcement tier:** nightly
- **Oracle** `signed_in_thread_smoke_receipt_pass.unit` (bun-test, unit): The SignedInThreadSmoke Maestro receipt (docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md) exists and records a PASS for the seeded signed-in flow — thread opens, the composer's lane picker (Send with Claude) is visible, and a message sends and renders — on the iPhone 17 Pro iOS 26.5 Release simulator. — `clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts`
- **Verification:** Enforced by the SignedInThreadSmoke Maestro flow (clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml), run on an iPhone 17 Pro iOS 26.5 Release-configuration simulator and auto-signed-in as the seeded public-safe test account: it asserts the thread list, opens the seeded thread, asserts the lane picker (Send with Claude), and sends a message that renders in the transcript. Receipt: docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md (PASS, two green runs; commit cd3122682c). The bun-test oracle clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts asserts that receipt exists and records PASS and runs in the package test glob / repo test:khala-mobile sweep; the Maestro flow itself runs as the opt-in mobile step of the QA nightly matrix (docs/qa/khala-code-nightly-matrix.md, OA_QA_NIGHTLY_INCLUDE_MOBILE=1) given a booted simulator + installed Release build via clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh. Android launch + GitHub sign-in handoff are separately receipted in docs/khala-mobile/2026-07-06-android-emulator-launch-smoke-receipt.md. A real physical-iOS-device signed-in interaction pass remains future hardening tracked outside this contract.
- **Authority boundary:** Enforced at the launched-app-smoke tier: the coverage oracle is a receipt-asserting bun-test, and the heavy proof is the SignedInThreadSmoke Maestro flow run on an iOS Release-configuration simulator (not a device farm). It binds the signed-in iOS-simulator interaction (auto sign-in resolves, thread list renders, seeded thread opens, lane picker visible, message sends and renders) plus the independently-receipted Android emulator launch + GitHub sign-in handoff. It does NOT prove a physical-iOS-device signed-in interaction, real Skia/Reanimated on-device rendering, or gesture/touch physics beyond what Maestro drives on the simulator/emulator — a physical-device pass remains future hardening tracked outside this contract.

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
- **Authority boundary:** Binds only the balance-gate decision function (never block on undetermined/unavailable data, only on a confirmed non-positive balance) and the suggested-task/title-derivation content. It does not prove the full onboarding screen mounts correctly end to end on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics). It also does not claim a live balance check is exercisable today: the balance endpoint itself is still proposed, not built (#8480), so this gate is currently always permissive in practice until that lands.

### `khala_mobile.push.notification_tap_opens_thread.v1` — ENFORCED

- **Surface:** khala-mobile (push notifications)
- **Stated by:** owner via khala-code-session on 2026-07-05
- **Statement:** Push-on-completion is not just a notification that fires (MM-G2, #8486) — tapping it must take the user straight to the thread it's about, reusing the app's own khala://thread/:threadId deep-link scheme.
- **Enforcement tier:** test-sweep
- **Oracle** `notification_tap_opens_thread_deep_link.unit` (bun-test, unit): Extracts the server-emitted khala://thread/<threadId> deep link from a notification's data payload when well-formed, and rejects a missing/non-string/wrong-scheme value — so Linking.openURL is never handed an arbitrary or malformed URL from a push payload. — `clients/khala-mobile/tests/push-notify-deep-link-core.test.ts`
- **Verification:** bun test tests/push-notify-deep-link-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds only the extraction/validation of the deep-link string from a notification's data payload, and that a well-formed one is handed to Linking.openURL — it does not prove the OS actually delivers the notification, that the resulting navigation lands on the exact right screen state (that's the broader on-device navigation claim under khala_mobile.platform.launched_app_interaction_smoke.v1, which is enforced at the launched-app-smoke tier but does not assert notification-tap navigation), or that the server always includes a threadId (MM-G2, #8486, is outside this lane's scope).

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

### `khala_mobile.seam.mobile_session_token_bridge.v1` — PENDING

- **Surface:** khala-mobile (auth)
- **Seam:** client `clients/khala-mobile/src/auth/mobile-openauth.ts` <-> server `apps/openagents.com/workers/api/src/auth/mobile-session.ts`
- **Stated by:** owner via issue on 2026-07-06
- **Statement:** The OpenAuth access token the mobile app signs in with is accepted end to end as the sync bearer: the exact token the mobile OpenAuth flow stores is what the Worker's mobile-session boundary verifies for a cookie-less client's authenticated calls — the two sides can never silently drift on where the token is carried or how it is validated, because that drift is exactly the class of bug that shipped the builds 10-13 WebSocket 401 loop.
- **Enforcement tier:** unenforced
- **Verification:** No two-sided oracle yet: the Worker-side mobile-session.test.ts and the client-side tests/mobile-openauth.test.ts are one-sided suites and are deliberately not acceptable as this seam contract's oracle. Needs an e2e (*.e2e.test.ts) that imports the REAL client token flow and the REAL server verification boundary (or drives a real/staging Worker) and proves a mobile-issued access token authenticates an actual request. Flip to enforced with that ref once it lands; the seam coverage checker in packages/behavior-contracts will then hold it to the e2e requirement.
- **Blockers:** `blocker.khala_mobile.needs_two_sided_mobile_session_bridge_e2e`
- **Authority boundary:** This SEAM contract (ST-5 #8511, two-sided convention from packages/behavior-contracts) binds the boundary where the mobile app's OpenAuth access token becomes the server's accepted sync bearer: the token clients/khala-mobile/src/auth/mobile-openauth.ts obtains and the auth context stores MEETS the Worker's mobile-session verification (apps/openagents.com/workers/api/src/auth/mobile-session.ts). Each side already has its own one-sided suite (tests/mobile-openauth.test.ts; the Worker's mobile-session.test.ts) — by the seam convention neither can ever be this contract's oracle. It does not bind OpenAuth issuer availability, token lifetime policy, or Khala Sync scope membership.

### `khala_mobile.seam.ota_manifest_fingerprint_roundtrip.v1` — PENDING

- **Surface:** khala-mobile (updates)
- **Seam:** client `clients/khala-mobile/app.json` <-> server `apps/oa-updates/src/manifest-resolver.ts`
- **Stated by:** owner via issue on 2026-07-06
- **Statement:** An OTA published for the current runtime fingerprint actually reaches devices built with that fingerprint: the fingerprint the client build embeds and the runtimeVersion the updates server serves manifests for must round-trip, and a dependency change that silently shifts the runtime fingerprint must fail a check instead of stranding shipped builds. Filed after a font dependency silently changed the runtime fingerprint earlier on 2026-07-06, so build-12 devices could no longer receive the OTA while client and server each looked healthy on their own.
- **Enforcement tier:** unenforced
- **Verification:** No two-sided oracle yet. Needs a fingerprint-roundtrip e2e (*.e2e.test.ts): compute the REAL runtime fingerprint from the client package (npx expo-updates fingerprint:generate over clients/khala-mobile, the same computation publish-ota.sh uses), then drive the REAL apps/oa-updates manifest resolver with that value as Expo-Runtime-Version and prove a published update for it resolves — and that a fingerprint drift against the latest published runtimeVersion fails loudly. Flip to enforced with that ref once it lands; the seam coverage checker in packages/behavior-contracts will then hold it to the e2e requirement.
- **Blockers:** `blocker.khala_mobile.needs_ota_fingerprint_roundtrip_e2e`
- **Authority boundary:** This SEAM contract (ST-5 #8511) binds the OTA runtime-fingerprint round trip between the client build (expo-updates runtimeVersion policy "fingerprint" in clients/khala-mobile/app.json, resolved from the native dependency graph at build time) and the updates server's manifest resolution (apps/oa-updates/src/manifest-resolver.ts matching Expo-Runtime-Version). It does not bind manifest signing, asset serving, or the reload path (khala_mobile.sync.reload_drains_sqlite_runtime_first.v1 owns reload ordering). Each side already 'works' alone — publish succeeds and the resolver matches exactly — which is precisely why only a round-trip check can catch a silent fingerprint shift.

### `khala_mobile.thread.new_thread_action_always_reachable.v1` — ENFORCED

- **Surface:** khala-mobile (thread view)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** There must be an obvious one-tap way to start a new thread from the thread view. Filed after the owner's report: "loads but sucks ... also no way to start a new thread." The thread header always shows a New thread action that creates a fresh thread and navigates to it.
- **Enforcement tier:** test-sweep
- **Oracle** `new_thread_button_present_and_calls_handler.unit` (bun-test, unit): The mounted KhalaThreadHeader renders exactly one 'New thread' button; when a handler is provided the button is enabled and pressing it calls the handler exactly once. — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Oracle** `new_thread_button_disabled_without_runtime.unit` (bun-test, unit): With no handler (sync runtime not yet ready) the 'New thread' button still renders — never hidden — but is disabled (no onPress, accessibilityState.disabled), so the affordance can never disappear mid-session. — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Oracle** `thread_screen_wires_new_thread_action.source` (bun-test, unit): The thread-messages screen passes onNewThread to KhalaThreadHeader and its handler creates a fresh thread via runtime.createThread and navigates to it with navigation.replace('ThreadMessages'). — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Verification:** bun test tests/khala-thread-header.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds KhalaThreadHeader's own render/press wiring (proven by a real mounted component tree) plus the thread screen's wiring of the action. It does not prove the full ThreadMessagesScreen mounts end to end, nor that createThread succeeds against a live server — the header owns the affordance; the sync runtime's createThread is exercised separately. When the runtime isn't ready the button is rendered disabled rather than hidden, so the affordance never disappears mid-session.

### `khala_mobile.thread.active_turn_never_traps_user.v1` — ENFORCED

- **Surface:** khala-mobile (thread view)
- **Stated by:** owner via khala-code-session on 2026-07-06
- **Statement:** A queued or running turn must never trap the user. Filed after the owner's report: "just the one message and button shows stop, cant do anything." Stop cancels the turn and returns the composer to an editable state, and the user can always open the drawer menu or start a new thread even with a turn in flight.
- **Enforcement tier:** test-sweep
- **Oracle** `header_escape_hatches_always_render.unit` (bun-test, unit): The mounted KhalaThreadHeader always renders both an 'Open menu' (drawer hamburger) and a 'New thread' button; the header takes no turn state, so neither escape hatch can be hidden while a turn is queued/running/waiting_for_input. — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Oracle** `stop_interrupts_and_composer_reverts_when_idle.source` (bun-test, unit): Stop builds a runtime.interruptTurn intent (a real cancel, not a visual no-op), the composer exposes an editable Send button for the idle state, and the thread screen renders KhalaThreadHeader above the only turn-gated element. — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Verification:** bun test tests/khala-thread-header.test.tsx tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds that the thread view's two ways out (Open menu, New thread) live in a turn-agnostic header that renders unconditionally, and that Stop dispatches a real interrupt while an idle composer is editable. The live idle↔active render transition of the composer itself is owned/proven by khala_mobile.composer.rn_component_mount_coverage.v1 (idle-shows-Send + Stop-calls-interrupt oracles); this contract binds the never-trapped structural guarantee around it. It does not prove the server actually settles an interrupted turn.

### `khala_mobile.thread.header_menu_opens_drawer.v1` — ENFORCED

- **Surface:** khala-mobile (thread view)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The chat/thread header's left button is a hamburger that opens the drawer flyout menu — not a back button (the old back chevron did not work). Filed after the owner's report that the chat header back button didn't work and should open the drawer nav instead.
- **Enforcement tier:** test-sweep
- **Oracle** `menu_button_present_and_calls_handler.unit` (bun-test, unit): The mounted KhalaThreadHeader renders exactly one 'Open menu' hamburger button (and zero 'Back' buttons); pressing it calls onOpenMenu exactly once. — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Oracle** `thread_screen_wires_menu_to_open_drawer.source` (bun-test, unit): The thread-messages screen wires the header's onOpenMenu to open the root Drawer via navigation.getParent()?.openDrawer(), so the flyout menu (nav items + credit balance) opens from the chat view. — `clients/khala-mobile/tests/khala-thread-header.test.tsx`
- **Verification:** bun test tests/khala-thread-header.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds KhalaThreadHeader's left-action render/press wiring (a real mounted component tree) plus the thread screen's source wiring of that action to the root Drawer. It does not prove the Drawer actually animates open on a device (that is React Navigation's own behavior), nor that the thread screen mounts end to end — the header owns the affordance; the navigator wiring is asserted at the source level.

### `khala_mobile.chat.optimistic_message_renders_on_send.v1` — ENFORCED

- **Surface:** khala-mobile (chat composer)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** When a user sends a message it must appear in the transcript immediately (optimistic) and persist. Filed after the owner's report that typing a message and hitting send did nothing — the message vanished from the input and never showed in the list.
- **Enforcement tier:** test-sweep
- **Oracle** `optimistic_append_visible_via_hook.unit` (bun-test, unit): An optimistic chat append (overlay-backed, its server push not yet resolved) is surfaced by useKhalaSyncScopeEntities immediately — proving the hook reads the overlay (confirmed base + pending optimistic), not the confirmed-only store. A store-only read would return an empty thread, the exact 'sending does nothing' bug. — `clients/khala-mobile/tests/use-khala-sync-scope-entities.test.ts`
- **Oracle** `composer_send_uses_optimistic_append.unit` (bun-test, unit): Pressing Send routes the chat message through the optimistic appendMessage() (shows immediately + persists) and sends only the runtime.startTurn control intent via push — so a plain send always produces a locally-visible message. — `clients/khala-mobile/tests/chat-composer.test.tsx`
- **Verification:** bun test tests/use-khala-sync-scope-entities.test.ts tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the local-first read path: an optimistic chat-message append is visible through useKhalaSyncScopeEntities (the transcript's read hook) before the server confirms it, and the composer routes a send through that optimistic append. It does not prove server-side turn dispatch, nor the on-device FlatList render — it binds that the user's own just-sent message exists in the read model the transcript renders from, which is exactly what regressed.

### `khala_mobile.auth.signout_ends_web_session_for_account_switch.v1` — ENFORCED

- **Surface:** khala-mobile (auth)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** Signing out fully ends the web auth session so the next sign-in can choose a different account. Filed after the owner could not switch to a test account: after Sign out, 'Log in with GitHub' silently re-authenticated the same account because the persistent Safari/issuer cookies were reused.
- **Enforcement tier:** test-sweep
- **Oracle** `signout_ephemeral_web_session.source` (bun-test, unit): signInWithGitHub runs the auth prompt with preferEphemeralSession: true (a non-persistent ASWebAuthenticationSession that does not reuse Safari/issuer/GitHub cookies), and signOut revokes the server session (deleteMobileOpenAuthSession) — so re-auth after sign-out presents a fresh GitHub login / account picker instead of silently re-signing-in the previous account. — `clients/khala-mobile/tests/auth-ephemeral-session.test.ts`
- **Verification:** bun test tests/auth-ephemeral-session.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the client-side web-auth session posture: the OAuth prompt runs in an ephemeral (non-cookie-sharing) web session, and sign-out revokes the server session. It does not control the OpenAuth issuer's or GitHub's own server-side session lifetime beyond the revoke call, and it is a source-level assertion of the flow, not a device-driven account-switch e2e.

### `khala_mobile.theme.base_background_dark_navy.v1` — ENFORCED

- **Surface:** khala-mobile (theme)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The app's primary background is a very dark navy blue across every screen — dark and clearly navy, not the old warm brown and not pure black.
- **Enforcement tier:** test-sweep
- **Oracle** `base_background_is_dark_navy.unit` (bun-test, unit): The dark theme's base `background` (and the `neutral200` palette slot it reads) is a very dark navy blue in the #05xx–#07xx range with blue as the dominant channel — not the retired warm brown #191015 and not pure black. — `clients/khala-mobile/tests/theme-colors.test.ts`
- **Verification:** bun test tests/theme-colors.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Binds the app's single base background color (the token every Ignite `Screen` reads) only. It does not repaint per-surface accent/tint colors, the baked hero artwork on the sign-in screen, or any component-local background override.

### `khala_mobile.onboarding.welcome_greeting_uses_github_username.v1` — ENFORCED

- **Surface:** khala-mobile (onboarding)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The onboarding welcome heading greets the signed-in user by their GitHub username (`Welcome, <username>`), falling back to a warm generic greeting only when the username is genuinely unavailable.
- **Enforcement tier:** test-sweep
- **Oracle** `welcome_heading_personalizes_or_falls_back.unit` (bun-test, unit): The welcome heading personalizes to `Welcome, <login>` (trimmed) when a GitHub login is available, and falls back to `Welcome to Khala Code` for a blank/whitespace login (email-provider session, or a Worker deploy predating the session field). — `clients/khala-mobile/tests/onboarding-core.test.ts`
- **Oracle** `github_login_flows_through_session_and_store.unit` (bun-test, unit): The mobile-session client surfaces `githubLogin` when the bridge returns it and omits it otherwise, and a saved credential round-trips `githubLogin` through SecureStore so the greeting survives a relaunch. — `clients/khala-mobile/tests/mobile-openauth.test.ts`
- **Verification:** bun test tests/onboarding-core.test.ts tests/mobile-openauth.test.ts tests/khala-auth-store.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. The server side (githubLogin in the /api/mobile/session response) is covered by apps/openagents.com/workers/api/src/auth/mobile-session.test.ts and needs a monolith redeploy before the greeting personalizes on-device.
- **Authority boundary:** Binds the onboarding welcome heading's text selection only (personalize when a GitHub login is available, fall back to the product name otherwise) and the plumbing that carries the login from the mobile-session response through the auth context. It does not change who the session belongs to or any server-side scope/authority; the greeting is display-only.

### `khala_mobile.onboarding.get_started_cta_fill_on_inner_view.v1` — ENFORCED

- **Surface:** khala-mobile (onboarding)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The onboarding 'Get started' button is a real, obviously-tappable filled CTA (high-contrast cyan pill with a dark bold label), never an invisible no-fill control — the fill lives on an inner View so it paints under the New Architecture (Fabric), matching the login button fix.
- **Enforcement tier:** test-sweep
- **Oracle** `get_started_cta_fill_on_inner_view.unit` (bun-test, unit): The mounted WelcomeStep renders a single cyan (#4fd0ff) filled inner View — button-sized (minHeight 54, borderRadius 12) — wrapping the dark bold `Get started` label, while the Pressable owns only the touch target (accessibilityRole button, onPress) and carries NO backgroundColor of its own. This is the Fabric no-paint fix: a Pressable with a function style does not paint its own background, so the fill must live on a plain View. — `clients/khala-mobile/tests/onboarding-welcome-cta.test.tsx`
- **Verification:** bun test tests/onboarding-welcome-cta.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting uses the bun test React Native harness in tests/support/rn-test-environment.ts (extended here with a ScrollView leaf stub).
- **Authority boundary:** Binds the onboarding 'Get started' CTA's fill placement (fill on an inner plain View, not on the Pressable's function style) and its high-contrast appearance, as proven by a real mounted component tree. It does not cover real native touch/gesture physics on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics).

### `khala_mobile.settings.sign_out_button_fill_on_inner_view.v1` — ENFORCED

- **Surface:** khala-mobile (settings)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The Settings 'Sign out' button is a real, clearly-visible tappable control (a neutral outlined pill with a legible label on the dark-navy background), never invisible near-dark text — the fill lives on an inner View so it paints under the New Architecture (Fabric), matching the login and 'Get started' button fixes. It is styled as a secondary/neutral action, not as loud as a primary CTA.
- **Enforcement tier:** test-sweep
- **Oracle** `sign_out_button_fill_on_inner_view.unit` (bun-test, unit): The mounted AccountSection renders a single filled inner View (backgroundColor #141d33, minHeight 48, borderRadius 10, borderWidth 1) wrapping the light bold `Sign out` label, while the Pressable owns only the touch target (accessibilityRole button, onPress) and carries NO backgroundColor of its own. This is the Fabric no-paint fix: a Pressable with a function style does not paint its own background/border, so the fill must live on a plain View. — `clients/khala-mobile/tests/settings-sign-out-button.test.tsx`
- **Oracle** `sign_out_uses_fabric_safe_inner_view_fill.source` (bun-test, unit): The Settings source builds Sign out with the Pressable+inner-View fill pattern (styles.signOutButton/signOutPressable, an RNText label) inside AccountSection, and no longer uses the invisible Ignite `Button preset="reversed" text="Sign out"`. — `clients/khala-mobile/tests/settings-screen-composition.test.ts`
- **Verification:** bun test tests/settings-sign-out-button.test.tsx tests/settings-screen-composition.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting uses the bun test React Native harness in tests/support/rn-test-environment.ts.
- **Authority boundary:** Binds the Settings 'Sign out' control's fill placement (fill/border on an inner plain View, not on the Pressable's function style) and its visible-but-secondary appearance, as proven by a real mounted component tree. It does not cover real native touch/gesture physics on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics).

### `khala_mobile.settings.delete_account_isolated_at_bottom.v1` — ENFORCED

- **Surface:** khala-mobile (settings)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** Delete account is a destructive action isolated in its own section at the very bottom of Settings (below About & diagnostics), never placed directly under Sign out, so a mistap near Sign out cannot land on irreversible account deletion. It stays clearly marked destructive (red), just out of the way.
- **Enforcement tier:** test-sweep
- **Oracle** `delete_account_isolated_at_bottom.source` (bun-test, unit): The Delete account trigger (and its deleteAccount() call) is no longer inside AccountSection; it lives in a dedicated DeleteAccountSection (red outlined pill + red-bordered $dangerCard) that is rendered as the LAST section in the Settings body, after About & diagnostics — never adjacent to Sign out. — `clients/khala-mobile/tests/settings-screen-composition.test.ts`
- **Verification:** bun test tests/settings-screen-composition.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.
- **Authority boundary:** Source-composition assertion (explicitly labeled stopgap, same allowance as khala_mobile.settings.no_desktop_dependent_sections.v1): proves WHICH section the Delete account trigger is declared and rendered in, and its render order relative to the other sections. It cannot prove on-device layout pixels; the confirmation modal + KHALA_ACCOUNT_DELETION_POLICY_COPY + deleteAccount() behavior it relocates are unchanged and covered where they already were. A real RN-mount oracle for Settings is future work under khala_mobile.platform.launched_app_interaction_smoke.v1 (a full Settings mount needs a Modal harness stub not yet in tests/support/rn-test-environment.ts).

### `khala_mobile.qa.nightly_mobile_row_owned_runner_discipline.v1` — ENFORCED

- **Surface:** khala-mobile (qa)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The Khala Mobile nightly row is defined as owned-runner-only launchd work: iOS Maestro, seeded device monkey, QAM-4 visual capture, named mobile perf budgets, and seam probes all report through typed QA receipts/projection metadata, never hosted CI/EAS; exit remains blocked until seven consecutive passed real nightly receipts exist.
- **Enforcement tier:** test-sweep
- **Oracle** `nightly_mobile_row_owned_runner_discipline.unit` (bun-test, unit): The QAM-5 row definition schedules only owned Tailnet Mac launchd work for iOS Maestro, seeded device monkey, visual capture, named perf budgets, and seam probes; excludes hosted CI/EAS; carries the required perf IDs and khala-sync live-classification seam probe; emits a public-safe local failure digest instead of auto-filing GitHub issues; and refuses to satisfy exit before seven consecutive passed nightly receipts. — `apps/qa-runner/src/mobile-nightly.test.ts`
- **Verification:** bun test src/mobile-nightly.test.ts inside apps/qa-runner plus the Khala mobile UX-contract sweep; real nightly execution remains blocked/pending under #8540 until owned-runner receipts exist.
- **Authority boundary:** Binds the QAM-5 nightly mobile row definition, launchd-owned runner posture, QA Swarm projection node, required named perf budgets, required seam probes, owner-scoped local failure digest, explicit no-auto-GitHub-issue policy, and seven-receipt exit evaluator. It does not prove any real nightly has run, does not claim the seven-consecutive-night exit is satisfied, and keeps QAM-4 Storybook V1 visual capture blocked until #8539 has a proven device-walk receipt.

### `khala_mobile.qa.android_emulator_lane_definition.v1` — ENFORCED

- **Surface:** khala-mobile (qa)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** The Khala Mobile Android lane has a local owned-runner emulator harness and QAM-5 nightly row entries for boot proof, shared Maestro launch/sign-in flow parity, and adb screencap visual-capture parity; #8541 remains open until those flows are green in the nightly row with Android-keyed baseline receipts.
- **Enforcement tier:** test-sweep
- **Oracle** `android_emulator_lane_definition.source` (bun-test, unit): The Android emulator lane exposes a local package script, boots/creates an AVD, waits for sys.boot_completed, installs the debug APK, runs the shared launch/sign-in Maestro flows, records an honest receipt, captures adb exec-out screencaps, and is represented in the QAM-5 nightly row without hosted CI/EAS. — `clients/khala-mobile/tests/maestro-policy.test.ts`
- **Oracle** `android_nightly_row_membership.unit` (bun-test, unit): The QAM-5 nightly row includes Android emulator Maestro and adb screencap nodes, with public-safe scheduled commands and artifact refs. — `apps/qa-runner/src/mobile-nightly.test.ts`
- **Verification:** bun test tests/maestro-policy.test.ts inside clients/khala-mobile plus bun test src/mobile-nightly.test.ts inside apps/qa-runner; real Android emulator execution remains the #8541 exit proof.
- **Authority boundary:** Binds the Android emulator lane definition, runner command surface, QAM-5 nightly row membership, and public-safe receipt/capture paths. It does not prove that the Android emulator flows have passed in the current nightly row, does not claim Android-keyed baselines exist yet, and does not replace the required #8541 green emulator receipts.

### `khala_mobile.qa.planned_feature_eval_suites_fixture_first.v1` — ENFORCED

- **Surface:** khala-mobile (qa)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** Every named post-MVP planned feature suite is authored fixture-first before implementation: Sarah SR-1..3, IAP/minerals, push E2E, Codex connect CX-2, and Agents panel AE-2 all have expected fixture refs, named blocker refs, and honest red/waived cases that future implementation PRs must turn green.
- **Enforcement tier:** test-sweep
- **Oracle** `planned_feature_eval_suites_fixture_first.unit` (bun-test, unit): The QAM-7 catalog includes Sarah SR-1..3, IAP/minerals, push E2E, Codex connect CX-2, and Agents panel AE-2 suites; every suite is red/waived before implementation, every case has an expected fixture ref and blocker ref, and the suite captures discount-pressure, injection-bearing email, fake checkout, StoreKitTest, 3.1.1 copy, simctl push, account_exhausted/rate_limited, and run-status truth oracles. — `clients/khala-mobile/tests/planned-feature-eval-suites.test.ts`
- **Verification:** bun test tests/planned-feature-eval-suites.test.ts inside clients/khala-mobile; runs in the package test glob and the qa:mobile:gate sweep before pushes to main.
- **Authority boundary:** Binds only the fixture-first catalog for planned P1+ features: suite IDs, expected fixture refs, named blockers, honest waived cases, and source references. It does not implement Sarah, IAP/minerals, push device delivery, Codex connect, or the Agents panel; those implementation lanes must turn these cases green rather than authoring acceptance from scratch.

### `khala_mobile.qa.launch_readiness_honesty.v1` — PENDING

- **Surface:** khala-mobile (qa)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** P0.8 launch readiness must stay honest: Khala Mobile is not launch-ready until the owner-gated seed account exists, the full straight-line E2E is receipted on both iOS simulator and Android emulator, and launch promises/copy are reviewed against those receipts.
- **Blockers:** `owner.github_seeded_public_safe_account`, `blocker.ios.full_straight_line_e2e_missing_receipt`, `blocker.android.full_straight_line_e2e_missing_receipt`, `blocker.launch_copy_owner_signoff_missing`
- **Enforcement tier:** unenforced
- **Verification:** bun test tests/launch-readiness.test.ts inside clients/khala-mobile asserts the pending launch-readiness receipt and owner gate stay honest. The contract itself remains pending until #8543's device-only launch truth has real iOS and Android full straight-line receipts.
- **Authority boundary:** Binds the honesty of the P0.8 launch-readiness state only: the seeded-account owner gate, the missing iOS/Android full straight-line E2E receipts, and the copy/pass no-green-without-receipts rule. It does not claim the app is launch-ready and does not enforce the older launched-app smoke as sufficient for #8543.

### `khala_mobile.qa.store_submission_receipts.v1` — PENDING

- **Surface:** khala-mobile (qa)
- **Stated by:** owner via khala-code-session on 2026-07-07
- **Statement:** P0.9 store submission evidence must stay honest: Khala Mobile P0 is not exited until App Store Connect and Play Console both have real submission IDs and in-review states recorded as registry evidence.
- **Blockers:** `owner.app_store_connect_submission_required`, `owner.play_console_submission_required`, `blocker.ios.submission_id_missing`, `blocker.android.submission_id_missing`, `blocker.p08.full_launch_e2e_not_green`
- **Enforcement tier:** unenforced
- **Verification:** bun test tests/store-submissions.test.ts inside clients/khala-mobile asserts the not-submitted receipt and owner-console action list stay explicit. The contract remains pending until real App Store Connect and Play Console submission receipts exist.
- **Authority boundary:** Binds only the P0.9 store-submission evidence ledger: both stores require real owner-console submission IDs and review states before P0 can be called exited. It does not submit a build, upload binaries, imply approval, or replace App Store Connect / Play Console as the authority.
