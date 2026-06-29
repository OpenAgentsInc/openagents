import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  WasmPluginMarketplaceEndpoint,
  handleWasmPluginMarketplaceApi,
} from './wasm-plugin-marketplace-routes'
import {
  type WasmPluginExecutionPolicy,
  type WasmPluginPackageManifest,
  executeWasmPluginFixture,
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

const sha256Hex = async (bytes: Uint8Array): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    bytes.buffer.slice(
      bytes.byteOffset,
      bytes.byteOffset + bytes.byteLength,
    ) as ArrayBuffer,
  )
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const addFixtureWasm = Uint8Array.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
  0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
  0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
  0x00, 0x20, 0x01, 0x6a, 0x0b,
])

const unauthorizedImportFixtureWasm = Uint8Array.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60,
  0x00, 0x00, 0x02, 0x0e, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x06, 0x73, 0x65,
  0x63, 0x72, 0x65, 0x74, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07,
  0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x01, 0x0a, 0x06, 0x01, 0x04, 0x00,
  0x10, 0x00, 0x0b,
])

const allowedHostCallFixtureWasm = Uint8Array.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x60,
  0x00, 0x00, 0x60, 0x00, 0x01, 0x7f, 0x02, 0x1e, 0x01, 0x0f, 0x6f, 0x70,
  0x65, 0x6e, 0x61, 0x67, 0x65, 0x6e, 0x74, 0x73, 0x3a, 0x68, 0x6f, 0x73,
  0x74, 0x0a, 0x6c, 0x6f, 0x67, 0x2e, 0x70, 0x75, 0x62, 0x6c, 0x69, 0x63,
  0x00, 0x00, 0x03, 0x02, 0x01, 0x01, 0x07, 0x07, 0x01, 0x03, 0x72, 0x75,
  0x6e, 0x00, 0x01, 0x0a, 0x08, 0x01, 0x06, 0x00, 0x10, 0x00, 0x41, 0x07,
  0x0b,
])

const executionPolicy = (
  overrides: Partial<WasmPluginExecutionPolicy> = {},
): WasmPluginExecutionPolicy => ({
  maxModuleBytes: 4096,
  maxInputBytes: 128,
  maxOutputBytes: 128,
  maxDurationMs: 50,
  maxMemoryPages: 1,
  allowedHostCalls: ['clock.read', 'log.public'],
  policyRef: 'policy.openagents.wasm_plugin_execution_fixture.v1',
  ...overrides,
})

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

describe('WASM plugin execution sandbox policy (#6832)', () => {
  test('runs a digest-pinned fixture module and returns metering-ready evidence', async () => {
    const wasmModuleDigestRef = `wasm.sha256.${await sha256Hex(addFixtureWasm)}`
    const result = await executeWasmPluginFixture({
      manifest: manifest({
        wasmModuleDigestRef,
        interfaceDecls: [
          {
            kind: 'marketplace.transform.v1',
            exportName: 'add',
            inputSchemaRef: 'schema.public.example.input.v1',
            outputSchemaRef: 'schema.public.example.output.v1',
          },
        ],
      }),
      moduleBytes: addFixtureWasm,
      exportName: 'add',
      args: [19, 23],
      policy: executionPolicy(),
      nowMs: (() => {
        let now = 100
        return () => now++
      })(),
    })

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.result).toBe(42)
      expect(result.evidence).toMatchObject({
        schema: 'openagents.wasm_plugin_execution_evidence.v1',
        promiseId: 'marketplace.wasm_plugins.v1',
        packageRef: 'wasm_plugin.example_transform',
        version: '1.0.0',
        wasmModuleDigestRef,
        exportName: 'add',
        moduleBytes: addFixtureWasm.byteLength,
        inputBytes: 7,
        outputBytes: 2,
        maxDurationMs: 50,
        maxMemoryPages: 1,
        hostCallsAllowed: ['clock.read', 'log.public'],
        hostCallsAttempted: [],
        hostCallsRejected: [],
        runtimeRef: 'runtime.javascript_webassembly.source_fixture.v1',
        policyRef: 'policy.openagents.wasm_plugin_execution_fixture.v1',
      })
      expect(result.evidence.inputHash).toMatch(/^sha256:[a-f0-9]{64}$/)
      expect(result.evidence.outputHash).toMatch(/^sha256:[a-f0-9]{64}$/)
      expect(result.evidence.evidenceRef).toMatch(
        /^evidence\.wasm_plugin_execution\.[a-f0-9]{32}$/,
      )
    }
  })

  test('rejects module bytes that do not match the admitted digest ref', async () => {
    const result = await executeWasmPluginFixture({
      manifest: manifest({ wasmModuleDigestRef: `wasm.sha256.${'b'.repeat(64)}` }),
      moduleBytes: addFixtureWasm,
      exportName: 'add',
      args: [1, 2],
      policy: executionPolicy(),
    })

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_sandbox.digest_mismatch',
      )
    }
  })

  test('rejects undeclared host imports before instantiation', async () => {
    const wasmModuleDigestRef = `wasm.sha256.${await sha256Hex(
      unauthorizedImportFixtureWasm,
    )}`
    const result = await executeWasmPluginFixture({
      manifest: manifest({
        wasmModuleDigestRef,
        interfaceDecls: [
          {
            kind: 'marketplace.transform.v1',
            exportName: 'run',
            inputSchemaRef: 'schema.public.example.input.v1',
            outputSchemaRef: 'schema.public.example.output.v1',
          },
        ],
      }),
      moduleBytes: unauthorizedImportFixtureWasm,
      exportName: 'run',
      args: [],
      policy: executionPolicy({ allowedHostCalls: [] }),
    })

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_sandbox.unauthorized_import',
      )
    }
  })

  test('admits declared OpenAgents host imports from the policy allowlist', async () => {
    const wasmModuleDigestRef = `wasm.sha256.${await sha256Hex(
      allowedHostCallFixtureWasm,
    )}`
    const result = await executeWasmPluginFixture({
      manifest: manifest({
        wasmModuleDigestRef,
        interfaceDecls: [
          {
            kind: 'marketplace.transform.v1',
            exportName: 'run',
            inputSchemaRef: 'schema.public.example.input.v1',
            outputSchemaRef: 'schema.public.example.output.v1',
          },
        ],
      }),
      moduleBytes: allowedHostCallFixtureWasm,
      exportName: 'run',
      args: [],
      policy: executionPolicy({ allowedHostCalls: ['log.public'] }),
    })

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.result).toBe(7)
      expect(result.evidence.hostCallsAllowed).toEqual(['log.public'])
      expect(result.evidence.hostCallsAttempted).toEqual([
        'openagents:host.log.public:function',
      ])
      expect(result.evidence.hostCallsRejected).toEqual([])
    }
  })

  test('rejects policy-allowed host imports that the manifest did not declare', async () => {
    const wasmModuleDigestRef = `wasm.sha256.${await sha256Hex(
      allowedHostCallFixtureWasm,
    )}`
    const result = await executeWasmPluginFixture({
      manifest: manifest({
        wasmModuleDigestRef,
        permissions: ['clock.read'],
        interfaceDecls: [
          {
            kind: 'marketplace.transform.v1',
            exportName: 'run',
            inputSchemaRef: 'schema.public.example.input.v1',
            outputSchemaRef: 'schema.public.example.output.v1',
          },
        ],
      }),
      moduleBytes: allowedHostCallFixtureWasm,
      exportName: 'run',
      args: [],
      policy: executionPolicy({ allowedHostCalls: ['log.public'] }),
    })

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.blockerRef).toBe(
        'blocker.wasm_plugin_sandbox.unauthorized_import',
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
