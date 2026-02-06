import { getAppConfig } from './config';
import { makeAppRuntime } from './runtime';

import type { AppRuntime } from './runtime';
import type { Layer } from 'effect';

export type ServerRuntime = {
  readonly runtime: AppRuntime;
  readonly memoMap: Layer.MemoMap;
};

let singleton: ServerRuntime | null = null;

/**
 * Shared server runtime + MemoMap.
 *
 * Used by:
 * - route loaders / server functions via `context.effectRuntime` (created in `router.tsx`)
 * - API handlers (e.g. Effect RPC route) via `memoMap` passed to `RpcServer.toWebHandler`
 */
export const getServerRuntime = (): ServerRuntime => {
  if (singleton) return singleton;
  const config = getAppConfig();
  const runtime = makeAppRuntime(config);
  singleton = { runtime, memoMap: runtime.memoMap };
  return singleton;
};
