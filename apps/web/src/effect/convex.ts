import { Context, Effect, FiberRef, Layer, Schema, Stream } from 'effect';
import { ConvexClient, ConvexHttpClient, type ConnectionState } from 'convex/browser';
import { AuthService } from './auth';
import { AppConfigService } from './config';
import { RequestContextService, makeDefaultRequestContext } from './requestContext';

import type { FunctionArgs, FunctionReference, FunctionReturnType } from 'convex/server';

export class ConvexServiceError extends Schema.TaggedError<ConvexServiceError>()('ConvexServiceError', {
  operation: Schema.String,
  error: Schema.Defect,
}) {}

export type ConvexServiceApi = {
  readonly query: <TQuery extends FunctionReference<'query'>>(
    query: TQuery,
    args: FunctionArgs<TQuery>,
  ) => Effect.Effect<Awaited<FunctionReturnType<TQuery>>, ConvexServiceError, RequestContextService>;

  readonly mutation: <TMutation extends FunctionReference<'mutation'>>(
    mutation: TMutation,
    args: FunctionArgs<TMutation>,
  ) => Effect.Effect<Awaited<FunctionReturnType<TMutation>>, ConvexServiceError, RequestContextService>;

  readonly action: <TAction extends FunctionReference<'action'>>(
    action: TAction,
    args: FunctionArgs<TAction>,
  ) => Effect.Effect<Awaited<FunctionReturnType<TAction>>, ConvexServiceError, RequestContextService>;

  /** Client-only realtime query subscription (WS). Server impl fails deterministically. */
  readonly subscribeQuery: <TQuery extends FunctionReference<'query'>>(
    query: TQuery,
    args: FunctionArgs<TQuery>,
  ) => Stream.Stream<Awaited<FunctionReturnType<TQuery>>, ConvexServiceError>;

  /**
   * Client-only Convex websocket connection state. Returns `null` on the server.
   * Useful for debugging "hang" scenarios where mutations/subscriptions never resolve.
   */
  readonly connectionState: () => Effect.Effect<ConnectionState | null, never, RequestContextService>;
};

export class ConvexService extends Context.Tag('@openagents/web/ConvexService')<
  ConvexService,
  ConvexServiceApi
>() {}

const serverHttpClientRef = FiberRef.unsafeMake<ConvexHttpClient | null>(null);

export const ConvexServiceLive = Layer.scoped(
  ConvexService,
  Effect.gen(function* () {
    const config = yield* AppConfigService;
    const auth = yield* AuthService;

    if (typeof window === 'undefined') {
      const getHttp = Effect.fn('ConvexService.getHttp')(function* () {
        const existing = yield* FiberRef.get(serverHttpClientRef);
        if (existing) return existing;

        const http = new ConvexHttpClient(config.convexUrl, { logger: false });
        const token = yield* auth
          .getAccessToken({ forceRefreshToken: false })
          .pipe(Effect.catchAll(() => Effect.succeed(null)));
        if (token) http.setAuth(token);
        yield* FiberRef.set(serverHttpClientRef, http);
        return http;
      });

      const query = Effect.fn('ConvexService.query')(function* <TQuery extends FunctionReference<'query'>>(
        queryRef: TQuery,
        args: FunctionArgs<TQuery>,
      ) {
        const http = yield* getHttp();
        return yield* Effect.tryPromise({
          try: () =>
            http.query(queryRef, args as any) as Promise<Awaited<FunctionReturnType<TQuery>>>,
          catch: (error) => ConvexServiceError.make({ operation: 'query.http', error }),
        });
      });

      const mutation = Effect.fn('ConvexService.mutation')(function* <TMutation extends FunctionReference<'mutation'>>(
        mutationRef: TMutation,
        args: FunctionArgs<TMutation>,
      ) {
        const http = yield* getHttp();
        return yield* Effect.tryPromise({
          try: () =>
            http.mutation(mutationRef, args as any) as Promise<Awaited<FunctionReturnType<TMutation>>>,
          catch: (error) => ConvexServiceError.make({ operation: 'mutation.http', error }),
        });
      });

      const action = Effect.fn('ConvexService.action')(function* <TAction extends FunctionReference<'action'>>(
        actionRef: TAction,
        args: FunctionArgs<TAction>,
      ) {
        const http = yield* getHttp();
        return yield* Effect.tryPromise({
          try: () =>
            http.action(actionRef, args as any) as Promise<Awaited<FunctionReturnType<TAction>>>,
          catch: (error) => ConvexServiceError.make({ operation: 'action.http', error }),
        });
      });

      const subscribeQuery = <TQuery extends FunctionReference<'query'>>(
        _queryRef: TQuery,
        _args: FunctionArgs<TQuery>,
      ): Stream.Stream<Awaited<FunctionReturnType<TQuery>>, ConvexServiceError> =>
        Stream.fail(
          ConvexServiceError.make({
            operation: 'subscribeQuery.ssr',
            error: new Error('subscribeQuery is client-only'),
          }),
        );

      const connectionState = () => Effect.sync(() => null);

      return ConvexService.of({ query, mutation, action, subscribeQuery, connectionState });
    }

    // Client: one WS client for the app.
    const client = yield* Effect.acquireRelease(
      Effect.sync(() => {
        const c = new ConvexClient(config.convexUrl, { unsavedChangesWarning: false });

        // Enable debug instrumentation for E2E sessions (SSR injects <meta name="oa-e2e" content="1" />).
        // This is intentionally console-based so our browser test harness can snapshot it on failure.
        const isE2e =
          typeof document !== 'undefined' &&
          document.querySelector('meta[name="oa-e2e"]')?.getAttribute('content') === '1';

        let unsubConnection: (() => void) | null = null;
        if (isE2e) {
          const simplify = (st: ConnectionState) => ({
            isWebSocketConnected: st.isWebSocketConnected,
            hasInflightRequests: st.hasInflightRequests,
            hasEverConnected: st.hasEverConnected,
            connectionCount: st.connectionCount,
            connectionRetries: st.connectionRetries,
            inflightMutations: st.inflightMutations,
            inflightActions: st.inflightActions,
            timeOfOldestInflightRequest: st.timeOfOldestInflightRequest
              ? st.timeOfOldestInflightRequest.toISOString()
              : null,
          });

          try {
            console.log('[convex:connection:init]', JSON.stringify(simplify(c.connectionState())));
          } catch {
            // ignore
          }

          unsubConnection = c.subscribeToConnectionState((st) => {
            try {
              const snapshot = simplify(st);
              (globalThis as any).__oaDebug = (globalThis as any).__oaDebug ?? {};
              (globalThis as any).__oaDebug.convex = { url: config.convexUrl, ...snapshot, ts: Date.now() };
              console.log('[convex:connection]', JSON.stringify(snapshot));
            } catch {
              // ignore
            }
          });
        }

        (c as any).__oaUnsubConnectionState = unsubConnection;
        (c as any).__oaIsE2e = isE2e;

        c.setAuth(async () => {
          const startedAt = Date.now();
          let token: string | null = null;
          try {
            token = await auth
              .getAccessToken({ forceRefreshToken: true })
              .pipe(
                Effect.catchAll((err) => {
                  console.warn('[convex:setAuth] getAccessToken failed', err);
                  return Effect.succeed(null);
                }),
                Effect.provideService(RequestContextService, makeDefaultRequestContext()),
                Effect.runPromise,
              );
          } catch (err) {
            console.warn('[convex:setAuth] runPromise failed', err);
          }
          const ms = Date.now() - startedAt;
          console.log('[convex:setAuth]', { hasToken: !!token, tokenLength: token?.length ?? 0, ms });
          return token ?? null;
        });
        return c;
      }),
      (c) =>
        Effect.sync(() => {
          try {
            const unsub = (c as any).__oaUnsubConnectionState as (() => void) | null | undefined;
            unsub?.();
            c.close();
          } catch {
            // ignore
          }
        }),
    );

    const query = Effect.fn('ConvexService.query')(function* <TQuery extends FunctionReference<'query'>>(
      queryRef: TQuery,
      args: FunctionArgs<TQuery>,
    ) {
      return yield* Effect.tryPromise({
        try: () =>
          client.query(queryRef, args as any) as Promise<Awaited<FunctionReturnType<TQuery>>>,
        catch: (error) => ConvexServiceError.make({ operation: 'query.ws', error }),
      });
    });

    const mutation = Effect.fn('ConvexService.mutation')(function* <TMutation extends FunctionReference<'mutation'>>(
      mutationRef: TMutation,
      args: FunctionArgs<TMutation>,
    ) {
      return yield* Effect.tryPromise({
        try: () => client.mutation(mutationRef, args as any),
        catch: (error) => ConvexServiceError.make({ operation: 'mutation.ws', error }),
      });
    });

    const action = Effect.fn('ConvexService.action')(function* <TAction extends FunctionReference<'action'>>(
      actionRef: TAction,
      args: FunctionArgs<TAction>,
    ) {
      return yield* Effect.tryPromise({
        try: () => client.action(actionRef, args as any),
        catch: (error) => ConvexServiceError.make({ operation: 'action.ws', error }),
      });
    });

    const subscribeQuery = <TQuery extends FunctionReference<'query'>>(
      queryRef: TQuery,
      args: FunctionArgs<TQuery>,
    ): Stream.Stream<Awaited<FunctionReturnType<TQuery>>, ConvexServiceError> =>
      Stream.asyncPush(
        (emit) =>
          Effect.acquireRelease(
            Effect.sync(() => {
              const { unsubscribe, getCurrentValue } = client.onUpdate(
                queryRef,
                args as any,
                (value) => {
                  emit.single(value as any);
                },
                (err) => {
                  emit.fail(
                    ConvexServiceError.make({
                      operation: 'subscribeQuery.onUpdate',
                      error: err,
                    }),
                  );
                },
              );

              const current = getCurrentValue();
              if (current !== undefined) emit.single(current as any);

              return unsubscribe;
            }),
            (unsubscribe) =>
              Effect.sync(() => {
                try {
                  unsubscribe();
                } catch {
                  // ignore
                }
              }),
          ),
        { bufferSize: 16, strategy: 'sliding' },
      );

    const connectionState = () => Effect.sync(() => client.connectionState());

    return ConvexService.of({ query, mutation, action, subscribeQuery, connectionState });
  }),
);
