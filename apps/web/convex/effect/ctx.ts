import { Context } from 'effect';
import { EffectAuthImpl } from './auth';
import { EffectSchedulerImpl } from './scheduler';
import { EffectStorageReaderImpl, EffectStorageWriterImpl } from './storage';
import { tryPromise } from './tryPromise';

import type { Effect } from 'effect';
import type { FunctionReference, FunctionReturnType, OptionalRestArgs } from 'convex/server';
import type { ActionCtx, MutationCtx, QueryCtx } from '../_generated/server';
import type { EffectAuth } from './auth';
import type { EffectScheduler } from './scheduler';
import type { EffectStorageReader, EffectStorageWriter } from './storage';

export type EffectQueryCtx = {
  readonly ctx: QueryCtx;
  readonly db: QueryCtx['db'];
  readonly auth: EffectAuth;
  readonly storage: EffectStorageReader;
};

export const EffectQueryCtx = Context.GenericTag<EffectQueryCtx>('@openagents/web/convex/effect/QueryCtx');

export type EffectMutationCtx = {
  readonly ctx: MutationCtx;
  readonly db: MutationCtx['db'];
  readonly auth: EffectAuth;
  readonly storage: EffectStorageWriter;
  readonly scheduler: EffectScheduler;
};

export const EffectMutationCtx = Context.GenericTag<EffectMutationCtx>('@openagents/web/convex/effect/MutationCtx');

export type EffectActionCtx = {
  readonly ctx: ActionCtx;
  readonly auth: EffectAuth;
  readonly storage: EffectStorageWriter;
  readonly scheduler: EffectScheduler;
  readonly runQuery: <TQuery extends FunctionReference<'query', 'public' | 'internal'>>(
    query: TQuery,
    ...args: OptionalRestArgs<TQuery>
  ) => Effect.Effect<FunctionReturnType<TQuery>, Error>;
  readonly runMutation: <TMutation extends FunctionReference<'mutation', 'public' | 'internal'>>(
    mutation: TMutation,
    ...args: OptionalRestArgs<TMutation>
  ) => Effect.Effect<FunctionReturnType<TMutation>, Error>;
  readonly runAction: <TAction extends FunctionReference<'action', 'public' | 'internal'>>(
    action: TAction,
    ...args: OptionalRestArgs<TAction>
  ) => Effect.Effect<FunctionReturnType<TAction>, Error>;
};

export const EffectActionCtx = Context.GenericTag<EffectActionCtx>('@openagents/web/convex/effect/ActionCtx');

export const makeEffectQueryCtx = (ctx: QueryCtx): EffectQueryCtx => ({
  ctx,
  db: ctx.db,
  auth: new EffectAuthImpl(ctx.auth),
  storage: new EffectStorageReaderImpl(ctx.storage),
});

export const makeEffectMutationCtx = (ctx: MutationCtx): EffectMutationCtx => ({
  ctx,
  db: ctx.db,
  auth: new EffectAuthImpl(ctx.auth),
  storage: new EffectStorageWriterImpl(ctx.storage),
  scheduler: new EffectSchedulerImpl(ctx.scheduler),
});

export const makeEffectActionCtx = (ctx: ActionCtx): EffectActionCtx => ({
  ctx,
  auth: new EffectAuthImpl(ctx.auth),
  storage: new EffectStorageWriterImpl(ctx.storage),
  scheduler: new EffectSchedulerImpl(ctx.scheduler),
  runQuery: (query, ...args) => tryPromise(() => ctx.runQuery(query, ...args)),
  runMutation: (mutation, ...args) => tryPromise(() => ctx.runMutation(mutation, ...args)),
  runAction: (action, ...args) => tryPromise(() => ctx.runAction(action, ...args)),
});
