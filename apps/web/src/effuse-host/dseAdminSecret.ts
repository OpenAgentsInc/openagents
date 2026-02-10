import { Effect, Stream } from "effect";

import { ConvexHttpClient } from "convex/browser";
import type { FunctionArgs, FunctionReference, FunctionReturnType } from "convex/server";

import { mintE2eJwt } from "../auth/e2eAuth";
import type { AuthServiceApi } from "../effect/auth";
import { AuthService, AuthSession, AuthSessionUser } from "../effect/auth";
import type { ConvexServiceApi } from "../effect/convex";
import { ConvexService, ConvexServiceError } from "../effect/convex";

import type { WorkerEnv } from "./env";

// Must match `apps/web/convex/dse/opsAdmin.ts`.
export const DSE_OPS_ADMIN_SUBJECT = "user_dse_admin";

export const isDseAdminSecretAuthorized = (request: Request, env: WorkerEnv): boolean => {
  const secret = env.OA_DSE_ADMIN_SECRET ?? process.env.OA_DSE_ADMIN_SECRET;
  if (!secret) return false;
  const h = request.headers.get("authorization") ?? "";
  return h.trim() === `Bearer ${secret}`;
};

export const withDseAdminSecretServices = <A, R>(
  env: WorkerEnv,
  convexUrl: string,
  program: Effect.Effect<A, unknown, R | AuthService | ConvexService>,
): Effect.Effect<A, unknown, R> =>
  Effect.gen(function* () {
    const privateJwkJson = env.OA_E2E_JWT_PRIVATE_JWK ?? process.env.OA_E2E_JWT_PRIVATE_JWK;
    if (!privateJwkJson) return yield* Effect.fail(new Error("missing_OA_E2E_JWT_PRIVATE_JWK"));

    const token = yield* mintE2eJwt({
      privateJwkJson,
      user: { id: DSE_OPS_ADMIN_SUBJECT },
      ttlSeconds: 60 * 15,
    });

    const session = AuthSession.make({
      userId: DSE_OPS_ADMIN_SUBJECT,
      sessionId: null,
      user: AuthSessionUser.make({
        id: DSE_OPS_ADMIN_SUBJECT,
        email: null,
        firstName: null,
        lastName: null,
      }),
    });

    const authService: AuthServiceApi = {
      getSession: () => Effect.succeed(session),
      getAccessToken: (_options) => Effect.succeed(token),
      sessionScopeKey: () => Effect.succeed(`user:${DSE_OPS_ADMIN_SUBJECT}`),
    };

    const http = new ConvexHttpClient(convexUrl, { logger: false });
    http.setAuth(token);

    const query: ConvexServiceApi["query"] = <TQuery extends FunctionReference<"query">>(
      queryRef: TQuery,
      args: FunctionArgs<TQuery>,
    ) =>
      Effect.tryPromise({
        try: () => http.query(queryRef, args as any) as Promise<Awaited<FunctionReturnType<TQuery>>>,
        catch: (error) => ConvexServiceError.make({ operation: "query.http.admin", error }),
      });

    const mutation: ConvexServiceApi["mutation"] = <TMutation extends FunctionReference<"mutation">>(
      mutationRef: TMutation,
      args: FunctionArgs<TMutation>,
    ) =>
      Effect.tryPromise({
        try: () => http.mutation(mutationRef, args as any) as Promise<Awaited<FunctionReturnType<TMutation>>>,
        catch: (error) => ConvexServiceError.make({ operation: "mutation.http.admin", error }),
      });

    const action: ConvexServiceApi["action"] = <TAction extends FunctionReference<"action">>(
      actionRef: TAction,
      args: FunctionArgs<TAction>,
    ) =>
      Effect.tryPromise({
        try: () => http.action(actionRef, args as any) as Promise<Awaited<FunctionReturnType<TAction>>>,
        catch: (error) => ConvexServiceError.make({ operation: "action.http.admin", error }),
      });

    const subscribeQuery: ConvexServiceApi["subscribeQuery"] = () =>
      Stream.fail(
        ConvexServiceError.make({
          operation: "subscribeQuery.admin",
          error: new Error("subscribeQuery is client-only"),
        }),
      );

    const connectionState: ConvexServiceApi["connectionState"] = () => Effect.succeed(null);

    const refreshAuth: ConvexServiceApi["refreshAuth"] = () => Effect.void;

    const convexService = ConvexService.of({ query, mutation, action, subscribeQuery, connectionState, refreshAuth });

    return yield* program.pipe(
      Effect.provideService(AuthService, AuthService.of(authService)),
      Effect.provideService(ConvexService, convexService),
    );
  });
