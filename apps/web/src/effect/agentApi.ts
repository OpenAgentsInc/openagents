import { Context, Effect, Layer, Schema } from 'effect';
import { TelemetryService } from './telemetry';
import { RequestContextService } from './requestContext';
import type { ChatMessage } from "./chatProtocol"

export class AgentApiError extends Schema.TaggedError<AgentApiError>()('AgentApiError', {
  operation: Schema.String,
  status: Schema.optional(Schema.Number),
  error: Schema.Defect,
}) {}

export type AgentApi = {
  readonly getBlueprint: (
    chatId: string,
  ) => Effect.Effect<unknown, AgentApiError, RequestContextService>;
  readonly getMessages: (
    chatId: string,
  ) => Effect.Effect<Array<ChatMessage>, AgentApiError, RequestContextService>;
  readonly getToolContracts: (
    chatId: string,
  ) => Effect.Effect<Array<AgentToolContract>, AgentApiError, RequestContextService>;
  readonly getSignatureContracts: (
    chatId: string,
  ) => Effect.Effect<Array<DseSignatureContract>, AgentApiError, RequestContextService>;
  readonly getModuleContracts: (
    chatId: string,
  ) => Effect.Effect<Array<DseModuleContract>, AgentApiError, RequestContextService>;
  readonly resetAgent: (chatId: string) => Effect.Effect<void, AgentApiError, RequestContextService>;
  readonly importBlueprint: (
    chatId: string,
    blueprint: unknown,
  ) => Effect.Effect<void, AgentApiError, RequestContextService>;
};

export type AgentToolContract = {
  readonly name: string;
  readonly description: string;
  readonly usage?: string;
  readonly inputSchemaJson: unknown;
  readonly outputSchemaJson: unknown | null;
};

export type DseSignatureContract = {
  readonly format: string;
  readonly formatVersion: number;
  readonly signatureId: string;
  readonly inputSchemaJson: unknown;
  readonly outputSchemaJson: unknown;
  readonly promptIr: unknown;
  readonly defaultParams: unknown;
  readonly defaultConstraints: unknown;
};

export type DseModuleContract = {
  readonly format: string;
  readonly formatVersion: number;
  readonly moduleId: string;
  readonly description: string;
  readonly signatureIds: ReadonlyArray<string>;
};

export class AgentApiService extends Context.Tag('@openagents/web/AgentApi')<
  AgentApiService,
  AgentApi
>() {}

const fetchNoStore = Effect.fn('AgentApi.fetchNoStore')(function* (input: {
  readonly operation: string;
  readonly url: string;
  readonly init?: RequestInit;
}) {
  const ctx = yield* RequestContextService;

  const url =
    ctx._tag === 'Server'
      ? // Resolve relative URLs against the incoming request so server/RPC calls
        // stay on-origin and can be intercepted in Workers tests.
        new URL(input.url, ctx.request.url).toString()
      : input.url;

  const headers = new Headers(input.init?.headers);
  if (ctx._tag === 'Server') {
    // Forward auth state for same-origin subrequests (e.g. /agents/*) from RPC/SSR.
    const cookie = ctx.request.headers.get('cookie');
    if (cookie && !headers.has('cookie')) headers.set('cookie', cookie);

    const authorization = ctx.request.headers.get('authorization');
    if (authorization && !headers.has('authorization')) {
      headers.set('authorization', authorization);
    }
  }

  return yield* Effect.tryPromise({
    try: () => fetch(url, { ...(input.init ?? {}), cache: 'no-store', headers }),
    catch: (error) =>
      AgentApiError.make({
        operation: input.operation,
        error,
      }),
  });
});

export const AgentApiLive = Layer.effect(
  AgentApiService,
  Effect.gen(function* () {
    const telemetry = yield* TelemetryService;
    const t = telemetry.withNamespace('agents.api');

    const getBlueprint = Effect.fn('AgentApi.getBlueprint')(function* (chatId: string) {
      const url = `/agents/chat/${chatId}/blueprint`;
      const response = yield* fetchNoStore({ operation: 'getBlueprint', url });
      if (!response.ok) {
        yield* t.event('blueprint.fetch', { ok: false, status: response.status });
        yield* AgentApiError.make({
          operation: 'getBlueprint',
          status: response.status,
          error: new Error(`HTTP ${response.status}`),
        });
      }

      yield* t.event('blueprint.fetch', { ok: true });

      return yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) =>
          AgentApiError.make({
            operation: 'getBlueprint',
            status: response.status,
            error,
          }),
      });
    });

    const getMessages = Effect.fn('AgentApi.getMessages')(function* (chatId: string) {
      const url = `/agents/chat/${chatId}/get-messages`;
      const response = yield* fetchNoStore({ operation: 'getMessages', url });
      if (!response.ok) {
        yield* AgentApiError.make({
          operation: 'getMessages',
          status: response.status,
          error: new Error(`HTTP ${response.status}`),
        });
      }

      const text = yield* Effect.tryPromise({
        try: () => response.text(),
        catch: (error) =>
          AgentApiError.make({
            operation: 'getMessages',
            status: response.status,
            error,
          }),
      });

	      if (!text.trim()) return [];

	      const parsed = yield* Effect.try({
	        try: () => JSON.parse(text) as Array<ChatMessage>,
	        catch: (error) =>
	          AgentApiError.make({
	            operation: 'getMessages',
	            status: response.status,
            error,
          }),
      });

      return parsed;
    });

    const getToolContracts = Effect.fn('AgentApi.getToolContracts')(function* (chatId: string) {
      const url = `/agents/chat/${chatId}/tool-contracts`;
      const response = yield* fetchNoStore({ operation: 'getToolContracts', url });
      if (!response.ok) {
        yield* AgentApiError.make({
          operation: 'getToolContracts',
          status: response.status,
          error: new Error(`HTTP ${response.status}`),
        });
      }

      const json = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) =>
          AgentApiError.make({
            operation: 'getToolContracts',
            status: response.status,
            error,
          }),
      });
      return json as Array<AgentToolContract>;
    });

    const getSignatureContracts = Effect.fn('AgentApi.getSignatureContracts')(function* (chatId: string) {
      const url = `/agents/chat/${chatId}/signature-contracts`;
      const response = yield* fetchNoStore({ operation: 'getSignatureContracts', url });
      if (!response.ok) {
        yield* AgentApiError.make({
          operation: 'getSignatureContracts',
          status: response.status,
          error: new Error(`HTTP ${response.status}`),
        });
      }

      const json = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) =>
          AgentApiError.make({
            operation: 'getSignatureContracts',
            status: response.status,
            error,
          }),
      });
      return json as Array<DseSignatureContract>;
    });

    const getModuleContracts = Effect.fn('AgentApi.getModuleContracts')(function* (chatId: string) {
      const url = `/agents/chat/${chatId}/module-contracts`;
      const response = yield* fetchNoStore({ operation: 'getModuleContracts', url });
      if (!response.ok) {
        yield* AgentApiError.make({
          operation: 'getModuleContracts',
          status: response.status,
          error: new Error(`HTTP ${response.status}`),
        });
      }

      const json = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (error) =>
          AgentApiError.make({
            operation: 'getModuleContracts',
            status: response.status,
            error,
          }),
      });
      return json as Array<DseModuleContract>;
    });

    const resetAgent = Effect.fn('AgentApi.resetAgent')(function* (chatId: string) {
      const url = `/agents/chat/${chatId}/reset-agent`;
      const response = yield* fetchNoStore({
        operation: 'resetAgent',
        url,
        init: { method: 'POST' },
      });
      if (!response.ok) {
        yield* AgentApiError.make({
          operation: 'resetAgent',
          status: response.status,
          error: new Error(`HTTP ${response.status}`),
        });
      }

      yield* t.event('agent.reset', { ok: true });
    });

    const importBlueprint = Effect.fn('AgentApi.importBlueprint')(function* (
      chatId: string,
      blueprint: unknown,
    ) {
      const url = `/agents/chat/${chatId}/blueprint`;
      const response = yield* fetchNoStore({
        operation: 'importBlueprint',
        url,
        init: {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(blueprint),
        },
      });

      if (!response.ok) {
        const body = yield* Effect.tryPromise({
          try: () => response.text(),
          catch: () => '',
        }).pipe(Effect.catchAll(() => Effect.succeed('')));

        yield* AgentApiError.make({
          operation: 'importBlueprint',
          status: response.status,
          error: new Error(body ? `HTTP ${response.status}: ${body}` : `HTTP ${response.status}`),
        });
      }

      yield* t.event('blueprint.import', { ok: true });
    });

    return AgentApiService.of({
      getBlueprint,
      getMessages,
      getToolContracts,
      getSignatureContracts,
      getModuleContracts,
      resetAgent,
      importBlueprint,
    });
  }),
);
