import { createHash } from "node:crypto";
import { lstat, mkdir } from "node:fs/promises";
import { isAbsolute, join } from "node:path";

import type {
  PortablePhaseOperationRecord,
  PortablePhaseOperationRequest,
} from "@openagentsinc/portable-session-contract";

import {
  makePylonPortablePhaseOperationClient,
  type PylonPortablePhaseOperationClient,
} from "./portable-phase-operation-client.js";
import { makePylonPortablePhaseClaimJournal } from "./portable-phase-operation-claim-journal.js";
import {
  makePylonPortablePhaseExecutor,
  PylonPortablePhaseWorker,
  type PylonPortablePhaseTargetResolver,
} from "./portable-phase-operation-worker.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const PRIVATE_STATE_DIRECTORY = "portable-phase";
const CLAIM_JOURNAL_DIRECTORY = "claims";

type ExactPhaseContext = Exclude<
  Awaited<ReturnType<PylonPortablePhaseTargetResolver["resolve"]>>,
  undefined
>;

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const fingerprint = (request: PortablePhaseOperationRequest): string =>
  createHash("sha256").update(canonical(request)).digest("hex");

export class PylonPortablePhaseProductionError extends Error {
  override readonly name = "PylonPortablePhaseProductionError";

  constructor(
    readonly errorRef:
      | "error.pylon.portable-phase.invalid-production-config"
      | "error.pylon.portable-phase.unsupported-exact-context"
      | "error.pylon.portable-phase.worker-failed",
  ) {
    super("Pylon portable phase production worker failed closed");
  }
}

/**
 * Process-private phase context registry. It holds local target objects and
 * exact call inputs. The durable server exchange and logs receive refs only.
 */
export const makePylonPrivatePortablePhaseContextResolver = () => {
  const contexts = new Map<
    string,
    Readonly<{ requestFingerprint: string; context: ExactPhaseContext }>
  >();

  const resolver: PylonPortablePhaseTargetResolver = {
    resolve: async (request) => {
      const admitted = contexts.get(request.operationRef);
      if (admitted === undefined || admitted.requestFingerprint !== fingerprint(request)) {
        return undefined;
      }
      return admitted.context;
    },
  };

  return {
    resolver,
    admit: (request: PortablePhaseOperationRequest, context: ExactPhaseContext): void => {
      if (
        request.operationRef !== context.call.input.operationRef ||
        request.targetRef !== context.target.targetRef ||
        (context.operationRefSemantics !== "not_proven" &&
          context.operationRefSemantics !== "operation_ref_idempotent")
      ) {
        throw new PylonPortablePhaseProductionError(
          "error.pylon.portable-phase.invalid-production-config",
        );
      }
      const exact = fingerprint(request);
      const current = contexts.get(request.operationRef);
      if (
        current !== undefined &&
        (current.requestFingerprint !== exact || current.context !== context)
      ) {
        throw new PylonPortablePhaseProductionError(
          "error.pylon.portable-phase.invalid-production-config",
        );
      }
      contexts.set(request.operationRef, { requestFingerprint: exact, context });
    },
    remove: (operationRef: string): void => {
      contexts.delete(operationRef);
    },
  } as const;
};

export type PylonPrivatePortablePhaseContextResolver = ReturnType<
  typeof makePylonPrivatePortablePhaseContextResolver
>;

export const pylonPrivatePortablePhaseContexts =
  makePylonPrivatePortablePhaseContextResolver();

export const portablePhaseWorkerInstanceRef = (pylonRef: string, targetRef: string): string => {
  if (!SAFE_REF.test(pylonRef) || !SAFE_REF.test(targetRef)) {
    throw new PylonPortablePhaseProductionError(
      "error.pylon.portable-phase.invalid-production-config",
    );
  }
  const digest = createHash("sha256")
    .update(`${pylonRef}\0${targetRef}`)
    .digest("hex")
    .slice(0, 32);
  return `worker.pylon.portable-phase.${digest}`;
};

const abortableDelay = (milliseconds: number, signal: AbortSignal): Promise<void> =>
  new Promise((resolve, reject) => {
    if (signal.aborted) return reject(signal.reason);
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, milliseconds);
    const onAbort = () => {
      clearTimeout(timer);
      reject(signal.reason);
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });

const preflightClient = (
  client: PylonPortablePhaseOperationClient,
  resolver: PylonPortablePhaseTargetResolver,
): PylonPortablePhaseOperationClient => ({
  ...client,
  pending: async (limit, signal) => {
    const pending = await client.pending(limit, signal);
    for (const operation of pending) {
      if ((await resolver.resolve(operation.request)) === undefined) {
        throw new PylonPortablePhaseProductionError(
          "error.pylon.portable-phase.unsupported-exact-context",
        );
      }
    }
    return pending;
  },
});

export type OpenPylonPortablePhaseProductionWorkerOptions = Readonly<{
  agentToken: string;
  baseUrl: string;
  pylonRef: string;
  targetRef: string;
  workerInstanceRef: string;
  stateDirectory: string;
  resolver: PylonPortablePhaseTargetResolver;
  fetchImpl?: typeof globalThis.fetch;
  pollIntervalMs?: number;
  onFault?: (errorRef: PylonPortablePhaseProductionError["errorRef"]) => void;
}>;

export type PylonPortablePhaseProductionWorker = Readonly<{
  close: () => Promise<void>;
  status: () => Readonly<{
    state: "running" | "stopped" | "failed";
    errorRef: PylonPortablePhaseProductionError["errorRef"] | null;
  }>;
}>;

export const openPylonPortablePhaseProductionWorker = async (
  options: OpenPylonPortablePhaseProductionWorkerOptions,
): Promise<PylonPortablePhaseProductionWorker> => {
  const pollIntervalMs = options.pollIntervalMs ?? 5_000;
  if (
    options.agentToken.trim() === "" ||
    ![options.pylonRef, options.targetRef, options.workerInstanceRef].every((ref) =>
      SAFE_REF.test(ref),
    ) ||
    !isAbsolute(options.stateDirectory) ||
    !Number.isSafeInteger(pollIntervalMs) ||
    pollIntervalMs < 250 ||
    pollIntervalMs > 300_000
  ) {
    throw new PylonPortablePhaseProductionError(
      "error.pylon.portable-phase.invalid-production-config",
    );
  }

  const privateDirectory = join(options.stateDirectory, PRIVATE_STATE_DIRECTORY);
  const journalDirectory = join(privateDirectory, CLAIM_JOURNAL_DIRECTORY);
  await mkdir(privateDirectory, { recursive: true, mode: 0o700 });
  const privateDirectoryInfo = await lstat(privateDirectory);
  if (
    !privateDirectoryInfo.isDirectory() ||
    privateDirectoryInfo.isSymbolicLink() ||
    (privateDirectoryInfo.mode & 0o077) !== 0
  ) {
    throw new PylonPortablePhaseProductionError(
      "error.pylon.portable-phase.invalid-production-config",
    );
  }
  const client = makePylonPortablePhaseOperationClient({
    agentToken: options.agentToken,
    baseUrl: options.baseUrl,
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
    ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
  });
  const worker = new PylonPortablePhaseWorker({
    client: preflightClient(client, options.resolver),
    executor: makePylonPortablePhaseExecutor(options.resolver),
    journal: makePylonPortablePhaseClaimJournal({
      directory: journalDirectory,
      pylonRef: options.pylonRef,
      targetRef: options.targetRef,
      workerInstanceRef: options.workerInstanceRef,
    }),
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
    workerInstanceRef: options.workerInstanceRef,
  });
  const abort = new AbortController();
  let state: "running" | "stopped" | "failed" = "running";
  let errorRef: PylonPortablePhaseProductionError["errorRef"] | null = null;
  const run = (async () => {
    try {
      while (!abort.signal.aborted) {
        await worker.runPass(abort.signal);
        await abortableDelay(pollIntervalMs, abort.signal);
      }
    } catch (error) {
      if (abort.signal.aborted) return;
      state = "failed";
      errorRef =
        error instanceof PylonPortablePhaseProductionError
          ? error.errorRef
          : "error.pylon.portable-phase.worker-failed";
      options.onFault?.(errorRef);
    }
  })();

  return {
    close: async () => {
      if (state === "stopped") return;
      abort.abort(new Error("portable phase production worker stopped"));
      await run;
      state = "stopped";
    },
    status: () => ({ state, errorRef }),
  };
};

export const assertPortablePhasePendingSupported = async (
  pending: ReadonlyArray<PortablePhaseOperationRecord>,
  resolver: PylonPortablePhaseTargetResolver,
): Promise<void> => {
  for (const operation of pending) {
    if ((await resolver.resolve(operation.request)) === undefined) {
      throw new PylonPortablePhaseProductionError(
        "error.pylon.portable-phase.unsupported-exact-context",
      );
    }
  }
};
