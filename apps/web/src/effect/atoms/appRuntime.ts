import { Atom } from '@effect-atom/atom';
import { getAppConfig } from '../config';
import { getAppLayer, getAppMemoMap } from '../runtime';

import type { AppServices } from '../layer';

/**
 * Atom runtime for `apps/web` that provides the same Effect services as the app's
 * `ManagedRuntime` and shares its MemoMap.
 *
 * This lets `@effect-atom` atoms run Effects that require app services (AgentApi, Telemetry, etc.)
 * without going back through React `useEffect` + `useState`.
 */
export const AppAtomRuntime = (() => {
  const config = getAppConfig();
  const factory = Atom.context({ memoMap: getAppMemoMap(config) });
  return factory(() => getAppLayer(config)) as Atom.AtomRuntime<AppServices, never>;
})();

