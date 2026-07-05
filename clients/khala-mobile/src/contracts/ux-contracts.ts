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
        "Binds discovery ordering and priority only. It does not authorize a second auth layer beyond Tailscale's own network ACL (the desktop pairing endpoint trusts reachability on the tailnet), and it does not promise discovery succeeds off-tailnet.",
      blockerRefs: [],
      contractId: "khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/auth/khala-auth-context.tsx",
        "clients/khala-mobile/src/auth/khala-mobile-pairing-core.ts",
        "clients/khala-mobile/src/auth/khala-mobile-pairing.ts",
        "clients/khala-mobile/tests/khala-mobile-pairing.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Probes multiple Tailnet candidate hosts concurrently (not serially) and returns a real credential pair when any host reports a signed-in desktop, so the app never blocks on a per-host timeout multiplied by candidate count.",
          id: "tailnet_discovery_concurrent_priority.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
        {
          description:
            "A paired outcome always wins over a merely-reachable-but-signed-out host, which always wins over unreachable; ties resolve to the first candidate in the documented host-priority list.",
          id: "tailnet_discovery_outcome_priority.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "auth",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-04",
      },
      state: "enforced",
      statement:
        "IF THERES A DEVICE ON TAILNET THATS AUTHED, USE THAT AUTOMATICALLY - NO LOGIN SCREEN. Before ever showing a manual sign-in screen, the app must look for an already-signed-in desktop Khala Code instance reachable on the same Tailnet and pull working credentials from it.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
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
        "Binds the pure intent-builder layer only (the payload a Steer/Queue/Stop tap constructs). It does not cover the composer's own React state wiring that selects which builder to call — see khala_mobile.composer.rn_component_mount_coverage.v1 for that gap.",
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
        "Binds display sort/format helpers only; does not claim the underlying Khala Sync fleet collection itself is verified live-correct on device (that is a broader claim tracked separately).",
      blockerRefs: [],
      contractId: "khala_mobile.fleet.account_rows_sorted_readiness_then_ref.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/khala-fleet-collections-core.ts",
        "clients/khala-mobile/tests/khala-fleet-collections-core.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Fleet account rows sort ready before cooldown before unavailable before unknown, tie-broken by account ref hash, so a user always sees actionable (ready) accounts first regardless of feed order.",
          id: "fleet_account_readiness_sort.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "fleet/settings",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "Fleet account rows in Settings are always ordered by readiness (ready first) rather than raw feed/insertion order, so the most actionable accounts are never buried.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
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
        "Binds probe concurrency/latency shape only; does not itself prove any particular Tailnet host is reachable from any given real device network.",
      blockerRefs: [],
      contractId: "khala_mobile.connectivity.tailnet_health_probe_concurrent_not_serial.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/status/khala-code-connectivity-core.ts",
        "clients/khala-mobile/src/status/khala-code-connectivity.ts",
        "clients/khala-mobile/tests/khala-code-connectivity.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "Resolving Khala Code connectivity against multiple Tailnet candidate hosts returns the first reachable host's profile without waiting a full serial multiple of the per-host timeout, and simulator/device target selection (loopback vs tailnet) matches the caller's isDevice flag.",
          id: "connectivity_profile_resolution.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/ux-contracts.test.ts",
        },
      ],
      productArea: "connectivity",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "The desktop-connectivity status dot must resolve promptly on both simulator and device, without the wait time growing linearly with the number of candidate Tailnet hosts.",
      surface: "khala-mobile",
      verification:
        "bun test tests/ux-contracts.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
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
        "This is a test-infrastructure gap, not a product defect: the underlying pure builder functions ARE unit-tested (see the enforced contracts above). The gap is specifically that no test mounts the actual React Native component tree.",
      blockerRefs: [
        "blocker.khala_mobile.no_rn_component_render_harness_in_bun_test",
      ],
      contractId: "khala_mobile.composer.rn_component_mount_coverage.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/package.json",
      ],
      oracles: [],
      productArea: "chat composer",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "pending",
      statement:
        "ChatComposer's Steer/Queue picker, Stop button, and idle lane picker actually render the correct visible state and respond to real presses when mounted as a live React Native component tree, not just via their pure intent-builder functions.",
      surface: "khala-mobile",
      verification:
        "No automated oracle yet. react-test-renderer is a devDependency but no test in this package currently mounts ChatComposer or any routed screen; needs a component-render test harness (react-test-renderer or an RN Testing Library equivalent wired into `bun test`) before this can move to enforced.",
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
        "clients/khala-mobile/README.md",
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
        "No automated oracle yet. Current evidence is source-level scaffold, unit tests, local typecheck, iOS simulator build success, Android clean Gradle assemble, and two TestFlight uploads confirmed VALID via the App Store Connect API — none of that is a launched-and-interacted device pass. Needs an owner/device manual-check receipt per platform, or a Maestro/Detox-style scripted device flow, before this can move to enforced.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-05.1",
}
