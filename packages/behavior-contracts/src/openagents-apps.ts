import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"
import { audioBehaviorContracts } from "./audio"

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
        "This binds the visible activity indicator to the renderer's existing working phase and valid browser animation state. It does not redefine turn, queue, retry, or completion authority, and reduced-motion preference intentionally replaces motion with a stable visible indicator.",
      blockerRefs: [],
      contractId: "openagents_desktop.working_indicator_continuous_motion.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents-desktop/src/renderer/react-timeline.tsx",
        "packages/ui/src/desktop-workbench.css",
        "apps/openagents-desktop/tests/working-indicator-motion.e2e.test.ts",
        "github:OpenAgentsInc/openagents#8930",
      ],
      oracles: [
        {
          description:
            "Real Chromium DOM/WAAPI oracle: while the working indicator is mounted, all three bars have the named animation, remain in running play state, and advance between two samples; under prefers-reduced-motion they remain visibly mounted while animation is absent.",
          id: "openagents_desktop.working_indicator.motion_runtime",
          kind: "bun-test",
          mode: "e2e",
          ref: "apps/openagents-desktop/tests/working-indicator-motion.e2e.test.ts",
        },
      ],
      productArea: "Desktop conversation activity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-16",
      },
      state: "enforced",
      statement: "[working] bars stopped animating — fix it, in ~/work/openagents",
      surface: "openagents-desktop",
      verification:
        "The normal Desktop test sweep runs working-indicator-motion.e2e.test.ts against the production shared CSS in real Chromium, asserting accessible working state, three live advancing animations, and the explicit reduced-motion exception.",
    },
    {
      authorityBoundary:
        "This selects the product shell and host; it does not authorize release before #8574's signing, security, migration, and clean-machine gates pass.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8574"],
      contractId: "openagents_apps.greenfield_desktop_electron.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
        "docs/sol/issues/app-desktop.md",
      ],
      oracles: [
        {
          description:
            "Planned scaffold and security oracle proving Electron, Effect Native, and no legacy Electrobun app import.",
          id: "openagents_desktop.greenfield_electron.planned",
          kind: "planned",
          mode: "headless",
          ref: "github:OpenAgentsInc/openagents#8574",
        },
      ],
      productArea: "desktop application architecture",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        "Deprecate the Khala Code Desktop electrobun app and mobile app. I want a new OpenAgents desktop app to be Electron.",
      surface: "openagents-desktop",
      verification:
        "Pending #8574: verify the new app root uses Electron + Effect Native, passes the secure IPC oracle, and does not import or release the deprecated Electrobun client.",
    },
    {
      authorityBoundary:
        "Template selection does not authorize copying unsafe defaults, retaining a second UI architecture, or publishing against the template owner's update repository.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8574"],
      contractId: "openagents_apps.desktop_starting_template.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "https://github.com/LuanRoger/electron-shadcn",
        "docs/sol/2026-07-09-greenfield-mobile-desktop-decision.md",
        "docs/sol/issues/app-desktop.md",
      ],
      oracles: [
        {
          description:
            "Planned provenance oracle for the pinned template commit plus required security and Effect Native adaptations.",
          id: "openagents_desktop.template_provenance.planned",
          kind: "planned",
          mode: "headless",
          ref: "github:OpenAgentsInc/openagents#8574",
        },
      ],
      productArea: "desktop scaffold provenance",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-09",
      },
      state: "pending",
      statement:
        "Update anything re desktop relevant to reference https://github.com/LuanRoger/electron-shadcn as the starting template the desktop app must use.",
      surface: "openagents-desktop",
      verification:
        "Pending #8574: the new app records its imported electron-shadcn commit and MIT attribution, retains the Forge/Vite/fuse/test bootstrap, removes the template updater/publisher wiring, asserts nodeIntegration=false and sandbox=true, verifies packaged fuses, and mechanically replaces starter application semantics with Effect Native/Effect Schema.",
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
        "the mobile app -- which should be also built from scratch -- must use the existing app identifier \"com.openagents.app\" (it's called \"OpenAgents\") and that should use the same app icon Khala Code mobile now does.",
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
          description:
            "Planned capability-disposition and cross-device Sarah/FleetRun oracle.",
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
      statement:
        "All Khala Code ideas are to be folded into the Sarah-first OpenAgents app.",
      surface: "openagents-mobile-and-desktop",
      verification:
        "Retired by the 2026-07-10 owner decision that removed Sarah as a product surface. The preserved capability-disposition requirement continues under openagents_apps.desktop_runtime_and_early_mobile_sync.v1 and MASTER_ROADMAP R0–R7.",
    },
    {
      authorityBoundary:
        "This fixes Desktop process/data boundaries and makes early mobile continuation part of the first real conversation exit. It does not authorize renderer-held credentials, mobile local-filesystem or shell authority, a second Pylon/run universe, optimistic completion claims, or release before R7.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8574",
        "github:OpenAgentsInc/openagents#8597",
      ],
      contractId: "openagents_apps.desktop_runtime_and_early_mobile_sync.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "docs/sol/2026-07-10-openagents-desktop-product-architecture.md",
        "docs/sol/2026-07-10-r1-r2-identity-sync-contract.md",
        "docs/sol/MASTER_ROADMAP.md",
        "docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md",
        "docs/sol/issues/native-streamed-conversation-handoff.md",
        "packages/khala-sync-server/src/runtime-mutators.test.ts",
        "apps/openagents-desktop/tests/runtime-gateway.e2e.test.ts",
        "apps/openagents-mobile/tests/mobile-conversation.test.ts",
      ],
      oracles: [
        {
          description:
            "Planned cross-client oracle: a tokenless Desktop renderer drives one real streamed durable thread through the host-owned runtime gateway; mobile observes matching thread/message refs, versions, phases, and terminal outcome, submits one safe follow-up or interrupt, and both clients reconcile across restart, revocation, cursor gap, duplicate delivery, and a lost acknowledgement without invented completion.",
          id: "openagents_apps.desktop_runtime_mobile_sync.planned",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8574",
        },
      ],
      productArea: "Desktop runtime architecture and cross-device continuity",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "pending",
      statement:
        "Let's get the desktop architecture dialed in solidly in place, with mobile sync working soon in that process but otherwise plan to get your planned openagents product adaptation working fastest.",
      surface: "openagents-mobile-and-desktop",
      verification:
        "The deterministic #8676 slice now enforces exact durable runtime→agent-run binding, protocol-v6 tokenless Desktop projection, same-thread mobile start/follow-up/interrupt, restart reconstruction, and revoke-without-replay. This program contract remains pending until the public-safe live receipt proves one named isolated provider account in built Electron and one physical mobile continuation.",
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
      statement:
        "Remote sessions on my own cloud (my homelab) OR a managed cloud (e.g. Daytona)",
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
        "This registers only the Desktop public-client authorization redirect policy. It does not launch a browser, accept a callback, exchange a code, authenticate the renderer, make Sync live, or freeze package identity.",
      blockerRefs: [],
      contractId: "openagents_desktop.session.loopback_pkce_policy.v1",
      enforcementTier: "test-sweep",
      evidenceRefs: [
        "apps/openagents.com/workers/api/src/auth/mobile-session.ts",
        "docs/sol/issues/desktop-session-loopback-policy.md",
        "github:OpenAgentsInc/openagents#8663",
      ],
      oracles: [
        {
          description:
            "Proves the distinct Desktop public client accepts only literal IPv4 loopback, a required ephemeral port, exact callback path, GitHub code + S256, and no userinfo/query/fragment while preserving web/mobile redirect behavior.",
          id: "openagents_desktop.session.loopback_pkce_policy",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents.com/workers/api/src/auth/mobile-session.test.ts",
        },
      ],
      productArea: "Desktop native OpenAuth entry",
      source: {
        channel: "owner-codex-session",
        statedBy: "owner",
        statedOn: "2026-07-10",
      },
      state: "enforced",
      statement:
        "OpenAgents Desktop uses the distinct public client openagents-desktop with an RFC 8252 literal-loopback callback and GitHub authorization-code + S256 PKCE only; it never claims the mobile custom scheme.",
      surface: "openagents-desktop-and-api",
      verification:
        "The Worker native-session policy suite and API typecheck enforce the registered redirect boundary; behavior-contract validation gates its evidence record.",
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
      state: "enforced",
      statement:
        "The owner wants Sarah consumable in OpenAgents mobile with the native glass shell as soon as possible. V1 is the text availability floor over the same /sarah contracts as web: prospect/authenticated session, bounded SSE transcript, composer turns, and typed cards inside the GL-2 shell.",
      surface: "openagents-mobile",
      verification:
        "pnpm exec vp test apps/openagents-mobile/tests/sarah-surface.test.ts proves the view-program contract; the #8649 receipt carries the production pixel proof (real prospect session + live Sarah reply in the shell) and the restart-persistence + reconnect evidence.",
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
            "The /portal Effect Native surface is login-gated: logged-out renders only the login gate (never engagement content), and the surface offers no foreign-engagement lookup — it can only fetch the caller's own engagement.",
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
            "Surface proof: approve/reject dispatch typed intents, the optimistic card state commits on success with the minted receipt ref rendered inline, and a failed decision rolls the item back to draft.",
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
            "View + DOM proof: the authenticated empty state renders 'Signed in as <session email>' (login fallback, honest no-email fallback — never blank), the different-email guidance, and a 'Sign out / switch account' affordance targeting /logout.",
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
    {
      authorityBoundary:
        "This binds only the launch entry point (a lightning-bolt Full Auto action beside/under New session in the left rail). It does not itself define the run's lifecycle state machine (see full_auto_play_pause_stop_lifecycle.v1) or the read-only run view's contents (see full_auto_read_only_run_view.v1), and it grants no release or public-claim authority.",
      blockerRefs: ["github:OpenAgentsInc/openagents#8974"],
      contractId: "openagents_desktop.full_auto_dedicated_launcher.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "specs/desktop/full-auto.product-spec.md",
        "docs/fable/2026-07-17-full-auto-implementation-audit.md",
        "github:OpenAgentsInc/openagents#8968",
        "github:OpenAgentsInc/openagents#8974",
      ],
      oracles: [
        {
          description:
            "Planned real-Chromium DOM oracle proving the left rail renders a Full Auto launcher action beside/under New session, that Start collects title/objective/done-condition/workspace/provider-lane/turn-cap before it is enabled, and that Start applies the same workspace-authority refusal rule as the existing control-API start.",
          id: "openagents_desktop.full_auto_dedicated_launcher.planned",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8974",
        },
      ],
      productArea: "Desktop Full Auto launch surface",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-17",
      },
      state: "pending",
      statement:
        "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
      surface: "openagents-desktop",
      verification:
        "Planned: a real-Chromium e2e oracle under FA-UX-01 (#8974) drives the left rail, opens the launcher, fills the mission-contract fields, and asserts Start is disabled until required fields are present and refuses on a workspace mismatch exactly like the existing control-API start (FA-AC-28/FA-AC-54 in the ProductSpec).",
    },
    {
      authorityBoundary:
        "This binds only the visible run view while a Full Auto run is active: pinned objective/workspace, explicit lifecycle state, an inspectable per-turn transcript, and the absence of the ordinary chat composer. It does not grant live token-streaming, steering, or any release/public-claim authority, and it does not itself define Play/Pause/Stop transition legality (see full_auto_play_pause_stop_lifecycle.v1).",
      blockerRefs: ["github:OpenAgentsInc/openagents#8974"],
      contractId: "openagents_desktop.full_auto_read_only_run_view.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "specs/desktop/full-auto.product-spec.md",
        "docs/fable/2026-07-17-full-auto-implementation-audit.md",
        "github:OpenAgentsInc/openagents#8968",
        "github:OpenAgentsInc/openagents#8974",
      ],
      oracles: [
        {
          description:
            "Planned real-Chromium DOM oracle proving that once a run Starts, the main canvas renders a dedicated read-only run view (pinned objective/workspace, an explicit lifecycle state -- not a generic failure banner -- and an inspectable per-turn transcript) and that the ordinary chat composer, its Full Auto toggle, and its manual-send fencing are absent while the run is active.",
          id: "openagents_desktop.full_auto_read_only_run_view.planned",
          kind: "planned",
          mode: "e2e",
          ref: "github:OpenAgentsInc/openagents#8974",
        },
      ],
      productArea: "Desktop Full Auto run view",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-17",
      },
      state: "pending",
      statement:
        "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
      surface: "openagents-desktop",
      verification:
        "Planned: a real-Chromium e2e oracle under FA-UX-01 (#8974) asserts the read-only run view renders explicit lifecycle state (never a generic 'Turn failed' banner, closing the 2026-07-17 audit's observability gap) and that the ordinary composer, toggle, and badge (FA-AC-01/19/21 in the ProductSpec) are retired from the chat surface once the run view ships (FA-AC-55/FA-AC-56).",
    },
    {
      authorityBoundary:
        "This binds the lifecycle state machine and its Play/Pause/Stop (Resume/Pause/Stop) transition legality and attribution. It grants no autonomous provider-selection, mid-run steering, or concurrent multi-run authority, and it does not itself verify that a run's stated done condition was actually satisfied -- Completed remains a self-reported, owner-reviewable disposition.",
      blockerRefs: [
        "github:OpenAgentsInc/openagents#8969",
        "github:OpenAgentsInc/openagents#8974",
      ],
      contractId: "openagents_desktop.full_auto_play_pause_stop_lifecycle.v1",
      enforcementTier: "unenforced",
      evidenceRefs: [
        "specs/desktop/full-auto.product-spec.md",
        "docs/fable/2026-07-17-full-auto-implementation-audit.md",
        "github:OpenAgentsInc/openagents#8968",
        "github:OpenAgentsInc/openagents#8969",
        "github:OpenAgentsInc/openagents#8974",
      ],
      oracles: [
        {
          description:
            "FA-RUN-01 (#8969) landed: unit coverage over the full Draft/Running/Pausing/Paused/Retrying/Stalled/Completed/Failed/Stopped/Cap-reached lifecycle state machine, its exhaustive legal-transition matrix, the v1 one-active-run-per-profile concurrency policy, rerun/new-generation semantics, and the additive legacy-registry migration -- Pause with an active turn transitions through Pausing to Paused only once that turn resolves; Pause with no turn in flight transitions directly to Paused; Resume is legal only from Paused and dispatches exactly once; Stop is terminal, legal from any non-terminal state, and distinct from Pause (never resumable); and every transition persists actor/timestamp/typed reason.",
          id: "openagents_desktop.full_auto_play_pause_stop_lifecycle.run_model",
          kind: "bun-test",
          mode: "unit",
          ref: "apps/openagents-desktop/tests/full-auto-run-registry.test.ts",
        },
        {
          description:
            "Planned: FA-UX-01 (#8974) wires the read-only run view's Pause/Resume/Stop controls to the FA-RUN-01 transitions this contract already binds, so the full owner-facing statement (named launch, explicit objective/done condition, Play/Pause/Stop UI, read-only running view) is provable end to end.",
          id: "openagents_desktop.full_auto_play_pause_stop_lifecycle.planned",
          kind: "planned",
          mode: "unit",
          ref: "github:OpenAgentsInc/openagents#8974",
        },
      ],
      productArea: "Desktop Full Auto run lifecycle",
      source: {
        channel: "github-issue",
        statedBy: "owner",
        statedOn: "2026-07-17",
      },
      state: "pending",
      statement:
        "Full Auto is a named, durable autonomous run launched separately from ordinary chat, with an explicit objective/done condition, Play/Pause/Stop semantics, a read-only running view, run-level liveness/reporting, and provider-handoff evidence.",
      surface: "openagents-desktop",
      verification:
        "FA-RUN-01 (#8969) landed: the durable FullAutoRun lifecycle state machine (FA-AC-43/FA-AC-44/FA-AC-45 in the ProductSpec), its control-API Pause/Resume/Stop routes, and the legacy-registry migration are unit- and route-tested (apps/openagents-desktop/tests/full-auto-run-registry.test.ts, apps/openagents-desktop/src/full-auto-run-control-server.test.ts). Still pending: FA-UX-01 (#8974) wiring those exact typed transitions into the read-only run view's Pause/Resume/Stop controls -- an illegal transition (e.g. Resume from a non-Paused state) must be refused, never silently coerced, and this contract stays `pending` (not `enforced`) until that UI wiring lands.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-17.1",
}
