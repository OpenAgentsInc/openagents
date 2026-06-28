// Agentic-npm composition runtime — deterministic dependency resolver,
// verification-on-compose gate, and bounded install/use runtime core for the
// module-registry vision.
//
// Promise: marketplace.agentic_npm_module_registry.v1 (state: planned).
// Blocker advanced: blocker.product_promises.agentic_npm_module_composition_runtime_missing.
//
// Episode 238 ("learning by construction", docs/transcripts/238.md): verified
// programs become composable modules in an "agentic npm" registry — a library of
// verified, composable computation modules. The MISSING piece this file builds is
// the composition-runtime CORE: given a registry module surface and a requested
// root set, resolve the transitive dependency closure, GATE every resolved
// module on its exact-trace verification state (the verification reused from
// compute.tassadar_executor_poc.v1), check that every required interface is
// provided within the resolved set, detect missing modules and dependency
// cycles, emit a deterministic, content-addressed composition plan, materialize
// a verified install record, and invoke a registered module adapter with
// install/use evidence rows.
//
// SCOPE / HONESTY: this file is still a bounded runtime core, not a broad paid
// marketplace:
//   - invocation dispatches only to explicitly registered in-process adapters;
//   - it moves no money, reads no wallet, bills nothing, and settles nothing.
// The promise STAYS `planned`. This file clears the source-level registry +
// install/use runtime gap with receipt-backed tests; billing, attribution,
// rev-share, abuse handling, and settlement remain separate blockers.

import { sha256Hex } from './buy-mode-dispatcher'

export const AGENTIC_NPM_COMPOSITION_RUNTIME_SCHEMA =
  'openagents.agentic_npm_composition_runtime.v1' as const

export const AGENTIC_NPM_REGISTRY_RUNTIME_SCHEMA =
  'openagents.agentic_npm_registry_runtime.v1' as const

export const AGENTIC_NPM_MODULE_REGISTRY_PROMISE =
  'marketplace.agentic_npm_module_registry.v1' as const

// Public-safe ref guard, mirroring tassadar-module-library.ts: no raw/private,
// provider, customer, wallet, payment, or credential material may enter a plan.
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/@-]{0,260}$/
const unsafeCompositionPattern =
  /(\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer\s+|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage|secret)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const isPublicSafeRef = (ref: string): boolean =>
  safeRefPattern.test(ref) && !unsafeCompositionPattern.test(ref)

/**
 * One registry module spec the resolver reasons over. This is the minimal
 * subset of a registry listing needed to compose: an identity, the exact-trace
 * verification flags (all three must be cleared to be compose-eligible — the
 * same gate compiled-module listings carry), the interfaces the module
 * `provides`, the module refs it `dependsOn` (dependency-closure edges), and the
 * interface refs it `requiresInterfaces` (capabilities the resolved closure must
 * supply). Module deps and interface deps are kept separate so a missing module
 * and an unsatisfied capability produce distinct, honest verdicts.
 */
export type AgenticNpmModuleSpec = Readonly<{
  moduleRef: string
  version: string
  moduleDigest: string
  replayVerified: boolean
  compositionVerified: boolean
  linkCompatibilityVerified: boolean
  provides: ReadonlyArray<string>
  dependsOn: ReadonlyArray<string>
  requiresInterfaces: ReadonlyArray<string>
}>

export type AgenticNpmPublishedModule = AgenticNpmModuleSpec &
  Readonly<{
    publisherRef: string
    publishedAt: string
  }>

export type AgenticNpmInstallEvidenceRow = Readonly<{
  schema: typeof AGENTIC_NPM_REGISTRY_RUNTIME_SCHEMA
  evidenceKind: 'agentic_npm_install'
  installRef: string
  installerRef: string
  requestedRootRefs: ReadonlyArray<string>
  planDigest: string
  status: 'installed' | 'blocked'
  blockerRefs: ReadonlyArray<string>
  installedModuleRefs: ReadonlyArray<string>
  observedAt: string
}>

export type AgenticNpmUsageEvidenceRow = Readonly<{
  schema: typeof AGENTIC_NPM_REGISTRY_RUNTIME_SCHEMA
  evidenceKind: 'agentic_npm_usage'
  usageRef: string
  installRef: string
  moduleRef: string
  callerRef: string
  status: 'invoked' | 'blocked'
  blockerRefs: ReadonlyArray<string>
  inputDigest: string
  outputDigest: string | null
  observedAt: string
}>

export type AgenticNpmRegistryRuntimeStore = Readonly<{
  publishModule: (module: AgenticNpmPublishedModule) => Promise<void>
  listModules: () => Promise<ReadonlyArray<AgenticNpmPublishedModule>>
  readModule: (
    moduleRef: string,
  ) => Promise<AgenticNpmPublishedModule | undefined>
  writeInstallEvidence: (row: AgenticNpmInstallEvidenceRow) => Promise<void>
  readInstallEvidence: (
    installRef: string,
  ) => Promise<AgenticNpmInstallEvidenceRow | undefined>
  listInstallEvidence: () => Promise<ReadonlyArray<AgenticNpmInstallEvidenceRow>>
  writeUsageEvidence: (row: AgenticNpmUsageEvidenceRow) => Promise<void>
  listUsageEvidence: () => Promise<ReadonlyArray<AgenticNpmUsageEvidenceRow>>
}>

export type AgenticNpmModuleAdapter = (
  input: Readonly<Record<string, unknown>>,
) => Promise<Readonly<Record<string, unknown>>>

export type AgenticNpmInstallResult =
  | Readonly<{
      ok: true
      installRef: string
      plan: AgenticNpmCompositionPlan
      evidence: AgenticNpmInstallEvidenceRow
    }>
  | Readonly<{
      ok: false
      installRef: string
      plan: AgenticNpmCompositionPlan
      evidence: AgenticNpmInstallEvidenceRow
    }>

export type AgenticNpmInvokeResult =
  | Readonly<{
      ok: true
      output: Readonly<Record<string, unknown>>
      evidence: AgenticNpmUsageEvidenceRow
    }>
  | Readonly<{
      ok: false
      error: AgenticNpmInvokeError
      evidence: AgenticNpmUsageEvidenceRow
    }>

type AgenticNpmInvokeError =
  | 'install_not_found'
  | 'install_not_usable'
  | 'adapter_not_registered'

/**
 * One node in a resolved composition plan: a module pinned at a digest with the
 * deterministic install/compose order index it would occupy (topological).
 */
export type AgenticNpmResolvedModule = Readonly<{
  moduleRef: string
  version: string
  moduleDigest: string
  order: number
}>

export type AgenticNpmCompositionPlan = Readonly<{
  schema: typeof AGENTIC_NPM_COMPOSITION_RUNTIME_SCHEMA
  promiseId: typeof AGENTIC_NPM_MODULE_REGISTRY_PROMISE
  // Honest: this is a resolver, not a live runtime — the promise stays planned.
  promiseState: 'planned'
  inert: true
  composable: boolean
  requestedRootRefs: ReadonlyArray<string>
  // Topologically ordered set of modules in the transitive closure, dedup-pinned.
  resolved: ReadonlyArray<AgenticNpmResolvedModule>
  // Refs requested/required but absent from the registry.
  missingModuleRefs: ReadonlyArray<string>
  // Resolved modules that fail the verification-on-compose gate.
  unverifiedModuleRefs: ReadonlyArray<string>
  // Required interfaces not provided by any module in the resolved closure.
  unsatisfiedInterfaceRefs: ReadonlyArray<string>
  // Module refs participating in a dependency cycle (empty when acyclic).
  cyclicModuleRefs: ReadonlyArray<string>
  // Content-addressed plan digest (sha256 over the canonical resolved plan).
  planDigest: string
  blockerRefs: ReadonlyArray<string>
  // No-op authority surface: the resolver authorizes nothing.
  authority: Readonly<{
    installAuthority: false
    executionAuthority: false
    meteringAuthority: false
    billingAuthority: false
    settlementAuthority: false
  }>
  caveatRefs: ReadonlyArray<string>
}>

export class AgenticNpmCompositionUnsafe extends Error {
  override readonly name = 'AgenticNpmCompositionUnsafe'
}

const uniqueSorted = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)].sort()

const canonicalJson = (value: unknown): string => {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`
  }
  if (value !== null && typeof value === 'object') {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => `${JSON.stringify(key)}:${canonicalJson(entry)}`)
      .join(',')}}`
  }
  return JSON.stringify(value)
}

const moduleComposeEligible = (spec: AgenticNpmModuleSpec): boolean =>
  spec.replayVerified &&
  spec.compositionVerified &&
  spec.linkCompatibilityVerified

const assertSafe = (label: string, refs: ReadonlyArray<string>): void => {
  for (const ref of refs) {
    if (!isPublicSafeRef(ref)) {
      throw new AgenticNpmCompositionUnsafe(
        `${label} must be a public-safe ref without raw/private, provider, customer, wallet, payment, or credential material.`,
      )
    }
  }
}

export const makeInMemoryAgenticNpmRegistryRuntimeStore = (
  initialModules: ReadonlyArray<AgenticNpmPublishedModule> = [],
): AgenticNpmRegistryRuntimeStore => {
  const modules = new Map<string, AgenticNpmPublishedModule>()
  const installs = new Map<string, AgenticNpmInstallEvidenceRow>()
  const usageRows: Array<AgenticNpmUsageEvidenceRow> = []
  for (const module of initialModules) {
    modules.set(module.moduleRef, module)
  }
  return {
    listInstallEvidence: async () =>
      [...installs.values()].sort((left, right) =>
        left.installRef.localeCompare(right.installRef),
      ),
    listModules: async () =>
      [...modules.values()].sort((left, right) =>
        left.moduleRef.localeCompare(right.moduleRef),
      ),
    listUsageEvidence: async () =>
      [...usageRows].sort((left, right) =>
        left.usageRef.localeCompare(right.usageRef),
      ),
    publishModule: async module => {
      modules.set(module.moduleRef, module)
    },
    readInstallEvidence: async installRef => installs.get(installRef),
    readModule: async moduleRef => modules.get(moduleRef),
    writeInstallEvidence: async row => {
      installs.set(row.installRef, row)
    },
    writeUsageEvidence: async row => {
      usageRows.push(row)
    },
  }
}

/**
 * Resolve a deterministic, verified composition plan from a registry and a set
 * of requested root module refs.
 *
 * Determinism: registry order is irrelevant — modules are indexed by ref,
 * closures are accumulated in a stable visit order, and the topological sort
 * breaks ties lexicographically, so identical inputs always produce an
 * identical plan and planDigest.
 *
 * The plan is `composable` ONLY when: no requested/required module is missing,
 * no dependency cycle exists, every resolved module clears the verification gate,
 * and every required interface is provided within the resolved closure.
 */
export const resolveAgenticNpmComposition = async (input: {
  registry: ReadonlyArray<AgenticNpmModuleSpec>
  requestedRootRefs: ReadonlyArray<string>
}): Promise<AgenticNpmCompositionPlan> => {
  assertSafe('requested root ref', input.requestedRootRefs)
  for (const spec of input.registry) {
    assertSafe('module ref', [spec.moduleRef])
    assertSafe(`module ${spec.moduleRef} digest`, [spec.moduleDigest])
    assertSafe(`module ${spec.moduleRef} provides`, spec.provides)
    assertSafe(`module ${spec.moduleRef} dependsOn`, spec.dependsOn)
    assertSafe(
      `module ${spec.moduleRef} requiresInterfaces`,
      spec.requiresInterfaces,
    )
  }

  const byRef = new Map<string, AgenticNpmModuleSpec>()
  for (const spec of input.registry) {
    // First spec wins for a given ref; duplicates are ignored deterministically.
    if (!byRef.has(spec.moduleRef)) {
      byRef.set(spec.moduleRef, spec)
    }
  }

  const roots = uniqueSorted(input.requestedRootRefs)
  const closure = new Set<string>()
  const missing = new Set<string>()

  // Stable BFS/DFS over `requires` to build the transitive closure.
  const visit = (ref: string): void => {
    if (closure.has(ref)) {
      return
    }
    const spec = byRef.get(ref)
    if (spec === undefined) {
      missing.add(ref)
      return
    }
    closure.add(ref)
    for (const dep of uniqueSorted(spec.dependsOn)) {
      visit(dep)
    }
  }
  for (const root of roots) {
    visit(root)
  }

  // Cycle detection + topological sort (Kahn) over the resolved closure only.
  const resolvedRefs = [...closure].sort()
  const indegree = new Map<string, number>()
  const dependents = new Map<string, ReadonlyArray<string>>()
  for (const ref of resolvedRefs) {
    indegree.set(ref, 0)
  }
  for (const ref of resolvedRefs) {
    const spec = byRef.get(ref)!
    for (const dep of uniqueSorted(spec.dependsOn)) {
      if (!closure.has(dep)) {
        continue
      }
      // Edge dep -> ref (dep must be ordered before the module that needs it).
      indegree.set(ref, (indegree.get(ref) ?? 0) + 1)
      dependents.set(dep, [...(dependents.get(dep) ?? []), ref])
    }
  }

  const ready = resolvedRefs.filter(ref => (indegree.get(ref) ?? 0) === 0).sort()
  const ordered: Array<string> = []
  while (ready.length > 0) {
    const next = ready.shift()!
    ordered.push(next)
    for (const dependent of uniqueSorted(dependents.get(next) ?? [])) {
      const remaining = (indegree.get(dependent) ?? 0) - 1
      indegree.set(dependent, remaining)
      if (remaining === 0) {
        ready.push(dependent)
        ready.sort()
      }
    }
  }
  const cyclic = resolvedRefs.filter(ref => !ordered.includes(ref))

  // Verification-on-compose gate over the resolved closure.
  const unverified = resolvedRefs.filter(ref => {
    const spec = byRef.get(ref)
    return spec !== undefined && !moduleComposeEligible(spec)
  })

  // Interface satisfaction: every required interface ref must be provided by
  // some module in the resolved closure.
  const providedInterfaces = new Set<string>()
  for (const ref of resolvedRefs) {
    for (const provided of byRef.get(ref)!.provides) {
      providedInterfaces.add(provided)
    }
  }
  const unsatisfied = new Set<string>()
  for (const ref of resolvedRefs) {
    for (const required of byRef.get(ref)!.requiresInterfaces) {
      if (!providedInterfaces.has(required)) {
        unsatisfied.add(required)
      }
    }
  }

  const resolved: ReadonlyArray<AgenticNpmResolvedModule> = ordered.map(
    (ref, index) => {
      const spec = byRef.get(ref)!
      return {
        moduleRef: spec.moduleRef,
        version: spec.version,
        moduleDigest: spec.moduleDigest,
        order: index,
      }
    },
  )

  const missingModuleRefs = [...missing].sort()
  const unverifiedModuleRefs = unverified.sort()
  const unsatisfiedInterfaceRefs = [...unsatisfied].sort()
  const cyclicModuleRefs = cyclic.sort()

  const blockerRefs: Array<string> = []
  if (missingModuleRefs.length > 0) {
    blockerRefs.push('blocker.agentic_npm_composition.missing_module')
  }
  if (cyclicModuleRefs.length > 0) {
    blockerRefs.push('blocker.agentic_npm_composition.dependency_cycle')
  }
  if (unverifiedModuleRefs.length > 0) {
    blockerRefs.push('blocker.agentic_npm_composition.module_not_verified')
  }
  if (unsatisfiedInterfaceRefs.length > 0) {
    blockerRefs.push('blocker.agentic_npm_composition.interface_unsatisfied')
  }

  const composable = blockerRefs.length === 0

  // Content-addressed plan digest over the canonical, ordered plan. The digest
  // covers the resolved (ref, version, digest, order) tuples plus the gating
  // verdicts, so any change to the resolution or its safety verdicts changes it.
  const canonical = JSON.stringify({
    composable,
    cyclicModuleRefs,
    missingModuleRefs,
    requestedRootRefs: roots,
    resolved,
    schema: AGENTIC_NPM_COMPOSITION_RUNTIME_SCHEMA,
    unsatisfiedInterfaceRefs,
    unverifiedModuleRefs,
  })
  const planDigest = `plan.agentic_npm.${(await sha256Hex(canonical)).slice(0, 32)}`

  return {
    authority: {
      billingAuthority: false,
      executionAuthority: false,
      installAuthority: false,
      meteringAuthority: false,
      settlementAuthority: false,
    },
    blockerRefs,
    caveatRefs: [
      'caveat.agentic_npm_composition.resolver_is_inert_no_install',
      'caveat.agentic_npm_composition.verification_gate_reuses_exact_trace_poc',
      'caveat.agentic_npm_composition.plan_is_not_settlement',
    ],
    composable,
    cyclicModuleRefs,
    inert: true,
    missingModuleRefs,
    planDigest,
    promiseId: AGENTIC_NPM_MODULE_REGISTRY_PROMISE,
    promiseState: 'planned',
    requestedRootRefs: roots,
    resolved,
    schema: AGENTIC_NPM_COMPOSITION_RUNTIME_SCHEMA,
    unsatisfiedInterfaceRefs,
    unverifiedModuleRefs,
  }
}

export const publishAgenticNpmModule = async (input: {
  store: AgenticNpmRegistryRuntimeStore
  module: AgenticNpmModuleSpec
  publisherRef: string
  publishedAt: string
}): Promise<AgenticNpmPublishedModule> => {
  assertSafe('publisher ref', [input.publisherRef])
  assertSafe('published module ref', [input.module.moduleRef])
  assertSafe('published module digest', [input.module.moduleDigest])
  assertSafe('published module provides', input.module.provides)
  assertSafe('published module dependsOn', input.module.dependsOn)
  assertSafe(
    'published module requiresInterfaces',
    input.module.requiresInterfaces,
  )
  const published: AgenticNpmPublishedModule = {
    ...input.module,
    publishedAt: input.publishedAt,
    publisherRef: input.publisherRef,
  }
  await input.store.publishModule(published)
  return published
}

export const discoverAgenticNpmModules = async (input: {
  store: AgenticNpmRegistryRuntimeStore
  providesInterfaceRef?: string
}): Promise<ReadonlyArray<AgenticNpmPublishedModule>> => {
  if (input.providesInterfaceRef !== undefined) {
    assertSafe('provided interface ref', [input.providesInterfaceRef])
  }
  const modules = await input.store.listModules()
  if (input.providesInterfaceRef === undefined) {
    return modules
  }
  return modules.filter(module =>
    module.provides.includes(input.providesInterfaceRef!),
  )
}

export const installAgenticNpmModules = async (input: {
  store: AgenticNpmRegistryRuntimeStore
  installerRef: string
  requestedRootRefs: ReadonlyArray<string>
  observedAt: string
}): Promise<AgenticNpmInstallResult> => {
  assertSafe('installer ref', [input.installerRef])
  const registry = await input.store.listModules()
  const plan = await resolveAgenticNpmComposition({
    registry,
    requestedRootRefs: input.requestedRootRefs,
  })
  const installRef = `install.agentic_npm.${(
    await sha256Hex(
      canonicalJson({
        installerRef: input.installerRef,
        planDigest: plan.planDigest,
        requestedRootRefs: plan.requestedRootRefs,
      }),
    )
  ).slice(0, 32)}`
  const evidence: AgenticNpmInstallEvidenceRow = {
    blockerRefs: plan.blockerRefs,
    evidenceKind: 'agentic_npm_install',
    installRef,
    installedModuleRefs: plan.resolved.map(module => module.moduleRef),
    installerRef: input.installerRef,
    observedAt: input.observedAt,
    planDigest: plan.planDigest,
    requestedRootRefs: plan.requestedRootRefs,
    schema: AGENTIC_NPM_REGISTRY_RUNTIME_SCHEMA,
    status: plan.composable ? 'installed' : 'blocked',
  }
  await input.store.writeInstallEvidence(evidence)
  if (!plan.composable) {
    return { evidence, installRef, ok: false, plan }
  }
  return { evidence, installRef, ok: true, plan }
}

export const invokeAgenticNpmModule = async (input: {
  store: AgenticNpmRegistryRuntimeStore
  adapters: Readonly<Record<string, AgenticNpmModuleAdapter>>
  installRef: string
  moduleRef: string
  callerRef: string
  payload: Readonly<Record<string, unknown>>
  observedAt: string
}): Promise<AgenticNpmInvokeResult> => {
  assertSafe('install ref', [input.installRef])
  assertSafe('module ref', [input.moduleRef])
  assertSafe('caller ref', [input.callerRef])
  const install = await input.store.readInstallEvidence(input.installRef)
  const inputDigest = `input.agentic_npm.${(
    await sha256Hex(canonicalJson(input.payload))
  ).slice(0, 32)}`
  const usageRef = `usage.agentic_npm.${(
    await sha256Hex(
      canonicalJson({
        callerRef: input.callerRef,
        inputDigest,
        installRef: input.installRef,
        moduleRef: input.moduleRef,
        observedAt: input.observedAt,
      }),
    )
  ).slice(0, 32)}`
  const blocked = async (
    error: AgenticNpmInvokeError,
    blockerRef: string,
  ): Promise<AgenticNpmInvokeResult> => {
    const evidence: AgenticNpmUsageEvidenceRow = {
      blockerRefs: [blockerRef],
      callerRef: input.callerRef,
      evidenceKind: 'agentic_npm_usage',
      inputDigest,
      installRef: input.installRef,
      moduleRef: input.moduleRef,
      observedAt: input.observedAt,
      outputDigest: null,
      schema: AGENTIC_NPM_REGISTRY_RUNTIME_SCHEMA,
      status: 'blocked',
      usageRef,
    }
    await input.store.writeUsageEvidence(evidence)
    return { error, evidence, ok: false }
  }
  if (install === undefined) {
    return blocked(
      'install_not_found',
      'blocker.agentic_npm_runtime.install_not_found',
    )
  }
  if (
    install.status !== 'installed' ||
    !install.installedModuleRefs.includes(input.moduleRef)
  ) {
    return blocked(
      'install_not_usable',
      'blocker.agentic_npm_runtime.install_not_usable',
    )
  }
  const adapter = input.adapters[input.moduleRef]
  if (adapter === undefined) {
    return blocked(
      'adapter_not_registered',
      'blocker.agentic_npm_runtime.adapter_not_registered',
    )
  }
  const output = await adapter(input.payload)
  const outputDigest = `output.agentic_npm.${(
    await sha256Hex(canonicalJson(output))
  ).slice(0, 32)}`
  const evidence: AgenticNpmUsageEvidenceRow = {
    blockerRefs: [],
    callerRef: input.callerRef,
    evidenceKind: 'agentic_npm_usage',
    inputDigest,
    installRef: input.installRef,
    moduleRef: input.moduleRef,
    observedAt: input.observedAt,
    outputDigest,
    schema: AGENTIC_NPM_REGISTRY_RUNTIME_SCHEMA,
    status: 'invoked',
    usageRef,
  }
  await input.store.writeUsageEvidence(evidence)
  return { evidence, ok: true, output }
}
