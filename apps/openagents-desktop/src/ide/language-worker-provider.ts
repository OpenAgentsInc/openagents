import { Worker } from "node:worker_threads";

import {
  IdeLanguageProviderStartSchema,
  type IdeLanguageCancelRequest,
  type IdeLanguageProviderStart,
  type IdeLanguageStopRequest,
} from "./language-contract.ts";
import type { IdeLanguageProvider } from "./language-service.ts";
import type {
  IdePortableMutationAuthority,
  IdePortableMutationPermit,
} from "./portable-mutation-authority.ts";

type WorkerOutput =
  | Readonly<{ kind: "ready"; providerVersion: string }>
  | Readonly<{ kind: "result"; result: unknown }>
  | Readonly<{ kind: "failed"; requestRef: string; message: string; recoverable: boolean }>;

type PendingRequest = Readonly<{
  resolve: (value: unknown) => void;
  reject: (cause: Error) => void;
}>;

type LanguageUtilityWorker = Readonly<{
  postMessage: (value: unknown) => void;
  onMessage: (listener: (value: unknown) => void) => void;
  onceError: (listener: (error: Error) => void) => void;
  onceExit: (listener: (code: number) => void) => void;
  terminate: () => Promise<number>;
}>;

type LanguageUtilityWorkerFactory = (url: URL) => LanguageUtilityWorker | Worker;

const adaptNodeLanguageUtilityWorker = (worker: Worker): LanguageUtilityWorker => ({
  postMessage: (value) => worker.postMessage(value),
  onMessage: (listener) => {
    worker.on("message", listener);
  },
  onceError: (listener) => {
    worker.once("error", listener);
  },
  onceExit: (listener) => {
    worker.once("exit", listener);
  },
  terminate: () => worker.terminate(),
});

const makeNodeLanguageUtilityWorker: LanguageUtilityWorkerFactory = (url) => new Worker(url);

const adaptLanguageUtilityWorker = (
  worker: LanguageUtilityWorker | Worker,
): LanguageUtilityWorker =>
  worker instanceof Worker ? adaptNodeLanguageUtilityWorker(worker) : worker;

const capabilities = [
  "diagnostics",
  "completion",
  "completion_resolve",
  "hover",
  "definition",
  "declaration",
  "type_definition",
  "references",
  "document_symbols",
  "workspace_symbols",
  "rename_preview",
  "format_document",
  "format_range",
  "code_actions",
  "semantic_tokens",
  "inlay_hints",
  "folding_ranges",
] as const;

const isWorkerOutput = (value: unknown): value is WorkerOutput => {
  if (typeof value !== "object" || value === null) return false;
  const kind = (value as { kind?: unknown }).kind;
  return kind === "ready" || kind === "result" || kind === "failed";
};

export const makeIdeLanguageWorkerProvider = (
  root: string,
  workerUrl: URL,
  grantRefOrMakeWorker: string | LanguageUtilityWorkerFactory,
  mutationAuthority?: IdePortableMutationAuthority,
  makeWorkerOverride?: LanguageUtilityWorkerFactory,
): IdeLanguageProvider => {
  const grantRef =
    typeof grantRefOrMakeWorker === "string" ? grantRefOrMakeWorker : "workspace.language";
  const makeWorker =
    typeof grantRefOrMakeWorker === "function"
      ? grantRefOrMakeWorker
      : (makeWorkerOverride ?? makeNodeLanguageUtilityWorker);
  let worker: LanguageUtilityWorker | null = null;
  let workerPermit: IdePortableMutationPermit | null = null;
  let startProjection: IdeLanguageProviderStart | null = null;
  let startPromise: Promise<IdeLanguageProviderStart> | null = null;
  let rejectStart: ((cause: Error) => void) | null = null;
  let startOrdinal = 0;
  const pending = new Map<string, PendingRequest>();

  const rejectPending = (cause: Error): void => {
    for (const request of pending.values()) request.reject(cause);
    pending.clear();
  };

  const capturePermit = (): IdePortableMutationPermit | null => {
    if (mutationAuthority === undefined) return null;
    const authorization = mutationAuthority.authorize(grantRef);
    if (authorization._tag === "Refused") {
      throw new Error(`IDE language utility authority was refused: ${authorization.reason}.`);
    }
    return authorization.permit;
  };

  const permitIsCurrent = (permit: IdePortableMutationPermit | null): boolean =>
    mutationAuthority === undefined || (permit !== null && mutationAuthority.reauthorize(permit));

  const stopWorker = async (): Promise<void> => {
    const current = worker;
    worker = null;
    workerPermit = null;
    startProjection = null;
    startPromise = null;
    const rejectStarting = rejectStart;
    rejectStart = null;
    rejectStarting?.(new Error("IDE language utility stopped before startup completed."));
    if (current === null) return;
    try {
      current.postMessage({ kind: "stop" });
    } catch {
      // Worker already exited; termination below remains idempotent.
    }
    await current.terminate().catch(() => undefined);
  };

  const revokeWorker = (message: string): void => {
    rejectPending(new Error(message));
    void stopWorker();
  };

  const start = async (): Promise<IdeLanguageProviderStart> => {
    if (startProjection !== null) return startProjection;
    if (startPromise !== null) return startPromise;
    const permit = capturePermit();
    if (!permitIsCurrent(permit)) {
      throw new Error("IDE language utility authority changed before worker spawn.");
    }
    startPromise = new Promise<IdeLanguageProviderStart>((resolve, reject) => {
      rejectStart = reject;
      const utility = adaptLanguageUtilityWorker(makeWorker(workerUrl));
      worker = utility;
      workerPermit = permit;
      if (!permitIsCurrent(permit)) {
        reject(new Error("IDE language utility authority changed during worker spawn."));
        void stopWorker();
        return;
      }
      const onMessage = (value: unknown): void => {
        if (worker !== utility || workerPermit !== permit || !permitIsCurrent(permit)) {
          revokeWorker(
            "IDE language utility authority changed before a worker event could be accepted.",
          );
          return;
        }
        if (!isWorkerOutput(value)) {
          rejectPending(new Error("IDE language utility emitted a malformed message."));
          return;
        }
        if (value.kind === "ready") {
          const projection = IdeLanguageProviderStartSchema.make({
            executable: "typescript/lib/tsserverlibrary",
            providerVersion: value.providerVersion,
            capabilities: capabilities.map((capability) => ({
              capability,
              available: true,
              reason: null,
            })),
          });
          startProjection = projection;
          rejectStart = null;
          resolve(projection);
          return;
        }
        if (value.kind === "failed") {
          const request = pending.get(value.requestRef);
          if (request === undefined) return;
          pending.delete(value.requestRef);
          request.reject(new Error(value.message));
          return;
        }
        const requestRef =
          typeof value.result === "object" && value.result !== null
            ? String((value.result as { requestRef?: unknown }).requestRef ?? "")
            : "";
        const request = pending.get(requestRef);
        if (request === undefined) return;
        pending.delete(requestRef);
        request.resolve(value.result);
      };
      utility.onMessage(onMessage);
      utility.onceError((error) => {
        if (worker !== utility) return;
        rejectStart = null;
        startProjection = null;
        startPromise = null;
        rejectPending(error);
        reject(error);
      });
      utility.onceExit((code) => {
        if (worker === utility) worker = null;
        if (workerPermit === permit) workerPermit = null;
        const startupWasPending = startProjection === null && rejectStart !== null;
        const rejectPendingStart = rejectStart;
        rejectStart = null;
        startProjection = null;
        startPromise = null;
        if (code !== 0 || startupWasPending) {
          const error = new Error(
            `IDE language utility exited with code ${code} before startup completed.`,
          );
          rejectPending(error);
          rejectPendingStart?.(error);
          reject(error);
        }
      });
    });
    const starting = startPromise;
    try {
      const projection = await starting;
      startOrdinal += 1;
      return projection;
    } catch (cause) {
      if (startPromise === starting) startPromise = null;
      throw cause;
    }
  };

  return {
    start,
    request: async (request) => {
      if (startProjection === null && startOrdinal > 0) {
        throw new Error("IDE language utility exited and requires a supervised service restart.");
      }
      const projection = await start();
      const utility = worker;
      if (utility === null) throw new Error("IDE language utility is unavailable.");
      const permit = workerPermit;
      if (!permitIsCurrent(permit)) {
        revokeWorker("IDE language utility authority changed before the request was sent.");
        throw new Error("IDE language utility authority changed before the request was sent.");
      }
      const serviceGeneration = startOrdinal;
      const restartCount = Math.max(0, startOrdinal - 1);
      const requestRef = String(request.requestRef);
      return await new Promise<unknown>((resolve, reject) => {
        pending.set(requestRef, { resolve, reject });
        utility.postMessage({
          kind: "request",
          root,
          request,
          service: {
            serviceRef: "ide.language-service.typescript",
            serviceGeneration,
            startRef: `ide.language-start.${serviceGeneration}.${restartCount}`,
            placementRef: "ide.placement.project-local",
            executable: projection.executable,
            providerVersion: projection.providerVersion,
          },
        });
        if (!permitIsCurrent(permit)) {
          revokeWorker("IDE language utility authority changed while the request was sent.");
        }
      });
    },
    cancel: async (request: IdeLanguageCancelRequest) => {
      const requestRef = String(request.requestRef);
      if (!permitIsCurrent(workerPermit)) {
        revokeWorker("IDE language utility authority changed before cancellation.");
        return;
      }
      worker?.postMessage({
        kind: "cancel",
        requestRef,
        reason: request.reason,
      });
    },
    stop: async (_request: IdeLanguageStopRequest) => {
      rejectPending(new Error("IDE language project stopped."));
      await stopWorker();
    },
  };
};
