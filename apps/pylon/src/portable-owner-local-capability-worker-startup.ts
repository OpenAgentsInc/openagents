import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";

import { OwnerLocalPortableCapabilityInstallationPort } from "@openagentsinc/khala-sync-server/portable-capability-installation-ports";
import type { PortableOwnerLocalCapabilityOperationRequest } from "@openagentsinc/portable-session-contract";
import { Duration, Effect, Schedule, Schema } from "effect";

import type { PylonPortablePhaseContextAdmissionStore } from "./portable-phase-context-admission.js";
import { makePylonPortableOwnerLocalCapabilityMaterialClient } from "./portable-owner-local-capability-material-client.js";
import { makePylonPortableOwnerLocalCapabilityOperationClient } from "./portable-owner-local-capability-operation-client.js";
import { makePylonPortableOwnerLocalCapabilityOperationExecutor } from "./portable-owner-local-capability-operation-executor.js";
import { makePylonPortableOwnerLocalCapabilityOperationJournal } from "./portable-owner-local-capability-operation-journal.js";
import {
  PylonPortableOwnerLocalCapabilityWorker,
  type PylonPortableOwnerLocalCapabilityExecutor,
  type PylonPortableOwnerLocalCapabilityWorkerOptions,
} from "./portable-owner-local-capability-operation-worker.js";
import type {
  PylonPortableControlBinding,
  PylonPortableSessionOperationLedger,
} from "./portable-session-operation-ledger.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

export class PylonOwnerLocalCapabilityWorkerStartupError extends Schema.TaggedErrorClass<PylonOwnerLocalCapabilityWorkerStartupError>()(
  "PylonOwnerLocalCapabilityWorkerStartupError",
  {
    reason: Schema.Literals(["binding_lost", "invalid_configuration", "worker_failed"]),
    failureRef: Schema.String,
  },
) {}

export type PylonOwnerLocalCapabilityWorkerStartupStatus = Readonly<{
  state: "waiting_binding" | "running" | "binding_lost" | "faulted" | "stopped";
  pylonRef: string;
  targetRef: string;
  sessionRef: string;
  workerInstanceRef: string;
  active: boolean;
  failureRef: string | null;
  material: "excluded";
}>;

type Worker = Pick<PylonPortableOwnerLocalCapabilityWorker, "runPass">;

type StartupDependencies = Readonly<{
  makeWorker?: (options: PylonPortableOwnerLocalCapabilityWorkerOptions) => Worker;
}>;

export type OpenPylonOwnerLocalCapabilityWorkerStartupOptions = Readonly<{
  agentToken: string;
  baseUrl: string;
  pylonHome: string;
  pylonRef: string;
  targetRef: string;
  sessionRef: string;
  workerInstanceRef: string;
  binding: PylonPortableControlBinding;
  ledger: PylonPortableSessionOperationLedger;
  authorityStore: Pick<PylonPortablePhaseContextAdmissionStore, "authorizesCapability">;
  targetBindingIsCurrent: () => boolean;
  pollIntervalMs?: number;
  bindingCheckIntervalMs?: number;
  dependencies?: StartupDependencies;
}>;

export type PylonOwnerLocalCapabilityWorkerStartup = Readonly<{
  reconcile: () => Promise<PylonOwnerLocalCapabilityWorkerStartupStatus>;
  status: () => PylonOwnerLocalCapabilityWorkerStartupStatus;
  close: () => Promise<void>;
}>;

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

const bindingFingerprint = (binding: PylonPortableControlBinding): string =>
  JSON.stringify({
    sessionRef: binding.sessionRef,
    attachmentRef: binding.attachmentRef,
    generation: binding.generation,
    runtimeInstanceRef: binding.runtimeInstanceRef,
    state: binding.state,
    revision: binding.revision,
    agents: binding.agents.map((agent) => ({
      agentRef: agent.agentRef,
      parentAgentRef: agent.parentAgentRef ?? null,
      controlSessionRef: agent.controlSessionRef,
      workspaceRef: agent.workspaceRef,
      processLifecycle: agent.processLifecycle,
      workspaceLifecycle: agent.workspaceLifecycle,
    })),
  });

const failure = (
  reason: PylonOwnerLocalCapabilityWorkerStartupError["reason"],
  scopeRef: string,
): PylonOwnerLocalCapabilityWorkerStartupError =>
  new PylonOwnerLocalCapabilityWorkerStartupError({
    reason,
    failureRef: stableRef(`failure.pylon.portable-capability-startup.${reason}`, scopeRef),
  });

export const openPylonOwnerLocalCapabilityWorkerStartup = (
  options: OpenPylonOwnerLocalCapabilityWorkerStartupOptions,
): PylonOwnerLocalCapabilityWorkerStartup => {
  const pollIntervalMs = options.pollIntervalMs ?? 1_000;
  const bindingCheckIntervalMs = options.bindingCheckIntervalMs ?? 500;
  if (
    options.agentToken.trim() === "" ||
    !isAbsolute(options.pylonHome) ||
    resolve(options.pylonHome) === resolve(homedir(), ".codex") ||
    ![
      options.pylonRef,
      options.targetRef,
      options.sessionRef,
      options.workerInstanceRef,
    ].every((ref) => SAFE_REF.test(ref)) ||
    options.binding.sessionRef !== options.sessionRef ||
    options.binding.state === "cleaned" ||
    options.binding.agents.length === 0 ||
    !Number.isSafeInteger(pollIntervalMs) ||
    pollIntervalMs < 10 ||
    pollIntervalMs > 30_000 ||
    !Number.isSafeInteger(bindingCheckIntervalMs) ||
    bindingCheckIntervalMs < 10 ||
    bindingCheckIntervalMs > 30_000
  ) {
    throw failure("invalid_configuration", options.targetRef);
  }

  const workerInstanceRef = options.workerInstanceRef;
  const expectedBinding = bindingFingerprint(options.binding);
  let state: PylonOwnerLocalCapabilityWorkerStartupStatus["state"] = "waiting_binding";
  let failureRef: string | null = null;
  let active:
    | Readonly<{ controller: AbortController; completion: Promise<void> }>
    | undefined;

  const status = (): PylonOwnerLocalCapabilityWorkerStartupStatus => ({
    state,
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
    sessionRef: options.sessionRef,
    workerInstanceRef,
    active: active !== undefined,
    failureRef,
    material: "excluded",
  });

  const exactBindingIsCurrent = async (): Promise<boolean> => {
    if (!options.targetBindingIsCurrent()) return false;
    try {
      const [binding, fence] = await Promise.all([
        Effect.runPromise(options.ledger.readControlBinding(options.sessionRef)),
        Effect.runPromise(options.ledger.readSession(options.sessionRef)),
      ]);
      return (
        bindingFingerprint(binding) === expectedBinding &&
        binding.state !== "cleaned" &&
        fence.sessionRef === binding.sessionRef &&
        fence.attachmentRef === binding.attachmentRef &&
        fence.generation === binding.generation &&
        (binding.state !== "accepting" || fence.acceptingWork)
      );
    } catch {
      return false;
    }
  };

  const requireRequestAuthority = async (
    request: PortableOwnerLocalCapabilityOperationRequest,
  ): Promise<void> => {
    if (
      !(await exactBindingIsCurrent()) ||
      request.pylonRef !== options.pylonRef ||
      request.targetRef !== options.targetRef ||
      request.sessionRef !== options.sessionRef ||
      request.attachmentRef !== options.binding.attachmentRef ||
      request.attachmentGeneration !== options.binding.generation ||
      !options.authorityStore.authorizesCapability({
        commandExecutionClaimRef: request.commandExecutionClaimRef,
        ownerRef: request.ownerRef,
        pylonRef: request.pylonRef,
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        attachmentGeneration: request.attachmentGeneration,
        targetRef: request.targetRef,
      })
    ) {
      throw failure("binding_lost", request.operationRef);
    }
  };

  const materialClient = makePylonPortableOwnerLocalCapabilityMaterialClient({
    agentToken: options.agentToken,
    baseUrl: options.baseUrl,
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
  });
  const executor: PylonPortableOwnerLocalCapabilityExecutor = {
    recoverySemantics: async () => "operation_ref_idempotent",
    execute: async (request, claim, signal) => {
      await requireRequestAuthority(request);
      const installationPort = new OwnerLocalPortableCapabilityInstallationPort({
        pylonHome: options.pylonHome,
        ownerRef: request.ownerRef,
        targetRef: options.targetRef,
      });
      return makePylonPortableOwnerLocalCapabilityOperationExecutor({
        materialClient,
        installationPort,
      }).execute(request, claim, signal);
    },
  };
  const client = makePylonPortableOwnerLocalCapabilityOperationClient({
    agentToken: options.agentToken,
    baseUrl: options.baseUrl,
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
  });
  const journal = makePylonPortableOwnerLocalCapabilityOperationJournal({
    directory: join(options.pylonHome, "portable-capability-operations"),
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
    workerInstanceRef,
  });
  const makeWorker = options.dependencies?.makeWorker ?? ((workerOptions) =>
    new PylonPortableOwnerLocalCapabilityWorker(workerOptions));

  const stop = async (
    nextState: PylonOwnerLocalCapabilityWorkerStartupStatus["state"],
    nextFailureRef: string | null,
  ): Promise<void> => {
    const running = active;
    if (running === undefined) {
      state = nextState;
      failureRef = nextFailureRef;
      return;
    }
    active = undefined;
    state = nextState;
    failureRef = nextFailureRef;
    running.controller.abort();
    await running.completion.catch(() => undefined);
  };

  const reconcile = async (): Promise<PylonOwnerLocalCapabilityWorkerStartupStatus> => {
    if (!(await exactBindingIsCurrent())) {
      const lost = failure("binding_lost", options.targetRef);
      await stop(active === undefined ? "waiting_binding" : "binding_lost", lost.failureRef);
      return status();
    }
    if (active !== undefined) return status();

    const controller = new AbortController();
    const worker = makeWorker({
      client,
      executor,
      journal,
      pylonRef: options.pylonRef,
      targetRef: options.targetRef,
      workerInstanceRef,
    });
    state = "running";
    failureRef = null;
    const workerPass = Effect.tryPromise({
      try: () => worker.runPass(controller.signal),
      catch: () => failure("worker_failed", options.targetRef),
    }).pipe(
      Effect.repeat(Schedule.spaced(Duration.millis(pollIntervalMs))),
    );
    const bindingPass = Effect.tryPromise({
      try: async () => {
        if (!(await exactBindingIsCurrent())) {
          const lost = failure("binding_lost", options.targetRef);
          controller.abort(lost);
          throw lost;
        }
      },
      catch: (error) =>
        error instanceof PylonOwnerLocalCapabilityWorkerStartupError
          ? error
          : failure("binding_lost", options.targetRef),
    }).pipe(
      Effect.repeat(Schedule.spaced(Duration.millis(bindingCheckIntervalMs))),
    );
    const completion = Effect.runPromise(
      Effect.all([workerPass, bindingPass], { concurrency: "unbounded", discard: true }),
      { signal: controller.signal },
    ).then(
      () => undefined,
      (error: unknown) => {
        if (active?.controller !== controller) return;
        active = undefined;
        const reason = controller.signal.reason;
        if (reason instanceof PylonOwnerLocalCapabilityWorkerStartupError) {
          state = reason.reason === "binding_lost" ? "binding_lost" : "faulted";
          failureRef = reason.failureRef;
        } else if (error instanceof PylonOwnerLocalCapabilityWorkerStartupError) {
          state = error.reason === "binding_lost" ? "binding_lost" : "faulted";
          failureRef = error.failureRef;
        } else if (state !== "stopped") {
          const fault = failure("worker_failed", options.targetRef);
          state = "faulted";
          failureRef = fault.failureRef;
        }
      },
    );
    active = { controller, completion };
    return status();
  };

  return {
    reconcile,
    status,
    close: () => stop("stopped", null),
  };
};
