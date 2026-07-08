# Settings And Configuration System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #33 from the Bun/Effect terminal-agent systems list. It captures
the configuration plane a new terminal coding agent should own: typed settings,
policy precedence, live reload, remote sync, and safe mutation.

The key rule is that configuration is a domain service, not a pile of files.
Files, command flags, environment variables, remote policy, organization policy,
and plugin-provided defaults are inputs to one resolved snapshot.

## Target

Build a settings system that can serve:

- The local terminal UI.
- Headless agent runs.
- Hosted or delegated agent sessions.
- Team policy and managed defaults.
- Workspace-scoped overrides.
- Plugin and connector configuration.
- Future OpenAgents work-order runs.

Every runtime decision should be able to say which resolved setting it used and
which source provided it.

## User-Visible Capability

Users should be able to:

- Set global preferences once.
- Add workspace settings that travel with a repository when appropriate.
- Keep local-only overrides out of version control.
- See validation errors without losing the rest of their settings.
- Reload settings during a running session.
- Sync selected user and workspace settings across environments.
- Understand when a team or operator policy overrides local choice.

Administrators should be able to:

- Lock specific setting surfaces.
- Provide managed defaults.
- Allow or deny connectors, models, marketplaces, hooks, and permission modes.
- Audit the effective policy without reading private user files.

## Core Shape

Use Effect Schema for every durable shape.

```ts
type ConfigSourceKind =
  | "defaults"
  | "environment"
  | "cli_flag"
  | "user"
  | "workspace"
  | "workspace_local"
  | "plugin"
  | "managed_policy"
  | "remote_policy"
  | "runtime"

interface ConfigSourceRef {
  readonly kind: ConfigSourceKind
  readonly scopeId?: string
  readonly pathRef?: string
  readonly priority: number
  readonly mutable: boolean
}

interface ConfigSnapshot {
  readonly snapshotId: string
  readonly createdAt: string
  readonly workspaceId?: string
  readonly sources: readonly ConfigSourceRef[]
  readonly values: AgentSettings
  readonly provenance: Record<string, ConfigSourceRef>
  readonly validation: readonly ConfigValidationIssue[]
}

interface ConfigChangeEvent {
  readonly source: ConfigSourceRef
  readonly changedAt: string
  readonly changedKeys: readonly string[]
  readonly blockedByPolicy: boolean
}
```

The runtime should treat `ConfigSnapshot` as immutable. Every run, turn, tool
decision, and external-agent handoff should reference the snapshot it used.

## Settings Domains

The first schema should cover these domains:

- Identity and account selection.
- Model and provider selection.
- Budget and rate-limit behavior.
- Workspace trust state.
- Permission defaults and durable grants.
- Additional readable or writable directories.
- Shell execution policy.
- Git and pull-request behavior.
- Agent memory and context collection limits.
- Hooks and event subscriptions.
- MCP and connector configuration.
- Plugin and marketplace policy.
- IDE, browser, and desktop integration toggles.
- Telemetry, tracing, and redaction levels.
- Output style and locale.

Fields should be optional unless the runtime cannot operate without them.
Backward compatibility matters because settings files persist longer than any
single runtime release.

## Source Precedence

Use a deterministic resolver.

Recommended precedence from lowest to highest:

1. Built-in defaults.
2. Plugin-provided defaults for allowlisted settings.
3. User settings.
4. Workspace settings.
5. Workspace-local settings.
6. Command-line or SDK-provided settings.
7. Managed policy.
8. Remote managed policy.
9. Runtime emergency disables.

Managed policy should not always deep-merge with lower sources. For some
policy families, the highest-priority policy source should win as a whole.
This avoids a weak lower-priority policy accidentally mixing with a stronger
remote policy.

Arrays need field-specific semantics:

- Permission allow and deny lists usually merge and dedupe.
- Ordered hook lists may concatenate with stable source ordering.
- Blocklists should merge from all sources.
- Explicit policy locks should override all lower sources.
- Some arrays, such as enabled providers for a run, should replace.

Do not use one global merge rule for every setting.

## Validation

Validation should be layered:

- Parse JSON or structured config safely.
- Validate the full schema.
- Validate permission rules separately so one bad rule does not discard an
  entire settings file.
- Preserve unknown and future fields where compatibility requires it.
- Produce path-addressed, human-readable validation issues.
- Provide repair suggestions where the schema can identify intent.
- Refuse writes that would make the target settings file syntactically invalid.

Validation output should be a first-class object:

```ts
interface ConfigValidationIssue {
  readonly source: ConfigSourceRef
  readonly path: string
  readonly severity: "warning" | "error"
  readonly message: string
  readonly expected?: string
  readonly received?: string
  readonly suggestion?: string
}
```

The resolver should keep using valid portions of settings where safe. A single
bad optional field should not make the runtime forget unrelated policy.

## Mutation Model

Settings mutation should use typed patches, not raw text edits.

```ts
interface ConfigPatch {
  readonly target: ConfigSourceRef
  readonly operationId: string
  readonly set?: Partial<AgentSettings>
  readonly unset?: readonly string[]
  readonly reason: string
  readonly requestedBy: "user" | "system" | "policy_sync" | "test"
}
```

Mutation rules:

- Never write managed policy from normal user flows.
- Never overwrite a file with invalid JSON syntax unless the user explicitly
  chooses a repair flow.
- Preserve unrelated keys and formatting as much as the parser stack supports.
- Treat explicit `unset` differently from absence.
- Mark internal writes so file watchers do not echo a change back into the
  runtime as if it came from the user.
- Add local-only settings files to ignore rules when appropriate.

## Live Reload

The file watcher should be conservative:

- Watch only known settings files and policy drop-in directories.
- Debounce until writes stabilize.
- Apply a deletion grace window for delete-and-recreate update patterns.
- Ignore internal writes for a short bounded window.
- Poll non-file managed settings where the host OS has no reliable watcher.
- Centralize cache invalidation at the producer side so multiple listeners do
  not cause repeated disk reloads.
- Emit one typed change event per source.

Before applying a change, run configurable change hooks. If a hook returns a
blocking result, skip applying the new settings and keep the previous snapshot.

## Remote Sync

Remote sync should be opt-in and fail-open. It should never block startup or
turn execution.

Recommended sync boundaries:

- Sync global user settings.
- Sync global user memory only after separate user consent.
- Sync workspace-local settings only under a stable workspace identity.
- Do not sync secrets.
- Apply per-file size limits.
- Parse and validate server responses before writing.
- Write only known entry keys.
- Mark sync writes as internal so local watchers do not loop.
- Retry reads with bounded backoff.
- Treat auth failures as non-retryable until credentials change.

Sync should emit structured events for observability but should never log raw
config values.

## Effect Services

Model the settings plane as services:

- `ConfigResolver`: combines sources into a snapshot.
- `ConfigStore`: reads and writes editable sources.
- `PolicyProvider`: supplies managed and remote policy.
- `ConfigValidator`: schema and semantic validation.
- `ConfigWatcher`: file and host-policy change detection.
- `ConfigSync`: remote upload and download.
- `ConfigMutation`: typed patch application.
- `ConfigProvenance`: explains effective values.
- `ConfigRedactor`: prepares safe logs and API views.

Each service should expose Effect errors with typed causes such as parse error,
validation error, policy denial, write failure, sync unavailable, or watcher
failure.

## Safety Rules

- Do not execute settings-defined commands before workspace trust is accepted.
- Do not let workspace settings override account credentials.
- Do not read secrets into the settings snapshot unless the field is explicitly
  secret-typed and redacted.
- Do not let lower-priority sources unlock policy-locked surfaces.
- Do not silently downgrade a managed denylist.
- Do not include private paths or raw config payloads in public artifacts.
- Do not sync local-only files without a stable workspace identity.

## Tests

Minimum coverage:

- Schema round trips and backward-compatible fixture files.
- Precedence tests for every source kind.
- Policy-lock tests for connectors, hooks, permissions, and marketplaces.
- Invalid-field preservation tests.
- Internal-write watcher suppression tests.
- Delete-and-recreate watcher tests.
- Remote sync fail-open tests.
- Size-limit tests for synced files.
- Provenance explanation tests.
- Redaction tests for logs and public projections.

## OpenAgents Translation Notes

Checked the open OpenAgents issue list on 2026-06-11.

Related live roadmap issues:

- #4769 covers repo connect, per-mission data-scope UX, and placement
  explanations.
- #4770 covers team budgets and the spend-to-evidence join.
- #4771 covers provider peers and connected account flows.
- #4773 covers API parity for MVP surfaces.
- #4786 is the Autopilot MVP ladder epic.

No open issue explicitly names a unified settings and configuration plane.
That means OpenAgents should not claim this system as implemented until a
dedicated issue or accepted implementation record exists.

Recommended OpenAgents shape:

- Add a `SettingsSnapshot` or equivalent record under the Autopilot run model.
- Attach the snapshot id to every mission, work order, provider lease, and
  decision-action record.
- Keep team policy separate from user preference.
- Make per-mission data scope a config surface with provenance.
- Expose a read-only API projection that explains effective settings without
  leaking values.

## Decision

Build this early. It becomes the substrate for permissions, provider accounts,
Git delivery, IDE integration, browser control, desktop handoff, and delegated
agent runs. Without a typed settings plane, those systems will each invent
their own precedence and policy rules.
