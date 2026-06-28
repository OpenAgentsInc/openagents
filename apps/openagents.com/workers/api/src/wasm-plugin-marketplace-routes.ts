import { Effect } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
} from './http/responses'
import {
  type WasmPluginRegistryStore,
  emptyWasmPluginRegistryStore,
  listInstalledWasmPlugins,
} from './wasm-plugin-marketplace'

export const WasmPluginMarketplaceEndpoint =
  '/api/public/marketplace/wasm-plugins'

export type WasmPluginMarketplaceDeps = Readonly<{
  store?: WasmPluginRegistryStore
}>

export const handleWasmPluginMarketplaceApi = (
  request: Request,
  deps: WasmPluginMarketplaceDeps = {},
) => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.succeed(
    noStoreJsonResponse(
      listInstalledWasmPlugins(deps.store ?? emptyWasmPluginRegistryStore),
    ),
  )
}
