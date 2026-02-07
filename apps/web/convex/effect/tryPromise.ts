import { Effect } from 'effect';

const toError = (error: unknown): Error => (error instanceof Error ? error : new Error(String(error)));

export const tryPromise = <T>(thunk: () => Promise<T>) => Effect.tryPromise({ try: thunk, catch: toError });

