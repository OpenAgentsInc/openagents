// WASM-plugin marketplace scaffold for marketplace.wasm_plugins.v1 (#6833).
//
// HONESTY: this is package policy + install-state registry infrastructure only.
// It does not execute WASM, load third-party code, bill, settle, or make the
// public marketplace live. The live Worker mounts an empty registry and exposes
// a read-only installed-plugin discovery projection.

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

const decodeManifest = S.decodeUnknownSync(WasmPluginPackageManifest)
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
