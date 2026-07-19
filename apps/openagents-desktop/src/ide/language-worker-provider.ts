import { Worker } from "node:worker_threads";

import {
  IdeLanguageProviderStartSchema,
  type IdeLanguageCancelRequest,
  type IdeLanguageProviderStart,
  type IdeLanguageRequest,
  type IdeLanguageStopRequest,
} from "./language-contract.ts";
import type { IdeLanguageProvider } from "./language-service.ts";

type WorkerOutput =
  | Readonly<{ kind: "ready"; providerVersion: string }>
  | Readonly<{ kind: "result"; result: unknown }>
  | Readonly<{ kind: "failed"; requestRef: string; message: string; recoverable: boolean }>;

type PendingRequest = Readonly<{
  resolve: (value: unknown) => void;
  reject: (cause: Error) => void;
}>;

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
  makeWorker: (url: URL) => Worker = url => new Worker(url),
): IdeLanguageProvider => {
  let worker: Worker | null = null;
  let startProjection: IdeLanguageProviderStart | null = null;
  let startPromise: Promise<IdeLanguageProviderStart> | null = null;
  let startOrdinal = 0;
  const pending = new Map<string, PendingRequest>();

  const rejectPending = (cause: Error): void => {
    for (const request of pending.values()) request.reject(cause);
    pending.clear();
  };

  const stopWorker = async (): Promise<void> => {
    const current = worker;
    worker = null;
    startProjection = null;
    startPromise = null;
    if (current === null) return;
    try {
      current.postMessage({ kind: "stop" });
    } catch {
      // Worker already exited; termination below remains idempotent.
    }
    await current.terminate().catch(() => undefined);
  };

  const start = async (): Promise<IdeLanguageProviderStart> => {
    if (startProjection !== null) return startProjection;
    if (startPromise !== null) return startPromise;
    startPromise = new Promise<IdeLanguageProviderStart>((resolve, reject) => {
      const utility = makeWorker(workerUrl);
      worker = utility;
      const onMessage = (value: unknown): void => {
        if (!isWorkerOutput(value)) {
          rejectPending(new Error("IDE language utility emitted a malformed message."));
          return;
        }
        if (value.kind === "ready") {
          const projection = IdeLanguageProviderStartSchema.make({
            executable: "typescript/lib/tsserverlibrary",
            providerVersion: value.providerVersion,
            capabilities: capabilities.map(capability => ({
              capability,
              available: true,
              reason: null,
            })),
          });
          startProjection = projection;
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
        const requestRef = typeof value.result === "object" && value.result !== null
          ? String((value.result as { requestRef?: unknown }).requestRef ?? "")
          : "";
        const request = pending.get(requestRef);
        if (request === undefined) return;
        pending.delete(requestRef);
        request.resolve(value.result);
      };
      utility.on("message", onMessage);
      utility.once("error", error => {
        startProjection = null;
        startPromise = null;
        rejectPending(error);
        reject(error);
      });
      utility.once("exit", code => {
        if (worker === utility) worker = null;
        startProjection = null;
        startPromise = null;
        if (code !== 0) {
          const error = new Error(`IDE language utility exited with code ${code}.`);
          rejectPending(error);
          reject(error);
        }
      });
    });
    const projection = await startPromise;
    startOrdinal += 1;
    return projection;
  };

  return {
    start,
    request: async request => {
      if (startProjection === null && startOrdinal > 0) {
        throw new Error("IDE language utility exited and requires a supervised service restart.");
      }
      const projection = await start();
      const utility = worker;
      if (utility === null) throw new Error("IDE language utility is unavailable.");
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
      });
    },
    cancel: async (request: IdeLanguageCancelRequest) => {
      const requestRef = String(request.requestRef);
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
