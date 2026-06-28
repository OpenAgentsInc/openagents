import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  WasmPluginMarketplaceEndpoint,
  handleWasmPluginMarketplaceApi,
} from './wasm-plugin-marketplace-routes'
import {
  type WasmPluginPackageManifest,
  installWasmPluginPackage,
  listInstalledWasmPlugins,
  makeInMemoryWasmPluginRegistryStore,
  uninstallWasmPluginPackage,
} from './wasm-plugin-marketplace'

const validDigest = `wasm.sha256.${'a'.repeat(64)}`

const manifest = (
  overrides: Partial<WasmPluginPackageManifest> = {},
): WasmPluginPackageManifest => ({
  schema: 'openagents.wasm_plugin_marketplace.v1',
  packageRef: 'wasm_plugin.example_transform',
  displayName: 'Example transform',
  version: '1.0.0',
  wasmModuleDigestRef: validDigest,
  interfaceDecls: [
    {
      kind: 'marketplace.transform.v1',
      exportName: 'transform',
      inputSchemaRef: 'schema.public.example.input.v1',
      outputSchemaRef: 'schema.public.example.output.v1',
    },
  ],
  permissions: ['clock.read', 'log.public'],
  sourceRefs: ['https://github.com/OpenAgentsInc/openagents/tree/main/examples'],
  policyRefs: ['policy.openagents.wasm_plugin_admission.v1'],
  ...overrides,
})

const request = () =>
  new Request(`https://openagents.com${WasmPluginMarketplaceEndpoint}`)

describe('WASM plugin marketplace policy (#6833)', () => {
  test('admits a manifest with version, interface declarations, permissions, and policy refs', () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const result = installWasmPluginPackage(
      store,
      manifest(),
      '2026-06-28T00:00:00.000Z',
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.entry.state).toBe('installed')
      expect(result.admissionRef).toBe(
        'wasm_plugin.admission.wasm_plugin.example_transform.1.0.0',
      )
    }
  })

  test('rejects malformed manifests fail-closed', () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const result = installWasmPluginPackage(
      store,
      manifest({ version: 'latest' }),
    )

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_manifest.version_invalid',
      )
    }
    expect(listInstalledWasmPlugins(store).plugins).toHaveLength(0)
  })

  test('rejects over-privileged or unknown permissions through schema admission', () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const result = installWasmPluginPackage(
      store,
      manifest({ permissions: ['http.fetch.private' as 'clock.read'] }),
    )

    expect(result.ok).toBe(false)
    expect(listInstalledWasmPlugins(store).plugins).toHaveLength(0)
  })

  test('persists install and uninstall lifecycle state in the registry store', () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const installed = installWasmPluginPackage(
      store,
      manifest(),
      '2026-06-28T00:00:00.000Z',
    )
    expect(installed.ok).toBe(true)
    expect(listInstalledWasmPlugins(store).plugins).toHaveLength(1)

    const uninstalled = uninstallWasmPluginPackage(
      store,
      'wasm_plugin.example_transform',
      '1.0.0',
      '2026-06-28T01:00:00.000Z',
    )
    expect(uninstalled?.state).toBe('uninstalled')
    expect(listInstalledWasmPlugins(store).plugins).toHaveLength(0)
  })
})

describe('WASM plugin marketplace discovery route (#6833)', () => {
  test('returns installed plugins from the injected registry', async () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    installWasmPluginPackage(store, manifest(), '2026-06-28T00:00:00.000Z')

    const response = await Effect.runPromise(
      handleWasmPluginMarketplaceApi(request(), { store }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      plugins: ReadonlyArray<{ manifest: { packageRef: string } }>
    }

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('planned')
    expect(body.plugins.map(plugin => plugin.manifest.packageRef)).toEqual([
      'wasm_plugin.example_transform',
    ])
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handleWasmPluginMarketplaceApi(
        new Request(`https://openagents.com${WasmPluginMarketplaceEndpoint}`, {
          method: 'POST',
        }),
      ),
    )

    expect(response.status).toBe(405)
  })
})
