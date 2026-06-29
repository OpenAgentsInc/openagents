// WASM-plugin marketplace scaffold for marketplace.wasm_plugins.v1 (#6833/#6832).
//
// HONESTY: this is package policy + install-state registry infrastructure plus
// a fixture-only sandbox receipt path. It does not load arbitrary third-party
// code from the public route, bill, settle, or make the marketplace live. The
// live Worker mounts an empty registry and exposes a read-only installed-plugin
// discovery projection.

import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const WASM_PLUGIN_MARKETPLACE_SCHEMA =
  'openagents.wasm_plugin_marketplace.v1' as const
export const WASM_PLUGIN_MARKETPLACE_PROMISE =
  'marketplace.wasm_plugins.v1' as const

export const WasmPluginPermission = S.Literals([
  'clock.read',
  'http.fetch.public',
  'kv.read.plugin',
  'kv.write.plugin',
  'log.public',
])
export type WasmPluginPermission = typeof WasmPluginPermission.Type

export const WasmPluginSandboxHostCall = S.Literals([
  'clock.read',
  'log.public',
])
export type WasmPluginSandboxHostCall = typeof WasmPluginSandboxHostCall.Type

export const WasmPluginInterfaceKind = S.Literals([
  'agent.tool.v1',
  'blueprint.signature.v1',
  'marketplace.transform.v1',
])
export type WasmPluginInterfaceKind = typeof WasmPluginInterfaceKind.Type

export const WasmPluginInterfaceDeclaration = S.Struct({
  kind: WasmPluginInterfaceKind,
  exportName: S.String,
  inputSchemaRef: S.String,
  outputSchemaRef: S.String,
})
export type WasmPluginInterfaceDeclaration =
  typeof WasmPluginInterfaceDeclaration.Type

export const WasmPluginPackageManifest = S.Struct({
  schema: S.Literal(WASM_PLUGIN_MARKETPLACE_SCHEMA),
  packageRef: S.String,
  displayName: S.String,
  version: S.String,
  wasmModuleDigestRef: S.String,
  interfaceDecls: S.Array(WasmPluginInterfaceDeclaration),
  permissions: S.Array(WasmPluginPermission),
  sourceRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
})
export type WasmPluginPackageManifest = typeof WasmPluginPackageManifest.Type

export const WasmPluginInstallState = S.Literals(['installed', 'uninstalled'])
export type WasmPluginInstallState = typeof WasmPluginInstallState.Type

export const WasmPluginRegistryEntry = S.Struct({
  manifest: WasmPluginPackageManifest,
  state: WasmPluginInstallState,
  installedAt: S.NullOr(S.String),
  uninstalledAt: S.NullOr(S.String),
})
export type WasmPluginRegistryEntry = typeof WasmPluginRegistryEntry.Type

export class WasmPluginAdmissionRejected extends S.TaggedErrorClass<WasmPluginAdmissionRejected>()(
  'WasmPluginAdmissionRejected',
  {
    reason: S.String,
    blockerRef: S.String,
  },
) {}

export class WasmPluginSandboxRejected extends S.TaggedErrorClass<WasmPluginSandboxRejected>()(
  'WasmPluginSandboxRejected',
  {
    reason: S.String,
    blockerRef: S.String,
  },
) {}

const decodeManifest = S.decodeUnknownSync(WasmPluginPackageManifest)
const decodeSandboxHostCall = S.decodeUnknownSync(WasmPluginSandboxHostCall)
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const versionPattern = /^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/
const wasmDigestPattern = /^wasm\.sha256\.[a-f0-9]{64}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|email|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lnurl|mdk|mnemonic|oauth|payment|payout|preimage|private|provider|raw|secret|sk-[a-z0-9]|source_archive|token|wallet)/i

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const uniqueSorted = <Value extends string>(
  values: ReadonlyArray<Value>,
): ReadonlyArray<Value> =>
  [...new Set(values.map(value => value.trim() as Value))]
    .filter(isNonEmpty)
    .sort()

const reject = (
  reason: string,
  blockerRef: string,
): WasmPluginAdmissionRejected =>
  new WasmPluginAdmissionRejected({ reason, blockerRef })

const rejectSandbox = (
  reason: string,
  blockerRef: string,
): WasmPluginSandboxRejected =>
  new WasmPluginSandboxRejected({ reason, blockerRef })

const publicSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const normalized = uniqueSorted(refs)
  const unsafe = normalized.find(
    ref => !safeRefPattern.test(ref) || unsafeRefPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw reject(
      `${label} must contain only public-safe refs.`,
      'blocker.wasm_plugin_manifest.private_or_malformed_ref',
    )
  }

  return normalized
}

const normalizeManifest = (
  manifest: WasmPluginPackageManifest,
): WasmPluginPackageManifest => ({
  ...manifest,
  packageRef: manifest.packageRef.trim(),
  displayName: manifest.displayName.trim(),
  version: manifest.version.trim(),
  wasmModuleDigestRef: manifest.wasmModuleDigestRef.trim(),
  interfaceDecls: manifest.interfaceDecls.map(decl => ({
    kind: decl.kind,
    exportName: decl.exportName.trim(),
    inputSchemaRef: decl.inputSchemaRef.trim(),
    outputSchemaRef: decl.outputSchemaRef.trim(),
  })),
  permissions: uniqueSorted(manifest.permissions),
  sourceRefs: publicSafeRefs('WASM plugin source refs', manifest.sourceRefs),
  policyRefs: publicSafeRefs('WASM plugin policy refs', manifest.policyRefs),
})

export const admitWasmPluginPackage = (
  manifest: WasmPluginPackageManifest,
):
  | { ok: true; manifest: WasmPluginPackageManifest; admissionRef: string }
  | { ok: false; error: WasmPluginAdmissionRejected } => {
  try {
    const normalized = normalizeManifest(decodeManifest(manifest))

    if (!safeRefPattern.test(normalized.packageRef)) {
      return {
        ok: false,
        error: reject(
          'packageRef must be a public-safe stable ref.',
          'blocker.wasm_plugin_manifest.package_ref_invalid',
        ),
      }
    }
    if (!isNonEmpty(normalized.displayName)) {
      return {
        ok: false,
        error: reject(
          'displayName is required.',
          'blocker.wasm_plugin_manifest.display_name_missing',
        ),
      }
    }
    if (!versionPattern.test(normalized.version)) {
      return {
        ok: false,
        error: reject(
          'version must be an explicit semver string.',
          'blocker.wasm_plugin_manifest.version_invalid',
        ),
      }
    }
    if (!wasmDigestPattern.test(normalized.wasmModuleDigestRef)) {
      return {
        ok: false,
        error: reject(
          'wasmModuleDigestRef must be a wasm.sha256.<digest> ref.',
          'blocker.wasm_plugin_manifest.digest_missing',
        ),
      }
    }
    if (normalized.interfaceDecls.length < 1) {
      return {
        ok: false,
        error: reject(
          'at least one interface declaration is required.',
          'blocker.wasm_plugin_manifest.interface_decls_missing',
        ),
      }
    }
    for (const decl of normalized.interfaceDecls) {
      if (
        !isNonEmpty(decl.exportName) ||
        !safeRefPattern.test(decl.inputSchemaRef) ||
        !safeRefPattern.test(decl.outputSchemaRef)
      ) {
        return {
          ok: false,
          error: reject(
            'interface declarations require an export plus public-safe input/output schema refs.',
            'blocker.wasm_plugin_manifest.interface_decl_invalid',
          ),
        }
      }
    }
    if (normalized.policyRefs.length < 1) {
      return {
        ok: false,
        error: reject(
          'policyRefs must cite the admission policy used for install.',
          'blocker.wasm_plugin_manifest.policy_refs_missing',
        ),
      }
    }

    return {
      ok: true,
      manifest: normalized,
      admissionRef: `wasm_plugin.admission.${normalized.packageRef}.${normalized.version}`,
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof WasmPluginAdmissionRejected
        ? error
        : reject(
          'manifest is malformed.',
          'blocker.wasm_plugin_manifest.malformed',
        ),
    }
  }
}

export type WasmPluginRegistryStore = {
  list: () => ReadonlyArray<WasmPluginRegistryEntry>
  upsert: (entry: WasmPluginRegistryEntry) => void
}

export const makeInMemoryWasmPluginRegistryStore = (
  entries: ReadonlyArray<WasmPluginRegistryEntry> = [],
): WasmPluginRegistryStore => {
  const byPackage = new Map<string, WasmPluginRegistryEntry>(
    entries.map(entry => [
      `${entry.manifest.packageRef}@${entry.manifest.version}`,
      entry,
    ]),
  )

  return {
    list: () => [...byPackage.values()],
    upsert: entry =>
      void byPackage.set(
        `${entry.manifest.packageRef}@${entry.manifest.version}`,
        entry,
      ),
  }
}

export const emptyWasmPluginRegistryStore =
  makeInMemoryWasmPluginRegistryStore()

export const installWasmPluginPackage = (
  store: WasmPluginRegistryStore,
  manifest: WasmPluginPackageManifest,
  nowIso: string = currentIsoTimestamp(),
):
  | { ok: true; entry: WasmPluginRegistryEntry; admissionRef: string }
  | { ok: false; error: WasmPluginAdmissionRejected } => {
  const admitted = admitWasmPluginPackage(manifest)
  if (!admitted.ok) {
    return admitted
  }

  const entry = {
    manifest: admitted.manifest,
    state: 'installed' as const,
    installedAt: nowIso,
    uninstalledAt: null,
  }
  store.upsert(entry)

  return { ok: true, entry, admissionRef: admitted.admissionRef }
}

export const uninstallWasmPluginPackage = (
  store: WasmPluginRegistryStore,
  packageRef: string,
  version: string,
  nowIso: string = currentIsoTimestamp(),
): WasmPluginRegistryEntry | null => {
  const entry = store
    .list()
    .find(
      candidate =>
        candidate.manifest.packageRef === packageRef &&
        candidate.manifest.version === version,
    )

  if (entry === undefined) {
    return null
  }

  const uninstalled = {
    ...entry,
    state: 'uninstalled' as const,
    uninstalledAt: nowIso,
  }
  store.upsert(uninstalled)

  return uninstalled
}

export type WasmPluginSandboxLimits = {
  maxExecutionMs: number
  maxMemoryPages: number
  maxModuleBytes: number
}

export type WasmPluginSandboxEvidence = {
  schema: 'openagents.wasm_plugin_sandbox_evidence.v1'
  promiseId: typeof WASM_PLUGIN_MARKETPLACE_PROMISE
  packageRef: string
  exportName: string
  inputHashRef: string
  outputHashRef: string
  moduleDigestRef: string
  resourceUsage: {
    elapsedMs: number
    memoryPages: number
    moduleBytes: number
  }
  limits: WasmPluginSandboxLimits
  hostCallPolicy: {
    mode: 'deny_by_default'
    allowedHostCalls: ReadonlyArray<WasmPluginSandboxHostCall>
    rejectedHostCalls: ReadonlyArray<string>
  }
  meteringReady: true
  fixtureOnly: true
}

export type WasmPluginSandboxRunResult =
  | {
    ok: true
    evidence: WasmPluginSandboxEvidence
    output: number
  }
  | { ok: false; error: WasmPluginSandboxRejected }

const defaultSandboxLimits: WasmPluginSandboxLimits = {
  maxExecutionMs: 25,
  maxMemoryPages: 1,
  maxModuleBytes: 4096,
}

const bytesToHex = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const sha256Ref = async (prefix: string, value: unknown): Promise<string> => {
  const encoded = new TextEncoder().encode(JSON.stringify(value))
  const digest = await crypto.subtle.digest('SHA-256', toArrayBuffer(encoded))
  return `${prefix}.sha256.${bytesToHex(digest)}`
}

const toArrayBuffer = (bytes: Uint8Array): ArrayBuffer => {
  const buffer = new ArrayBuffer(bytes.byteLength)
  new Uint8Array(buffer).set(bytes)
  return buffer
}

const normalizeSandboxHostCalls = (
  values: ReadonlyArray<WasmPluginSandboxHostCall>,
): ReadonlyArray<WasmPluginSandboxHostCall> =>
  uniqueSorted(values).map(value => decodeSandboxHostCall(value))

const readMemoryPages = (
  exports: WebAssembly.Exports,
): number => {
  const memories = Object.values(exports).filter(
    value => value instanceof WebAssembly.Memory,
  ) as ReadonlyArray<WebAssembly.Memory>

  return memories.reduce(
    (maxPages, memory) =>
      Math.max(maxPages, memory.buffer.byteLength / 65_536),
    0,
  )
}

export const runWasmPluginSandboxFixture = async (input: {
  manifest: WasmPluginPackageManifest
  wasmModuleBytes: Uint8Array
  exportName: string
  input: unknown
  limits?: Partial<WasmPluginSandboxLimits>
  allowedHostCalls?: ReadonlyArray<WasmPluginSandboxHostCall>
}): Promise<WasmPluginSandboxRunResult> => {
  const admitted = admitWasmPluginPackage(input.manifest)
  if (!admitted.ok) {
    return {
      ok: false,
      error: rejectSandbox(
        admitted.error.reason,
        admitted.error.blockerRef,
      ),
    }
  }

  const limits = { ...defaultSandboxLimits, ...input.limits }
  if (input.wasmModuleBytes.byteLength > limits.maxModuleBytes) {
    return {
      ok: false,
      error: rejectSandbox(
        'WASM module exceeds the configured sandbox byte limit.',
        'blocker.wasm_plugin_sandbox.module_size_limit_exceeded',
      ),
    }
  }

  let module: WebAssembly.Module
  try {
    module = new WebAssembly.Module(toArrayBuffer(input.wasmModuleBytes))
  } catch {
    return {
      ok: false,
      error: rejectSandbox(
        'WASM module bytes are not valid WebAssembly.',
        'blocker.wasm_plugin_sandbox.module_invalid',
      ),
    }
  }

  const allowedHostCalls = normalizeSandboxHostCalls(
    input.allowedHostCalls ?? [],
  )
  const allowedHostCallSet = new Set(allowedHostCalls)
  const imports = WebAssembly.Module.imports(module)
  const rejectedHostCalls = imports
    .map(entry => `${entry.module}.${entry.name}:${entry.kind}`)
    .filter(ref => {
      const hostCall = ref.replace(/^openagents\./, '').replace(/:.+$/, '')
      return !allowedHostCallSet.has(hostCall as WasmPluginSandboxHostCall)
    })

  if (rejectedHostCalls.length > 0) {
    return {
      ok: false,
      error: rejectSandbox(
        'WASM plugin requested an unauthorized host call or ambient import.',
        'blocker.wasm_plugin_sandbox.unauthorized_host_call',
      ),
    }
  }

  const start = performance.now()
  try {
    const instance = await WebAssembly.instantiate(module, {})
    const exported = instance.exports[input.exportName]
    if (typeof exported !== 'function') {
      return {
        ok: false,
        error: rejectSandbox(
          'requested WASM export is missing or is not callable.',
          'blocker.wasm_plugin_sandbox.export_missing',
        ),
      }
    }

    const output = exported()
    const elapsedMs = performance.now() - start
    const memoryPages = readMemoryPages(instance.exports)
    if (memoryPages > limits.maxMemoryPages) {
      return {
        ok: false,
        error: rejectSandbox(
          'WASM plugin exceeded the configured memory-page limit.',
          'blocker.wasm_plugin_sandbox.memory_limit_exceeded',
        ),
      }
    }
    if (elapsedMs > limits.maxExecutionMs) {
      return {
        ok: false,
        error: rejectSandbox(
          'WASM plugin exceeded the configured execution-time limit.',
          'blocker.wasm_plugin_sandbox.execution_time_limit_exceeded',
        ),
      }
    }
    if (typeof output !== 'number') {
      return {
        ok: false,
        error: rejectSandbox(
          'fixture sandbox currently accepts numeric WASM outputs only.',
          'blocker.wasm_plugin_sandbox.output_type_unsupported',
        ),
      }
    }

    return {
      ok: true,
      output,
      evidence: {
        schema: 'openagents.wasm_plugin_sandbox_evidence.v1',
        promiseId: WASM_PLUGIN_MARKETPLACE_PROMISE,
        packageRef: admitted.manifest.packageRef,
        exportName: input.exportName,
        inputHashRef: await sha256Ref('wasm_plugin.input', input.input),
        outputHashRef: await sha256Ref('wasm_plugin.output', output),
        moduleDigestRef: admitted.manifest.wasmModuleDigestRef,
        resourceUsage: {
          elapsedMs,
          memoryPages,
          moduleBytes: input.wasmModuleBytes.byteLength,
        },
        limits,
        hostCallPolicy: {
          mode: 'deny_by_default',
          allowedHostCalls,
          rejectedHostCalls: [],
        },
        meteringReady: true,
        fixtureOnly: true,
      },
    }
  } catch {
    return {
      ok: false,
      error: rejectSandbox(
        'WASM plugin fixture execution failed inside the sandbox.',
        'blocker.wasm_plugin_sandbox.execution_failed',
      ),
    }
  }
}

export const WasmPluginMarketplaceStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['wasm_plugin_registry_install_state_changed'])

export const listInstalledWasmPlugins = (
  store: WasmPluginRegistryStore,
): {
  schema: typeof WASM_PLUGIN_MARKETPLACE_SCHEMA
  promiseId: typeof WASM_PLUGIN_MARKETPLACE_PROMISE
  promiseState: 'planned'
  inert: true
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  plugins: ReadonlyArray<WasmPluginRegistryEntry>
} => ({
  schema: WASM_PLUGIN_MARKETPLACE_SCHEMA,
  promiseId: WASM_PLUGIN_MARKETPLACE_PROMISE,
  promiseState: 'planned',
  inert: true,
  generatedAt: currentIsoTimestamp(),
  maxStalenessSeconds: WasmPluginMarketplaceStaleness.maxStalenessSeconds,
  staleness: WasmPluginMarketplaceStaleness,
  plugins: store.list().filter(entry => entry.state === 'installed'),
})
