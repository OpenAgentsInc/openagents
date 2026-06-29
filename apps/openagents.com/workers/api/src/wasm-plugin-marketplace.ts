// WASM-plugin marketplace scaffold for marketplace.wasm_plugins.v1 (#6833, #6832).
//
// HONESTY: this is package policy, install-state registry infrastructure, and
// fixture-backed sandbox execution evidence only. It does not expose public
// third-party WASM execution, bill, settle, or make the marketplace live. The
// live Worker mounts an empty registry and exposes a read-only installed-plugin
// discovery projection.

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

export const WasmPluginHostCallName = S.Literals([
  'clock.read',
  'log.public',
])
export type WasmPluginHostCallName = typeof WasmPluginHostCallName.Type

export const WasmPluginHostCallDeclaration = S.Struct({
  module: S.Literal('openagents'),
  name: WasmPluginHostCallName,
  permission: WasmPluginPermission,
  inputSchemaRef: S.String,
  outputSchemaRef: S.String,
})
export type WasmPluginHostCallDeclaration =
  typeof WasmPluginHostCallDeclaration.Type

export const WasmPluginSandboxLimits = S.Struct({
  maxModuleBytes: S.Number,
  maxMemoryPages: S.Number,
  maxExecutionMs: S.Number,
})
export type WasmPluginSandboxLimits = typeof WasmPluginSandboxLimits.Type

export const WasmPluginExecutionResourceUsage = S.Struct({
  moduleBytes: S.Number,
  elapsedMs: S.Number,
  memoryPages: S.Number,
  importCount: S.Number,
  hostCallCount: S.Number,
})
export type WasmPluginExecutionResourceUsage =
  typeof WasmPluginExecutionResourceUsage.Type

export const WasmPluginExecutionEvidence = S.Struct({
  schema: S.Literal('openagents.wasm_plugin_execution_evidence.v1'),
  promiseId: S.Literal(WASM_PLUGIN_MARKETPLACE_PROMISE),
  packageRef: S.String,
  version: S.String,
  interfaceKind: WasmPluginInterfaceKind,
  exportName: S.String,
  sandboxPolicyRef: S.String,
  inputHash: S.String,
  outputHash: S.String,
  resourceUsage: WasmPluginExecutionResourceUsage,
  hostCalls: S.Array(WasmPluginHostCallDeclaration),
  receiptRef: S.String,
  generatedAt: S.String,
})
export type WasmPluginExecutionEvidence =
  typeof WasmPluginExecutionEvidence.Type

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

export class WasmPluginExecutionRejected extends S.TaggedErrorClass<WasmPluginExecutionRejected>()(
  'WasmPluginExecutionRejected',
  {
    reason: S.String,
    blockerRef: S.String,
  },
) {}

const decodeManifest = S.decodeUnknownSync(WasmPluginPackageManifest)
const decodeLimits = S.decodeUnknownSync(WasmPluginSandboxLimits)
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

const rejectExecution = (
  reason: string,
  blockerRef: string,
): WasmPluginExecutionRejected =>
  new WasmPluginExecutionRejected({ reason, blockerRef })

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

export const defaultWasmPluginSandboxLimits: WasmPluginSandboxLimits = {
  maxModuleBytes: 64 * 1024,
  maxMemoryPages: 1,
  maxExecutionMs: 25,
}

export const defaultWasmPluginHostCalls: ReadonlyArray<WasmPluginHostCallDeclaration> =
  [
    {
      module: 'openagents',
      name: 'clock.read',
      permission: 'clock.read',
      inputSchemaRef: 'schema.openagents.wasm_host.empty.v1',
      outputSchemaRef: 'schema.openagents.wasm_host.i64_timestamp_ms.v1',
    },
    {
      module: 'openagents',
      name: 'log.public',
      permission: 'log.public',
      inputSchemaRef: 'schema.openagents.wasm_host.i32_public_event_code.v1',
      outputSchemaRef: 'schema.openagents.wasm_host.i32_ack.v1',
    },
  ]

const textEncoder = new TextEncoder()

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes)
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer => {
  const buffer = new ArrayBuffer(bytes.byteLength)
  new Uint8Array(buffer).set(bytes)

  return buffer
}

const hashJsonRef = async (prefix: string, value: unknown): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    arrayBufferFromBytes(textEncoder.encode(JSON.stringify(value))),
  )

  return `${prefix}.${bytesToHex(new Uint8Array(digest))}`
}

const importedHostCallRef = (entry: WebAssembly.ModuleImportDescriptor) =>
  `${entry.module}.${entry.name}`

const hostCallRef = (entry: WasmPluginHostCallDeclaration) =>
  `${entry.module}.${entry.name}`

const readVarUint = (
  bytes: Uint8Array,
  offset: number,
): { value: number; nextOffset: number } => {
  const current = bytes[offset] ?? 0

  return current < 0x80
    ? { value: current, nextOffset: offset + 1 }
    : { value: current & 0x7f, nextOffset: offset + 1 }
}

const codeSectionBytes = (moduleBytes: Uint8Array): Uint8Array | null => {
  const walkSections = (offset: number): Uint8Array | null => {
    if (offset >= moduleBytes.byteLength) {
      return null
    }

    const sectionId = moduleBytes[offset]
    const sectionSize = readVarUint(moduleBytes, offset + 1)
    const sectionStart = sectionSize.nextOffset
    const sectionEnd = sectionStart + sectionSize.value

    if (sectionId === 0x0a) {
      return moduleBytes.slice(sectionStart, sectionEnd)
    }

    return walkSections(sectionEnd)
  }

  return walkSections(8)
}

const hasPotentiallyUnboundedControlFlow = (moduleBytes: Uint8Array): boolean =>
  codeSectionBytes(moduleBytes)?.includes(0x03) ?? false

const findInterfaceDeclaration = (
  manifest: WasmPluginPackageManifest,
  exportName: string,
): WasmPluginInterfaceDeclaration | undefined =>
  manifest.interfaceDecls.find(decl => decl.exportName === exportName)

const exportedMemoryPages = (instance: WebAssembly.Instance): number => {
  const memory = instance.exports.memory

  return memory instanceof WebAssembly.Memory
    ? memory.buffer.byteLength / 65_536
    : 0
}

const makeHostImports = (
  hostCalls: ReadonlyArray<WasmPluginHostCallDeclaration>,
  now: () => number,
): WebAssembly.Imports => ({
  openagents: Object.fromEntries(
    hostCalls.map(hostCall => [
      hostCall.name,
      hostCall.name === 'clock.read' ? now : (_eventCode: number) => 0,
    ]),
  ),
})

type WasmPluginFixtureExecutionInput = Readonly<{
  entry: WasmPluginRegistryEntry
  moduleBytes: Uint8Array
  exportName: string
  inputI32: number
  limits?: WasmPluginSandboxLimits
  hostCalls?: ReadonlyArray<WasmPluginHostCallDeclaration>
  now?: () => number
}>

type WasmPluginFixtureExecutionResult =
  | { ok: true; outputI32: number; evidence: WasmPluginExecutionEvidence }
  | { ok: false; error: WasmPluginExecutionRejected }

export const executeWasmPluginFixture = async (
  input: WasmPluginFixtureExecutionInput,
): Promise<WasmPluginFixtureExecutionResult> => {
  const limits = decodeLimits(input.limits ?? defaultWasmPluginSandboxLimits)
  const hostCalls = input.hostCalls ?? defaultWasmPluginHostCalls
  const manifest = input.entry.manifest
  const interfaceDecl = findInterfaceDeclaration(manifest, input.exportName)

  if (input.entry.state !== 'installed') {
    return {
      ok: false,
      error: rejectExecution(
        'WASM plugin must be installed before execution.',
        'blocker.wasm_plugin_execution.plugin_not_installed',
      ),
    }
  }
  if (interfaceDecl === undefined) {
    return {
      ok: false,
      error: rejectExecution(
        'exportName must match an admitted interface declaration.',
        'blocker.wasm_plugin_execution.interface_not_declared',
      ),
    }
  }
  if (input.moduleBytes.byteLength > limits.maxModuleBytes) {
    return {
      ok: false,
      error: rejectExecution(
        'WASM module exceeds the sandbox module-byte limit.',
        'blocker.wasm_plugin_execution.module_too_large',
      ),
    }
  }
  if (hasPotentiallyUnboundedControlFlow(input.moduleBytes)) {
    return {
      ok: false,
      error: rejectExecution(
        'Fixture sandbox rejects WASM loop opcodes until preemptive fuel metering is available.',
        'blocker.wasm_plugin_execution.unbounded_control_flow_rejected',
      ),
    }
  }

  try {
    const module = new WebAssembly.Module(arrayBufferFromBytes(input.moduleBytes))
    const imports = WebAssembly.Module.imports(module)
    const allowedHostCallRefs = new Set(hostCalls.map(hostCallRef))
    const deniedImport = imports.find(
      entry =>
        entry.kind !== 'function' ||
        !allowedHostCallRefs.has(importedHostCallRef(entry)),
    )

    if (deniedImport !== undefined) {
      return {
        ok: false,
        error: rejectExecution(
          'WASM plugin requested an undeclared or unsupported host call.',
          'blocker.wasm_plugin_execution.host_call_denied',
        ),
      }
    }

    const now = input.now ?? currentEpochMillis
    const startedAt = now()
    const instance = new WebAssembly.Instance(
      module,
      makeHostImports(hostCalls, now),
    )
    const memoryPages = exportedMemoryPages(instance)

    if (memoryPages > limits.maxMemoryPages) {
      return {
        ok: false,
        error: rejectExecution(
          'WASM plugin exceeded the sandbox memory-page limit.',
          'blocker.wasm_plugin_execution.memory_limit_exceeded',
        ),
      }
    }

    const exportValue = instance.exports[input.exportName]
    if (typeof exportValue !== 'function') {
      return {
        ok: false,
        error: rejectExecution(
          'WASM plugin export is not an executable function.',
          'blocker.wasm_plugin_execution.export_not_callable',
        ),
      }
    }

    const outputI32 = exportValue(input.inputI32)
    const elapsedMs = now() - startedAt

    if (!Number.isInteger(outputI32)) {
      return {
        ok: false,
        error: rejectExecution(
          'WASM plugin output did not match the declared i32 fixture interface.',
          'blocker.wasm_plugin_execution.output_schema_mismatch',
        ),
      }
    }
    if (elapsedMs > limits.maxExecutionMs) {
      return {
        ok: false,
        error: rejectExecution(
          'WASM plugin exceeded the sandbox execution-time limit.',
          'blocker.wasm_plugin_execution.time_limit_exceeded',
        ),
      }
    }

    const inputHash = await hashJsonRef('wasm_plugin.input.sha256', {
      exportName: input.exportName,
      inputI32: input.inputI32,
      packageRef: manifest.packageRef,
      version: manifest.version,
    })
    const outputHash = await hashJsonRef('wasm_plugin.output.sha256', {
      outputI32,
    })
    const resourceUsage = {
      moduleBytes: input.moduleBytes.byteLength,
      elapsedMs,
      memoryPages,
      importCount: imports.length,
      hostCallCount: imports.length,
    }
    const receiptRef = await hashJsonRef('receipt.wasm_plugin.execution', {
      inputHash,
      outputHash,
      resourceUsage,
    })

    return {
      ok: true,
      outputI32,
      evidence: {
        schema: 'openagents.wasm_plugin_execution_evidence.v1',
        promiseId: WASM_PLUGIN_MARKETPLACE_PROMISE,
        packageRef: manifest.packageRef,
        version: manifest.version,
        interfaceKind: interfaceDecl.kind,
        exportName: input.exportName,
        sandboxPolicyRef: 'policy.openagents.wasm_plugin_fixture_sandbox.v1',
        inputHash,
        outputHash,
        resourceUsage,
        hostCalls,
        receiptRef,
        generatedAt: currentIsoTimestamp(),
      },
    }
  } catch {
    return {
      ok: false,
      error: rejectExecution(
        'WASM plugin failed to compile or execute inside the sandbox.',
        'blocker.wasm_plugin_execution.trap_or_compile_failure',
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
