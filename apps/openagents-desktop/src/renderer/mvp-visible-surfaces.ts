/**
 * MVP visible-surface allowlist — the machine-checkable form of
 * `openagents_desktop.mvp.visible_surface_allowlist.v1` (UX-4, #8790).
 *
 * Owner statement (rc.10 review, 2026-07-14, verbatim): "This menu, when I
 * click the settings button, looks horrible. This folder thing looks
 * horrible. I thought we made a pass removing all screens that are not
 * specifically called for in the MVP. You need to clean all this up and make
 * a pass to remove everything from the sidebar and all UI that's not
 * specifically called for in our MVP spec."
 *
 * Authority: the admitted MVP scope, the Sol MASTER_ROADMAP MVP section, the
 * owner-issued MAINT-1 Settings harness-maintenance control (#8785), and the
 * 2026-07-14 owner direction that ProductSpec and AssuranceSpec remain
 * implementation tooling rather than user-facing Desktop destinations.
 *
 * This module is not a static wish list: `desktopMvpSurfaceViolations` walks
 * the ACTUAL rendered shell view tree and reports every dock item or screen
 * surface outside the allowlist. The test suite renders every reachable
 * workspace state through `desktopShellView` and fails on any violation, and
 * proves the oracle rejects a planted non-MVP surface.
 */
import type { View } from "@effect-native/core"

/**
 * The exact ordered sidebar dock composition, each entry with its MVP
 * authority. Nothing else may render in the dock.
 */
export const mvpDockSurfaces = [
  {
    id: "workspace-new-chat",
    authority: "ProductSpec Scope: session catalog with new, resume, fork, archive, delete",
  },
  {
    id: "workspace-chat",
    authority: "ProductSpec Scope + CW-AC-10/CW-AC-11: session navigation and the typed causal timeline",
  },
  {
    id: "workspace-home",
    authority: "CW-AC-03: explicit repository grant and stable coding-session home",
  },
  {
    id: "shell-settings-toggle",
    authority: "CW-AC-01/02 session truth, CW-AC-18 update/rollback, CW-AC-17 diagnostics, CW-AC-12 keyboard bindings, MAINT-1 #8785 harness maintenance",
  },
] as const

export const mvpAllowedDockItemIds: ReadonlyArray<string> = mvpDockSurfaces.map(surface => surface.id)

/**
 * Removed from the dock by UX-4. These surfaces either stay reachable through
 * the closed command registry only (CW-AC-12 palette/native-menu/deep-link
 * identities) or must not render at all.
 */
export const mvpRemovedDockItemIds: ReadonlyArray<string> = [
  // ProductSpec and AssuranceSpec remain internal authoring/verification
  // tooling for the MVP; neither is a user-facing destination.
  "workspace-product-spec",
  "workspace-assurance-spec",
  // The spec places bounded file/Git review "beside the conversation"
  // (ProductSpec Scope; CW-AC-14) — not as a top-level sidebar destination.
  "workspace-files",
  // The palette remains a CW-AC-12 entry point via ⌘K and the native
  // Commands menu; the spec calls for no dock icon.
  "shell-command-palette-toggle",
]

/**
 * Screen-root keys that must never appear anywhere in a rendered shell tree.
 * These are the root keys of internal or post-MVP substrates (the ProductSpec
 * Scope "out"/"cut" sections): Fleet, Terminal, sidebar accounts, provider/
 * model/reasoning selection, image attachment, voice controls, MCP/plugin
 * configuration, and visible Git mutation authority (CW-AC-14: review exposes
 * no "general filesystem or Git mutation authority"; commit/push/branch/PR
 * authoring are outside the MVP cut).
 */
export const forbiddenVisibleSurfaceKeys: ReadonlyArray<string> = [
  // Spec authoring/verification screens are not user-facing MVP surfaces.
  "product-spec-workspace",
  "assurance-spec-document",
  "assurance-spec-invalid",
  // Fleet (ProductSpec Scope out: "Fleet, multi-account dispatch, markets…")
  "workspace-fleet-panel",
  "fleet-desk",
  "sidebar-accounts",
  // Terminal / Inbox (Scope out: "interactive PTY"; no Inbox in the MVP)
  "workspace-terminal-panel",
  "workspace-inbox-panel",
  // Composer affordances outside the MVP cut
  "shell-attach-image",
  "shell-harness-select",
  "shell-model-select",
  "shell-reasoning-select",
  "shell-voice-toggle",
  // Settings surfaces outside the MVP cut
  "settings-accounts-title",
  "settings-mcp-title",
  "settings-plugins-title",
  "settings-lifecycle-title",
  // Git mutation authority (CW-AC-14 read-only review boundary)
  "git-commit",
  "git-commit-message",
  "git-push",
  "git-branches",
  "git-branch-create",
  "git-issues-prs",
  "git-discard-confirmation",
  // Removed dock affordances must not come back anywhere in the rail
  ...mvpRemovedDockItemIds,
]

type AnyNode = Readonly<Record<string, unknown>>

/** Collect every catalog node in a rendered view tree. */
const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

/** The item ids of the ACTUAL rendered sidebar workspace dock, in order. */
export const collectRenderedDockItemIds = (view: View): ReadonlyArray<string> => {
  const nav = collectNodes(view).find(node => node.key === "sidebar-navigation")
  const sections = (nav?.sections ?? []) as ReadonlyArray<{ id?: unknown; items?: ReadonlyArray<{ id?: unknown }> }>
  const dock = sections.find(section => section.id === "sidebar-workspace-dock")
  return (dock?.items ?? []).map(item => String(item.id))
}

/**
 * The composition oracle. Walks one rendered shell view and returns every
 * MVP visible-surface violation (empty array = conformant):
 *
 * - the sidebar dock must exist and must equal `mvpAllowedDockItemIds`
 *   exactly (order included) — additions AND silent losses both fail;
 * - dock sections must be exactly the workspace dock plus the session list;
 * - no node anywhere in the tree may carry a forbidden surface key.
 */
export const desktopMvpSurfaceViolations = (view: View): ReadonlyArray<string> => {
  const violations: Array<string> = []
  const nodes = collectNodes(view)

  const nav = nodes.find(node => node.key === "sidebar-navigation")
  if (nav === undefined) {
    violations.push("sidebar-navigation NavRail is not rendered")
  } else {
    const sections = (nav.sections ?? []) as ReadonlyArray<{ id?: unknown }>
    const sectionIds = sections.map(section => String(section.id))
    if (sectionIds[0] !== "sidebar-workspace-dock" || sectionIds.length !== 2 || sectionIds[1] !== "sidebar-history-list") {
      violations.push(`sidebar sections must be exactly [sidebar-workspace-dock, sidebar-history-list]; rendered [${sectionIds.join(", ")}]`)
    }
    const dockIds = collectRenderedDockItemIds(view)
    if (dockIds.length === 0) {
      violations.push("sidebar-workspace-dock rendered no items")
    } else {
      for (const id of dockIds) {
        if (!mvpAllowedDockItemIds.includes(id)) {
          violations.push(`dock item "${id}" is not on the MVP visible-surface allowlist`)
        }
      }
      for (const id of mvpAllowedDockItemIds) {
        if (!dockIds.includes(id)) {
          violations.push(`allowlisted dock item "${id}" is missing from the rendered dock`)
        }
      }
      if (violations.length === 0 && dockIds.join("|") !== mvpAllowedDockItemIds.join("|")) {
        violations.push(`dock order [${dockIds.join(", ")}] does not match the allowlist order`)
      }
    }
  }

  for (const node of nodes) {
    const key = typeof node.key === "string" ? node.key : null
    if (key !== null && forbiddenVisibleSurfaceKeys.includes(key)) {
      violations.push(`forbidden surface key "${key}" is rendered`)
    }
    // NavRail items are sections' data, not nodes — scan them explicitly.
    const sections = node.sections as ReadonlyArray<{ items?: ReadonlyArray<{ id?: unknown }> }> | undefined
    if (Array.isArray(sections)) {
      for (const section of sections) {
        for (const item of section.items ?? []) {
          const id = String(item.id)
          if (forbiddenVisibleSurfaceKeys.includes(id)) {
            violations.push(`forbidden nav item "${id}" is rendered`)
          }
        }
      }
    }
  }

  return violations
}
