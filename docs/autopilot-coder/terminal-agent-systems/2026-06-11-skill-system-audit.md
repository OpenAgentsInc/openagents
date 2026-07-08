# Skill System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #31 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should discover, describe, invoke, hot-reload, and
progressively disclose skill files, bundled skills, plugin skills, and
remote-provided skills.

## Target

Build a skill system that treats skills as explicit domain workflows, not
hidden prompt text.

A skill should provide a concise descriptor to the model and user. Its full
instructions, reference assets, scripts, and additional files should be loaded
only when the skill is selected or invoked.

## User-Visible Capability

The user should be able to:

- List available skills by source.
- See skill names, descriptions, and when-to-use guidance.
- Invoke user-visible skills directly.
- Let the model invoke model-allowed skills when they match the task.
- Add project and user skills.
- Receive hot reload after skill changes.
- Use bundled skills that ship with the agent.
- Use plugin and MCP-provided skills.
- See when a skill is hidden, disabled, or blocked by policy.
- Understand which tools or authority a skill requests.

Skills should improve workflow selection without bloating every model prompt.

## Skill Descriptor

A skill descriptor should include:

- Skill id.
- Display name.
- Description.
- When-to-use text.
- Version.
- Source.
- Root ref for associated assets.
- Allowed tool scopes.
- Argument schema or hint.
- Model/provider preference.
- Effort preference.
- Agent/workflow preference.
- Inline or fork execution context.
- User-invocable flag.
- Model-invocable flag.
- Path applicability rules.
- Hook declarations.
- Approximate descriptor token cost.

The descriptor is what discovery uses. The full skill body is loaded on
invocation.

## Skill Sources

Support these sources:

- Built-in registered skills.
- User skills.
- Project skills.
- Managed/policy skills.
- Plugin skills.
- Remote capability skills.
- Session-only dynamic skills.

Each source should have precedence, policy, refresh, and visibility rules.
Source labels should be visible in the skills UI.

## Progressive Disclosure

Skill loading should follow:

1. Discover descriptors from configured sources.
2. Add only descriptors to the command/skill catalog.
3. Select a skill by user command or model decision.
4. Load the full skill body and references.
5. Substitute typed arguments.
6. Add base asset refs if the skill has files.
7. Apply skill-scoped hooks.
8. Invoke the chosen runtime context.

Full skill content should not be included in the default system prompt or every
turn context.

## Asset And Script Handling

Skills may include reference files or scripts. The system should:

- Resolve assets relative to the skill root.
- Prevent path traversal.
- Extract bundled assets into private owner-only directories.
- Write files with restrictive permissions.
- Avoid following untrusted symlink escapes.
- Allow scripts to be referenced only through approved tools.
- Clean up temporary extraction roots when appropriate.

The model may read skill assets through normal file access policy. A skill root
does not grant unrestricted filesystem authority.

## Hot Reload

Skill directories should be watched with debouncing:

- User skill changes.
- Project skill changes.
- Command-directory compatibility changes.
- Additional workspace directories.
- Dynamic skill registrations.
- Feature-gate refreshes that affect skill visibility.

Reload should clear skill and command caches, notify listeners, and preserve
the current prompt. Config-change hooks may block a reload when policy requires
review.

## Core Design

Define a `SkillService` that owns discovery, descriptor indexing, invocation,
asset materialization, and hot reload.

Suggested service boundary:

```ts
interface SkillService {
  discover(request: SkillDiscoverRequest): Effect.Effect<SkillCatalog, SkillError>
  describe(request: SkillDescribeRequest): Effect.Effect<SkillDescriptor, SkillError>
  load(request: SkillLoadRequest): Effect.Effect<LoadedSkill, SkillError>
  invoke(request: SkillInvokeRequest): Effect.Effect<SkillInvocationPlan, SkillError>
  watch(request: SkillWatchRequest): Effect.Effect<SkillWatchReceipt, SkillError>
  clearCaches(request: SkillCacheClearRequest): Effect.Effect<void, SkillError>
}
```

The command system can project skills as prompt commands, but the skill service
should remain the source of truth.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for skill discovery and invocation.
- `Schema` for descriptors, frontmatter, loaded bodies, assets, hooks, and
  invocation plans.
- `Layer` for filesystem, plugin, remote, built-in, and policy skill sources.
- `Cache` for descriptor and body parsing.
- `Stream` for file-watch events.
- `Schedule` for debounce and file-write stabilization.
- `Scope` for temporary asset extraction.
- `Ref` for active catalog and watch state.

Parsing should be pure enough for fixture tests. File watching should be
replaceable in tests.

## Safety Rules

- Do not hide full skill text inside every prompt.
- Do not let a skill expand allowed tools beyond policy.
- Do not let user-invocable false skills appear as normal commands.
- Do not let model-invocable false skills be selected by the model.
- Do not load skill assets outside the skill root.
- Do not follow symlink escapes when extracting bundled assets.
- Do not keep stale skill descriptors after file changes.
- Do not let one broken skill disable the entire skill catalog.
- Do not let skill hooks run before workspace trust.
- Do not store secrets in skill descriptors or public summaries.

## Tests

Minimum regression coverage:

- Discover built-in, user, project, managed, plugin, remote, and dynamic
  skills.
- Parse descriptors without loading full bodies.
- Load full skill body on invocation.
- Substitute positional and named arguments.
- Enforce user-invocable and model-invocable flags.
- Apply allowed-tool and path applicability rules.
- Materialize bundled assets with safe paths and restrictive permissions.
- Reject asset path traversal.
- Hot-reload after file add, change, and delete.
- Debounce bulk changes.
- Allow config-change hooks to block reload.
- Keep broken skills isolated as diagnostics.

## OpenAgents Translation Notes

When promoted, map skills to OpenAgents capability refs, workflow refs, policy
refs, typed prompt assets, operator docs, and public-safe receipts. Verify live
issue state before claiming skill discovery, invocation, or hot reload behavior
is implemented.

## Decision

Skills should be a progressive-disclosure workflow layer. The agent should
index concise descriptors, load full instructions only when needed, and keep
skill authority bounded by normal tool, workspace, and policy controls.
