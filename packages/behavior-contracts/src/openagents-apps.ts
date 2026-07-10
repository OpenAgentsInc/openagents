import {
  BehaviorContractSchemaVersion,
  type BehaviorContractRegistryDocument,
} from "./contract"

/**
 * Pending owner contracts for the greenfield OpenAgents mobile/desktop apps.
 * These live in the shared registry until each new app exists and can own an
 * enforced registry plus executable identity, security, and cross-device
 * oracles.
 */
export const openAgentsAppsContractRegistry: BehaviorContractRegistryDocument = {
  contracts: [
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
        "clients/khala-mobile/assets/images/icon.png",
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
      state: "pending",
      statement:
        "All Khala Code ideas are to be folded into the Sarah-first OpenAgents app.",
      surface: "openagents-mobile-and-desktop",
      verification:
        "Pending #8566/#8574/#8597: every Khala Code idea has an explicit fold-into-Sarah, retain-as-OpenAgents-capability, or extract-as-shared-engine disposition; only its superseded legacy implementation may retire, and one Sarah/FleetRun continues across all retained apps without a Khala Code product shell.",
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
        "bun test apps/openagents-mobile/tests/home-shell-core.test.ts proves the sheet survives video-ended and video-tap-dismiss events and closes only on the user's pack-selection or Not-now intents; the simulator pixel proof on #8648 shows the sheet still open past the video loop boundary.",
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
        "bun test apps/openagents-mobile/tests/sarah-surface.test.ts proves the view-program contract; the #8649 receipt carries the production pixel proof (real prospect session + live Sarah reply in the shell) and the restart-persistence + reconnect evidence.",
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
        "bun run --cwd apps/openagents.com/workers/api test -- src/portal-routes.test.ts proves cross-client isolation against the real migration schema; bun run --cwd apps/openagents.com/apps/start test -- src/routes/-portal.test.tsx proves the login gate and own-engagement-only surface.",
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
        "bun run --cwd apps/openagents.com/workers/api test -- src/portal-routes.test.ts proves receipt minting, idempotency, and immutability; bun run --cwd apps/openagents.com/apps/start test -- src/routes/-portal.test.tsx proves the rendered receipt ref and optimistic rollback.",
    },
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-10.2",
}
