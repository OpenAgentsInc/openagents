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
  ],
  schemaVersion: BehaviorContractSchemaVersion,
  version: "2026-07-09.2",
}
