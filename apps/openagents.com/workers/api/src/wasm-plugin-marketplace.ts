// WASM-plugin marketplace scaffold for marketplace.wasm_plugins.v1 (#6833).
//
// HONESTY: this is package policy + install-state registry infrastructure plus
// a bounded source-level execution gate for fixtures. It does not make the
// public marketplace live, bill, settle, or grant broad third-party execution
// authority. The live Worker mounts an empty registry and exposes a read-only
// installed-plugin discovery projection.

import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentEpochMillis, currentIsoTimestamp } from './runtime-primitives'

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

export const WasmPluginSandboxPolicy = S.Struct({
  maxWasmBytes: S.Number,
  maxInputBytes: S.Number,
  maxOutputBytes: S.Number,
  maxDurationMs: S.Number,
  allowedHostCalls: S.Array(WasmPluginPermission),
  policyRefs: S.Array(S.String),
})
export type WasmPluginSandboxPolicy = typeof WasmPluginSandboxPolicy.Type

export const WasmPluginSandboxExecutionEvidence = S.Struct({
  schema: S.Literal('openagents.wasm_plugin_execution_evidence.v1'),
  packageRef: S.String,
  version: S.String,
  wasmModuleDigestRef: S.String,
  exportName: S.String,
  status: S.Literals(['accepted', 'rejected']),
  inputHash: S.String,
  outputHash: S.NullOr(S.String),
  errorHash: S.NullOr(S.String),
  durationMs: S.Number,
  wasmBytes: S.Number,
  inputBytes: S.Number,
  outputBytes: S.Number,
  hostCallsAllowed: S.Array(WasmPluginPermission),
  hostCallsAttempted: S.Array(S.String),
  hostCallsRejected: S.Array(S.String),
  policyRefs: S.Array(S.String),
  runtimeRef: S.String,
  meteringReady: S.Boolean,
  generatedAt: S.String,
})
export type WasmPluginSandboxExecutionEvidence =
  typeof WasmPluginSandboxExecutionEvidence.Type

export class WasmPluginAdmissionRejected extends S.TaggedErrorClass<WasmPluginAdmissionRejected>()(
  'WasmPluginAdmissionRejected',
  {
    reason: S.String,
    blockerRef: S.String,
  },
) {}

export class WasmPluginExecutionRejected extends S.TaggedErrorClass<WasmPluginExecutionRejected>()(
  'WasmPluginExecutionRejected',
  {
    reason: S.String,
    blockerRef: S.String,
    evidence: WasmPluginSandboxExecutionEvidence,
  },
) {}

const decodeManifest = S.decodeUnknownSync(WasmPluginPackageManifest)
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const versionPattern = /^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/
const wasmDigestPattern = /^wasm\.sha256\.[a-f0-9]{64}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|email|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lnurl|mdk|mnemonic|oauth|payment|payout|preimage|private|provider|raw|secret|sk-[a-z0-9]|source_archive|token|wallet)/i

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const defaultSandboxRuntimeRef =
  'runtime.openagents.wasm_plugin_sandbox.worker_js_webassembly.v1'

const permissionHostCallRefs = {
  'clock.read': ['openagents:host/clock.read'],
  'http.fetch.public': ['openagents:host/http.fetch_public'],
  'kv.read.plugin': ['openagents:host/kv.read_plugin'],
  'kv.write.plugin': ['openagents:host/kv.write_plugin'],
  'log.public': ['openagents:host/log.public'],
} satisfies Record<WasmPluginPermission, ReadonlyArray<string>>

const textEncoder = new TextEncoder()

const byteLength = (value: string): number => textEncoder.encode(value).byteLength

const toHex = (bytes: Uint8Array): string =>
  [...bytes].map(byte => byte.toString(16).padStart(2, '0')).join('')

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer

const sha256Hex = async (value: Uint8Array | string): Promise<string> => {
  const bytes = typeof value === 'string' ? textEncoder.encode(value) : value
  return toHex(
    new Uint8Array(
      await crypto.subtle.digest('SHA-256', arrayBufferFromBytes(bytes)),
    ),
  )
}

const stableErrorHash = async (reason: string): Promise<string> =>
  `error.sha256.${await sha256Hex(reason)}`

const stableValueHash = async (label: string, value: unknown): Promise<string> =>
  `${label}.sha256.${await sha256Hex(JSON.stringify(value))}`

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

const allowedHostCallRefs = (
  policy: WasmPluginSandboxPolicy,
): ReadonlySet<string> =>
  new Set(
    policy.allowedHostCalls.flatMap(
      permission => permissionHostCallRefs[permission],
    ),
  )

const importedHostCallRef = (importEntry: WebAssembly.ModuleImportDescriptor) =>
  `${importEntry.module}.${importEntry.name}`

const makeRejectedExecution = async (input: {
  manifest: WasmPluginPackageManifest
  exportName: string
  policy: WasmPluginSandboxPolicy
  wasmBytes: number
  inputBytes: number
  outputBytes?: number
  inputHash: string
  durationMs: number
  reason: string
  blockerRef: string
  hostCallsAttempted?: ReadonlyArray<string>
  hostCallsRejected?: ReadonlyArray<string>
}): Promise<WasmPluginExecutionRejected> =>
  new WasmPluginExecutionRejected({
    reason: input.reason,
    blockerRef: input.blockerRef,
    evidence: {
      schema: 'openagents.wasm_plugin_execution_evidence.v1',
      packageRef: input.manifest.packageRef,
      version: input.manifest.version,
      wasmModuleDigestRef: input.manifest.wasmModuleDigestRef,
      exportName: input.exportName,
      status: 'rejected',
      inputHash: input.inputHash,
      outputHash: null,
      errorHash: await stableErrorHash(input.reason),
      durationMs: input.durationMs,
      wasmBytes: input.wasmBytes,
      inputBytes: input.inputBytes,
      outputBytes: input.outputBytes ?? 0,
      hostCallsAllowed: input.policy.allowedHostCalls,
      hostCallsAttempted: input.hostCallsAttempted ?? [],
      hostCallsRejected: input.hostCallsRejected ?? [],
      policyRefs: input.policy.policyRefs,
      runtimeRef: defaultSandboxRuntimeRef,
      meteringReady: true,
      generatedAt: currentIsoTimestamp(),
    },
  })

const makeHostImports = (
  imports: ReadonlyArray<WebAssembly.ModuleImportDescriptor>,
  attempted: Array<string>,
) => {
  const importObject: WebAssembly.Imports = {}
  for (const importEntry of imports) {
    if (importEntry.kind !== 'function') {
      continue
    }
    const namespace =
      importObject[importEntry.module] ??
      (importObject[importEntry.module] = {})
    namespace[importEntry.name] = () => {
      attempted.push(importedHostCallRef(importEntry))
      return 0
    }
  }
  return importObject
}

export const executeWasmPluginSandboxed = async (input: {
  manifest: WasmPluginPackageManifest
  wasmBytes: Uint8Array
  exportName: string
  args: ReadonlyArray<number>
  policy: WasmPluginSandboxPolicy
}): Promise<
  | {
      ok: true
      result: unknown
      evidence: WasmPluginSandboxExecutionEvidence
    }
  | { ok: false; error: WasmPluginExecutionRejected }
> => {
  const startedAt = currentEpochMillis()
  const admitted = admitWasmPluginPackage(input.manifest)
  const manifest = admitted.ok ? admitted.manifest : input.manifest
  const normalizedPolicy = {
    ...input.policy,
    allowedHostCalls: uniqueSorted(input.policy.allowedHostCalls),
    policyRefs: publicSafeRefs(
      'WASM plugin sandbox policy refs',
      input.policy.policyRefs,
    ),
  }
  const encodedInput = JSON.stringify(input.args)
  const inputBytes = byteLength(encodedInput)
  const inputHash = await stableValueHash('input', input.args)
  const duration = () => currentEpochMillis() - startedAt
  const rejectExecution = (reason: string, blockerRef: string) =>
    makeRejectedExecution({
      manifest,
      exportName: input.exportName,
      policy: normalizedPolicy,
      wasmBytes: input.wasmBytes.byteLength,
      inputBytes,
      inputHash,
      durationMs: duration(),
      reason,
      blockerRef,
    })

  if (!admitted.ok) {
    return {
      ok: false,
      error: await rejectExecution(
        admitted.error.reason,
        admitted.error.blockerRef,
      ),
    }
  }

  if (input.wasmBytes.byteLength > normalizedPolicy.maxWasmBytes) {
    return {
      ok: false,
      error: await rejectExecution(
        'WASM module exceeds sandbox byte limit.',
        'blocker.wasm_plugin_execution.module_too_large',
      ),
    }
  }
  if (inputBytes > normalizedPolicy.maxInputBytes) {
    return {
      ok: false,
      error: await rejectExecution(
        'WASM plugin input exceeds sandbox byte limit.',
        'blocker.wasm_plugin_execution.input_too_large',
      ),
    }
  }
  if (
    !manifest.interfaceDecls.some(decl => decl.exportName === input.exportName)
  ) {
    return {
      ok: false,
      error: await rejectExecution(
        'requested export is not declared by the admitted plugin interface.',
        'blocker.wasm_plugin_execution.export_not_declared',
      ),
    }
  }

  try {
    const module = await WebAssembly.compile(arrayBufferFromBytes(input.wasmBytes))
    const imports = WebAssembly.Module.imports(module)
    const allowed = allowedHostCallRefs(normalizedPolicy)
    const hostCallsAttempted = imports.map(importedHostCallRef).sort()
    const hostCallsRejected = hostCallsAttempted.filter(ref => !allowed.has(ref))

    if (hostCallsRejected.length > 0) {
      return {
        ok: false,
        error: await makeRejectedExecution({
          manifest,
          exportName: input.exportName,
          policy: normalizedPolicy,
          wasmBytes: input.wasmBytes.byteLength,
          inputBytes,
          inputHash,
          durationMs: duration(),
          reason: 'WASM module imports undeclared or unauthorized host calls.',
          blockerRef: 'blocker.wasm_plugin_execution.host_call_denied',
          hostCallsAttempted,
          hostCallsRejected,
        }),
      }
    }

    const invokedHostCalls: Array<string> = []
    const instance = await WebAssembly.instantiate(
      module,
      makeHostImports(imports, invokedHostCalls),
    )
    const exported = instance.exports[input.exportName]
    if (typeof exported !== 'function') {
      return {
        ok: false,
        error: await rejectExecution(
          'requested export is not a callable function.',
          'blocker.wasm_plugin_execution.export_not_callable',
        ),
      }
    }

    const result = exported(...input.args)
    const outputText = JSON.stringify(result)
    const outputBytes = byteLength(outputText)
    const durationMs = duration()

    if (durationMs > normalizedPolicy.maxDurationMs) {
      return {
        ok: false,
        error: await makeRejectedExecution({
          manifest,
          exportName: input.exportName,
          policy: normalizedPolicy,
          wasmBytes: input.wasmBytes.byteLength,
          inputBytes,
          inputHash,
          outputBytes,
          durationMs,
          reason: 'WASM plugin execution exceeded sandbox duration limit.',
          blockerRef: 'blocker.wasm_plugin_execution.duration_limit_exceeded',
          hostCallsAttempted: invokedHostCalls,
          hostCallsRejected: [],
        }),
      }
    }
    if (outputBytes > normalizedPolicy.maxOutputBytes) {
      return {
        ok: false,
        error: await makeRejectedExecution({
          manifest,
          exportName: input.exportName,
          policy: normalizedPolicy,
          wasmBytes: input.wasmBytes.byteLength,
          inputBytes,
          inputHash,
          outputBytes,
          durationMs,
          reason: 'WASM plugin output exceeds sandbox byte limit.',
          blockerRef: 'blocker.wasm_plugin_execution.output_too_large',
          hostCallsAttempted: invokedHostCalls,
          hostCallsRejected: [],
        }),
      }
    }

    return {
      ok: true,
      result,
      evidence: {
        schema: 'openagents.wasm_plugin_execution_evidence.v1',
        packageRef: manifest.packageRef,
        version: manifest.version,
        wasmModuleDigestRef: manifest.wasmModuleDigestRef,
        exportName: input.exportName,
        status: 'accepted',
        inputHash,
        outputHash: await stableValueHash('output', result),
        errorHash: null,
        durationMs,
        wasmBytes: input.wasmBytes.byteLength,
        inputBytes,
        outputBytes,
        hostCallsAllowed: normalizedPolicy.allowedHostCalls,
        hostCallsAttempted: invokedHostCalls.sort(),
        hostCallsRejected: [],
        policyRefs: normalizedPolicy.policyRefs,
        runtimeRef: defaultSandboxRuntimeRef,
        meteringReady: true,
        generatedAt: currentIsoTimestamp(),
      },
    }
  } catch (error) {
    return {
      ok: false,
      error: await rejectExecution(
        error instanceof Error ? error.message : 'WASM execution failed.',
        'blocker.wasm_plugin_execution.runtime_error',
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
