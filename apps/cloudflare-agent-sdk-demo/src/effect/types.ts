/// <reference types="@cloudflare/workers-types" />

import * as Effect from "effect/Effect";

export type ExecutionContextLike = Pick<
  ExecutionContext,
  "waitUntil" | "passThroughOnException"
>;

export type FetchHandler<Env = unknown> = (
  request: Request,
  env: Env,
  ctx: ExecutionContextLike
) => Effect.Effect<Response, unknown, never>;

export type ScheduledHandler<Env = unknown> = (
  controller: ScheduledController,
  env: Env,
  ctx: ExecutionContextLike
) => Effect.Effect<void, unknown, never>;
