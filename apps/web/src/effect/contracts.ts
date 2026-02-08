import { Context, Effect, Layer, Schema } from "effect";

import { RequestContextService } from "./requestContext";
import { TelemetryService } from "./telemetry";

export class ContractsApiError extends Schema.TaggedError<ContractsApiError>()("ContractsApiError", {
  operation: Schema.String,
  status: Schema.optional(Schema.Number),
  error: Schema.Defect,
}) {}

export type ToolContract = {
  readonly name: string;
  readonly description: string;
  readonly usage?: string;
  readonly inputSchemaJson: unknown;
  readonly outputSchemaJson: unknown | null;
};

export type SignatureContract = {
  readonly format: string;
  readonly formatVersion: number;
  readonly signatureId: string;
  readonly inputSchemaJson: unknown;
  readonly outputSchemaJson: unknown;
  readonly promptIr: unknown;
  readonly defaultParams: unknown;
  readonly defaultConstraints: unknown;
};

export type ModuleContract = {
  readonly format: string;
  readonly formatVersion: number;
  readonly moduleId: string;
  readonly description: string;
  readonly signatureIds: ReadonlyArray<string>;
};

export type ContractsApi = {
  readonly getToolContracts: () => Effect.Effect<Array<ToolContract>, ContractsApiError, RequestContextService>;
  readonly getSignatureContracts: () => Effect.Effect<Array<SignatureContract>, ContractsApiError, RequestContextService>;
  readonly getModuleContracts: () => Effect.Effect<Array<ModuleContract>, ContractsApiError, RequestContextService>;
};

export class ContractsApiService extends Context.Tag("@openagents/web/ContractsApi")<
  ContractsApiService,
  ContractsApi
>() {}

const fetchNoStore = Effect.fn("ContractsApi.fetchNoStore")(function* (input: {
  readonly operation: string;
  readonly url: string;
}) {
  const ctx = yield* RequestContextService;

  const url =
    ctx._tag === "Server"
      ? new URL(input.url, ctx.request.url).toString()
      : input.url;

  const headers = new Headers({ accept: "application/json" });
  if (ctx._tag === "Server") {
    const cookie = ctx.request.headers.get("cookie");
    if (cookie) headers.set("cookie", cookie);
    const authorization = ctx.request.headers.get("authorization");
    if (authorization) headers.set("authorization", authorization);
  }

  return yield* Effect.tryPromise({
    try: () => fetch(url, { method: "GET", cache: "no-store", headers }),
    catch: (error) => ContractsApiError.make({ operation: input.operation, error }),
  });
});

export const ContractsApiLive = Layer.effect(
  ContractsApiService,
  Effect.gen(function* () {
    const telemetry = yield* TelemetryService;
    const t = telemetry.withNamespace("contracts.api");

    const getToolContracts = Effect.fn("ContractsApi.getToolContracts")(function* () {
      const response = yield* fetchNoStore({ operation: "getToolContracts", url: "/api/contracts/tools" });
      if (!response.ok) {
        yield* t.event("tools.fetch", { ok: false, status: response.status });
        return yield* Effect.fail(
          ContractsApiError.make({
            operation: "getToolContracts",
            status: response.status,
            error: new Error(`HTTP ${response.status}`),
          }),
        );
      }
      yield* t.event("tools.fetch", { ok: true });
      return (yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) => ContractsApiError.make({ operation: "getToolContracts.json", error }),
      })) as Array<ToolContract>;
    });

    const getSignatureContracts = Effect.fn("ContractsApi.getSignatureContracts")(function* () {
      const response = yield* fetchNoStore({
        operation: "getSignatureContracts",
        url: "/api/contracts/signatures",
      });
      if (!response.ok) {
        yield* t.event("signatures.fetch", { ok: false, status: response.status });
        return yield* Effect.fail(
          ContractsApiError.make({
            operation: "getSignatureContracts",
            status: response.status,
            error: new Error(`HTTP ${response.status}`),
          }),
        );
      }
      yield* t.event("signatures.fetch", { ok: true });
      return (yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) => ContractsApiError.make({ operation: "getSignatureContracts.json", error }),
      })) as Array<SignatureContract>;
    });

    const getModuleContracts = Effect.fn("ContractsApi.getModuleContracts")(function* () {
      const response = yield* fetchNoStore({ operation: "getModuleContracts", url: "/api/contracts/modules" });
      if (!response.ok) {
        yield* t.event("modules.fetch", { ok: false, status: response.status });
        return yield* Effect.fail(
          ContractsApiError.make({
            operation: "getModuleContracts",
            status: response.status,
            error: new Error(`HTTP ${response.status}`),
          }),
        );
      }
      yield* t.event("modules.fetch", { ok: true });
      return (yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) => ContractsApiError.make({ operation: "getModuleContracts.json", error }),
      })) as Array<ModuleContract>;
    });

    return ContractsApiService.of({ getToolContracts, getSignatureContracts, getModuleContracts });
  }),
);

