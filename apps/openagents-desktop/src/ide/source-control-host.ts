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

export type IdeSourceControlWorkspaceBinding = Readonly<{ root: string; grantRef: string }>;

export type IdeSourceControlHost = Readonly<{
  snapshot: () => Promise<IdeSourceControlSnapshot | null>;
  command: (value: unknown) => Promise<IdeSourceControlCommandResult>;
  dispose: () => Promise<void>;
}>;

export type IdeSourceControlHostOptions = Readonly<{
  workspace: () => IdeSourceControlWorkspaceBinding | null;
  now?: () => string;
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

const unavailable = (code: "invalid_command" | "repository_unavailable", message: string) =>
  IdeSourceControlCommandResultSchema.cases.Failure.make({
    failure: IdeSourceControlFailureSchema.make({
      schemaVersion: "openagents.desktop.ide-source-control.v1",
      operationRef: null,
      code,
      message,
      currentVersion: null,
      conflictPaths: [],
      recoveryRef: null,
      retryable: code === "repository_unavailable",
    }),
  });

export const openIdeSourceControlHost = async (
  options: IdeSourceControlHostOptions,
): Promise<IdeSourceControlHost> => {
  const now = options.now ?? (() => new Date().toISOString());
  let runtime: Runtime | null = null;
  let disposed = false;

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
    const adapter = makeIdeSourceControlGitAdapter({ root, seed, now });
    const context = await Effect.runPromise(Layer.buildWithScope(
      makeIdeSourceControlServiceLayer(seed, adapter, { now }),
      scope,
    ));
    const opened = { root, grantRef: workspace.grantRef, scope, binding, service: Context.get(context, IdeSourceControlService) };
    runtime = opened;
    return opened;
  };

  const snapshot = async (): Promise<IdeSourceControlSnapshot | null> => {
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

  const command = async (value: unknown): Promise<IdeSourceControlCommandResult> => {
    const decoded = decodeIdeSourceControlCommand(value);
    if (decoded === null) return unavailable("invalid_command", "The source-control command is invalid.");
    const current = await ensureRuntime();
    if (current === null) return unavailable("repository_unavailable", "Choose an available Git workspace.");
    return Effect.runPromise(current.service.execute(decoded).pipe(Effect.match({
      onFailure: (error) => IdeSourceControlCommandResultSchema.cases.Failure.make({ failure: error.failure }),
      onSuccess: (result) => IdeSourceControlCommandResultSchema.cases.Success.make(result),
    })));
  };

  const dispose = async (): Promise<void> => {
    if (disposed) return;
    disposed = true;
    await closeRuntime("host disposed");
  };

  return { snapshot, command, dispose };
};
