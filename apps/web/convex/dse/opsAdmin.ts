import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";

import { getSubject } from "../autopilot/access";

// Headless overnight ops are executed by the Worker using an admin-only token.
// Keep the admin identity explicit so normal WorkOS users cannot mutate/read ops records.
export const DSE_OPS_ADMIN_SUBJECT = "user_dse_admin";

export const requireOpsAdmin = (ctx: EffectQueryCtx | EffectMutationCtx) =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) =>
      subject === DSE_OPS_ADMIN_SUBJECT ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")),
    ),
  );

