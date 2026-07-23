import { Effect, Schema as S } from "effect";
import type {
  AgentDefinitionHarnessKind,
  AgentRuntimeAdapterKind,
  KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import type { HarnessBootstrap } from "./bootstrap.ts";
import type { HarnessBuiltinTool } from "./common-tool.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessBuiltinToolFiltering, HarnessPermissionMode } from "./permission.ts";
import type { HarnessSession } from "./session.ts";
import type { HarnessSkill } from "./skill.ts";

/** Typed failure of session startup. */
export class HarnessStartError extends S.TaggedErrorClass<HarnessStartError>()(
  "AgentHarness.StartError",
  {
    harnessId: S.String,
    sessionId: S.String,
    failureClass: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/**
 * Options passed to {@link AgentHarness.start}. The framework creates any
 * sandbox/workspace resource and the per-session working directory BEFORE
 * calling `start`, so the adapter never derives provider-specific paths and
 * never owns the sandbox lifecycle. `source` labels every event the adapter
 * emits (lane / adapterKind / model refs).
 */
export interface HarnessStartOptions {
  readonly sessionId: string;
  /** Event source labelling for this session's stream events. */
  readonly source: KhalaRuntimeSource;
  /** Skills available to the runtime for the session's lifetime. */
  readonly skills?: ReadonlyArray<HarnessSkill>;
  /** Approval policy for adapter-native built-in tools. */
  readonly permissionMode?: HarnessPermissionMode;
  /** Which built-in tools are active this session. */
  readonly builtinToolFiltering?: HarnessBuiltinToolFiltering;
  /** Resume a parked session; the adapter validates `data` against its schema. */
  readonly resumeFrom?: HarnessResumeState;
  /** Resume for continuation of an unfinished turn (slice boundary). */
  readonly continueFrom?: HarnessContinuationState;
}

/**
 * A harness adapter: the integration point for one third-party coding-agent
 * runtime (Codex, Claude Code, an ACP peer, …). Modelled after a
 * `LanguageModelV4` provider — a tagged spec, descriptive fields, one entry
 * method `start`.
 *
 * There is intentionally NO static capability object. Optional behavior is
 * signalled by the presence/absence of the optional session methods
 * (`suspendTurn`, `continueTurn`, `detach`, `compact`) and the two built-in
 * tool flags; a request an adapter cannot satisfy fails with
 * `HarnessCapabilityUnsupported`.
 */
export interface AgentHarness {
  readonly specificationVersion: "agent-harness-v1";
  /** Stable kebab-case slug, conventionally matching the runtime (`codex`, `claude-code`). */
  readonly harnessId: string;
  /** The canonical harness kind vocabulary (`@openagentsinc/agent-runtime-schema`). */
  readonly harnessKind: AgentDefinitionHarnessKind;
  /** The runtime-adapter kind this harness dispatches as. */
  readonly adapterKind: AgentRuntimeAdapterKind;
  /** Built-in tools the runtime exposes natively (for normalization/filtering). */
  readonly builtinTools: ReadonlyArray<HarnessBuiltinTool>;
  /** Whether the adapter can emit approval requests for built-in tools natively. */
  readonly supportsBuiltinToolApprovals?: boolean;
  /** Whether the adapter can hide inactive built-in tools from the runtime natively. */
  readonly supportsBuiltinToolFiltering?: boolean;
  /**
   * Schema validating the adapter-defined `data` payload of resume/continuation
   * state. Present when the adapter promises re-importable exported state.
   */
  readonly lifecycleStateSchema?: S.Top;
  /**
   * Bootstrap recipe (install deps / ship bridge files). Present only for
   * adapters that need sandbox setup; wired to snapshot identity in HARN-07.
   */
  readonly getBootstrap?: (options?: {
    readonly abortSignal?: AbortSignal;
  }) => Effect.Effect<HarnessBootstrap>;
  /**
   * Start a fresh session, resume a parked session (`resumeFrom`), or resume an
   * interrupted turn for continuation (`continueFrom`).
   */
  readonly start: (
    options: HarnessStartOptions,
  ) => Effect.Effect<HarnessSession, HarnessStartError>;
}
