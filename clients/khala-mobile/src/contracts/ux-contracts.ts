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
        "Binds only whether the app trusts a locally stored credential before showing itself as signed in. It does not change server-side session/token validation itself (Khala Sync's own bootstrap check remains authoritative), and it does not cover the initial sign-in flow (already exercised fresh at sign-in time).",
      blockerRefs: [],
      contractId: "khala_mobile.auth.stored_credential_revalidated_on_launch.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/auth/khala-auth-context.tsx",
        "clients/khala-mobile/src/auth/khala-auth-resume-verify-core.ts",
        "clients/khala-mobile/tests/khala-auth-resume-verify-core.test.ts",
      ],
      oracles: [
        {
          description:
            "A stored credential that fails server-side validation is cleared and treated as signed-out; a stored credential that validates is trusted unchanged; no stored credential never triggers a validation call.",
          id: "resolve_verified_stored_credentials.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-auth-resume-verify-core.test.ts",
        },
      ],
      productArea: "auth",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "A stored/leftover credential (e.g. from a prior auth model, or a revoked session that Keychain data otherwise carries across a TestFlight build update) must never silently skip the sign-in screen. Every app launch re-validates a stored credential against the server before treating the app as signed in, exactly like a fresh sign-in does; an invalid one is cleared so the user sees the real GitHub sign-in screen. Filed after a real TestFlight build carried forward a stale pre-pivot session and skipped straight to old signed-in UI.",
      surface: "khala-mobile",
      verification:
        "bun test tests/khala-auth-resume-verify-core.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "This binds only the local credential store's own trust decision (whether a stored token is even considered before/independent of server validation). It exists because khala_mobile.auth.stored_credential_revalidated_on_launch.v1's server re-validation was not sufficient on its own: a leftover token from a retired auth model (e.g. the old Tailnet-pairing flow) can still validate successfully server-side, which is exactly the wrong outcome — server validity is not the same as \"issued by the current auth model\".",
      blockerRefs: [],
      contractId: "khala_mobile.auth.stored_credential_epoch_purged_on_model_change.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/auth/khala-auth-store.ts",
        "clients/khala-mobile/tests/khala-auth-store.test.ts",
      ],
      oracles: [
        {
          description:
            "A stored ownerUserId/token pair written without a matching current credential-epoch marker (e.g. a leftover Tailnet-pairing or pre-GitHub-OpenAuth write) is unconditionally purged on load and never returned — independent of whether that token would still pass server-side validation.",
          id: "khala_auth_store.credential_epoch_purge.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-auth-store.test.ts",
        },
      ],
      productArea: "auth",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "A credential predating the current auth model is force-cleared on the next launch, regardless of whether it would still authenticate against the server. Filed after a TestFlight build shipping the server-revalidation fix still skipped the GitHub sign-in screen and landed on a stale identity's UI — the leftover token was technically still valid server-side, so revalidation alone let it through.",
      surface: "khala-mobile",
      verification:
        "bun test tests/khala-auth-store.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds only the thread-list/scope-entities read hook's status mapping (what the UI shows for a given sync phase). It does not change the underlying session's bootstrap retry/backoff policy itself, and it does not cover why a scope's bootstrap fails in the first place — only that a failure is never silently indistinguishable from still-loading.",
      blockerRefs: [],
      contractId: "khala_mobile.sync.must_refetch_never_stuck_loading.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/use-khala-sync-scope-entities.ts",
        "clients/khala-mobile/tests/resolve-scope-entities-status.test.ts",
      ],
      oracles: [
        {
          description:
            "A scope parked in the session's must_refetch phase (bootstrap retries exhausted) always maps to an error state with a clear message, regardless of item count — never silently 'loading' forever.",
          id: "resolve_scope_entities_status.must_refetch.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/resolve-scope-entities-status.test.ts",
        },
      ],
      productArea: "sync",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "A thread-list (or any scope-entities read) that gets stuck in the sync session's must_refetch phase is never shown as an eternal, unexplained loading spinner. It surfaces as a real error with a restart hint, and the hook makes one bounded automatic retry attempt before giving up. Filed after a fresh GitHub sign-in landed on the Khala nav with a permanent 'Loading threads' spinner and no way to tell anything had gone wrong.",
      surface: "khala-mobile",
      verification:
        "bun test tests/resolve-scope-entities-status.test.ts tests/use-khala-sync-scope-entities.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds only the ORDERING of our own reload trigger relative to closing our own sync runtime. It does not (and cannot) fix expo-sqlite's own native concurrency bug (github.com/expo/expo #33754, #38168) — it only avoids one known way to hit it from our own reload path. A close() that hangs is bounded by a timeout so this can never turn into a stuck/unresponsive reload.",
      blockerRefs: [],
      contractId: "khala_mobile.sync.reload_drains_sqlite_runtime_first.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/khala-mobile-sync-runtime-registry.ts",
        "clients/khala-mobile/src/updates/ota-update-gate.tsx",
        "clients/khala-mobile/tests/khala-mobile-sync-runtime-registry.test.ts",
      ],
      oracles: [
        {
          description:
            "The OTA reload path drains the active sync runtime's close() before calling Updates.reloadAsync(); a hung close() is bounded by a timeout rather than blocking the reload forever, and a missing runtime (signed out) is an instant no-op.",
          id: "khala_mobile_sync_runtime_registry.drain_before_reload.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-mobile-sync-runtime-registry.test.ts",
        },
      ],
      productArea: "sync",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "An OTA reload never fires while the local Khala Sync SQLite runtime still has an open connection that hasn't been given a chance to close first. Filed after a confirmed, reproducible native crash (EXC_BAD_ACCESS/SIGSEGV inside expo-sqlite's AsyncQueue, three occurrences in a row on build 11, 2026-07-06) that hit right around Updates.reloadAsync() — a known expo-sqlite race between an in-flight database request and the JS context being torn down.",
      surface: "khala-mobile",
      verification:
        "bun test tests/khala-mobile-sync-runtime-registry.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds only the client-side status the scope-entities hook reports when nothing else has resolved by the timeout. It does not diagnose or fix WHY a scope hangs (network, server, or session bug) — it only guarantees the user is never left staring at a silent, unexplained spinner forever, regardless of the cause.",
      blockerRefs: [],
      contractId: "khala_mobile.sync.stuck_loading_watchdog.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/use-khala-sync-scope-entities.ts",
        "clients/khala-mobile/tests/use-khala-sync-scope-entities-watchdog.test.ts",
      ],
      oracles: [
        {
          description:
            "A scope stuck in a non-terminal sync phase (e.g. bootstrapping/catching_up) with zero items force-errors with a restart hint after watchdogMs, even though it never rejects and never reaches the session's own must_refetch give-up phase. A scope that resolves to ready before the watchdog fires is never force-errored afterward.",
          id: "use_khala_sync_scope_entities.watchdog.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/use-khala-sync-scope-entities-watchdog.test.ts",
        },
      ],
      productArea: "sync",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "Loading threads (or any scope-entities read) never spins forever, even when the underlying sync phase genuinely hangs rather than rejecting or reaching the session's must_refetch give-up phase. Filed after build 13 still landed on a permanent 'Loading threads' spinner despite the earlier must_refetch fix — that fix only covers the session's own bounded-retries-exhausted phase, not a request that never settles at all.",
      surface: "khala-mobile",
      verification:
        "bun test tests/use-khala-sync-scope-entities-watchdog.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
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
        "Binds ChatComposer's own React state/render/effect wiring (button swap, lane-picker visibility, controlled-input value, push() call shape) as proven by a REAL mounted component tree via `tests/support/rn-test-environment.ts`. It does not cover real native rendering, gesture/touch physics, Skia drawing, or Reanimated worklet execution on an actual device/simulator — those stay under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics). The Skia-drawn ArwesButton/BackgroundGradient/ActivityIndicator leaves and react-native-reanimated are test-doubled (documented in tests/chat-composer.test.tsx's header comment) because they have no meaningful non-native equivalent; everything else in the real import graph (react-native core primitives, push-to-talk-core, khala-runtime-compose-core, khala-sync-push-core, swipe-quote-core, theme/tokens) is the real, unmocked module.",
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
            "Pressing the real Send button's onPress after typing idle text optimistically appends the chat message through the injected appendMessage() exactly once (body/threadId/messageId), then calls push() exactly once with a [runtime.startTurn] control intent whose bodyRef references that message — proving the component's send-dispatch wiring end to end (the message goes through the optimistic overlay path, not the raw control-intent push).",
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
        "Binds RepoPickerScreen's own load/search/select state wiring — including the REAL (unmocked) KhalaListItem and khala-mobile-repos-api client — as proven by a mounted component tree. It does not cover real native scroll/list virtualization (FlatList's real windowing behavior is test-doubled — see tests/support/rn-test-environment.ts's FlatList leaf stub, added for this contract), real touch/gesture physics, or a live GitHub-token-backed server response; those stay under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics).",
      blockerRefs: [],
      contractId: "khala_mobile.repo_picker.rn_component_mount_coverage.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/repo-picker-screen.tsx",
        "clients/khala-mobile/tests/repo-picker-screen.test.tsx",
        "clients/khala-mobile/tests/support/rn-test-environment.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The real RepoPickerScreen mounts, calls through the REAL (unmocked) khala-mobile-repos-api client against a scripted globalThis.fetch, and renders both scripted repos via the REAL KhalaListItem.",
          id: "repo_picker_mounts_loads_renders_repos.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/repo-picker-screen.test.tsx",
        },
        {
          description:
            "Typing in the real search TextInput filters the rendered rows through the real (unmocked) khala-mobile-repo-search-core functions.",
          id: "repo_picker_search_filters_real_repo_list.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/repo-picker-screen.test.tsx",
        },
        {
          description:
            "Pressing a real repo row's onPress calls the sync runtime's real bindThreadRepo() exactly once with the picked repo's owner/name/defaultBranch and the screen's threadId.",
          id: "repo_picker_select_calls_bind_thread_repo.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/repo-picker-screen.test.tsx",
        },
        {
          description:
            "A failed fetch renders the real client's error-mapped empty state (\"Repositories unavailable\"), not a silent blank screen.",
          id: "repo_picker_failed_fetch_renders_error_branch.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/repo-picker-screen.test.tsx",
        },
      ],
      productArea: "repo picker",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "RepoPickerScreen's loading, search-filter, repo-select, and error states actually render and respond correctly when mounted as a live React Native component tree — extending the same real-component-mount coverage ChatComposer proved out to the mobile-only MVP straight line's repo-pick step.",
      surface: "khala-mobile",
      verification:
        "bun test tests/repo-picker-screen.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Extends tests/support/rn-test-environment.ts with a FlatList leaf stub (data.map(renderItem) inside a plain View, no virtualization) — the first contract to need it beyond ChatComposer's original primitives.",
    },
    {
      authorityBoundary:
        "Enforced at the launched-app-smoke tier: the coverage oracle is a receipt-asserting bun-test, and the heavy proof is the SignedInThreadSmoke Maestro flow run on an iOS Release-configuration simulator (not a device farm). It binds the signed-in iOS-simulator interaction (auto sign-in resolves, thread list renders, seeded thread opens, lane picker visible, message sends and renders) plus the independently-receipted Android emulator launch + GitHub sign-in handoff. It does NOT prove a physical-iOS-device signed-in interaction, real Skia/Reanimated on-device rendering, or gesture/touch physics beyond what Maestro drives on the simulator/emulator — a physical-device pass remains future hardening tracked outside this contract.",
      blockerRefs: [],
      contractId: "khala_mobile.platform.launched_app_interaction_smoke.v1",
      enforcementTier: "nightly",
      evidenceRefs: [
        "clients/khala-mobile/.maestro/shared/_OnFlowStart.yaml",
        "clients/khala-mobile/.maestro/flows/LaunchFallback.yaml",
        "clients/khala-mobile/.maestro/flows/LaunchGitHubSignInInteraction.yaml",
        "clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml",
        "clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh",
        "clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts",
        "clients/khala-mobile/README.md",
        "clients/khala-mobile/tests/maestro-policy.test.ts",
        "docs/khala-mobile/2026-07-05-maestro-launched-app-smoke-receipt.md",
        "docs/khala-mobile/2026-07-05-mobile-qa-swarm-audit.md",
        "docs/khala-mobile/2026-07-06-android-emulator-launch-smoke-receipt.md",
        "docs/khala-mobile/2026-07-06-android-build-and-upload-runbook.md",
        "docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md",
        "docs/qa/khala-code-nightly-matrix.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The SignedInThreadSmoke Maestro receipt (docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md) exists and records a PASS for the seeded signed-in flow — thread opens, the composer's lane picker (Send with Claude) is visible, and a message sends and renders — on the iPhone 17 Pro iOS 26.5 Release simulator.",
          id: "signed_in_thread_smoke_receipt_pass.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts",
        },
      ],
      productArea: "app lifecycle",
      source: {
        channel: "khala-code-session",
        statedBy: "operator-agent",
        statedOn: "2026-07-05",
      },
      state: "enforced",
      statement:
        "The built app launches and is interactable end to end beyond a bare local build: on an iOS Release-configuration simulator, with a seeded public-safe signed-in account, sign-in resolves, the thread list renders, a seeded thread opens, the composer's lane picker is visible, and a typed message sends and appears in the transcript; the same launch and GitHub sign-in handoff are independently proven on a real Android emulator.",
      surface: "khala-mobile",
      verification:
        "Enforced by the SignedInThreadSmoke Maestro flow (clients/khala-mobile/.maestro/flows/SignedInThreadSmoke.yaml), run on an iPhone 17 Pro iOS 26.5 Release-configuration simulator and auto-signed-in as the seeded public-safe test account: it asserts the thread list, opens the seeded thread, asserts the lane picker (Send with Claude), and sends a message that renders in the transcript. Receipt: docs/khala-mobile/2026-07-07-signed-in-thread-smoke-receipt.md (PASS, two green runs; commit cd3122682c). The bun-test oracle clients/khala-mobile/tests/signed-in-thread-smoke-receipt.test.ts asserts that receipt exists and records PASS and runs in the package test glob / repo test:khala-mobile sweep; the Maestro flow itself runs as the opt-in mobile step of the QA nightly matrix (docs/qa/khala-code-nightly-matrix.md, OA_QA_NIGHTLY_INCLUDE_MOBILE=1) given a booted simulator + installed Release build via clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh. Android launch + GitHub sign-in handoff are separately receipted in docs/khala-mobile/2026-07-06-android-emulator-launch-smoke-receipt.md. A real physical-iOS-device signed-in interaction pass remains future hardening tracked outside this contract.",
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
        "Binds only the balance-gate decision function (never block on undetermined/unavailable data, only on a confirmed non-positive balance) and the suggested-task/title-derivation content. It does not prove the full onboarding screen mounts correctly end to end on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics). It also does not claim a live balance check is exercisable today: the balance endpoint itself is still proposed, not built (#8480), so this gate is currently always permissive in practice until that lands.",
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
        "Binds only the extraction/validation of the deep-link string from a notification's data payload, and that a well-formed one is handed to Linking.openURL — it does not prove the OS actually delivers the notification, that the resulting navigation lands on the exact right screen state (that's the broader on-device navigation claim under khala_mobile.platform.launched_app_interaction_smoke.v1, which is enforced at the launched-app-smoke tier but does not assert notification-tap navigation), or that the server always includes a threadId (MM-G2, #8486, is outside this lane's scope).",
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
    {
      authorityBoundary:
        "This SEAM contract (ST-5 #8511, two-sided convention from packages/behavior-contracts) binds the boundary where the mobile app's OpenAuth access token becomes the server's accepted sync bearer: the token clients/khala-mobile/src/auth/mobile-openauth.ts obtains and the auth context stores MEETS the Worker's mobile-session verification (apps/openagents.com/workers/api/src/auth/mobile-session.ts). Each side already has its own one-sided suite (tests/mobile-openauth.test.ts; the Worker's mobile-session.test.ts) — by the seam convention neither can ever be this contract's oracle. It does not bind OpenAuth issuer availability, token lifetime policy, or Khala Sync scope membership.",
      blockerRefs: [
        "blocker.khala_mobile.needs_two_sided_mobile_session_bridge_e2e",
      ],
      contractId: "khala_mobile.seam.mobile_session_token_bridge.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8511",
        "docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md",
        "clients/khala-mobile/src/auth/mobile-openauth.ts",
        "clients/khala-mobile/src/auth/khala-auth-context.tsx",
        "apps/openagents.com/workers/api/src/auth/mobile-session.ts",
        "contract:khala_sync.seam.bearer_ws_connect_reaches_live.v1",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [],
      productArea: "auth",
      seam: {
        client: "clients/khala-mobile/src/auth/mobile-openauth.ts",
        server: "apps/openagents.com/workers/api/src/auth/mobile-session.ts",
      },
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "pending",
      statement:
        "The OpenAuth access token the mobile app signs in with is accepted end to end as the sync bearer: the exact token the mobile OpenAuth flow stores is what the Worker's mobile-session boundary verifies for a cookie-less client's authenticated calls — the two sides can never silently drift on where the token is carried or how it is validated, because that drift is exactly the class of bug that shipped the builds 10-13 WebSocket 401 loop.",
      surface: "khala-mobile",
      verification:
        "No two-sided oracle yet: the Worker-side mobile-session.test.ts and the client-side tests/mobile-openauth.test.ts are one-sided suites and are deliberately not acceptable as this seam contract's oracle. Needs an e2e (*.e2e.test.ts) that imports the REAL client token flow and the REAL server verification boundary (or drives a real/staging Worker) and proves a mobile-issued access token authenticates an actual request. Flip to enforced with that ref once it lands; the seam coverage checker in packages/behavior-contracts will then hold it to the e2e requirement.",
    },
    {
      authorityBoundary:
        "This SEAM contract (ST-5 #8511) binds the OTA runtime-fingerprint round trip between the client build (expo-updates runtimeVersion policy \"fingerprint\" in clients/khala-mobile/app.json, resolved from the native dependency graph at build time) and the updates server's manifest resolution (apps/oa-updates/src/manifest-resolver.ts matching Expo-Runtime-Version). It does not bind manifest signing, asset serving, or the reload path (khala_mobile.sync.reload_drains_sqlite_runtime_first.v1 owns reload ordering). Each side already 'works' alone — publish succeeds and the resolver matches exactly — which is precisely why only a round-trip check can catch a silent fingerprint shift.",
      blockerRefs: [
        "blocker.khala_mobile.needs_ota_fingerprint_roundtrip_e2e",
      ],
      contractId: "khala_mobile.seam.ota_manifest_fingerprint_roundtrip.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/OpenAgentsInc/openagents/issues/8511",
        "docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md",
        "clients/khala-mobile/app.json",
        "apps/oa-updates/src/manifest-resolver.ts",
        "apps/oa-updates/scripts/publish-ota.sh",
        "contract:khala_mobile.updates.ota_manifest_points_at_openagents_updates_only.v1",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [],
      productArea: "updates",
      seam: {
        client: "clients/khala-mobile/app.json",
        server: "apps/oa-updates/src/manifest-resolver.ts",
      },
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "pending",
      statement:
        "An OTA published for the current runtime fingerprint actually reaches devices built with that fingerprint: the fingerprint the client build embeds and the runtimeVersion the updates server serves manifests for must round-trip, and a dependency change that silently shifts the runtime fingerprint must fail a check instead of stranding shipped builds. Filed after a font dependency silently changed the runtime fingerprint earlier on 2026-07-06, so build-12 devices could no longer receive the OTA while client and server each looked healthy on their own.",
      surface: "khala-mobile",
      verification:
        "No two-sided oracle yet. Needs a fingerprint-roundtrip e2e (*.e2e.test.ts): compute the REAL runtime fingerprint from the client package (npx expo-updates fingerprint:generate over clients/khala-mobile, the same computation publish-ota.sh uses), then drive the REAL apps/oa-updates manifest resolver with that value as Expo-Runtime-Version and prove a published update for it resolves — and that a fingerprint drift against the latest published runtimeVersion fails loudly. Flip to enforced with that ref once it lands; the seam coverage checker in packages/behavior-contracts will then hold it to the e2e requirement.",
    },
    {
      authorityBoundary:
        "Binds KhalaThreadHeader's own render/press wiring (proven by a real mounted component tree) plus the thread screen's wiring of the action. It does not prove the full ThreadMessagesScreen mounts end to end, nor that createThread succeeds against a live server — the header owns the affordance; the sync runtime's createThread is exercised separately. When the runtime isn't ready the button is rendered disabled rather than hidden, so the affordance never disappears mid-session.",
      blockerRefs: [],
      contractId: "khala_mobile.thread.new_thread_action_always_reachable.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/components/khala-thread-header.tsx",
        "clients/khala-mobile/src/screens/thread-messages-screen.tsx",
        "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The mounted KhalaThreadHeader renders exactly one 'New thread' button; when a handler is provided the button is enabled and pressing it calls the handler exactly once.",
          id: "new_thread_button_present_and_calls_handler.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
        {
          description:
            "With no handler (sync runtime not yet ready) the 'New thread' button still renders — never hidden — but is disabled (no onPress, accessibilityState.disabled), so the affordance can never disappear mid-session.",
          id: "new_thread_button_disabled_without_runtime.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
        {
          description:
            "The thread-messages screen passes onNewThread to KhalaThreadHeader and its handler creates a fresh thread via runtime.createThread and navigates to it with navigation.replace('ThreadMessages').",
          id: "thread_screen_wires_new_thread_action.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
      ],
      productArea: "thread view",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "There must be an obvious one-tap way to start a new thread from the thread view. Filed after the owner's report: \"loads but sucks ... also no way to start a new thread.\" The thread header always shows a New thread action that creates a fresh thread and navigates to it.",
      surface: "khala-mobile",
      verification:
        "bun test tests/khala-thread-header.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds that the thread view's two ways out (Open menu, New thread) live in a turn-agnostic header that renders unconditionally, and that Stop dispatches a real interrupt while an idle composer is editable. The live idle↔active render transition of the composer itself is owned/proven by khala_mobile.composer.rn_component_mount_coverage.v1 (idle-shows-Send + Stop-calls-interrupt oracles); this contract binds the never-trapped structural guarantee around it. It does not prove the server actually settles an interrupted turn.",
      blockerRefs: [],
      contractId: "khala_mobile.thread.active_turn_never_traps_user.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/components/khala-thread-header.tsx",
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/src/screens/thread-messages-screen.tsx",
        "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        "contract:khala_mobile.composer.rn_component_mount_coverage.v1",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The mounted KhalaThreadHeader always renders both an 'Open menu' (drawer hamburger) and a 'New thread' button; the header takes no turn state, so neither escape hatch can be hidden while a turn is queued/running/waiting_for_input.",
          id: "header_escape_hatches_always_render.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
        {
          description:
            "Stop builds a runtime.interruptTurn intent (a real cancel, not a visual no-op), the composer exposes an editable Send button for the idle state, and the thread screen renders KhalaThreadHeader above the only turn-gated element.",
          id: "stop_interrupts_and_composer_reverts_when_idle.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
      ],
      productArea: "thread view",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-06",
      },
      state: "enforced",
      statement:
        "A queued or running turn must never trap the user. Filed after the owner's report: \"just the one message and button shows stop, cant do anything.\" Stop cancels the turn and returns the composer to an editable state, and the user can always open the drawer menu or start a new thread even with a turn in flight.",
      surface: "khala-mobile",
      verification:
        "bun test tests/khala-thread-header.test.tsx tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds KhalaThreadHeader's left-action render/press wiring (a real mounted component tree) plus the thread screen's source wiring of that action to the root Drawer. It does not prove the Drawer actually animates open on a device (that is React Navigation's own behavior), nor that the thread screen mounts end to end — the header owns the affordance; the navigator wiring is asserted at the source level.",
      blockerRefs: [],
      contractId: "khala_mobile.thread.header_menu_opens_drawer.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/components/khala-thread-header.tsx",
        "clients/khala-mobile/src/screens/thread-messages-screen.tsx",
        "clients/khala-mobile/src/navigators/AppNavigator.tsx",
        "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The mounted KhalaThreadHeader renders exactly one 'Open menu' hamburger button (and zero 'Back' buttons); pressing it calls onOpenMenu exactly once.",
          id: "menu_button_present_and_calls_handler.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
        {
          description:
            "The thread-messages screen wires the header's onOpenMenu to open the root Drawer via navigation.getParent()?.openDrawer(), so the flyout menu (nav items + credit balance) opens from the chat view.",
          id: "thread_screen_wires_menu_to_open_drawer.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/khala-thread-header.test.tsx",
        },
      ],
      productArea: "thread view",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The chat/thread header's left button is a hamburger that opens the drawer flyout menu — not a back button (the old back chevron did not work). Filed after the owner's report that the chat header back button didn't work and should open the drawer nav instead.",
      surface: "khala-mobile",
      verification:
        "bun test tests/khala-thread-header.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the local-first read path: an optimistic chat-message append is visible through useKhalaSyncScopeEntities (the transcript's read hook) before the server confirms it, and the composer routes a send through that optimistic append. It does not prove server-side turn dispatch, nor the on-device FlatList render — it binds that the user's own just-sent message exists in the read model the transcript renders from, which is exactly what regressed.",
      blockerRefs: [],
      contractId: "khala_mobile.chat.optimistic_message_renders_on_send.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/sync/use-khala-sync-scope-entities.ts",
        "clients/khala-mobile/src/components/chat-composer.tsx",
        "clients/khala-mobile/src/screens/thread-messages-screen.tsx",
        "clients/khala-mobile/tests/use-khala-sync-scope-entities.test.ts",
        "clients/khala-mobile/tests/chat-composer.test.tsx",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "An optimistic chat append (overlay-backed, its server push not yet resolved) is surfaced by useKhalaSyncScopeEntities immediately — proving the hook reads the overlay (confirmed base + pending optimistic), not the confirmed-only store. A store-only read would return an empty thread, the exact 'sending does nothing' bug.",
          id: "optimistic_append_visible_via_hook.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/use-khala-sync-scope-entities.test.ts",
        },
        {
          description:
            "Pressing Send routes the chat message through the optimistic appendMessage() (shows immediately + persists) and sends only the runtime.startTurn control intent via push — so a plain send always produces a locally-visible message.",
          id: "composer_send_uses_optimistic_append.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/chat-composer.test.tsx",
        },
      ],
      productArea: "chat composer",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "When a user sends a message it must appear in the transcript immediately (optimistic) and persist. Filed after the owner's report that typing a message and hitting send did nothing — the message vanished from the input and never showed in the list.",
      surface: "khala-mobile",
      verification:
        "bun test tests/use-khala-sync-scope-entities.test.ts tests/chat-composer.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the client-side web-auth session posture: the OAuth prompt runs in an ephemeral (non-cookie-sharing) web session, and sign-out revokes the server session. It does not control the OpenAuth issuer's or GitHub's own server-side session lifetime beyond the revoke call, and it is a source-level assertion of the flow, not a device-driven account-switch e2e.",
      blockerRefs: [],
      contractId: "khala_mobile.auth.signout_ends_web_session_for_account_switch.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/auth/khala-auth-context.tsx",
        "clients/khala-mobile/tests/auth-ephemeral-session.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "signInWithGitHub runs the auth prompt with preferEphemeralSession: true (a non-persistent ASWebAuthenticationSession that does not reuse Safari/issuer/GitHub cookies), and signOut revokes the server session (deleteMobileOpenAuthSession) — so re-auth after sign-out presents a fresh GitHub login / account picker instead of silently re-signing-in the previous account.",
          id: "signout_ephemeral_web_session.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/auth-ephemeral-session.test.ts",
        },
      ],
      productArea: "auth",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "Signing out fully ends the web auth session so the next sign-in can choose a different account. Filed after the owner could not switch to a test account: after Sign out, 'Log in with GitHub' silently re-authenticated the same account because the persistent Safari/issuer cookies were reused.",
      surface: "khala-mobile",
      verification:
        "bun test tests/auth-ephemeral-session.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the app's single base background color (the token every Ignite `Screen` reads) only. It does not repaint per-surface accent/tint colors, the baked hero artwork on the sign-in screen, or any component-local background override.",
      blockerRefs: [],
      contractId: "khala_mobile.theme.base_background_dark_navy.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/ignite/theme/colorsDark.ts",
        "clients/khala-mobile/tests/theme-colors.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The dark theme's base `background` (and the `neutral200` palette slot it reads) is a very dark navy blue in the #05xx–#07xx range with blue as the dominant channel — not the retired warm brown #191015 and not pure black.",
          id: "base_background_is_dark_navy.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/theme-colors.test.ts",
        },
      ],
      productArea: "theme",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The app's primary background is a very dark navy blue across every screen — dark and clearly navy, not the old warm brown and not pure black.",
      surface: "khala-mobile",
      verification:
        "bun test tests/theme-colors.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the onboarding welcome heading's text selection only (personalize when a GitHub login is available, fall back to the product name otherwise) and the plumbing that carries the login from the mobile-session response through the auth context. It does not change who the session belongs to or any server-side scope/authority; the greeting is display-only.",
      blockerRefs: [],
      contractId: "khala_mobile.onboarding.welcome_greeting_uses_github_username.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/onboarding-core.ts",
        "clients/khala-mobile/src/screens/onboarding-flow.tsx",
        "clients/khala-mobile/src/auth/mobile-openauth.ts",
        "clients/khala-mobile/src/auth/khala-auth-store.ts",
        "clients/khala-mobile/src/auth/khala-auth-context.tsx",
        "apps/openagents.com/workers/api/src/index.ts",
        "clients/khala-mobile/tests/onboarding-core.test.ts",
        "clients/khala-mobile/tests/mobile-openauth.test.ts",
        "clients/khala-mobile/tests/khala-auth-store.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The welcome heading personalizes to `Welcome, <login>` (trimmed) when a GitHub login is available, and falls back to `Welcome to Khala Code` for a blank/whitespace login (email-provider session, or a Worker deploy predating the session field).",
          id: "welcome_heading_personalizes_or_falls_back.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/onboarding-core.test.ts",
        },
        {
          description:
            "The mobile-session client surfaces `githubLogin` when the bridge returns it and omits it otherwise, and a saved credential round-trips `githubLogin` through SecureStore so the greeting survives a relaunch.",
          id: "github_login_flows_through_session_and_store.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/mobile-openauth.test.ts",
        },
      ],
      productArea: "onboarding",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The onboarding welcome heading greets the signed-in user by their GitHub username (`Welcome, <username>`), falling back to a warm generic greeting only when the username is genuinely unavailable.",
      surface: "khala-mobile",
      verification:
        "bun test tests/onboarding-core.test.ts tests/mobile-openauth.test.ts tests/khala-auth-store.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. The server side (githubLogin in the /api/mobile/session response) is covered by apps/openagents.com/workers/api/src/auth/mobile-session.test.ts and needs a monolith redeploy before the greeting personalizes on-device.",
    },
    {
      authorityBoundary:
        "Binds the onboarding 'Get started' CTA's fill placement (fill on an inner plain View, not on the Pressable's function style) and its high-contrast appearance, as proven by a real mounted component tree. It does not cover real native touch/gesture physics on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics).",
      blockerRefs: [],
      contractId: "khala_mobile.onboarding.get_started_cta_fill_on_inner_view.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/onboarding-flow.tsx",
        "clients/khala-mobile/src/components/sign-in-screen.tsx",
        "clients/khala-mobile/tests/onboarding-welcome-cta.test.tsx",
        "clients/khala-mobile/tests/support/rn-test-environment.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The mounted WelcomeStep renders a single cyan (#4fd0ff) filled inner View — button-sized (minHeight 54, borderRadius 12) — wrapping the dark bold `Get started` label, while the Pressable owns only the touch target (accessibilityRole button, onPress) and carries NO backgroundColor of its own. This is the Fabric no-paint fix: a Pressable with a function style does not paint its own background, so the fill must live on a plain View.",
          id: "get_started_cta_fill_on_inner_view.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/onboarding-welcome-cta.test.tsx",
        },
      ],
      productArea: "onboarding",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The onboarding 'Get started' button is a real, obviously-tappable filled CTA (high-contrast cyan pill with a dark bold label), never an invisible no-fill control — the fill lives on an inner View so it paints under the New Architecture (Fabric), matching the login button fix.",
      surface: "khala-mobile",
      verification:
        "bun test tests/onboarding-welcome-cta.test.tsx inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting uses the bun test React Native harness in tests/support/rn-test-environment.ts (extended here with a ScrollView leaf stub).",
    },
    {
      authorityBoundary:
        "Binds the Settings 'Sign out' control's fill placement (fill/border on an inner plain View, not on the Pressable's function style) and its visible-but-secondary appearance, as proven by a real mounted component tree. It does not cover real native touch/gesture physics on a device — that stays under khala_mobile.platform.launched_app_interaction_smoke.v1 (now enforced at the launched-app-smoke tier — a Release-simulator + Android-emulator Maestro pass — which still does not cover real on-device gesture/touch physics).",
      blockerRefs: [],
      contractId: "khala_mobile.settings.sign_out_button_fill_on_inner_view.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/settings-screen.tsx",
        "clients/khala-mobile/src/components/sign-in-screen.tsx",
        "clients/khala-mobile/tests/settings-sign-out-button.test.tsx",
        "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        "clients/khala-mobile/tests/support/rn-test-environment.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The mounted AccountSection renders a single filled inner View (backgroundColor #141d33, minHeight 48, borderRadius 10, borderWidth 1) wrapping the light bold `Sign out` label, while the Pressable owns only the touch target (accessibilityRole button, onPress) and carries NO backgroundColor of its own. This is the Fabric no-paint fix: a Pressable with a function style does not paint its own background/border, so the fill must live on a plain View.",
          id: "sign_out_button_fill_on_inner_view.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/settings-sign-out-button.test.tsx",
        },
        {
          description:
            "The Settings source builds Sign out with the Pressable+inner-View fill pattern (styles.signOutButton/signOutPressable, an RNText label) inside AccountSection, and no longer uses the invisible Ignite `Button preset=\"reversed\" text=\"Sign out\"`.",
          id: "sign_out_uses_fabric_safe_inner_view_fill.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        },
      ],
      productArea: "settings",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The Settings 'Sign out' button is a real, clearly-visible tappable control (a neutral outlined pill with a legible label on the dark-navy background), never invisible near-dark text — the fill lives on an inner View so it paints under the New Architecture (Fabric), matching the login and 'Get started' button fixes. It is styled as a secondary/neutral action, not as loud as a primary CTA.",
      surface: "khala-mobile",
      verification:
        "bun test tests/settings-sign-out-button.test.tsx tests/settings-screen-composition.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main. Real component mounting uses the bun test React Native harness in tests/support/rn-test-environment.ts.",
    },
    {
      authorityBoundary:
        "Source-composition assertion (explicitly labeled stopgap, same allowance as khala_mobile.settings.no_desktop_dependent_sections.v1): proves WHICH section the Delete account trigger is declared and rendered in, and its render order relative to the other sections. It cannot prove on-device layout pixels; the confirmation modal + KHALA_ACCOUNT_DELETION_POLICY_COPY + deleteAccount() behavior it relocates are unchanged and covered where they already were. A real RN-mount oracle for Settings is future work under khala_mobile.platform.launched_app_interaction_smoke.v1 (a full Settings mount needs a Modal harness stub not yet in tests/support/rn-test-environment.ts).",
      blockerRefs: [],
      contractId: "khala_mobile.settings.delete_account_isolated_at_bottom.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/src/screens/settings-screen.tsx",
        "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The Delete account trigger (and its deleteAccount() call) is no longer inside AccountSection; it lives in a dedicated DeleteAccountSection (red outlined pill + red-bordered $dangerCard) that is rendered as the LAST section in the Settings body, after About & diagnostics — never adjacent to Sign out.",
          id: "delete_account_isolated_at_bottom.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/settings-screen-composition.test.ts",
        },
      ],
      productArea: "settings",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "Delete account is a destructive action isolated in its own section at the very bottom of Settings (below About & diagnostics), never placed directly under Sign out, so a mistap near Sign out cannot land on irreversible account deletion. It stays clearly marked destructive (red), just out of the way.",
      surface: "khala-mobile",
      verification:
        "bun test tests/settings-screen-composition.test.ts inside clients/khala-mobile; runs in the package test glob and the repo test:khala-mobile sweep before pushes to main.",
    },
    {
      authorityBoundary:
        "Binds the QAM-5 nightly mobile row definition, launchd-owned runner posture, QA Swarm projection node, required named perf budgets, required seam probes, strict auto-filed issue body, and seven-receipt exit evaluator. It does not prove any real nightly has run, does not claim the seven-consecutive-night exit is satisfied, and keeps QAM-4 Storybook V1 visual capture blocked until #8539 has a proven device-walk receipt.",
      blockerRefs: [],
      contractId: "khala_mobile.qa.nightly_mobile_row_owned_runner_discipline.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/qa-runner/src/mobile-nightly.ts",
        "apps/qa-runner/src/mobile-nightly.test.ts",
        "docs/khala-code/receipts/2026-07-07-qam-5-mobile-nightly-row.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The QAM-5 row definition schedules only owned Tailnet Mac launchd work for iOS Maestro, seeded device monkey, visual capture, named perf budgets, and seam probes; excludes hosted CI/EAS; carries the required perf IDs and khala-sync live-classification seam probe; emits a public-safe strict issue body; and refuses to satisfy exit before seven consecutive passed nightly receipts.",
          id: "nightly_mobile_row_owned_runner_discipline.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/qa-runner/src/mobile-nightly.test.ts",
        },
      ],
      productArea: "qa",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The Khala Mobile nightly row is defined as owned-runner-only launchd work: iOS Maestro, seeded device monkey, QAM-4 visual capture, named mobile perf budgets, and seam probes all report through typed QA receipts/projection metadata, never hosted CI/EAS; exit remains blocked until seven consecutive passed real nightly receipts exist.",
      surface: "khala-mobile",
      verification:
        "bun test src/mobile-nightly.test.ts inside apps/qa-runner plus the Khala mobile UX-contract sweep; real nightly execution remains blocked/pending under #8540 until owned-runner receipts exist.",
    },
    {
      authorityBoundary:
        "Binds the Android emulator lane definition, runner command surface, QAM-5 nightly row membership, and public-safe receipt/capture paths. It does not prove that the Android emulator flows have passed in the current nightly row, does not claim Android-keyed baselines exist yet, and does not replace the required #8541 green emulator receipts.",
      blockerRefs: [],
      contractId: "khala_mobile.qa.android_emulator_lane_definition.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "clients/khala-mobile/scripts/android-emulator-test-run.sh",
        "clients/khala-mobile/tests/maestro-policy.test.ts",
        "apps/qa-runner/src/mobile-nightly.ts",
        "apps/qa-runner/src/mobile-nightly.test.ts",
        "docs/khala-code/receipts/2026-07-07-qam-6-android-lane-definition.md",
        "docs/khala-mobile/khala-mobile-ux-contract.md",
      ],
      oracles: [
        {
          description:
            "The Android emulator lane exposes a local package script, boots/creates an AVD, waits for sys.boot_completed, installs the debug APK, runs the shared launch/sign-in Maestro flows, records an honest receipt, captures adb exec-out screencaps, and is represented in the QAM-5 nightly row without hosted CI/EAS.",
          id: "android_emulator_lane_definition.source",
          kind: "bun-test",
          mode: "unit",
          ref: "clients/khala-mobile/tests/maestro-policy.test.ts",
        },
        {
          description:
            "The QAM-5 nightly row includes Android emulator Maestro and adb screencap nodes, with public-safe scheduled commands and artifact refs.",
          id: "android_nightly_row_membership.unit",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/qa-runner/src/mobile-nightly.test.ts",
        },
      ],
      productArea: "qa",
      source: {
        channel: "khala-code-session",
        statedBy: "owner",
        statedOn: "2026-07-07",
      },
      state: "enforced",
      statement:
        "The Khala Mobile Android lane has a local owned-runner emulator harness and QAM-5 nightly row entries for boot proof, shared Maestro launch/sign-in flow parity, and adb screencap visual-capture parity; #8541 remains open until those flows are green in the nightly row with Android-keyed baseline receipts.",
      surface: "khala-mobile",
      verification:
        "bun test tests/maestro-policy.test.ts inside clients/khala-mobile plus bun test src/mobile-nightly.test.ts inside apps/qa-runner; real Android emulator execution remains the #8541 exit proof.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-07.6",
}
