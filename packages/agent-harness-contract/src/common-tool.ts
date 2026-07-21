import { Schema as S } from "effect";

/**
 * The small cross-runtime common tool vocabulary. Adapters map their runtime's
 * native tool names onto these so one UI, one usage accountant, and one
 * transcript analytic (and MemoHarness policy learning) can reason about "bash"
 * without a per-lane name table. A native tool with no common equivalent (e.g.
 * `WebFetch`, `NotebookEdit`) keeps its native name and has no common name.
 */
export const COMMON_TOOL_NAMES = [
  "read",
  "write",
  "edit",
  "bash",
  "glob",
  "grep",
  "webSearch",
] as const;

export type CommonToolName = (typeof COMMON_TOOL_NAMES)[number];

export const CommonToolNameSchema = S.Literals(COMMON_TOOL_NAMES);

/**
 * Native tool name -> common name. Covers the known built-ins of the currently
 * integrated runtimes (Claude Code PascalCase, Codex/shell snake/lower). Names
 * outside this map have no common equivalent and are forwarded as-is.
 */
const NATIVE_TO_COMMON: Readonly<Record<string, CommonToolName>> = {
  // Claude Code
  Read: "read",
  Write: "write",
  Edit: "edit",
  Bash: "bash",
  Glob: "glob",
  Grep: "grep",
  WebSearch: "webSearch",
  // Codex / shell-style
  shell: "bash",
  read_file: "read",
  write_file: "write",
  apply_patch: "edit",
  web_search: "webSearch",
};

/** The common name for a native tool name, or `undefined` when there is none. */
export const commonToolName = (nativeName: string): CommonToolName | undefined =>
  NATIVE_TO_COMMON[nativeName];

/**
 * Normalized tool identity for a `tool.call` / `tool.result` event: the wire
 * name (`commonName ?? nativeName`), the preserved native name, and whether the
 * runtime executed the tool itself (a built-in) versus the host needing to
 * dispatch it.
 */
export const HarnessToolIdentity = S.Struct({
  /** The name to key transcript/analytics on: common when known, else native. */
  wireName: S.NonEmptyString,
  /** The runtime's native tool name, always preserved. */
  nativeName: S.NonEmptyString,
  /** The common vocabulary name, when the native tool has one. */
  commonName: S.optionalKey(CommonToolNameSchema),
  /** True for runtime-executed built-ins; false/absent for host-dispatched tools. */
  providerExecuted: S.optionalKey(S.Boolean),
});
export interface HarnessToolIdentity extends S.Schema.Type<typeof HarnessToolIdentity> {}

/** Build the normalized identity for a native tool name. */
export const toolIdentity = (
  nativeName: string,
  options?: { readonly providerExecuted?: boolean },
): HarnessToolIdentity => {
  const commonName = commonToolName(nativeName);
  return {
    wireName: commonName ?? nativeName,
    nativeName,
    ...(commonName === undefined ? {} : { commonName }),
    ...(options?.providerExecuted === undefined
      ? {}
      : { providerExecuted: options.providerExecuted }),
  };
};

/**
 * A built-in tool the adapter's underlying runtime exposes natively, as adapter
 * metadata (distinct from a host-executed {@link HarnessHostToolSpec}). The
 * framework uses `nativeName`/`commonName` to normalize tool events and to route
 * built-in tool filtering/approval.
 */
export const HarnessBuiltinTool = S.Struct({
  /** The runtime's native tool name (`Bash`, `shell`, `apply_patch`). */
  nativeName: S.NonEmptyString,
  /** Common vocabulary name, when the native tool maps onto one. */
  commonName: S.optionalKey(CommonToolNameSchema),
  /** Human/model-facing description. */
  description: S.optionalKey(S.String),
});
export interface HarnessBuiltinTool extends S.Schema.Type<typeof HarnessBuiltinTool> {}
