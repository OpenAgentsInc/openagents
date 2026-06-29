import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  WasmPluginMarketplaceEndpoint,
  handleWasmPluginMarketplaceApi,
} from './wasm-plugin-marketplace-routes'
import {
  type WasmPluginPackageManifest,
  executeWasmPluginFixture,
  installWasmPluginPackage,
  listInstalledWasmPlugins,
  makeInMemoryWasmPluginRegistryStore,
  uninstallWasmPluginPackage,
} from './wasm-plugin-marketplace'

const validDigest = `wasm.sha256.${'a'.repeat(64)}`
const fixtureTransformWasm = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60,
  0x01, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0d, 0x01, 0x09,
  0x74, 0x72, 0x61, 0x6e, 0x73, 0x66, 0x6f, 0x72, 0x6d, 0x00, 0x00, 0x0a,
  0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x41, 0x01, 0x6a, 0x0b,
])
const unauthorizedHostCallWasm = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60,
  0x01, 0x7f, 0x01, 0x7f, 0x02, 0x21, 0x01, 0x0a, 0x6f, 0x70, 0x65, 0x6e,
  0x61, 0x67, 0x65, 0x6e, 0x74, 0x73, 0x12, 0x68, 0x74, 0x74, 0x70, 0x2e,
  0x66, 0x65, 0x74, 0x63, 0x68, 0x2e, 0x70, 0x72, 0x69, 0x76, 0x61, 0x74,
  0x65, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0d, 0x01, 0x09, 0x74,
  0x72, 0x61, 0x6e, 0x73, 0x66, 0x6f, 0x72, 0x6d, 0x00, 0x01, 0x0a, 0x08,
  0x01, 0x06, 0x00, 0x20, 0x00, 0x10, 0x00, 0x0b,
])
const exportedMemoryWasm = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60,
  0x01, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x05, 0x03, 0x01, 0x00,
  0x02, 0x07, 0x16, 0x02, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02,
  0x00, 0x09, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x66, 0x6f, 0x72, 0x6d, 0x00,
  0x00, 0x0a, 0x06, 0x01, 0x04, 0x00, 0x20, 0x00, 0x0b,
])

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

describe('WASM plugin execution sandbox (#6832)', () => {
  test('runs a fixture plugin under resource limits and records metering evidence', async () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const installed = installWasmPluginPackage(
      store,
      manifest(),
      '2026-06-28T00:00:00.000Z',
    )
    expect(installed.ok).toBe(true)
    if (!installed.ok) {
      return
    }

    const result = await executeWasmPluginFixture({
      entry: installed.entry,
      moduleBytes: fixtureTransformWasm,
      exportName: 'transform',
      inputI32: 41,
      now: () => 100,
    })

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.outputI32).toBe(42)
      expect(result.evidence.promiseId).toBe('marketplace.wasm_plugins.v1')
      expect(result.evidence.inputHash).toMatch(/^wasm_plugin\.input\.sha256\./)
      expect(result.evidence.outputHash).toMatch(
        /^wasm_plugin\.output\.sha256\./,
      )
      expect(result.evidence.receiptRef).toMatch(
        /^receipt\.wasm_plugin\.execution\./,
      )
      expect(result.evidence.resourceUsage).toEqual({
        elapsedMs: 0,
        hostCallCount: 0,
        importCount: 0,
        memoryPages: 0,
        moduleBytes: fixtureTransformWasm.byteLength,
      })
    }
  })

  test('rejects undeclared or unsupported host calls fail-closed', async () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const installed = installWasmPluginPackage(store, manifest())
    expect(installed.ok).toBe(true)
    if (!installed.ok) {
      return
    }

    const result = await executeWasmPluginFixture({
      entry: installed.entry,
      moduleBytes: unauthorizedHostCallWasm,
      exportName: 'transform',
      inputI32: 1,
    })

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_execution.host_call_denied',
      )
    }
  })

  test('enforces sandbox memory limits', async () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const installed = installWasmPluginPackage(store, manifest())
    expect(installed.ok).toBe(true)
    if (!installed.ok) {
      return
    }

    const result = await executeWasmPluginFixture({
      entry: installed.entry,
      moduleBytes: exportedMemoryWasm,
      exportName: 'transform',
      inputI32: 7,
      limits: {
        maxExecutionMs: 25,
        maxMemoryPages: 1,
        maxModuleBytes: 64 * 1024,
      },
    })

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_execution.memory_limit_exceeded',
      )
    }
  })

  test('enforces elapsed execution-time limits', async () => {
    const store = makeInMemoryWasmPluginRegistryStore()
    const installed = installWasmPluginPackage(store, manifest())
    expect(installed.ok).toBe(true)
    if (!installed.ok) {
      return
    }

    const ticks = [100, 200]
    const result = await executeWasmPluginFixture({
      entry: installed.entry,
      moduleBytes: fixtureTransformWasm,
      exportName: 'transform',
      inputI32: 7,
      limits: {
        maxExecutionMs: 25,
        maxMemoryPages: 1,
        maxModuleBytes: 64 * 1024,
      },
      now: () => ticks.shift() ?? 200,
    })

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_execution.time_limit_exceeded',
      )
    }
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
