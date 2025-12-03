/**
 * SDK-compatible permission schemas for Claude Agent SDK integration.
 *
 * Mirrors the Claude Agent SDK permission system so we can validate and
 * transform permission data when interoperating with Claude Code.
 */

import * as S from "effect/Schema";

// =============================================================================
// Core Permission Types
// =============================================================================

/**
 * PermissionMode controls Claude Code's permission behavior.
 */
export const PermissionMode = S.Literal("default", "acceptEdits", "bypassPermissions", "plan");
export type PermissionMode = S.Schema.Type<typeof PermissionMode>;

/**
 * PermissionBehavior describes how a rule should behave.
 */
export const PermissionBehavior = S.Literal("allow", "deny", "ask");
export type PermissionBehavior = S.Schema.Type<typeof PermissionBehavior>;

/**
 * Rule value describing tool-specific permissions.
 */
export const PermissionRuleValue = S.Struct({
  toolName: S.String,
  ruleContent: S.optional(S.String),
});
export type PermissionRuleValue = S.Schema.Type<typeof PermissionRuleValue>;

/**
 * Where permission updates should be applied.
 */
export const PermissionUpdateDestination = S.Literal("userSettings", "projectSettings", "localSettings", "session");
export type PermissionUpdateDestination = S.Schema.Type<typeof PermissionUpdateDestination>;

// =============================================================================
// Permission Updates
// =============================================================================

const PermissionUpdateBase = {
  rules: S.Array(PermissionRuleValue),
  behavior: PermissionBehavior,
  destination: PermissionUpdateDestination,
};

export const AddRulesPermissionUpdate = S.Struct({
  type: S.Literal("addRules"),
  ...PermissionUpdateBase,
});

export const ReplaceRulesPermissionUpdate = S.Struct({
  type: S.Literal("replaceRules"),
  ...PermissionUpdateBase,
});

export const RemoveRulesPermissionUpdate = S.Struct({
  type: S.Literal("removeRules"),
  ...PermissionUpdateBase,
});

export const SetModePermissionUpdate = S.Struct({
  type: S.Literal("setMode"),
  mode: PermissionMode,
  destination: PermissionUpdateDestination,
});

export const AddDirectoriesPermissionUpdate = S.Struct({
  type: S.Literal("addDirectories"),
  directories: S.Array(S.String),
  destination: PermissionUpdateDestination,
});

export const RemoveDirectoriesPermissionUpdate = S.Struct({
  type: S.Literal("removeDirectories"),
  directories: S.Array(S.String),
  destination: PermissionUpdateDestination,
});

/**
 * Union of all permission update operations.
 */
export const PermissionUpdate = S.Union(
  AddRulesPermissionUpdate,
  ReplaceRulesPermissionUpdate,
  RemoveRulesPermissionUpdate,
  SetModePermissionUpdate,
  AddDirectoriesPermissionUpdate,
  RemoveDirectoriesPermissionUpdate
);
export type PermissionUpdate = S.Schema.Type<typeof PermissionUpdate>;

// =============================================================================
// Permission Results
// =============================================================================

/**
 * Allow result with optional updated permissions.
 */
export const AllowPermissionResult = S.Struct({
  behavior: S.Literal("allow"),
  updatedInput: S.Unknown,
  updatedPermissions: S.optional(S.Array(PermissionUpdate)),
});

/**
 * Deny result with optional interrupt flag.
 */
export const DenyPermissionResult = S.Struct({
  behavior: S.Literal("deny"),
  message: S.String,
  interrupt: S.optional(S.Boolean),
});

/**
 * PermissionResult models the callback response from permission checks.
 */
export const PermissionResult = S.Union(AllowPermissionResult, DenyPermissionResult);
export type PermissionResult = S.Schema.Type<typeof PermissionResult>;

// =============================================================================
// Function Types (non-schema)
// =============================================================================

export type CanUseTool = (
  toolName: string,
  input: unknown,
  options: { signal: AbortSignal; suggestions?: PermissionUpdate[] }
) => Promise<PermissionResult>;
