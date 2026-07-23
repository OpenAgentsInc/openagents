import { Schema as S } from "effect";

/**
 * Permission mode for adapter-native (built-in) tools. Carried as policy on the
 * session, not inferred from the adapter. The desktop owner-local danger profile
 * is a legitimate mode here — the AI SDK's own host-process Pi adapter, which
 * sandboxes only the workspace and runs the runtime in-process, legitimizes an
 * always-allow posture — so the contract must be able to express it explicitly
 * rather than smuggling it in as an accident.
 *
 * - `allow-all`   — every built-in tool call is auto-approved. The owner-local
 *                   danger profile. Isolation, when wanted, comes from the
 *                   sandbox/account-home boundary, not the permission mode.
 * - `default`     — the adapter's native approval flow decides. Adapters that
 *                   cannot emit approvals fail with a capability error when this
 *                   mode requires one and no host interaction path is wired.
 * - `reject-all`  — every built-in tool call is denied before it executes. Used
 *                   both as a hard read-only posture and as the framework's
 *                   emulation of built-in tool filtering for adapters without
 *                   native filtering (an inactive tool routes through the
 *                   approval path and is auto-denied).
 */
export const HARNESS_PERMISSION_MODES = ["allow-all", "default", "reject-all"] as const;

export type HarnessPermissionMode = (typeof HARNESS_PERMISSION_MODES)[number];

export const HarnessPermissionModeSchema = S.Literals(HARNESS_PERMISSION_MODES);

/**
 * Per-session selection of which built-in tools are active. Absent means every
 * built-in tool the adapter exposes is active. When an adapter cannot filter
 * built-ins natively (see `builtin_tool_filtering` capability) the framework
 * emulates filtering by routing inactive built-in calls through the approval
 * path and auto-denying them, which requires `default`/`reject-all` semantics.
 */
export const HarnessBuiltinToolFiltering = S.Struct({
  activeTools: S.optionalKey(S.Array(S.String)),
  inactiveTools: S.optionalKey(S.Array(S.String)),
});

export interface HarnessBuiltinToolFiltering extends S.Schema.Type<
  typeof HarnessBuiltinToolFiltering
> {}
