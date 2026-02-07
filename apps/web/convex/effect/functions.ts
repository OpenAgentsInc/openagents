import { Effect } from 'effect';
import { action, mutation, query } from '../_generated/server';
import { makeEffectActionCtx, makeEffectMutationCtx, makeEffectQueryCtx } from './ctx';

import type { DefaultArgsForOptionalValidator } from 'convex/server';
import type { Infer, PropertyValidators, Validator } from 'convex/values';
import type { EffectActionCtx, EffectMutationCtx, EffectQueryCtx } from './ctx';

type EffectHandler<TContext, TArgs, TReturn> = (ctx: TContext, args: TArgs) => Effect.Effect<TReturn, unknown>;

type HandlerArgs<TArgs extends PropertyValidators> = DefaultArgsForOptionalValidator<TArgs>[0];

export const effectQuery = <TArgs extends PropertyValidators, TReturns extends Validator<any, any, any>>({
  args,
  returns,
  handler,
}: {
  readonly args: TArgs;
  readonly returns: TReturns;
  readonly handler: EffectHandler<EffectQueryCtx, HandlerArgs<TArgs>, Infer<TReturns>>;
}) =>
  query({
    args,
    returns,
    handler: (ctx, ...handlerArgs: DefaultArgsForOptionalValidator<TArgs>) =>
      Effect.runPromise(handler(makeEffectQueryCtx(ctx), handlerArgs[0])),
  });

export const effectMutation = <TArgs extends PropertyValidators, TReturns extends Validator<any, any, any>>({
  args,
  returns,
  handler,
}: {
  readonly args: TArgs;
  readonly returns: TReturns;
  readonly handler: EffectHandler<EffectMutationCtx, HandlerArgs<TArgs>, Infer<TReturns>>;
}) =>
  mutation({
    args,
    returns,
    handler: (ctx, ...handlerArgs: DefaultArgsForOptionalValidator<TArgs>) =>
      Effect.runPromise(handler(makeEffectMutationCtx(ctx), handlerArgs[0])),
  });

export const effectAction = <TArgs extends PropertyValidators, TReturns extends Validator<any, any, any>>({
  args,
  returns,
  handler,
}: {
  readonly args: TArgs;
  readonly returns: TReturns;
  readonly handler: EffectHandler<EffectActionCtx, HandlerArgs<TArgs>, Infer<TReturns>>;
}) =>
  action({
    args,
    returns,
    handler: (ctx, ...handlerArgs: DefaultArgsForOptionalValidator<TArgs>) =>
      Effect.runPromise(handler(makeEffectActionCtx(ctx), handlerArgs[0])),
  });
