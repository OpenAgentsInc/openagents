import { Effect, Schema as S } from "effect";
import type { HarnessBootstrap } from "./bootstrap.ts";

/**
 * Typed failure of any sandbox-provider operation. `operation` names the verb
 * that failed (`createSession`, `writeTextFile`, `run`, …) so telemetry and
 * conformance tests can name a refusal precisely. `cause` carries the raw defect
 * (a spawn error, an IO error) without leaking it into the neutral event stream.
 *
 * This is the sandbox-layer analogue of {@link HarnessTurnError}: adapters never
 * see provider-specific error shapes, only this tagged error.
 */
export class HarnessSandboxError extends S.TaggedErrorClass<HarnessSandboxError>()(
  "AgentHarness.SandboxError",
  {
    operation: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/** Result of a command executed inside a sandbox session. */
export interface HarnessSandboxRunResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

/**
 * A live sandbox session workspace: the per-session directory the framework
 * composed, with file I/O and command execution. This is the seam a harness
 * adapter's runtime runs against — write bridge files, read outputs, run the
 * coding-agent process — without ever knowing which concrete substrate backs it
 * (a local child-process sandbox, the managed Phase-1 sandbox, just-bash).
 *
 * The fail-closed capability posture matches the rest of the contract: there is
 * NO static capability matrix. Optional behavior is signalled by the presence or
 * absence of an optional method. A provider that cannot expose ports omits
 * `getPortUrl`; a caller uses an optional call and treats absence as "no port
 * capability". This mirrors how just-bash omits ports and how the Phase-1
 * managed sandbox refuses them.
 *
 * Lifecycle is provider-owned. `stop` exists for the framework, NOT the adapter:
 * the harness adapter must NEVER call `stop`, exactly as it never owns the
 * sandbox lifecycle in {@link HarnessStartOptions}. The provider that created
 * the session tears it down.
 */
export interface HarnessSandboxSession {
  /** Stable id for this session workspace (conventionally the harness `sessionId`). */
  readonly id: string;
  /**
   * The per-session working directory the framework composed. For a base and a
   * `sessionId` the provider composes `<base>/<sessionId>`; the adapter reads
   * this and never derives provider-specific paths.
   */
  readonly workingDirectory: string;
  /** Write a UTF-8 text file into the session workspace. */
  readonly writeTextFile: (params: {
    readonly path: string;
    readonly content: string;
  }) => Effect.Effect<void, HarnessSandboxError>;
  /** Read a UTF-8 text file from the session workspace. */
  readonly readTextFile: (params: {
    readonly path: string;
  }) => Effect.Effect<string, HarnessSandboxError>;
  /** Run a command in the session workspace (defaults to `workingDirectory`). */
  readonly run: (params: {
    readonly command: string;
    readonly cwd?: string;
  }) => Effect.Effect<HarnessSandboxRunResult, HarnessSandboxError>;
  /**
   * OPTIONAL. Expose a URL for a port the session is listening on. A provider
   * that cannot expose ports OMITS this method; the caller uses an optional call
   * and treats absence as "no port capability". just-bash omits it and the
   * Phase-1 managed sandbox omits it too.
   */
  readonly getPortUrl?: (params: {
    readonly port: number;
    readonly protocol?: "http" | "https" | "ws";
  }) => Effect.Effect<string, HarnessSandboxError>;
  /**
   * OPTIONAL. Constrain the session's network egress. A provider with no network
   * policy control OMITS this method; the caller treats absence as "no network
   * policy capability".
   */
  readonly setNetworkPolicy?: (policy: {
    readonly egress: "deny-all" | "allow-list";
    readonly allow?: ReadonlyArray<string>;
  }) => Effect.Effect<void, HarnessSandboxError>;
  /**
   * Framework-owned teardown. The adapter must NEVER call this — the provider
   * owns the sandbox lifecycle. Present so the framework can dispose the session
   * after a run completes.
   */
  readonly stop: () => Effect.Effect<void, HarnessSandboxError>;
}

/** Options for {@link HarnessSandboxProvider.createSession}. */
export interface HarnessSandboxCreateOptions {
  readonly sessionId: string;
  /**
   * Stable identity for snapshot/checkpoint reuse. Two `createSession` calls with
   * the same `identity` may resume from the same baked snapshot; `bootstrap` and
   * `onFirstCreate` run exactly ONCE per identity on fresh create. Absent when the
   * caller has no reusable identity.
   */
  readonly identity?: string;
  /**
   * Bootstrap recipe (files to ship, commands to run) applied once per identity
   * on fresh create. See {@link HarnessBootstrap}.
   */
  readonly bootstrap?: HarnessBootstrap;
  /**
   * Ran exactly once per `identity` on fresh create, after the session workspace
   * and any `bootstrap` files exist. A provider without snapshots runs it
   * immediately after create; a provider with snapshots runs it before baking the
   * snapshot so the effect is captured in the reusable image.
   */
  readonly onFirstCreate?: (
    session: HarnessSandboxSession,
  ) => Effect.Effect<void, HarnessSandboxError>;
}

/** Options for {@link HarnessSandboxProvider.resumeSession}. */
export interface HarnessSandboxResumeOptions {
  readonly sessionId: string;
}

/**
 * A sandbox provider: the stable, I/O-free-at-construction factory for
 * {@link HarnessSandboxSession} workspaces. It is modelled after a language-model
 * provider — a tagged spec version, a stable `providerId`, and factory methods
 * that do the actual I/O — so a provider can be constructed at module scope and
 * shared, exactly like a model provider.
 *
 * The concrete substrates that implement this port land in the desktop cutover:
 * the real local provider over `child_process`, and the managed-sandbox provider
 * from `@openagentsinc/managed-sandbox-contract`. This package ships only the
 * contract and the in-memory {@link makeLocalSandboxProvider} reference double.
 *
 * Fail-closed capability posture: `resumeSession` is OPTIONAL. A provider that
 * cannot rehydrate a session by id OMITS it; a resume attempt against such a
 * provider surfaces `HarnessCapabilityUnsupported("sandbox")` at the call site.
 * Phase-1 managed sandbox omits `getPortUrl` on its sessions for the same
 * fail-closed reason.
 */
export interface HarnessSandboxProvider {
  readonly specificationVersion: "harness-sandbox-v1";
  /** Stable kebab-case slug identifying the provider (`local-memory`, `managed`). */
  readonly providerId: string;
  /**
   * Create (or reuse, by `identity`) a session workspace. The framework composes
   * the per-session working directory as `<base>/<sessionId>`. `bootstrap` and
   * `onFirstCreate` run exactly once per `identity` on fresh create (snapshot
   * reuse). The provider OWNS the sandbox lifecycle — the adapter must never stop
   * the returned session.
   */
  readonly createSession: (
    options: HarnessSandboxCreateOptions,
  ) => Effect.Effect<HarnessSandboxSession, HarnessSandboxError>;
  /**
   * OPTIONAL. Rehydrate a previously-created session by id. Providers that cannot
   * rehydrate OMIT this method; a caller that needs resume against such a provider
   * surfaces `HarnessCapabilityUnsupported("sandbox")`.
   */
  readonly resumeSession?: (
    options: HarnessSandboxResumeOptions,
  ) => Effect.Effect<HarnessSandboxSession, HarnessSandboxError>;
}
