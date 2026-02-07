import { Effect } from 'effect';
import { tryPromise } from './tryPromise';

import type { OptionalRestArgs, SchedulableFunctionReference, Scheduler } from 'convex/server';

export interface EffectScheduler {
  readonly runAfter: <TFuncRef extends SchedulableFunctionReference>(
    delayMs: number,
    functionReference: TFuncRef,
    ...args: OptionalRestArgs<TFuncRef>
  ) => Effect.Effect<void, Error>;

  readonly runAt: <TFuncRef extends SchedulableFunctionReference>(
    timestamp: number | Date,
    functionReference: TFuncRef,
    ...args: OptionalRestArgs<TFuncRef>
  ) => Effect.Effect<void, Error>;
}

export class EffectSchedulerImpl implements EffectScheduler {
  constructor(private scheduler: Scheduler) {}

  runAfter<TFuncRef extends SchedulableFunctionReference>(
    delayMs: number,
    functionReference: TFuncRef,
    ...args: OptionalRestArgs<TFuncRef>
  ): Effect.Effect<void, Error> {
    return tryPromise(() => this.scheduler.runAfter(delayMs, functionReference, ...args)).pipe(Effect.asVoid);
  }

  runAt<TFuncRef extends SchedulableFunctionReference>(
    timestamp: number | Date,
    functionReference: TFuncRef,
    ...args: OptionalRestArgs<TFuncRef>
  ): Effect.Effect<void, Error> {
    return tryPromise(() => this.scheduler.runAt(timestamp, functionReference, ...args)).pipe(Effect.asVoid);
  }
}
