import { BehaviorContractSchemaVersion, type BehaviorContractRegistryDocument } from "./contract";
import { audioBehaviorContracts } from "./audio";

/**
 * Pending owner contracts for the greenfield OpenAgents mobile/desktop apps.
 * These live in the shared registry until each new app exists and can own an
 * enforced registry plus executable identity, security, and cross-device
 * oracles.
 */
export const openAgentsAppsContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
    ...audioBehaviorContracts,
    {
      authorityBoundary:
        "This contract covers public chat transport and external signer use only. It creates no OpenAgents application session. A chat identity grants no Pylon, task, payment, settlement, moderation, or release authority. OpenAgents configuration is one deployment profile and is not a private protocol.",
      blockerRefs: [],
      contractId: "openagents_web.public_nostr_chat.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/public-nostr-chat/src/profile.test.ts",
        "packages/public-nostr-chat/src/client.test.ts",
        "github:OpenAgentsInc/openagents#9258",
        "github:OpenAgentsInc/openagents#9258:issuecomment-5081084346",
        "github:OpenAgentsInc/nostr-effect#171:issuecomment-5081084500",
      ],
      oracles: [
        {
          description:
            "The protocol tests verify signed events, group tags, reply references, bounded media metadata, stable cursors, reconnect overlap, event ID deduplication, NIP-42, and relay result mapping.",
          id: "openagents_web.public_nostr_chat.protocol",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/public-nostr-chat/src/client.test.ts",
        },
      ],
      productArea: "Public Nostr chat",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-25",
      },
      state: "enforced",
      statement:
        "A signed-out reader and an independently signed human or agent client use the same standard NIP-29 event stream. The reusable client accepts configurable relay and group values. OpenAgents supplies only the default deployment profile.",
      surface: "openagents.com/agentchat",
      verification:
        "The shared package and web test sweeps enforce the standard Nostr protocol and external signer boundary. The production receipts verify the relay self key, relay-signed group state, authenticated writes, restart durability, and desktop and mobile browser behavior.",
    },
    {
      authorityBoundary:
        "This contract covers automatic projection into the read-only mobile home after the paired device bridge emits a mirror state. It grants no thread, run, file, terminal, git, sandbox, account, or release mutation authority. The live host-to-phone seam remains owned by the recorded cross-repository journey.",
      blockerRefs: [],
      contractId: "openagents_mobile.home_automatic_desktop_activity.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/omega-mobile-home.ts",
        "apps/openagents-mobile/tests/omega-mobile-home.test.ts",
        "docs/fable/2026-07-27-omega-mobile-tailnet-mirror-spec.md",
        "github:OpenAgentsInc/openagents#9261",
      ],
      oracles: [
        {
          description:
            "The mobile home program subscribes to the bridge client. The test emits a paired direct mirror after startup and verifies that the desktop name, newest thread, executor and model disclosure, run, and older thread appear in descending activity order without a selection intent.",
          id: "openagents_mobile.home_automatic_desktop_activity.projection",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/omega-mobile-home.test.ts",
        },
      ],
      productArea: "Omega mobile desktop mirror",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-27",
      },
      state: "enforced",
      statement: "The home screen shows desktop activity automatically when paired.",
      surface: "openagents-mobile",
      verification:
        "The OpenAgents mobile test sweep runs the Effect Native projection oracle. The cross-repository live pairing and transport journey is tracked separately in openagents#9262 and omega#121 through omega#123.",
    },
    {
      authorityBoundary:
        "This contract covers the visible mobile connection label and its evidence-derived staleness copy. It does not claim that a direct, relay, or offline transport exists when the bridge client has no matching evidence.",
      blockerRefs: [],
      contractId: "openagents_mobile.home_honest_connection_state.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/omega-mobile-home.ts",
        "apps/openagents-mobile/tests/omega-mobile-home.test.ts",
        "docs/fable/2026-07-27-omega-mobile-tailnet-mirror-spec.md",
        "github:OpenAgentsInc/openagents#9261",
      ],
      oracles: [
        {
          description:
            "The Effect Native view oracle renders direct, relay, and offline bridge evidence. Each result keeps the matching state in the desktop header and shows either a live label or an age for the last desktop update.",
          id: "openagents_mobile.home_honest_connection_state.header",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/omega-mobile-home.test.ts",
        },
      ],
      productArea: "Omega mobile desktop mirror",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-27",
      },
      state: "enforced",
      statement: "The connection state is always visible and honest.",
      surface: "openagents-mobile",
      verification:
        "The OpenAgents mobile test sweep runs the direct, relay, and offline header oracle.",
    },
    {
      authorityBoundary:
        "This fixes app identity and icon selection; it does not claim repository proof of the existing store records or authorize upload before owner/store verification.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8597"],
      contractId: "openagents_apps.greenfield_mobile_identity.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
        "docs/sol/issues/app-mobile.md",
        "apps/openagents-mobile/assets/images/icon.png",
      ],
      oracles: [
        {
          description:
            "Planned app-config and asset-digest oracle for name, iOS/Android identifiers, and copied icon.",
          id: "openagents_mobile.identity_icon.planned",
          kind: "planned",
          mode: "headless",
          ref: "github:OpenAgentsInc/openagents#8597",
        },
      ],
      productArea: "mobile application identity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        'the mobile app -- which should be also built from scratch -- must use the existing app identifier "com.openagents.app" (it\'s called "OpenAgents") and that should use the same app icon Khala Code mobile now does.',
      surface: "openagents-mobile",
      verification:
        "Pending #8597: assert display name OpenAgents, iOS bundle identifier and Android application ID com.openagents.app, and copied icon SHA-256 0a1865ac6d1efc792d365d9a37af9e6ffa3270fa7c8731f36129f35371bfc7ce.",
    },
    {
      authorityBoundary:
        "Capability folding preserves typed authority boundaries; Sarah does not inherit provider credentials, payment authority, or raw private worker events.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8566",
        "github:OpenAgentsInc/openagents#8574",
        "github:OpenAgentsInc/openagents#8597",
      ],
      contractId: "openagents_apps.sarah_first_khala_capabilities.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/MASTER_ROADMAP.md",
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
      ],
      oracles: [
        {
          description: "Planned capability-disposition and cross-device Sarah/FleetRun oracle.",
          id: "openagents_apps.sarah_khala_folding.planned",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8566",
        },
      ],
      productArea: "Sarah-first product consolidation",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "retired",
      statement: "All Khala Code ideas are to be folded into the Sarah-first OpenAgents app.",
      surface: "openagents-mobile-and-desktop",
      verification:
        "Retired by the 2026-07-10 owner decision that removed Sarah as a product surface. Its successor contract openagents_apps.desktop_runtime_and_early_mobile_sync.v1 was itself retired when the Electron desktop app was deleted; the surviving cross-device requirement continues under MASTER_ROADMAP R0–R7 and the enforced openagents_mobile.chat.authoritative_sync_mode.v1 contract.",
    },
    {
      authorityBoundary:
        "Remote-first binds durable session identity and fenced checkpoint/rehydrate movement. It does not promise transparent migration of process memory, PTYs, sockets, provider hidden state, raw host paths, or credentials, and it does not upload a local-only session until the owner explicitly adopts it.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8566",
        "github:OpenAgentsInc/openagents#8574",
        "github:OpenAgentsInc/openagents#8597",
        "github:OpenAgentsInc/openagents#8746",
        "github:OpenAgentsInc/openagents#8748",
        "github:OpenAgentsInc/openagents#8749",
        "github:OpenAgentsInc/openagents#8753",
      ],
      contractId: "openagents_apps.remote_first_portable_sessions.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/MASTER_ROADMAP.md",
        "docs/sol/issues/native-streamed-conversation-handoff.md",
      ],
      oracles: [
        {
          description:
            "Planned cross-host oracle: quiesce and checkpoint one durable session, fence its source attachment, rehydrate it on a compatible local or remote target under the same session/thread/run/WorkContext refs, and prove one live generation, exact repository post-image, fresh target grants, source cleanup, and idempotent failure/failback outcomes.",
          id: "openagents_apps.remote_first_portable_sessions.planned",
          kind: "planned",
          mode: "e2e",
          ref: "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        },
      ],
      productArea: "portable coding-session authority",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "pending",
      statement:
        "Remote-first, not local-first. Sessions can be stopped on any machine and moved to any other, local or remote. i.e. handoff to cloud.",
      surface: "openagents-mobile-desktop-pylon-cloud",
      verification:
        "PORT-00 #8745 freezes the executable schema/model boundary. PORT-01–PORT-08 #8746–#8753 remain pending for durable authority and real local-to-managed-to-owner-remote acceptance.",
    },
    {
      authorityBoundary:
        "This enforced contract freezes only the portable-session vocabulary, schemas, cross-record invariants, command parity, and real-host journey falsifiers. It grants no persistence, dispatch, broker redemption, target compatibility, movement, mobile control, or product acceptance authority.",
      blockerRefs: [],
      contractId: "openagents_apps.portable_session_contract_freeze.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/portable-session-contract/src/index.ts",
        "packages/portable-session-contract/src/model.ts",
        "packages/portable-session-contract/src/journeys.ts",
        "packages/portable-session-contract/src/portable-session-contract.test.ts",
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "github:OpenAgentsInc/openagents#8745",
      ],
      oracles: [
        {
          description:
            "Decodes the versioned public-safe schemas and rejects host-derived identity, graph flattening/leakage, two live attachments, incomplete descendant fencing, stale commands, secret/process checkpoint state, and silent target changes; also freezes the real-host journey and its first-paint/action-parity falsifiers.",
          id: "openagents_apps.portable_session_contract_freeze",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/portable-session-contract/src/portable-session-contract.test.ts",
        },
      ],
      productArea: "portable coding-session contract and invariant boundary",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "enforced",
      statement:
        "Portable coding sessions use owner-minted host-independent identity, a canonical nested graph with independent cursors, graph-wide generation fencing, secret-free content-addressed checkpoints, provider-neutral targets, target-scoped capability refs, shared typed movement commands, detail-independent first paint, and identical pointer/tap/key action semantics.",
      surface: "openagents-mobile-desktop-pylon-cloud",
      verification:
        "pnpm exec vp test --cwd packages/portable-session-contract and pnpm exec vp test --cwd packages/behavior-contracts run the executable PORT-00 contract/model and registry coverage oracles.",
    },
    {
      authorityBoundary:
        "The target contract authorizes only owner-scoped execution through declared capabilities and isolation. It does not make an owner's homelab public capacity, let clients call vendor APIs, accept an unaudited provider, or silently substitute provider, custody, account, region, data posture, or isolation rung.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8547",
        "github:OpenAgentsInc/openagents#8636",
        "github:OpenAgentsInc/openagents#8749",
        "github:OpenAgentsInc/openagents#8750",
      ],
      contractId: "openagents_cloud.user_or_managed_execution_targets.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/issues/fc-cloud-codex.md",
        "docs/sol/issues/fc-4-hybrid-cloud.md",
        "docs/cloud/ARCHITECTURE.md",
      ],
      oracles: [
        {
          description:
            "Planned target-adapter oracle: enroll and revoke an owner-managed remote node, select OpenAgents-managed capacity, and exercise one separately audited managed-provider adapter behind identical lifecycle/capability/checkpoint/preview/cleanup receipts without exposing vendor APIs or topology to either client.",
          id: "openagents_cloud.user_or_managed_targets.planned",
          kind: "planned",
          mode: "e2e",
          ref: "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        },
      ],
      productArea: "owner-managed and managed-cloud execution targets",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "pending",
      statement: "Remote sessions on my own cloud (my homelab) OR a managed cloud (e.g. Daytona)",
      surface: "openagents-mobile-desktop-cloud",
      verification:
        "Pending #8547/#8636 plus bounded target-adapter leaves: prove a real owner-managed node, the accepted Agent Computer path, and one audited managed-provider adapter through the provider-neutral contract with explicit fallback history and no silent isolation downgrade.",
    },
    {
      authorityBoundary:
        "The broker grants least-privilege capability access to one owner/session/attachment/target/tool/TTL scope. It is not a generic secret tunnel, does not place raw secrets in clients or checkpoints, and does not let a moved session reuse the source attachment's credential material.",
      blockerRefs: [],
      contractId: "openagents_cloud.brokered_session_secrets.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/2026-07-13-port-02-target-scoped-capability-broker-receipt.md",
        "docs/cloud/INVARIANTS.md",
        "docs/ops/2026-07-13-portable-capability-broker-runbook.md",
        "packages/portable-session-contract/src/capability-broker.ts",
      ],
      oracles: [
        {
          description:
            "Executable broker oracle: issue, redeem, renew, revoke, reissue, release, and wipe provider/SCM/tool/API leases across owner-local and accepted managed adapters; reject replay, expiry, outage, denial, and cleanup faults; scan every exported surface for raw material.",
          id: "openagents_cloud.brokered_session_secrets.test",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/portable-session-contract/src/capability-broker.test.ts",
        },
      ],
      productArea: "cross-target secret capability broker",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "enforced",
      statement: "Secrets access via a broker (i.e. gondolin or agyn style)",
      surface: "openagents-pylon-cloud-workrooms",
      verification:
        "PORT-02 enforces target-scoped short-lived leases and injected JIT materialization across owner-local and accepted OpenAgents-managed adapters, including reauthorization, revocation-during-move, lost-ACK replay, expiry, cleanup, outage/denial, and forbidden-material scans. PORT-03 separately proves a real process/session move.",
    },
    {
      authorityBoundary:
        "Mobile receives owner-scoped session, target, capability, freshness, isolation, and command projections only. Voice is an explicit ASR/TTS/barge-in modality over the normal typed policy/approval/outcome path; it does not grant host paths, credentials, vendor APIs, ambient capture, raw-audio retention by default, or voice-only authority, and it does not revive Sarah/avatar/video.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8566",
        "github:OpenAgentsInc/openagents#8597",
        "github:OpenAgentsInc/openagents#8751",
        "github:OpenAgentsInc/openagents#8752",
        "github:OpenAgentsInc/openagents#8753",
      ],
      contractId: "openagents_mobile.any_host_session_voice.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        "docs/sol/2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md",
        "docs/sol/MASTER_ROADMAP.md",
      ],
      oracles: [
        {
          description:
            "Planned physical-device oracle: list and access every authorized adopted session across enrolled host classes, use visible persona-neutral voice for one follow-up or interrupt, request one stop/checkpoint/move/resume transition, reconcile a lost acknowledgement, and prove text fallback, ordinary approvals, no raw-audio retention, and no client secret/vendor authority.",
          id: "openagents_mobile.any_host_session_voice.planned",
          kind: "planned",
          mode: "e2e",
          ref: "docs/sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md",
        },
      ],
      productArea: "mobile any-host session access and conversational voice",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "pending",
      statement:
        "Mobile client which can access any session on any host, with conversational voice",
      surface: "openagents-mobile",
      verification:
        "Pending bounded #8597/#8566 leaves: pass host/session-directory, typed movement, microphone lifecycle, ASR transcript, TTS, barge-in, approval, reconnect, privacy, and physical iOS/Android acceptance oracles against owner-managed and managed targets.",
    },
    {
      authorityBoundary:
        "The verified native session authorizes only the server-derived owner's personal Sync scope. Owner refs, credentials, database handles, transport/session objects, and raw rows remain host-only; authenticated replication substrate does not imply conversation projection, command acceptance, execution, or completion.",
      blockerRefs: [],
      contractId: "openagents_mobile.sync.host_owned_expo_sqlite.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/sync/mobile-sync-host.ts",
        "packages/khala-sync-client/src/expo-sqlite-store.ts",
        "docs/sol/issues/mobile-sync-host.md",
        "docs/sol/issues/native-authenticated-sync-hosts.md",
        "github:OpenAgentsInc/openagents#8657",
      ],
      oracles: [
        {
          description:
            "Proves restart-stable write-once installation identity, authorized personal-scope selection, dynamic token lookup, bounded live/freshness projection, native Expo composition outside the view program, and session-before-store close; the package adapter separately proves durable queue persistence, transaction rollback, and initialization cleanup.",
          id: "openagents_mobile.sync.host_owned_expo_sqlite",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-sync-host.test.ts",
        },
      ],
      productArea: "mobile cross-device continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile owns one private Expo SQLite cache through the shared Khala Sync store core and, only after native-session verification, composes the shared production transport on exactly the server-derived owner's personal scope. It re-reads rotated access custody host-side and closes session-before-store on OTA reload/unmount.",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test apps/openagents-mobile/tests/mobile-sync-host.test.ts plus the khala-sync-client Expo adapter suite prove the authenticated host/storage boundary; mobile OTA and Home tests prove close-before-reload ordering without credential projection.",
    },
    {
      authorityBoundary:
        "The Expo host selects confirmed account-linked Sync or the existing public-local conversation before mounting one Effect Native Home program. The modes are never merged. Runtime commands carry exact confirmed refs through the shared client contract and never imply provider acceptance or completion. Owner refs, credentials, store/session/transport objects, raw rows/provider events, and optimistic completion remain outside view state; denial or sign-out revokes queued hosted commands and clears account-linked projections.",
      blockerRefs: [],
      contractId: "openagents_mobile.chat.authoritative_sync_mode.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/conversation/mobile-conversation.ts",
        "apps/openagents-mobile/src/screens/home-core.ts",
        "apps/openagents-mobile/src/app.tsx",
        "packages/khala-sync-client/src/runtime.ts",
        "packages/khala-sync-client/src/session.ts",
        "docs/sol/issues/native-streamed-conversation-handoff.md",
        "docs/sol/issues/mobile-visible-sync-conversation.md",
        "github:OpenAgentsInc/openagents#8671",
      ],
      oracles: [
        {
          description:
            "Proves bounded live-vs-local selection, confirmed startup reconstruction, stable create/append refs, exact-ref start/follow-up/interrupt through the shared runtime contract, confirmed terminal observation, and pending-reconcile timeout honesty.",
          id: "openagents_mobile.chat.authoritative_sync_adapter",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-conversation.test.ts",
        },
        {
          description:
            "Proves confirmed refs/versions enter the existing Effect Native Home/thread surface, optimistic rows are visibly pending and replaced only by confirmed state, failures remove drafts, and denial clears account-linked projections.",
          id: "openagents_mobile.chat.authoritative_sync_home",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/authoritative-home.test.ts",
        },
      ],
      productArea: "mobile cross-device conversation continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile uses confirmed canonical chat_thread/chat_message plus bounded agent-run timeline projections for its visible Home conversation when verified personal Sync is live. Create, append, same-run follow-up, new start, and exact-run interrupt remain visibly pending until exact stable refs and a later confirmed outcome reconcile; unavailable or timed-out work never appears completed.",
      surface: "openagents-mobile",
      verification:
        "The mobile conversation adapter and authoritative Home tests run in the normal mobile sweep; mobile typecheck plus behavior-contract coverage guard the host/view boundary.",
    },
    {
      authorityBoundary:
        "SecureStore custody protects credential material but does not prove the credential is current, assign identity authority to the client, authorize Sync rows or commands, or make cached state live.",
      blockerRefs: [],
      contractId: "openagents_mobile.session.secure_store_custody.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/auth/native-session-vault.ts",
        "docs/sol/issues/mobile-session-vault.md",
        "github:OpenAgentsInc/openagents#8658",
      ],
      oracles: [
        {
          description:
            "Proves one versioned device-only SecureStore record, exact keychain service/options, schema and epoch validation, malformed-record purge, idempotent clear, bounded recovery classification, and public-safe storage failures.",
          id: "openagents_mobile.session.secure_store_custody",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/native-session-vault.test.ts",
        },
      ],
      productArea: "mobile native session custody",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile keeps native access and refresh tokens in a versioned device-only SecureStore record, purges invalid records, and projects only credential-present-unverified until server validation.",
      surface: "openagents-mobile",
      verification:
        "The native-session-vault and Home view-program tests prove custody, fail-closed recovery, and the no-credential view boundary; mobile typecheck and behavior-contract coverage gate the integration.",
    },
    {
      authorityBoundary:
        "Server verification establishes only a native OpenAgents session. It does not make Khala Sync live, authorize cached rows, create a device_session, execute a command, or expose replacement tokens to Effect Native.",
      blockerRefs: [],
      contractId: "openagents_mobile.session.recovered_validation_rotation.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/auth/mobile-session.ts",
        "apps/openagents-mobile/src/auth/native-session-recovery.ts",
        "docs/sol/issues/mobile-session-recovery.md",
        "github:OpenAgentsInc/openagents#8659",
      ],
      oracles: [
        {
          description:
            "The mobile recovery test proves verification, rotation rewrite, denial and identity-mismatch purge, unavailable retention, and bounded tokenless state.",
          id: "openagents_mobile.session.recovered_validation_rotation",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/native-session-recovery.test.ts",
        },
        {
          description:
            "The Worker boundary test proves only a bounded refresh header on the exact native session GET reaches the existing OpenAuth verifier; other routes and malformed values cannot trigger rotation.",
          id: "openagents_api.session.native_refresh_boundary",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/auth/mobile-session.test.ts",
        },
      ],
      productArea: "mobile native session recovery",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile validates recovered credentials through the native session boundary, persists bounded OpenAuth rotation, purges denial or owner mismatch, and never equates session readiness with live Sync.",
      surface: "openagents-mobile-and-api",
      verification:
        "Worker mobile-session tests plus mobile native-session-recovery and Home tests enforce both sides; API/mobile typechecks and behavior-contract coverage gate the integration.",
    },
    {
      authorityBoundary:
        "A verified native OpenAgents session does not make Khala Sync live, authorize cached rows or commands, create a device_session, or prove physical-device acceptance.",
      blockerRefs: [],
      contractId: "openagents_mobile.session.pkce_sign_in_sign_out.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/auth/native-session-pkce.ts",
        "docs/sol/issues/mobile-session-pkce.md",
        "github:OpenAgentsInc/openagents#8660",
      ],
      oracles: [
        {
          description:
            "Proves the exact public client/provider/S256/canonical redirect, one imperative state-validating request, ephemeral prompt, code exchange, server-derived owner verification, immediate rotation, bounded results, and revocation-before-clear sign-out.",
          id: "openagents_mobile.session.pkce_sign_in_sign_out",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/native-session-pkce.test.ts",
        },
        {
          description:
            "Proves the Effect Native surface renders session entry/exit from honest phases and routes both through typed intents to host-owned session actions.",
          id: "openagents_mobile.session.typed_intents",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/home-shell-core.test.ts",
        },
      ],
      productArea: "mobile native session entry and exit",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents mobile signs in through one state-validating GitHub authorization-code and S256 PKCE request using openagents://auth, verifies the server owner before custody, and revokes both credentials before local sign-out.",
      surface: "openagents-mobile",
      verification:
        "The native PKCE and Home view-program suites enforce the credential and typed-intent boundaries; mobile typecheck and behavior-contract coverage gate the integration.",
    },
    {
      authorityBoundary:
        "This binds sheet-dismissal authority to user intents only; it does not authorize StoreKit purchase flows or change how/when the shell opens the sheet.",
      blockerRefs: [],
      contractId: "openagents_mobile.minerals_sheet_user_dismiss_only.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/home-core.ts",
        "github:OpenAgentsInc/openagents#8648",
      ],
      oracles: [
        {
          description:
            "Drives the real Home view program: with the Buy Minerals sheet open, the AskVideoEnded playback event (playToEnd/loop boundary) and the AskVideoDismissed user video-tap both end the takeover while the sheet stays open; only MineralsSheetDismissed (Not now) or MineralPackSelected closes it.",
          id: "openagents_mobile.minerals_sheet.user_dismiss_only",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/home-shell-core.test.ts",
        },
      ],
      productArea: "mobile minerals purchase sheet",
      source: {
        channel: "owner-testflight-feedback",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "enforced",
      statement:
        "The Buy Minerals Liquid Glass sheet auto-dismisses when the background reply video ends/loops. Wrong. The sheet must stay open until the USER dismisses it (selecting a price pack or Not now).",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test apps/openagents-mobile/tests/home-shell-core.test.ts proves the sheet survives video-ended and video-tap-dismiss events and closes only on the user's pack-selection or Not-now intents; the simulator pixel proof on #8648 shows the sheet still open past the video loop boundary.",
    },
    {
      authorityBoundary:
        "This binds the text-first conversation floor only; voice/avatar tiers follow #8610 capacity policy, account linking unlocks operator posture only through server-owned policy, and the bundled demo video is ambient presentation — never conversation evidence.",
      blockerRefs: [],
      contractId: "openagents_mobile.sarah_text_surface.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/sarah-core.ts",
        "apps/openagents-mobile/src/sarah/sarah-client.ts",
        "github:OpenAgentsInc/openagents#8649",
      ],
      oracles: [
        {
          description:
            "Drives the real Home view program with a deterministic turn client and the real render-rn lowering: typed turn round-trips (submit -> user + thinking -> done reply), typed SSE transcript/card events with bounded dedupe and typed reconnect phases, honest typed degradation on turn/session failure with the composer alive, turn-bootstrap session adoption, persisted-session restore marking continuity, and the SSE frame parser contract.",
          id: "openagents_mobile.sarah_text_surface.view_program",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/sarah-surface.test.ts",
        },
      ],
      productArea: "mobile Sarah conversation surface",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "retired",
      statement:
        "The owner wants Sarah consumable in OpenAgents mobile with the native glass shell as soon as possible. V1 is the text availability floor over the same /sarah contracts as web: prospect/authenticated session, bounded SSE transcript, composer turns, and typed cards inside the GL-2 shell.",
      surface: "openagents-mobile",
      verification:
        "Retired by the 2026-07-10 surface-removal decision. The 2026-07-18 reboot is a different authenticated owner-orchestrator contract: it preserves neither the public /sarah endpoint nor the prospect/SSE/avatar state model.",
    },
    {
      authorityBoundary:
        "Sarah is an owner-authenticated principal projected into the existing mobile conversation system. Read access is owner-scoped and redacted; mutations require an exact admitted capability, root and Sarah authority grants, runtime gates, and receipts. Raw secrets, custody, legal/employment commitments, destructive customer-data actions, invariant weakening, self-amplification, unsupported claims, and stable releases without current direction remain reserved.",
      blockerRefs: [],
      contractId: "openagents_mobile.sarah_owner_orchestrator.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/sarah/src/index.ts",
        "packages/authority/src/index.ts",
        "apps/openagents.com/workers/api/src/sarah-owner-routes.ts",
        "apps/openagents.com/workers/api/src/sarah-runtime-tools.ts",
        "apps/openagents.com/workers/api/src/cloudrun/sarah-realtime-bridge.ts",
        "apps/openagents-mobile/src/screens/sarah-voice-screen.tsx",
        "apps/openagents-mobile/src/sarah-voice/client.ts",
      ],
      oracles: [
        {
          description:
            "Proves the authenticated route returns one opaque stable owner thread with durable cited memory and the admitted authority revision.",
          id: "openagents_mobile.sarah_owner_orchestrator.route",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/sarah-owner-routes.test.ts",
        },
        {
          description:
            "Proves the mobile command-center profile bootstraps the canonical owner thread, advertises only receipted server-owned delegation and existing Full Auto tools, renders bounded tool activity, sends heartbeats, and retains the voice-only fallback without granting editor or device authority.",
          id: "openagents_mobile.sarah_owner_orchestrator.voice_command_center",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/sarah-voice-client.test.ts",
        },
        {
          description:
            "Proves command-center tools execute through Sarah's existing authority receipts and typed coding/Full Auto brokers rather than through the phone or Omega editor protocol.",
          id: "openagents_mobile.sarah_owner_orchestrator.server_broker",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/cloudrun/sarah-realtime-bridge.test.ts",
        },
      ],
      productArea: "owner orchestration and business continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-18",
      },
      state: "enforced",
      statement:
        "Create an authority delegation to Sarah. Bring her back so I have a single point of contact with full persistent memory, tie her into the OpenAgents mobile app, and let me ask for release status, who is saying what, and the state of the business. Sarah is the main decision maker and orchestrator under delegated authority. Next, add full mobile support so I can do all this via the Open Agents mobile app. Talk to Sarah. Similar controls there.",
      surface: "openagents-mobile",
      verification:
        "The owner route, realtime gateway, mobile voice client, and Sarah runtime-tool tests prove the canonical owner-private thread, brokered coding delegation, visible activity, existing Full Auto control intents, heartbeat/reconnect behavior, and the absence of mobile editor/device authority.",
    },
    {
      authorityBoundary:
        "This contract binds what the owner-private room may claim about its own completeness. It is a reporting obligation, not a delivery guarantee: it does not promise every source reaches the device, and a withheld count is not authority to hide a record. The host may only state causes it can observe, so a device-side read failure is counted on the device and can never be asserted by a host.",
      blockerRefs: [],
      contractId: "openagents_mobile.owner_private_source_coverage.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "packages/sarah/src/issue31-nostr/withheld-sources.ts",
        "packages/sarah/fixtures/issue31-nostr/openagents.omega.issue31.withheld_sources.v1.canonical-partial.json",
        "apps/openagents-mobile/src/workroom/issue31-owner-private-read-model.ts",
        "omega:crates/omega_effectd/src/issue31_nostr.rs",
        "omega:crates/omega_effectd/src/sarah_conversation.rs",
        "github:OpenAgentsInc/omega#46",
        "github:OpenAgentsInc/omega#49",
      ],
      oracles: [
        {
          description:
            "Read-model proof of all three withholding paths, each with its own assertion: a host quarantine arrives as an exact count with a reason, the bounded projection scan arrives as a lower bound that is never rendered exact, an engram this device cannot read is counted rather than dropped in silence, a device holding no statement reads unknown rather than complete, and rows that did arrive still render beside the count that did not.",
          id: "openagents_mobile.owner_private_source_coverage.read_model",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/issue31-owner-private-read-model.test.ts",
        },
        {
          description:
            "Delivery proof over a real relay: the coverage statement is a real NIP-59 gift wrap the relay stores and serves, unwrapped and NIP-44 decrypted on the device from the byte-shared fixture the Omega host emits, and a newer statement of completeness clears the gap. Green against the in-process startTestRelay and against the deployed OpenAgents relay.",
          id: "openagents_mobile.owner_private_source_coverage.wire",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/issue31-owner-private-wire.test.ts",
        },
        {
          description:
            "Record-contract proof from the shared bytes: a complete statement and a partial statement are different records, a complete coverage over a non-empty count list is refused, an exact scan-bound count is refused, a zero count is refused, and a device-observed cause is not assertable by a host.",
          id: "openagents_mobile.owner_private_source_coverage.record",
          kind: "bun-test",
          mode: "unit",
          ref: "packages/sarah/src/issue31-nostr/issue31-nostr.test.ts",
        },
      ],
      productArea: "owner-private memory inspection",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-24",
      },
      state: "enforced",
      statement: "The owner can inspect every engram available to Sarah.",
      surface: "openagents-mobile",
      verification:
        "The read-model, wire, and record oracles above run in the normal sweep. The Omega host half is proven in OpenAgentsInc/omega by cargo test -p omega_effectd, which pins the same fixture digests, so a one-sided edit to what the phone is told fails in both repositories. A physical-device rendering of the coverage line is not claimed here and stays on omega#49.",
    },
    {
      authorityBoundary:
        "This contract binds display text only. Wire tokens keep their exact machine form: the pairing scope observe_issue31 is validated byte-for-byte by shipped mobile builds (TestFlight build 126), and the openagents.omega.issue31.* schema ids and refs are contract identifiers. None of them may be shown raw to a person; renaming a wire value is a separate compatibility decision this contract does not authorize.",
      blockerRefs: [],
      contractId: "openagents_mobile.no_internal_issue_references.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/screens/omega-bridge-session.ts",
        "packages/sarah/src/issue31-nostr/records.ts",
        "omega:crates/omega_deltas/src/omega_deltas.rs",
      ],
      oracles: [
        {
          description:
            "productSafeNotice passes product wording through untouched and collapses any message carrying the internal issue codename — the raw observe_issue31 scope token, an internal-jargon error sentence, or a schema id quoted by a decode failure — to product words before a person reads it.",
          id: "openagents_mobile.no_internal_issue_references.notice",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/omega-bridge-session.test.ts",
        },
      ],
      productArea: "desktop mirror pairing and status",
      source: {
        channel: "chat",
        statedBy: "owner",
        statedOn: "2026-07-28",
      },
      state: "enforced",
      statement:
        "WHAT THE FUCK IS THIS ISSUE 31 THING, FIX IT NOW AND NEVER EVER SHOW ISSUE 31.",
      surface: "openagents-mobile",
      verification:
        "The notice oracle runs in the normal sweep, and every prose error string in packages/sarah and the mobile app now names the product (device mirror, device pairing) instead of the issue number. The desktop half is enforced in OpenAgentsInc/omega by OMEGA-DELTA-0168 (internal_issue_references_never_render), which scans every string literal in crates/ for the prose form.",
    },
    {
      authorityBoundary:
        "Owner scoping binds the Worker portal API (/api/portal/*): engagement reads resolve only through the caller's verified session identity, and admin creation/binding/seeding stays behind the operator bearer token. This contract does not authorize any client-facing engagement-id lookup route.",
      blockerRefs: [],
      contractId: "openagents_web.portal_owner_scoped_engagement.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/portal-routes.ts",
        "apps/openagents.com/workers/api/migrations/0315_portal_engagements_and_content_items.sql",
        "github:OpenAgentsInc/openagents#8652",
      ],
      oracles: [
        {
          description:
            "Route-level isolation proof against the real 0315 migration schema: a second client (different user id, same or different email) reads engagement:null, cannot decide the first client's content item (404, no existence leak, item stays draft), and a bound client_user_id is authoritative over any email match.",
          id: "openagents_web.portal_owner_scoping.routes",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/portal-routes.test.ts",
        },
        {
          description:
            "The /portal surface is login-gated: the server-rendered loading and logged-out phases render only the login gate (never engagement content, identity, or receipt markup), and the surface offers no foreign-engagement lookup — it can only fetch the caller's own engagement.",
          id: "openagents_web.portal_owner_scoping.surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
        },
      ],
      productArea: "client portal engagement access",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "Clients see only their own engagement. Owner-scoped fail-closed: a client can NEVER read another engagement.",
      surface: "openagents-web",
      verification:
        "pnpm --dir apps/openagents.com/workers/api run test -- src/portal-routes.test.ts proves cross-client isolation against the real migration schema; pnpm --dir apps/openagents.com/apps/start run test -- src/routes/-portal.test.tsx proves the login gate and own-engagement-only surface.",
    },
    {
      authorityBoundary:
        "Receipts bind the decision write only: a decision receipt does not mark content as published, does not authorize publishing automation, and never flips after minting (idempotent repeats return the same receipt; opposite decisions are refused).",
      blockerRefs: [],
      contractId: "openagents_web.portal_decision_receipts.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/portal-store.ts",
        "github:OpenAgentsInc/openagents#8652",
      ],
      oracles: [
        {
          description:
            "Store + route proof: approve and reject each mint an immutable portal_content_decision:<id> receipt with decided_at, idempotent same-decision repeats return the identical receipt, and flipping a decided item is refused with a typed 422.",
          id: "openagents_web.portal_decision_receipts.routes",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/portal-routes.test.ts",
        },
        {
          description:
            "Surface proof: approve/reject post the decision to the owner-scoped API, the optimistic card state commits on success with the minted receipt ref rendered inline, and a failed decision rolls the item back to draft with no receipt invented.",
          id: "openagents_web.portal_decision_receipts.surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
        },
      ],
      productArea: "client portal content decisions",
      source: {
        channel: "issue",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement: "Decisions always produce receipts.",
      surface: "openagents-web",
      verification:
        "pnpm --dir apps/openagents.com/workers/api run test -- src/portal-routes.test.ts proves receipt minting, idempotency, and immutability; pnpm --dir apps/openagents.com/apps/start run test -- src/routes/-portal.test.tsx proves the rendered receipt ref and optimistic rollback.",
    },
    {
      authorityBoundary:
        "Presentation-only guarantee over the authenticated /portal empty state: it names the caller's own session identity (email, else provider login, else an honest fallback) and links the existing /logout route. It grants no engagement access, adds no lookup route, and never renders anyone else's identity.",
      blockerRefs: [],
      contractId: "openagents_web.portal_empty_state_account_identity.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/apps/start/src/routes/-portal-core.ts",
        "github:OpenAgentsInc/openagents#8652",
      ],
      oracles: [
        {
          description:
            "Server-render + DOM proof: the authenticated empty state renders 'Signed in as <session email>' (login fallback, honest no-email fallback — never blank), the different-email guidance, and a 'Sign out / switch account' affordance targeting /logout.",
          id: "openagents_web.portal_empty_state_identity.surface",
          kind: "bun-test",
          mode: "dom",
          ref: "apps/openagents.com/apps/start/src/routes/-portal.test.tsx",
        },
      ],
      productArea: "client portal engagement access",
      source: {
        channel: "session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "Owner, 2026-07-10, after seeing only 'Your setup is being prepared' on /portal while logged in with no engagement, no account context, and no way to log in or switch: \"it will [go out] when it actually works... theres something horribly missing about your QA process that you would put this in front of me as ready for testing.\" The authenticated empty state must always show WHICH account/email the caller is signed in as, say that an engagement set up under a different email is the likely cause, and offer a sign-out/switch-account affordance.",
      surface: "openagents-web",
      verification:
        "pnpm --dir apps/openagents.com/apps/start run test -- src/routes/-portal.test.tsx proves the signed-in identity line, the fallback chain, the different-email guidance, and the /logout affordance on the empty state; the #8652 reopen receipts carry the deployed browser screenshots (logged out, logged in without engagement, logged in with engagement).",
    },
    {
      authorityBoundary:
        "Re-evaluation only reads confirmed personal-scope rows once the scope reports the live phase; it never fabricates a conversation, never creates or duplicates a thread, and does not make cached or pre-live state authoritative. The account control is a phase-derived affordance, not an authorization decision.",
      blockerRefs: [],
      contractId: "openagents_mobile.chat.post_auth_live_upgrade.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-mobile/src/app.tsx",
        "apps/openagents-mobile/src/conversation/mobile-experience-reconciler.ts",
        "apps/openagents-mobile/src/screens/home-core.ts",
        "github:OpenAgentsInc/openagents#8676",
        "github:OpenAgentsInc/openagents#8689",
        "github:OpenAgentsInc/openagents#8677",
      ],
      oracles: [
        {
          description:
            "Proves the pre-live read stays local, a live scope upgrades the selection to sync exactly once (authority sync, 'OpenAgents' pill, 'Continue conversation' composer) with no duplicate conversation, the genuine local fallback is preserved when the scope never becomes live, and a closed reconciler never upgrades.",
          id: "openagents_mobile.chat.post_auth_live_upgrade.reconciler",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-experience-reconciler.test.ts",
        },
        {
          description:
            "Proves every confirmed post-authentication phase (session_ready, bootstrapping, catching_up, live, must_refetch, stale) renders 'Sign out', genuinely unauthenticated phases render 'Link OpenAgents account', and an in-flight authenticating step renders neither.",
          id: "openagents_mobile.chat.post_auth_live_upgrade.account_control",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-mobile/tests/mobile-account-control.test.ts",
        },
      ],
      productArea: "mobile cross-device conversation continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-11",
      },
      state: "enforced",
      statement:
        "Owner, 2026-07-11, after seeing the mobile app stuck on the local 'Message Khala' Khala surface with a 'Link OpenAgents account' button while the OpenAgents status surface already read 'Sync live': the visible conversation authority must re-evaluate when the verified personal scope reaches the live phase — upgrading the Khala surface from the pre-live local fallback to the confirmed sync conversation (title 'OpenAgents', 'Continue conversation' composer) exactly once and without inventing or duplicating a conversation — while a scope that never becomes live stays local; and the OpenAgents account control must read 'Sign out' for every confirmed post-authentication phase (session_ready, bootstrapping, catching_up, live, must_refetch, stale) and read 'Link OpenAgents account' only for genuinely unauthenticated phases.",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test --cwd apps/openagents-mobile runs the reconciler and account-control oracles in the normal mobile sweep; mobile typecheck plus behavior-contract coverage guard the phase-to-authority and phase-to-account-control boundaries.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-27.1",
};
