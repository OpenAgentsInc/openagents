/**
 * Unified extension lifecycle — CUT-23 (#8703).
 *
 * One typed lifecycle (declare → validate → enable → run → revoke) over the
 * three already-landed extension surfaces, DERIVED from their existing
 * public-safe projections:
 *
 * - MCP servers  — `mcp-config-contract.ts` (`McpConfigServerView`)
 * - plugins      — `plugin-config-contract.ts` (`PluginConfigView`)
 * - skills       — `PluginConfigView.skills` (host-discovered, invoked only
 *                  through the explicit `/skill <plugin>/<skill>` grammar)
 *
 * This module is a PURE projection. It owns no persistence, no IPC channel,
 * and no parallel registry: the per-surface hosts remain the only mutation
 * authorities, and their existing toggle/remove channels remain the only
 * revoke levers. What this contract adds is the single auditable view the
 * CUT-23 outcome names — per-item lifecycle stage, per-item grant scoping
 * (a skill's grant is scoped under its parent plugin's grant and is revoked
 * with it), duplicate-label detection across kinds, explicit provider
 * disagreement, and honest partial/offline state.
 *
 * Security posture is inherited, not re-decided: both input views are
 * already secret-free (MCP env/header/args VALUES never reach the renderer;
 * plugin refs are opaque), so every field here is public-safe by
 * construction. No absolute paths, no secret values, no new data crosses
 * any boundary because of this module.
 */
import { Exit, Schema } from "@effect-native/core/effect"

import type { McpConfigServerView } from "./mcp-config-contract.ts"
import type { PluginConfigView } from "./plugin-config-contract.ts"

// ---------------------------------------------------------------------------
// Contract.
// ---------------------------------------------------------------------------

export const ExtensionKindSchema = Schema.Literals(["mcp_server", "plugin", "skill"])
export type ExtensionKind = typeof ExtensionKindSchema.Type

/**
 * The derived lifecycle stage. "declare" and "validate" are transitions the
 * hosts already performed; the stage records where each item landed:
 *
 * - `declared` — registered with a host but not currently runnable for a
 *   non-validation reason (e.g. a plugin directory that went missing).
 * - `invalid`  — declared but failed validation (schema-invalid, corrupt
 *   manifest). Never runnable; enablement is irrelevant.
 * - `granted`  — validated AND enabled: the item's grant is active and it
 *   participates in the next matching run.
 * - `revoked`  — validated but disabled: the grant is withdrawn and the next
 *   run excludes the item. Re-enabling re-grants; nothing is lost.
 */
export const ExtensionLifecycleStageSchema = Schema.Literals([
  "declared",
  "invalid",
  "granted",
  "revoked",
])
export type ExtensionLifecycleStage = typeof ExtensionLifecycleStageSchema.Type

/**
 * Per-item grant. `active` grants also record HOW the grant is exercised:
 * MCP servers and plugins ride the next turn automatically once granted;
 * skills additionally require an explicit `/skill` invocation per use —
 * a granted skill still never runs without one.
 */
export const ExtensionGrantSchema = Schema.Struct({
  state: Schema.Literals(["active", "revoked", "blocked"]),
  use: Schema.Literals(["next_turn", "explicit_invocation"]),
  scope: Schema.Literal("app"),
})
export type ExtensionGrant = typeof ExtensionGrantSchema.Type

/**
 * Provider disagreement stays explicit (CUT-23 completion criterion). Every
 * current extension kind is consumed by the Claude Agent SDK lane only; the
 * bundled Codex runtime has no equivalent user-MCP/plugin/skill contract, so
 * it is `unsupported` — never silently emulated.
 */
export const ExtensionProviderSupportSchema = Schema.Struct({
  claude_agent: Schema.Literal("supported"),
  codex: Schema.Literals(["supported", "unsupported"]),
})
export type ExtensionProviderSupport = typeof ExtensionProviderSupportSchema.Type

export const ExtensionLifecycleEntrySchema = Schema.Struct({
  kind: ExtensionKindSchema,
  /**
   * Stable identifier within its kind: the MCP server name, the opaque
   * plugin ref, or `<plugin ref>/<skill name>` for a skill.
   */
  id: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  /** Human label: server name, plugin name, or `<plugin>/<skill>`. */
  label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  provenance: Schema.Literal("user_local"),
  scope: Schema.Literal("app"),
  stage: ExtensionLifecycleStageSchema,
  grant: ExtensionGrantSchema,
  restartRequired: Schema.Literal(false),
  providerSupport: ExtensionProviderSupportSchema,
  /**
   * True when another entry elsewhere in the unified list carries the same
   * label. The per-kind hosts already reject duplicates within a kind; this
   * flags CROSS-kind collisions (an MCP server and a plugin sharing a name)
   * so the audit view can surface the ambiguity instead of hiding it.
   */
  duplicateLabel: Schema.Boolean,
})
export type ExtensionLifecycleEntry = typeof ExtensionLifecycleEntrySchema.Type

export const ExtensionLifecycleAuditSchema = Schema.Struct({
  entries: Schema.Array(ExtensionLifecycleEntrySchema),
  /** Rows each host dropped as schema-invalid on its last read. */
  droppedInvalid: Schema.Struct({
    mcpServers: Schema.Number,
    plugins: Schema.Number,
  }),
  granted: Schema.Number,
  revoked: Schema.Number,
  blocked: Schema.Number,
  /**
   * True when at least one backing registry list was unavailable (bridge
   * offline, host error). A partial audit never pretends to be complete.
   */
  partial: Schema.Boolean,
})
export type ExtensionLifecycleAudit = typeof ExtensionLifecycleAuditSchema.Type

export const decodeExtensionLifecycleAudit = (
  value: unknown,
): ExtensionLifecycleAudit | null => {
  const decoded = Schema.decodeUnknownExit(ExtensionLifecycleAuditSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

// ---------------------------------------------------------------------------
// Pure per-kind projections.
// ---------------------------------------------------------------------------

const CLAUDE_ONLY: ExtensionProviderSupport = {
  claude_agent: "supported",
  codex: "unsupported",
}

/**
 * MCP server → lifecycle entry. A row that reached the renderer projection
 * already passed the frozen persisted schema in main (invalid rows are
 * dropped and only counted), so a visible server is validated by
 * construction: enabled → granted, disabled → revoked.
 */
export const mcpServerLifecycleEntry = (
  view: McpConfigServerView,
): ExtensionLifecycleEntry => ({
  kind: "mcp_server",
  id: view.name,
  label: view.name,
  provenance: "user_local",
  scope: "app",
  stage: view.enabled ? "granted" : "revoked",
  grant: {
    state: view.enabled ? "active" : "revoked",
    use: "next_turn",
    scope: "app",
  },
  restartRequired: false,
  providerSupport: CLAUDE_ONLY,
  duplicateLabel: false,
})

const pluginStage = (view: PluginConfigView): ExtensionLifecycleStage => {
  if (view.readiness === "invalid") return "invalid"
  if (view.readiness === "missing") return "declared"
  return view.enabled ? "granted" : "revoked"
}

/** Plugin → lifecycle entry. Readiness gates validation; enablement grants. */
export const pluginLifecycleEntry = (
  view: PluginConfigView,
): ExtensionLifecycleEntry => {
  const stage = pluginStage(view)
  return {
    kind: "plugin",
    id: view.ref,
    label: view.name,
    provenance: "user_local",
    scope: "app",
    stage,
    grant: {
      state:
        stage === "granted" ? "active" : stage === "revoked" ? "revoked" : "blocked",
      use: "next_turn",
      scope: "app",
    },
    restartRequired: false,
    providerSupport: CLAUDE_ONLY,
    duplicateLabel: false,
  }
}

/**
 * Skills → lifecycle entries, SCOPED under their parent plugin's grant:
 * a skill is granted only while its parent plugin is granted, and revoking
 * (or invalidating, or losing) the plugin revokes/blocks every skill under
 * it in the same projection — there is no independent skill enablement to
 * drift out of sync. Even an active skill grant is exercised only through
 * the explicit `/skill` invocation grammar, never automatically.
 */
export const skillLifecycleEntries = (
  view: PluginConfigView,
): ReadonlyArray<ExtensionLifecycleEntry> => {
  const parentStage = pluginStage(view)
  return view.skills.map((name) => ({
    kind: "skill" as const,
    id: `${view.ref}/${name}`,
    label: `${view.name}/${name}`,
    provenance: "user_local" as const,
    scope: "app" as const,
    stage: parentStage,
    grant: {
      state:
        parentStage === "granted"
          ? ("active" as const)
          : parentStage === "revoked"
            ? ("revoked" as const)
            : ("blocked" as const),
      use: "explicit_invocation" as const,
      scope: "app" as const,
    },
    restartRequired: false as const,
    providerSupport: CLAUDE_ONLY,
    duplicateLabel: false,
  }))
}

// ---------------------------------------------------------------------------
// Unified audit.
// ---------------------------------------------------------------------------

export type ExtensionLifecycleInput = Readonly<{
  /** `null` = that registry list is unavailable (offline/bridge missing). */
  mcpServers: ReadonlyArray<McpConfigServerView> | null
  mcpDropped: number
  plugins: ReadonlyArray<PluginConfigView> | null
  pluginsDropped: number
}>

const KIND_ORDER: Readonly<Record<ExtensionKind, number>> = {
  mcp_server: 0,
  plugin: 1,
  skill: 2,
}

/**
 * Compose the unified, deterministic audit: per-kind projections, cross-kind
 * duplicate-label detection, stable ordering (kind, then label, then id),
 * grant tallies, and honest `partial` when any backing list is unavailable.
 */
export const unifiedExtensionLifecycle = (
  input: ExtensionLifecycleInput,
): ExtensionLifecycleAudit => {
  const raw: Array<ExtensionLifecycleEntry> = []
  for (const server of input.mcpServers ?? []) raw.push(mcpServerLifecycleEntry(server))
  for (const plugin of input.plugins ?? []) {
    raw.push(pluginLifecycleEntry(plugin))
    raw.push(...skillLifecycleEntries(plugin))
  }

  const labelCounts = new Map<string, number>()
  for (const entry of raw) {
    labelCounts.set(entry.label, (labelCounts.get(entry.label) ?? 0) + 1)
  }

  const entries = raw
    .map((entry) =>
      (labelCounts.get(entry.label) ?? 0) > 1 ? { ...entry, duplicateLabel: true } : entry,
    )
    .sort((a, b) =>
      KIND_ORDER[a.kind] !== KIND_ORDER[b.kind]
        ? KIND_ORDER[a.kind] - KIND_ORDER[b.kind]
        : a.label !== b.label
          ? a.label < b.label
            ? -1
            : 1
          : a.id < b.id
            ? -1
            : a.id > b.id
              ? 1
              : 0,
    )

  return {
    entries,
    droppedInvalid: {
      mcpServers: input.mcpServers === null ? 0 : Math.max(0, input.mcpDropped),
      plugins: input.plugins === null ? 0 : Math.max(0, input.pluginsDropped),
    },
    granted: entries.filter((entry) => entry.grant.state === "active").length,
    revoked: entries.filter((entry) => entry.grant.state === "revoked").length,
    blocked: entries.filter((entry) => entry.grant.state === "blocked").length,
    partial: input.mcpServers === null || input.plugins === null,
  }
}
