/// <reference types="@cloudflare/workers-types" />

import * as Context from "effect/Context";
import * as Effect from "effect/Effect";
import * as Exit from "effect/Exit";
import * as Layer from "effect/Layer";

import { ExecutionContextError } from "./errors";
import type { ExecutionContextLike } from "./types";

export interface WorkersService {
  readonly waitUntil: <A, E>(
    effect: Effect.Effect<A, E, never>
  ) => Effect.Effect<void, ExecutionContextError, never>;
  readonly passThroughOnException: () => Effect.Effect<
    void,
    ExecutionContextError,
    never
  >;
}

export class WorkersTag extends Context.Tag("liteclaw/WorkersService")<
  WorkersTag,
  WorkersService
>() {}

const missingExecutionContext = () =>
  new ExecutionContextError({
    message: "ExecutionContext not provided"
  });

export function make(ctx?: ExecutionContextLike): WorkersService {
  const executionContext = ctx;

  const waitUntil: WorkersService["waitUntil"] = (effect) =>
    Effect.flatMap(
      executionContext
        ? Effect.succeed(executionContext)
        : Effect.fail(missingExecutionContext()),
      (ctx) =>
        Effect.try({
          try: () => {
            ctx.waitUntil(Effect.runPromise(effect));
          },
          catch: (cause) =>
            new ExecutionContextError({
              message: "waitUntil failed",
              cause
            })
        })
    );

  const passThroughOnException: WorkersService["passThroughOnException"] = () =>
    Effect.flatMap(
      executionContext
        ? Effect.succeed(executionContext)
        : Effect.fail(missingExecutionContext()),
      (ctx) =>
        Effect.try({
          try: () => {
            ctx.passThroughOnException();
          },
          catch: (cause) =>
            new ExecutionContextError({
              message: "passThroughOnException failed",
              cause
            })
        })
    );

  return {
    waitUntil,
    passThroughOnException
  };
}

export function layer(ctx?: ExecutionContextLike): Layer.Layer<WorkersTag> {
  return Layer.succeed(WorkersTag, make(ctx));
}

export const waitUntil = Effect.serviceFunctions(WorkersTag).waitUntil;
export const passThroughOnException =
  Effect.serviceFunctions(WorkersTag).passThroughOnException;

export function serve<Env = unknown>(
  handler: (
    request: Request,
    env: Env,
    ctx: ExecutionContextLike
  ) => Effect.Effect<Response, unknown, never>
): ExportedHandler<Env> {
  return {
    fetch: (request: Request, env: Env, ctx: ExecutionContext) => {
      const ctxLike: ExecutionContextLike = {
        waitUntil: ctx.waitUntil.bind(ctx),
        passThroughOnException: ctx.passThroughOnException.bind(ctx)
      };

      const effect = handler(request, env, ctxLike);

      return Effect.runPromise(
        Effect.catchAll(
          Effect.map(effect, (response): Response => response),
          (error): Effect.Effect<Response, never, never> =>
            Effect.succeed(
              new Response(JSON.stringify({ error: String(error) }), {
                status: 500,
                headers: { "Content-Type": "application/json" }
              })
            )
        )
      );
    }
  };
}

export function onSchedule<Env = unknown>(
  handler: (
    controller: ScheduledController,
    env: Env,
    ctx: ExecutionContextLike
  ) => Effect.Effect<void, unknown, never>
): ExportedHandler<Env> {
  return {
    scheduled: (controller: ScheduledController, env: Env, ctx: ExecutionContext) => {
      const ctxLike: ExecutionContextLike = {
        waitUntil: ctx.waitUntil.bind(ctx),
        passThroughOnException: ctx.passThroughOnException.bind(ctx)
      };

      const effect = handler(controller, env, ctxLike);

      ctx.waitUntil(
        Effect.runPromise(
          Effect.catchAll(
            Effect.map(effect, (): void => {}),
            (_error): Effect.Effect<void, never, never> => Effect.void
          )
        )
      );
    }
  };
}

export function runExit<A, E>(
  effect: Effect.Effect<A, E, never>
): Promise<Exit.Exit<A, E>> {
  return Effect.runPromise(Effect.exit(effect));
}

export function runPromise<A, E>(
  effect: Effect.Effect<A, E, never>
): Promise<A> {
  return Effect.runPromise(effect);
}
