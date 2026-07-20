import { Effect, Result, Schema } from "effect";

import {
  IdeLanguageCancelResponseSchema,
  IdeLanguageRequestResponseSchema,
  IdeLanguageServiceSnapshotSchema,
  IdeLanguageStopRequestSchema,
  IdeLanguageStopResponseSchema,
  type IdeLanguageCancelRequest,
  type IdeLanguageCancelResponse,
  type IdeLanguageRequest,
  type IdeLanguageRequestResponse,
  type IdeLanguageServiceSnapshot,
  type IdeLanguageStopRequest,
  type IdeLanguageStopResponse,
} from "./language-contract.ts";
import { IdeLanguageServiceRefSchema } from "./project-contract.ts";
import {
  type IdeLanguageServiceError,
  type IdeLanguageServiceShape,
  makeIdeLanguageService,
} from "./language-service.ts";
import { makeIdeLanguageWorkerProvider } from "./language-worker-provider.ts";
import type { IdePortableMutationAuthority } from "./portable-mutation-authority.ts";

export interface WorkspaceLanguageHost {
  readonly request: (request: IdeLanguageRequest) => Promise<IdeLanguageRequestResponse>;
  readonly cancel: (request: IdeLanguageCancelRequest) => Promise<IdeLanguageCancelResponse>;
  readonly stop: (request: IdeLanguageStopRequest) => Promise<IdeLanguageStopResponse>;
  readonly snapshot: () => Promise<IdeLanguageServiceSnapshot>;
  readonly dispose: () => void;
}

const rejectionReason = (
  error: IdeLanguageServiceError,
): Extract<IdeLanguageRequestResponse, { _tag: "Rejected" }>["reason"] => {
  switch (error._tag) {
    case "IdeLanguage.InvalidInput":
      return "invalid_request";
    case "IdeLanguage.StaleGeneration":
      return "stale_generation";
    case "IdeLanguage.ProviderUnavailable":
      return "provider_unavailable";
    case "IdeLanguage.TimedOut":
      return "timeout";
    case "IdeLanguage.MalformedResult":
      return "malformed_result";
    case "IdeLanguage.QueueFull":
      return "queue_full";
    case "IdeLanguage.Stopped":
      return "project_stopped";
  }
};

const rejectionMessage = (error: IdeLanguageServiceError): string => {
  switch (error._tag) {
    case "IdeLanguage.InvalidInput":
    case "IdeLanguage.MalformedResult":
      return error.detail.slice(0, 800);
    case "IdeLanguage.StaleGeneration":
      return `${error.generationKind} generation changed from ${error.expected} to ${error.actual}.`;
    case "IdeLanguage.ProviderUnavailable":
      return error.reason.slice(0, 800);
    case "IdeLanguage.TimedOut":
      return `Language request exceeded ${error.timeoutMs} ms.`;
    case "IdeLanguage.QueueFull":
      return `Language request queue reached its ${error.limit}-request bound.`;
    case "IdeLanguage.Stopped":
      return error.reason.slice(0, 800);
  }
};

/**
 * The only manual Effect runtime perimeter for IDE language IPC. The returned
 * host owns one schema-first service instance and its scoped utility worker;
 * renderer calls never instantiate services or receive filesystem authority.
 */
export const makeWorkspaceLanguageHost = (
  root: string,
  workerUrl: URL,
  grantRef: string,
  mutationAuthority?: IdePortableMutationAuthority,
): WorkspaceLanguageHost => {
  const provider = makeIdeLanguageWorkerProvider(root, workerUrl, grantRef, mutationAuthority);
  const servicePromise: Promise<IdeLanguageServiceShape> = Effect.runPromise(
    makeIdeLanguageService(provider),
  );
  let disposed = false;

  const snapshot = async (): Promise<IdeLanguageServiceSnapshot> => {
    const service = await servicePromise;
    return await Effect.runPromise(service.snapshot());
  };

  const request = async (input: IdeLanguageRequest): Promise<IdeLanguageRequestResponse> => {
    const service = await servicePromise;
    const outcome = await Effect.runPromise(Effect.result(service.request(input)));
    const serviceSnapshot = await Effect.runPromise(service.snapshot());
    if (Result.isSuccess(outcome)) {
      return Schema.decodeUnknownSync(IdeLanguageRequestResponseSchema)({
        _tag: "Result",
        result: outcome.success,
        service: serviceSnapshot,
      });
    }
    const error = outcome.failure;
    return IdeLanguageRequestResponseSchema.cases.Rejected.make({
      requestRef: input.requestRef,
      reason: rejectionReason(error),
      message: rejectionMessage(error),
      service: serviceSnapshot,
    });
  };

  const cancel = async (input: IdeLanguageCancelRequest): Promise<IdeLanguageCancelResponse> => {
    const service = await servicePromise;
    const outcome = await Effect.runPromise(Effect.result(service.cancel(input)));
    return IdeLanguageCancelResponseSchema.make({
      requestRef: input.requestRef,
      acknowledged: Result.isSuccess(outcome) && outcome.success,
    });
  };

  const stop = async (input: IdeLanguageStopRequest): Promise<IdeLanguageStopResponse> => {
    const service = await servicePromise;
    const outcome = await Effect.runPromise(Effect.result(service.stop(input)));
    const serviceSnapshot = Result.isSuccess(outcome)
      ? outcome.success
      : await Effect.runPromise(service.snapshot());
    return IdeLanguageStopResponseSchema.make({ service: serviceSnapshot });
  };

  return {
    request,
    cancel,
    stop,
    snapshot,
    dispose: () => {
      if (disposed) return;
      disposed = true;
      void servicePromise.then(service => Effect.runPromise(service.stop(
        IdeLanguageStopRequestSchema.make({
          schemaVersion: "openagents.desktop.ide-language-stop.v1",
          grantRef: "workspace.language.dispose",
          reason: "project_replaced",
        }),
      ))).catch(() => undefined);
    },
  };
};

export const unavailableLanguageServiceSnapshot = (): IdeLanguageServiceSnapshot =>
  IdeLanguageServiceSnapshotSchema.cases.Unconfigured.make({
    serviceRef: IdeLanguageServiceRefSchema.make("ide.language-service.typescript"),
  });
