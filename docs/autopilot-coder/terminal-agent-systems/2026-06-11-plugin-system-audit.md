# Plugin System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #30 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should discover, install, load, validate, update,
disable, and sandbox plugins.

## Target

Build a plugin system that treats plugins as packaged capability bundles with
clear manifests, policy gates, versioned materialization, and component-level
loading.

Plugins may provide commands, agents, skills, hooks, output styles, MCP
servers, language services, settings, and option schemas. Installing a plugin
should not automatically grant all possible authority.

## User-Visible Capability

The user should be able to:

- Browse available plugins from known marketplaces.
- Add trusted marketplaces.
- Install plugins at user, project, or local scope.
- Enable, disable, update, or uninstall plugins.
- See plugin description, author, version, source, and provided components.
- Configure plugin options without echoing sensitive values.
- See dependency warnings.
- See delisted, blocked, duplicate, or invalid plugin status.
- Refresh active plugins intentionally.
- Run doctor checks for plugin errors.

The system should explain whether a plugin is installed, enabled, loaded, or
active in the current session.

## Manifest Model

Plugin manifests should include:

- Name.
- Version.
- Description.
- Author and homepage.
- Repository and license.
- Keywords.
- Dependencies.
- Command paths or inline command definitions.
- Agent paths.
- Skill paths.
- Hook config.
- Output style paths.
- MCP server configs.
- Language service configs.
- Option schema with sensitivity markers.
- Required capabilities.

Manifest validation should reject path traversal, reserved names, malformed
component refs, invalid server config, invalid hook config, and ambiguous
dependency refs.

## Marketplace Model

Marketplaces should be typed and policy-governed:

- Marketplace id.
- Source kind: local directory, file, git, hosted index, package registry, or
  managed source.
- Source origin.
- Auto-update policy.
- Official or trusted marker.
- Blocklist and strict-allowlist policy.
- Materialized cache ref.
- Last refresh receipt.

Reserved marketplace names should require verified origins. Names should be
ASCII and path-safe to reduce impersonation and filesystem confusion.

## Materialization And Cache

Installed plugins should be materialized into a versioned cache:

- Marketplace id.
- Plugin id.
- Version or commit ref.
- Install scope.
- Project ref when project/local scoped.
- Source receipt.
- Cache path ref.
- Enabled state.
- Options ref.
- Orphaned or delisted marker.

Versioned cache paths should be sanitized. Local plugins should be copied or
linked only through validated paths. Updating should write a new version and
mark old versions for cleanup rather than mutating in place when possible.

## Core Design

Define a `PluginService` that owns marketplace reconciliation, install/update
operations, manifest validation, component loading, and active-session refresh.

Suggested service boundary:

```ts
interface PluginService {
  marketplaces(request: PluginMarketplaceRequest): Effect.Effect<MarketplaceSet, PluginError>
  install(request: PluginInstallRequest): Effect.Effect<PluginInstallReceipt, PluginError>
  update(request: PluginUpdateRequest): Effect.Effect<PluginUpdateReceipt, PluginError>
  uninstall(request: PluginUninstallRequest): Effect.Effect<PluginUninstallReceipt, PluginError>
  enable(request: PluginEnableRequest): Effect.Effect<PluginEnableReceipt, PluginError>
  disable(request: PluginDisableRequest): Effect.Effect<PluginDisableReceipt, PluginError>
  load(request: PluginLoadRequest): Effect.Effect<PluginLoadResult, PluginError>
  refresh(request: PluginRefreshRequest): Effect.Effect<PluginRefreshReceipt, PluginError>
}
```

Command-line and terminal UI flows should call the same service functions and
receive result objects, not exit the process directly.

## Component Loading

Loading should be component-specific:

- Commands become command descriptors.
- Agents become agent definitions.
- Skills become skill descriptors.
- Hooks become hook matchers.
- MCP server configs become scoped server configs.
- Language services become scoped diagnostics providers.
- Output styles become selectable render options.

Each component loader should report errors without preventing unrelated
components from loading.

## Refresh Model

Refresh should be explicit and predictable:

- Initial load happens at session start.
- Install or update may mark active plugins as needing refresh.
- A refresh operation clears relevant caches.
- Commands, skills, hooks, MCP servers, and language services are swapped as
  one active-session update.
- Remote server reconnects happen after server config changes.

Avoid silent mid-turn mutation of active plugin components.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for plugin operations.
- `Schema` for manifests, marketplaces, installs, options, errors, and
  receipts.
- `Layer` for filesystem, git, package registry, network, settings, and
  policy providers.
- `Stream` for marketplace refresh progress.
- `Queue` for background installs and update notifications.
- `Cache` for loaded manifests and component results.
- `Scope` for temporary extraction directories and process cleanup.
- `Redacted` wrappers for sensitive option values.

Plugin errors should be structured by type so UI and doctor output can show
useful remediation.

## Safety Rules

- Do not install from unknown marketplaces when policy blocks them.
- Do not allow marketplace-name impersonation.
- Do not trust plugin manifests before schema validation.
- Do not follow plugin paths outside the plugin root.
- Do not echo sensitive option values into prompts or UI.
- Do not let plugin hooks run before workspace trust.
- Do not auto-enable newly installed plugins without explicit policy.
- Do not let plugin-provided MCP servers duplicate or override manual servers
  silently.
- Do not let one plugin loader failure disable unrelated plugins.
- Do not refresh active runtime components in the middle of a tool call.

## Tests

Minimum regression coverage:

- Validate good and bad plugin manifests.
- Reject path traversal in component refs.
- Block reserved or impersonating marketplace names.
- Reconcile declared marketplaces to materialized cache state.
- Install from local and remote marketplace sources.
- Install at user, project, and local scope.
- Enable, disable, update, and uninstall plugins.
- Preserve dependency warnings for reverse dependents.
- Keep sensitive option fields blank in edit dialogs.
- Load each component type independently.
- Report invalid MCP, hook, and language-service configs.
- Mark active plugins as needing refresh after install/update.
- Refresh active components and reconnect affected servers.

## OpenAgents Translation Notes

When promoted, map plugins to OpenAgents extension refs, capability refs,
policy refs, marketplace refs, artifact refs, and operator receipts. Verify
live issue state before claiming plugin install, marketplace, refresh, or
component-loading behavior is implemented.

## Decision

The plugin system should be a versioned, policy-governed extension layer. It
should separate installation, enablement, loading, refresh, and runtime
authority so users can adopt extensions without surrendering control of the
agent.
