import { AsyncLocalStorage } from "node:async_hooks";
import { createHash } from "node:crypto";
import { realpathSync } from "node:fs";

import { Context, Effect, Exit, Layer, Scope } from "effect";

import {
  IdeRepositoryGenerationSchema,
  IdeRepositoryRefSchema,
  IdeSourceControlCommandResultSchema,
  IdeSourceControlConfigGenerationSchema,
  IdeSourceControlCredentialHelperGenerationSchema,
  IdeSourceControlFailureSchema,
  IdeSourceControlRefGenerationSchema,
  IdeSourceControlRemoteGenerationSchema,
  IdeSourceControlSnapshotSchema,
  decodeIdeSourceControlCommand,
  type IdeSourceControlBinding,
  type IdeSourceControlCommandResult,
  type IdeSourceControlFailure,
  type IdeSourceControlSnapshot,
} from "./source-control-contract.ts";
import { makeIdeSourceControlGitAdapter } from "./source-control-git-adapter.ts";
import {
  IdeSourceControlService,
  makeIdeSourceControlServiceLayer,
  type IdeSourceControlServiceShape,
} from "./source-control-service.ts";
import {
  IdeAttachmentGenerationSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

export type IdeSourceControlWorkspaceBinding = Readonly<{ root: string; grantRef: string }>;

export type IdeSourceControlHost = Readonly<{
  snapshot: () => Promise<IdeSourceControlSnapshot | null>;
  command: (value: unknown) => Promise<IdeSourceControlCommandResult>;
  dispose: () => Promise<void>;
}>;

export type IdeSourceControlHostOptions = Readonly<{
  workspace: () => IdeSourceControlWorkspaceBinding | null;
  mutationAuthority?: IdePortableMutationAuthority;
  now?: () => string;
  recoveryRoot?: string;
}>;

type Runtime = Readonly<{
  root: string;
  grantRef: string;
  scope: Scope.Closeable;
  service: IdeSourceControlServiceShape;
  binding: IdeSourceControlBinding;
}>;

const digest = (value: string): string => createHash("sha256").update(value).digest("hex");

export const ideSourceControlBindingFor = (
  workspace: IdeSourceControlWorkspaceBinding,
): IdeSourceControlBinding => {
  const identity = digest(`${workspace.root}\0${workspace.grantRef}`).slice(0, 24);
  return {
    projectRef: IdeProjectRefSchema.make(`ide.project.${identity}`),
    rootRef: IdeRootRefSchema.make(`ide.root.${identity}`),
    worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.${identity}`),
    attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
    repositoryRef: IdeRepositoryRefSchema.make(`ide.repository.${identity}`),
  };
};

const seedSnapshot = (
  binding: IdeSourceControlBinding,
  now: () => string,
): IdeSourceControlSnapshot => IdeSourceControlSnapshotSchema.make({
  schemaVersion: "openagents.desktop.ide-source-control.v1",
  binding,
  version: {
    repositoryGeneration: IdeRepositoryGenerationSchema.make(1),
    statusRef: "ide.scm-status.unobserved",
    headOid: null,
    indexOid: "unobserved-index",
    worktreeOid: "unobserved-worktree",
    refGeneration: IdeSourceControlRefGenerationSchema.make(1),
    configGeneration: IdeSourceControlConfigGenerationSchema.make(1),
    remoteGeneration: IdeSourceControlRemoteGenerationSchema.make(1),
    credentialHelperGeneration: IdeSourceControlCredentialHelperGenerationSchema.make(1),
  },
  branch: null,
  upstream: null,
  detached: true,
  ahead: 0,
  behind: 0,
  operation: { _tag: "Idle" },
  paths: [],
  worktrees: [],
  delivery: [],
  omittedPathCount: 0,
  truncated: false,
  observedAt: now(),
  stopped: false,
});

const unavailable = (
  code: "invalid_command" | "repository_unavailable" | "policy_refused",
  message: string,
  operationRef: IdeSourceControlFailure["operationRef"] = null,
  currentVersion: IdeSourceControlSnapshot["version"] | null = null,
) =>
  IdeSourceControlCommandResultSchema.cases.Failure.make({
    failure: IdeSourceControlFailureSchema.make({
      schemaVersion: "openagents.desktop.ide-source-control.v1",
      operationRef,
      code,
      message,
      currentVersion,
      conflictPaths: [],
      recoveryRef: null,
      retryable: code !== "invalid_command",
    }),
  });

export const openIdeSourceControlHost = async (
  options: IdeSourceControlHostOptions,
): Promise<IdeSourceControlHost> => {
  const now = options.now ?? (() => new Date().toISOString());
  let runtime: Runtime | null = null;
  let disposed = false;
  const mutationAuthority = options.mutationAuthority;
  const permitStorage = new AsyncLocalStorage<IdePortableMutationPermit>();
  let operationTail: Promise<void> = Promise.resolve();

  const serialized = async <A>(operation: () => Promise<A>): Promise<A> => {
    const previous = operationTail;
    let release: () => void = () => undefined;
    operationTail = new Promise<void>((resolve) => { release = resolve; });
    await previous;
    try {
      return await operation();
    } finally {
      release();
    }
  };

  const closeRuntime = async (reason: string): Promise<void> => {
    const current = runtime;
    runtime = null;
    if (current === null) return;
    await Effect.runPromise(current.service.stop(reason)).catch(() => undefined);
    await Effect.runPromise(Scope.close(current.scope, Exit.void)).catch(() => undefined);
  };

  const ensureRuntime = async (): Promise<Runtime | null> => {
    if (disposed) return null;
    const workspace = options.workspace();
    if (workspace === null) {
      await closeRuntime("workspace unavailable");
      return null;
    }
    let root: string;
    try {
      root = realpathSync(workspace.root);
    } catch {
      await closeRuntime("repository unavailable");
      return null;
    }
    if (runtime !== null && runtime.root === root && runtime.grantRef === workspace.grantRef) {
      return runtime;
    }
    await closeRuntime("workspace grant changed");
    const binding = ideSourceControlBindingFor({ root, grantRef: workspace.grantRef });
    const seed = seedSnapshot(binding, now);
    const scope = await Effect.runPromise(Scope.make());
    const adapter = makeIdeSourceControlGitAdapter({
      root, seed, now, recoveryRoot: options.recoveryRoot,
      mutationAuthority,
      mutationPermit: () => permitStorage.getStore(),
    });
    const context = await Effect.runPromise(Layer.buildWithScope(
      makeIdeSourceControlServiceLayer(seed, adapter, { now }),
      scope,
    ));
    const opened = { root, grantRef: workspace.grantRef, scope, binding, service: Context.get(context, IdeSourceControlService) };
    runtime = opened;
    return opened;
  };

  const snapshotUnlocked = async (): Promise<IdeSourceControlSnapshot | null> => {
    const current = await ensureRuntime();
    if (current === null) return null;
    const settled = await Effect.runPromise(current.service.execute({
      _tag: "Refresh",
      binding: current.binding,
    }).pipe(Effect.match({
      onFailure: () => null,
      onSuccess: (result) => result.snapshot,
    })));
    return settled;
  };
  const snapshot = async (): Promise<IdeSourceControlSnapshot | null> => serialized(snapshotUnlocked);

  const commandUnlocked = async (value: unknown): Promise<IdeSourceControlCommandResult> => {
    const decoded = decodeIdeSourceControlCommand(value);
    if (decoded === null) return unavailable("invalid_command", "The source-control command is invalid.");
    const current = await ensureRuntime();
    if (current === null) return unavailable("repository_unavailable", "Choose an available Git workspace.");
    const mutatesRepository = !["Refresh", "History", "Blame", "ProviderRefresh"].includes(decoded._tag);
    let permit: IdePortableMutationPermit | null = null;
    if (mutatesRepository && mutationAuthority !== undefined) {
      const authorized = mutationAuthority.authorize(current.grantRef);
      if (authorized._tag === "Refused") {
        return unavailable(
          "policy_refused",
          `Portable source-control authority is unavailable (${authorized.reason}).`,
          decoded._tag === "Refresh" ? null : decoded.operationRef,
          (await Effect.runPromise(current.service.snapshot())).version,
        );
      }
      permit = authorized.permit;
      if (!mutationAuthority.reauthorize(permit)) {
        await closeRuntime("portable source-control authority changed before execution");
        return unavailable(
          "policy_refused",
          "Portable source-control authority changed before the Git operation.",
          decoded._tag === "Refresh" ? null : decoded.operationRef,
        );
      }
    }
    const execute = () => Effect.runPromise(current.service.execute(decoded).pipe(Effect.match({
      onFailure: (error) => IdeSourceControlCommandResultSchema.cases.Failure.make({ failure: error.failure }),
      onSuccess: (result) => IdeSourceControlCommandResultSchema.cases.Success.make(result),
    })));
    const result = permit === null ? await execute() : await permitStorage.run(permit, execute);
    if (permit !== null && mutationAuthority !== undefined && !mutationAuthority.reauthorize(permit)) {
      await closeRuntime("portable source-control authority changed during execution");
      return unavailable(
        "policy_refused",
        "Portable source-control authority changed during the Git operation. The runtime was stopped and its late result was withheld.",
        decoded._tag === "Refresh" ? null : decoded.operationRef,
        result._tag === "Success" ? result.snapshot.version : result.failure.currentVersion,
      );
    }
    return result;
  };
  const command = async (value: unknown): Promise<IdeSourceControlCommandResult> => serialized(() => commandUnlocked(value));

  const disposeUnlocked = async (): Promise<void> => {
    if (disposed) return;
    disposed = true;
    await closeRuntime("host disposed");
  };
  const dispose = async (): Promise<void> => serialized(disposeUnlocked);

  return { snapshot, command, dispose };
};
