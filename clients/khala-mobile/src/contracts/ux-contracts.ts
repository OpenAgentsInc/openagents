import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "@openagentsinc/behavior-contracts"

/**
 * Khala Mobile (Expo) UX behavior contracts.
 *
 * Mirrors the desktop pattern
 * (`clients/khala-code-desktop/src/contracts/ux-contracts.ts`): every entry
 * records the stated expectation verbatim, who stated it and where, and the
 * oracle tests that enforce it in the normal test sweep. The paired coverage
 * test in `tests/ux-contracts.test.ts` fails the sweep if an enforced
 * contract loses its oracle, so stated behavior cannot silently drift.
 *
 * This registry is seeded from the honest 2026-07-05 mobile audit
 * (`docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md`), not from
 * aspiration. Every `enforced` entry below cites an oracle that was run and
 * passed as part of writing this file. Anything real but not yet automatable
 * in this environment (physical-device native capture, full RN component
 * mounts) is recorded `pending` with an honest blocker ref rather than
 * silently assumed.
 *
 * Human rendering: docs/khala-mobile/khala-mobile-ux-contract.md (kept in
 * sync by the same test file).
 */
export const KHALA_MOBILE_UX_CONTRACT_DOC_PATH =
  "docs/khala-mobile/khala-mobile-ux-contract.md"

export const khalaMobileUxContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    {
      authorityBoundary:
        "Retired by owner-directed mobile-only pivot audit §0 on 2026-07-05. Kept as history only; it no longer binds the MVP launch auth path.",
      blockerRefs: [],
      contractId: "khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/src/auth/khala-mobile-pairing-core.ts",
        "clients/khala-mobile/src/auth/khala-mobile-pairing.ts",
        "clients/khala-mobile/tests/khala-mobile-pairing.test.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [],
      productArea: "auth",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "retired",
      statement:
        "IF THERES A DEVICE ON TAILNET THATS AUTHED, USE THAT AUTOMATICALLY - NO LOGIN SCREEN. Before ever showing a manual sign-in screen, the app must look for an already-signed-in desktop Khala Code instance reachable on the same Tailnet and pull working credentials from it.",
      surface: "khala-mobile",
      verification:
        "Retired by docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md §0; the diagnostic pairing core remains unit-tested in clients/khala-mobile/tests/khala-mobile-pairing.test.ts but is not the default auth path.",
    },
    {
      authorityBoundary:
        "Binds the signed-out mobile MVP auth surface and auth state machine only. It does not grant repo writeback, spend, payout, or admin authority; the server-side OpenAuth and Khala Sync scope checks remain authoritative.",
      blockerRefs: [],
      contractId: "khala_mobile.auth.github_sign_in_primary_action.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/auth/khala-auth-context.tsx",
        "clients/khala-mobile/src/auth/khala-auth-state-machine.ts",
        "clients/khala-mobile/src/auth/mobile-openauth.ts",
        "clients/khala-mobile/src/components/sign-in-screen.tsx",
        "clients/khala-mobile/tests/khala-auth-state-machine.test.ts",
        "clients/khala-mobile/tests/mobile-openauth.test.ts",
        "clients/khala-mobile/tests/ux-contracts.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "A fresh install with no stored/dev credentials enters signed_out and exposes exactly one primary action: GitHub sign-in.",
          id: "github_primary_action_only.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
        {
          description:
            "The mobile-only auth machine has no Tailnet discovery status, so a cold start cannot default into desktop probing before login.",
          id: "no_tailnet_discovery_status.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "auth",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Signed-out Khala Mobile users see exactly one primary action: Sign in with GitHub. The app must not probe Tailnet or require a desktop before showing that action.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts tests/khala-auth-state-machine.test.ts tests/mobile-openauth.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This binds only the mic button's own gating logic (whether a tap is allowed to attempt native recognition) and draft-merge semantics. It does not cover whether the underlying native call actually captures audio on a device — see khala_mobile.stt.real_device_capture_proof.v1 for that.",
      blockerRefs: [],
      contractId: "khala_mobile.composer.pushtotalk_disabled_when_unavailable.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/native/push-to-talk-core.ts",
        "clients/khala-mobile/src/native/use-push-to-talk.ts",
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/tests/push-to-talk-core.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The composer's mic button is only pressable while the phase is idle or recording; denied/unavailable/checking/error phases are never pressable, so a doomed native startRecognitionAsync() call never fires from a stray tap.",
          id: "pushtotalk_pressable_gating.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
        {
          description:
            "Merging a finished dictation transcript into the composer draft appends (with a separating space) rather than overwrites existing typed text, and an empty/cancelled transcript is a no-op that never clobbers an in-progress draft.",
          id: "dictation_merge_preserves_draft.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Push-to-talk dictation never attempts a native recognition call the availability probe already predicted would fail, and a finished dictation always merges into (never replaces) whatever the user had already typed.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the pure intent-builder layer only (the payload a Steer/Queue/Stop tap constructs). The composer's own React state wiring that selects which builder to call is now covered separately by khala_mobile.composer.rn_component_mount_coverage.v1.",
      blockerRefs: [],
      contractId: "khala_mobile.composer.steer_targets_active_turn_lane_not_idle_picker.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/khala-runtime-compose-core.ts",
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/tests/khala-runtime-compose-core.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Steering a follow-up while a turn is active builds a runtime.appendUserMessage intent targeting the ACTIVE turn's own lane, and queuing a new turn behind an active one inherits that same lane — never the idle lane picker's current (possibly stale) selection.",
          id: "steer_and_queue_use_active_turn_lane.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "A running turn's provider (Codex vs Claude) is fixed. Steering a follow-up or queuing behind it must always target that turn's own lane, never whatever the idle composer's lane picker happens to show.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This is not a filter applied to ordinary chat messages; it binds only the typed codex_agent_task delegation-prompt path documented in the repo-root Khala -> Pylon -> Codex runbook. It also binds only pattern-detectable secret shapes, not a guarantee that no private material can ever pass validation.",
      blockerRefs: [],
      contractId: "khala_mobile.security.delegation_prompt_rejects_secrets_and_local_paths.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/security/delegation-prompt.ts",
        "clients/khala-mobile/src/sync/khala-cross-agent-handoff-core.ts",
        "clients/khala-mobile/tests/delegation-prompt.test.ts",
        "clients/khala-mobile/AGENTS.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Rejects a coding-delegation prompt containing a local filesystem path, a .codex/auth.json reference, a Bearer token, an oa_agent_ API key, a named provider secret env var, mnemonic/password-shaped phrases, an email address, or a high-entropy string, and reports the specific blocker ref(s); a public-safe prompt with none of those validates ok.",
          id: "delegation_prompt_blocklist.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "security",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "A coding-delegation prompt built from the mobile app must never carry local paths, Codex auth material, bearer/API tokens, provider secrets, mnemonics/passwords, emails, or high-entropy strings into a typed codex_agent_task request.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Source-string assertion, explicitly labeled per the coverage-checker's allowance for stopgap oracles (packages/behavior-contracts docs). Confirms the FIX is present in source; the accompanying real-build evidence (docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md) is what proves it actually compiles clean on Android today. A follow-up should replace this with an automated Gradle-build CI oracle.",
      blockerRefs: [],
      contractId: "khala_mobile.android.stt_module_typed_asyncfunction_signature.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/modules/khala-push-to-talk-stt/android/src/main/java/com/openagents/khalaptt/KhalaPushToTalkSttModule.kt",
        "clients/khala-mobile/README.md",
        "docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The Kotlin STT module's startRecognitionAsync declares an explicit AsyncFunction<Map<String, Any>, String?> signature rather than leaving R to reified-generic inference, which is the exact fix for the 2026-07-05 clean-build failure ('Cannot use Nothing as reified type parameter') caused by the always-throwing shell inferring a Nothing return type.",
          id: "stt_asyncfunction_pinned_type.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "native modules (Android)",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "The Android build must not regress the 2026-07-05 Kotlin reified-generic fix that unblocked a total clean-build failure in the push-to-talk STT module.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real build evidence: `bun run build:android:local` -> BUILD SUCCESSFUL, recorded in clients/khala-mobile/README.md and the 2026-07-05 audit doc.",
    },
    {
      authorityBoundary:
        "Retired 2026-07-05 by MM-H1 (#8487, Settings rework): the desktop-oriented Fleet section this contract described has been removed from Settings entirely (acceptance criterion: \"Settings contains nothing that requires a desktop\"), so the statement no longer describes any rendered UI. The underlying sort helper (`sortAccountsByReadinessThenRef`) and its own unit test in `tests/khala-fleet-collections-core.test.ts` remain real, unmodified, and still pass — only this contract's claim about Settings surfacing fleet rows is retired. See `khala_mobile.settings.no_desktop_dependent_sections.v1` for the new Settings-composition contract.",
      blockerRefs: [],
      contractId: "khala_mobile.fleet.account_rows_sorted_readiness_then_ref.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/khala-fleet-collections-core.ts",
        "clients/khala-mobile/tests/khala-fleet-collections-core.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [],
      productArea: "fleet/settings",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "retired",
      statement:
        "Fleet account rows in Settings are always ordered by readiness (ready first) rather than raw feed/insertion order, so the most actionable accounts are never buried.",
      surface: "khala-mobile",
      verification:
        "RETIRED 2026-07-05 (#8487): Settings no longer renders a Fleet section at all. Historical verification: bun test tests/ux-contracts.test.ts.",
    },
    {
      authorityBoundary:
        "Source-string stopgap (explicitly labeled, same allowance as khala_mobile.android.stt_module_typed_asyncfunction_signature.v1): proves the exact shipped source text, not a mounted component tree. A real RN-mount oracle for Settings is future work under khala_mobile.platform.launched_app_interaction_smoke.v1.",
      blockerRefs: [],
      contractId: "khala_mobile.settings.no_desktop_dependent_sections.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/settings-screen.tsx",
        "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Settings never references the old Fleet section, its entities, or desktop-only copy (\"never leaves the desktop\"), so a fresh mobile-only install has nothing in Settings that assumes a paired desktop.",
          id: "settings_screen_excludes_fleet_desktop_copy.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        },
        {
          description:
            "Settings contains the mobile-only MVP sections: Account, Credits, Models, Notifications, and About/diagnostics.",
          id: "settings_screen_has_mobile_only_sections.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        },
        {
          description:
            "The Credits and Models sections (stubbed pending #8480/#8484) state what is real (the $10 signup grant; the single default model) and say \"coming soon\" for the rest, never fabricating a live balance figure or a working model picker.",
          id: "settings_screen_stubs_are_honest.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        },
      ],
      productArea: "settings",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Settings contains nothing that requires a desktop. Credits and model selection are shown honestly as coming soon until their own issues land, never as fabricated live data.",
      surface: "khala-mobile",
      verification:
        "bun test tests/settings-screen-composition.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the configured manifest URL and the forbidden-command list only; it does not itself prevent a future contributor from manually invoking an `eas` CLI command out of band.",
      blockerRefs: [],
      contractId: "khala_mobile.updates.ota_manifest_points_at_openagents_updates_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/config/updates.ts",
        "clients/khala-mobile/app.json",
        "clients/khala-mobile/tests/ota-policy.test.ts",
        "clients/khala-mobile/AGENTS.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The OTA contract's manifest URL resolves to updates.openagents.com (never expo.dev/u.expo.dev), and the forbidden-command list names eas build/submit/update as commands this package must never invoke.",
          id: "ota_self_hosted_only.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "updates",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "OTA updates for Khala Mobile are served exclusively from the self-hosted OpenAgents Updates server, never Expo's hosted EAS Update service.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Retired 2026-07-05 by MM-H3 (#8489, mobile-only MVP pivot): the desktop-connectivity status dot this contract governed has been removed from `AppHeader` entirely (it reported whether a paired DESKTOP Khala Code instance was reachable — a permanently-red, actively-misleading signal for the post-pivot normal case of a phone-only user with no desktop at all). The underlying probe logic (`khala-code-connectivity-core.ts`) and its unit test are untouched and still pass; only its status as a rendered UI element is retired. Desktop pairing is postponed, not deleted (launch audit §6), so this stays a candidate for a future desktop-pairing return rather than dead code to delete outright.",
      blockerRefs: [],
      contractId: "khala_mobile.connectivity.tailnet_health_probe_concurrent_not_serial.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/src/status/khala-code-connectivity-core.ts",
        "clients/khala-mobile/src/status/khala-code-connectivity.ts",
        "clients/khala-mobile/src/components/app-header.tsx",
        "clients/khala-mobile/tests/khala-code-connectivity.test.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [],
      productArea: "connectivity",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "retired",
      statement:
        "The desktop-connectivity status dot must resolve promptly on both simulator and device, without the wait time growing linearly with the number of candidate Tailnet hosts.",
      surface: "khala-mobile",
      verification:
        "RETIRED 2026-07-05 (#8489): the status dot is no longer rendered in AppHeader. Historical verification: bun test tests/ux-contracts.test.ts; the underlying probe function itself remains covered by tests/khala-code-connectivity.test.ts.",
    },
    {
      authorityBoundary:
        "Binds this package's own SecureStore call sites (keychain.ts) only. It does not audit every module in the tree for an accidental console.log or crash-report leak, which stays a manual-review responsibility until a dedicated scanner oracle exists.",
      blockerRefs: [],
      contractId: "khala_mobile.security.api_key_only_via_secure_store.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/security/keychain.ts",
        "clients/khala-mobile/tests/keychain.test.ts",
        "clients/khala-mobile/AGENTS.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Saving, loading, and deleting the Khala API key route through expo-secure-store's keychain-backed setItemAsync/getItemAsync/deleteItemAsync with the app's dedicated keychain service, and a blank/whitespace-only key is rejected before any store write.",
          id: "api_key_secure_store_round_trip.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "security",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "API keys and bearer material are stored only through the secure-store/keychain adapter, never persisted in SQLite, AsyncStorage, source files, or bundled config.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Blocked on hardware this environment cannot provide. Do not describe push-to-talk as working end to end until this contract moves to enforced with a real device-capture oracle or dated manual-check receipt.",
      blockerRefs: [
        "blocker.khala_mobile.needs_physical_ios_device_for_speech_capture",
        "blocker.khala_mobile.needs_physical_android_device_for_speechrecognizer_capture",
      ],
      contractId: "khala_mobile.stt.real_device_capture_proof.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/modules/khala-push-to-talk-stt/src/index.ts",
        "clients/khala-mobile/README.md",
      ],
      oracles: [],
      productArea: "native modules",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "pending",
      statement:
        "Push-to-talk dictation actually captures microphone audio and returns a real transcript on a physical iOS device (Speech framework) and a physical Android device (SpeechRecognizer), not just an availability probe.",
      surface: "khala-mobile",
      verification:
        "No automated oracle yet. Both native module shells currently report a runtime-pending state and reject on startRecognitionAsync by design (see module source); needs a physical-device manual-check receipt or an on-device XCTest/Espresso-style capture proof before this can move to enforced.",
    },
    {
      authorityBoundary:
        "Blocked on hardware and the local helper referenced in the mobile README's 'Owner-Gated Proof Still Needed' section.",
      blockerRefs: [
        "blocker.khala_mobile.needs_physical_ios_device_with_apple_intelligence",
        "blocker.khala_mobile.needs_local_fm_helper_proof",
      ],
      contractId: "khala_mobile.applefm.real_device_bridge_proof.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/modules/khala-apple-foundation-models/src/index.ts",
        "clients/khala-mobile/README.md",
      ],
      oracles: [],
      productArea: "native modules",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "pending",
      statement:
        "The Apple Foundation Models bridge actually returns real on-device model output on a physical iOS device with Apple Intelligence available, not just a readiness/availability probe.",
      surface: "khala-mobile",
      verification:
        "No automated oracle yet. The module reports a local-helper-proof blocker on iOS and explicit unavailability on Android by design; needs a physical iOS device plus the local helper referenced in the README before this can move to enforced.",
    },
    {
      authorityBoundary:
        "Binds ChatComposer's own React state/render/effect wiring (button swap, lane-picker visibility, controlled-input value, push() call shape) as proven by a REAL mounted component tree via `tests/support/rn-test-environment.ts`. It does not cover real native rendering, gesture/touch physics, Skia drawing, or Reanimated worklet execution on an actual device/simulator — those stay under khala_mobile.platform.launched_app_interaction_smoke.v1, which remains pending. The Skia-drawn ArwesButton/BackgroundGradient/ActivityIndicator leaves and react-native-reanimated are test-doubled (documented in tests/chat-composer.test.tsx's header comment) because they have no meaningful non-native equivalent; everything else in the real import graph (react-native core primitives, push-to-talk-core, khala-runtime-compose-core, khala-sync-push-core, swipe-quote-core, theme/tokens) is the real, unmocked module.",
      blockerRefs: [],
      contractId: "khala_mobile.composer.rn_component_mount_coverage.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/tests/chat-composer.test.tsx",
        "clients/khala-mobile/tests/support/rn-test-environment.ts",
        "clients/khala-mobile/package.json",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The real ChatComposer component mounts without crashing via react-test-renderer, and the idle (no active turn) state shows exactly one Send button and zero Stop buttons.",
          id: "composer_mounts_idle_shows_send.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
        {
          description:
            "With an active turn, the composer shows exactly one Stop button and zero Send buttons, and the idle-only lane picker (accessibilityLabel=\"Provider\") does not render.",
          id: "composer_active_turn_shows_stop_hides_lane_picker.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
        {
          description:
            "Calling the real TextInput's onChangeText prop updates the controlled input's value on next render, proving the component's own text state wiring, not just the pure text-merge helpers.",
          id: "composer_typing_updates_input_value.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
        {
          description:
            "Pressing the real Send button's onPress after typing idle text calls the injected push() exactly once with a [chat.appendMessage, runtime.startTurn] mutation pair, proving the component's own send-dispatch wiring end to end.",
          id: "composer_press_send_calls_push_start_turn.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
        {
          description:
            "Pressing the real Stop button's onPress while a turn is active calls the injected push() exactly once with a [runtime.interruptTurn] mutation.",
          id: "composer_press_stop_calls_push_interrupt_turn.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
        {
          description:
            "For each real turn-status value (queued, running, waiting_for_input), the mounted component renders the correct human status label and still shows a reachable Stop button.",
          id: "composer_turn_status_labels_render_per_status.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "ChatComposer's Steer/Queue picker, Stop button, and idle lane picker actually render the correct visible state and respond to real presses when mounted as a live React Native component tree, not just via their pure intent-builder functions.",
      surface: "khala-mobile",
      verification:
        "bun test tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting is enabled by the bun test React Native harness in tests/support/rn-test-environment.ts (see that file's header for how react-native itself becomes importable, and which native-bridge-touching leaves are stubbed).",
    },
    {
      authorityBoundary:
        "iOS has stronger automated/proof-adjacent evidence today (two independently-confirmed VALID TestFlight uploads) than Android (clean local Gradle assemble only, no launched APK). This contract exists specifically to keep that asymmetry visible rather than implying platform parity.",
      blockerRefs: [
        "blocker.khala_mobile.needs_physical_android_device_or_emulator_launch",
        "blocker.khala_mobile.needs_ios_testflight_install_and_interact_pass",
      ],
      contractId: "khala_mobile.platform.launched_app_interaction_smoke.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/.maestro/shared/_OnFlowStart.yaml",
        "clients/khala-mobile/.maestro/flows/LaunchFallback.yaml",
        "clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml",
        "clients/khala-mobile/README.md",
        "clients/khala-mobile/tests/maestro-policy.test.ts",
        "docs/khala-mobile/2026-07-05-maestro-launched-app-smoke-receipt.md",
        "docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md",
      ],
      oracles: [],
      productArea: "app lifecycle",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "pending",
      statement:
        "The built app actually launches and is interactable end to end on a real Android device/emulator and a real iOS device (beyond simulator/local-build success), for at least: sign-in resolves, a thread opens, a message sends, and the composer's lane picker is visible.",
      surface: "khala-mobile",
      verification:
        "Partial launched-app receipt recorded: docs/khala-mobile/2026-07-05-maestro-launched-app-smoke-receipt.md proves LaunchFallback.yaml passed on the iPhone 17 Pro iOS 26.5 simulator for app id com.openagents.khala.mobile, app version 0.1.0, iOS build 6, with local Metro serving the debug build. The broader contract remains pending because no public-safe seeded owner/token/thread precondition was available for SignedInThreadSmoke.yaml, and Android launched APK coverage is still unrecorded.",
    },
    {
      authorityBoundary:
        "Binds only WHEN the OS permission prompt is allowed to fire and how many times the app may trigger it automatically. It does not cover push delivery, payload content (see the server-side `push_payload_safety` oracle in apps/openagents.com/workers/api), or notification preference UI (owned by the mobile Settings lane, #8487).",
      blockerRefs: [],
      contractId: "khala_mobile.push.permission_prompt_on_first_task_dispatch.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/push/push-registration-core.ts",
        "clients/khala-mobile/src/push/push-device-store.ts",
        "clients/khala-mobile/src/push/push-notifications-client.ts",
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/tests/push-registration-core.test.ts",
        "clients/khala-mobile/tests/push-device-store.test.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The push permission prompt is only allowed to fire on a `task_dispatched` event that has never prompted before; an `app_launch` event, or any event once `hasEverPrompted` is true, must never trigger it.",
          id: "push_permission_prompt_gating.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/push-registration-core.test.ts",
        },
        {
          description:
            "The device id persisted for push registration is generated exactly once and reused thereafter, and the has-ever-prompted flag survives sign-out (clearPushDeviceId) since OS permission is a device-level fact, not an account-level one.",
          id: "push_device_id_and_prompt_flag_persistence.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/push-device-store.test.ts",
        },
      ],
      productArea: "push notifications",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Permission prompt at the right moment (first task dispatched, not first launch): the OS push-notification permission prompt only ever fires the first time a user dispatches a task (starts a brand-new turn), never on app launch, and never more than once automatically.",
      surface: "khala-mobile",
      verification:
        "bun test tests/push-registration-core.test.ts tests/push-device-store.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds only the balance-gate decision function (never block on undetermined/unavailable data, only on a confirmed non-positive balance) and the suggested-task/title-derivation content. It does not prove the full onboarding screen mounts correctly end to end on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1, which remains pending. It also does not claim a live balance check is exercisable today: the balance endpoint itself is still proposed, not built (#8480), so this gate is currently always permissive in practice until that lands.",
      blockerRefs: [],
      contractId: "khala_mobile.onboarding.first_task_straight_line.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/onboarding-core.ts",
        "clients/khala-mobile/src/screens/onboarding-flow.tsx",
        "clients/khala-mobile/src/screens/thread-list-screen.tsx",
        "clients/khala-mobile/tests/onboarding-core.test.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The onboarding first-task 'Start' action is blocked only when the balance is CONFIRMED zero or negative; when the balance cannot be determined at all (endpoint unavailable, network error), Start is never blocked — the straight line never stalls on missing billing data.",
          id: "onboarding_never_blocks_on_undetermined_balance.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/onboarding-core.test.ts",
        },
      ],
      productArea: "onboarding",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "A new user reaches a running first task in under a minute of active interaction, with honest states at every fork: sign in with GitHub, land with the $10 grant visible, guided repo pick (or skip), a suggested first task (or a custom one), then watch the turn stream — never blocked by a fork the app can't honestly resolve.",
      surface: "khala-mobile",
      verification:
        "bun test tests/onboarding-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds only the extraction/validation of the deep-link string from a notification's data payload, and that a well-formed one is handed to Linking.openURL — it does not prove the OS actually delivers the notification, that the resulting navigation lands on the exact right screen state (that's the broader real-device claim under khala_mobile.platform.launched_app_interaction_smoke.v1, pending), or that the server always includes a threadId (MM-G2, #8486, is outside this lane's scope).",
      blockerRefs: [],
      contractId: "khala_mobile.push.notification_tap_opens_thread.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/push/push-notify-deep-link-core.ts",
        "clients/khala-mobile/src/push/use-push-notification-deep-link.ts",
        "clients/khala-mobile/src/navigators/AppNavigator.tsx",
        "clients/khala-mobile/tests/push-notify-deep-link-core.test.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Extracts the server-emitted khala://thread/<threadId> deep link from a notification's data payload when well-formed, and rejects a missing/non-string/wrong-scheme value — so Linking.openURL is never handed an arbitrary or malformed URL from a push payload.",
          id: "notification_tap_opens_thread_deep_link.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/push-notify-deep-link-core.test.ts",
        },
      ],
      productArea: "push notifications",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Push-on-completion is not just a notification that fires (MM-G2, #8486) — tapping it must take the user straight to the thread it's about, reusing the app's own khala://thread/:threadId deep-link scheme.",
      surface: "khala-mobile",
      verification:
        "bun test tests/push-notify-deep-link-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This is an honest 'not yet true' record, not a claim about broken code: the onboarding welcome step already renders a CreditsBalanceChip and would show the real $10 grant the instant the balance endpoint exists. It documents exactly what's missing (the server route) so this contract can move to enforced the moment #8480's proposed contract lands, rather than the expectation living only in conversation.",
      blockerRefs: ["blocker.khala_mobile.needs_credits_balance_endpoint"],
      contractId: "khala_mobile.credits.ten_dollar_grant_visible_post_signin.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/onboarding-flow.tsx",
        "clients/khala-mobile/src/components/credits-balance-chip.tsx",
        "clients/khala-mobile/src/sync/khala-mobile-credits-api.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [],
      productArea: "onboarding",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "pending",
      statement:
        "Land with the $10 grant visible: a new user signing in with GitHub sees their $10 free credit balance on the onboarding welcome step, not just a promise that it was granted.",
      surface: "khala-mobile",
      verification:
        "No automated oracle yet — genuinely blocked on the server-side balance endpoint proposed in #8480 (GET /api/mobile/credits/balance does not exist on main). The client wiring (CreditsBalanceChip in onboarding-flow.tsx's WelcomeStep) is already built and will render the real figure automatically once that route lands; today it renders nothing (honest, not fabricated) because the endpoint is unavailable. Move to enforced with a real fetched-balance-renders oracle once #8480's server half ships.",
    },
    {
      authorityBoundary:
        "Source-string stopgap (explicitly labeled, same allowance as khala_mobile.android.stt_module_typed_asyncfunction_signature.v1 and khala_mobile.settings.no_desktop_dependent_sections.v1): scans a fixed, deliberately bounded set of user-facing copy files for a small forbidden-phrase list. It cannot catch every possible free-execution implication in prose, and does not itself enforce that the SERVER actually gates every turn on a credit balance (that invariant is MM-D2/#8479's, still open) — it only binds this app's own copy to never CLAIM a free/unlimited path exists.",
      blockerRefs: [],
      contractId: "khala_mobile.credits.no_free_execution_path_claims.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/i18n/copy.ts",
        "clients/khala-mobile/src/screens/onboarding-flow.tsx",
        "clients/khala-mobile/src/screens/onboarding-core.ts",
        "clients/khala-mobile/src/screens/settings-screen.tsx",
        "clients/khala-mobile/tests/ux-contracts.test.ts",
        "docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "None of the onboarding, settings, or i18n copy files claim unlimited, free-forever, or no-cost-ever usage — everything-uses-credits means the mobile app's own copy never implies a free execution path exists, even informally.",
          id: "mobile_copy_never_claims_free_execution.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "credits",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Everything uses credits — there is no free execution path. The app's own copy must never imply otherwise (no \"unlimited\", \"free forever\", or \"no cost\" language), even informally.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-05.9",
}
