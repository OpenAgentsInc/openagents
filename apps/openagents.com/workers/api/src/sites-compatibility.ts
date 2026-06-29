import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type {
  AutopilotSiteProject,
  AutopilotSiteSourceRepository,
} from './sites'
import {
  isRecord,
  optionalString,
  parseJsonRecord,
  parseJsonUnknown,
  stringArrayFromUnknown,
} from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const SiteCompatibilitySourceKind = S.Literals([
  'github_import',
  'operator_static',
])
export type SiteCompatibilitySourceKind =
  typeof SiteCompatibilitySourceKind.Type

export const SiteCompatibilityStatus = S.Literals([
  'ready',
  'warning',
  'blocked',
  'unknown',
])
export type SiteCompatibilityStatus = typeof SiteCompatibilityStatus.Type

export const SiteCompatibilityConfidence = S.Literals([
  'high',
  'medium',
  'low',
])
export type SiteCompatibilityConfidence =
  typeof SiteCompatibilityConfidence.Type

export const SiteCompatibilityOutputKind = S.Literals([
  'static',
  'worker_module',
  'ssr',
  'unknown',
])
export type SiteCompatibilityOutputKind =
  typeof SiteCompatibilityOutputKind.Type

export const SiteCompatibilityFindingSeverity = S.Literals([
  'blocker',
  'warning',
  'info',
])
export type SiteCompatibilityFindingSeverity =
  typeof SiteCompatibilityFindingSeverity.Type

export const SiteCompatibilityFinding = S.Struct({
  code: S.String,
  evidence: S.Array(S.String),
  message: S.String,
  severity: SiteCompatibilityFindingSeverity,
})
export type SiteCompatibilityFinding = typeof SiteCompatibilityFinding.Type

export const SiteCompatibilityProjectFile = S.Struct({
  path: S.String,
  text: S.String,
})
export type SiteCompatibilityProjectFile =
  typeof SiteCompatibilityProjectFile.Type

export const CheckSiteCompatibilityInput = S.Struct({
  actorUserId: S.optionalKey(S.String),
  files: S.Array(SiteCompatibilityProjectFile),
  site: S.Unknown,
  sourceKind: S.optionalKey(SiteCompatibilitySourceKind),
  sourceRepository: S.optionalKey(S.NullOr(S.Unknown)),
})
export type CheckSiteCompatibilityInput = Readonly<{
  actorUserId?: string | undefined
  files: ReadonlyArray<SiteCompatibilityProjectFile>
  site: AutopilotSiteProject
  sourceKind?: SiteCompatibilitySourceKind | undefined
  sourceRepository?: AutopilotSiteSourceRepository | null | undefined
}>

export type SiteCompatibilityReceipt = Readonly<{
  blockers: ReadonlyArray<SiteCompatibilityFinding>
  buildCommand: string | null
  checkedByUserId: string | null
  confidence: SiteCompatibilityConfidence
  createdAt: string
  customerSafeNextAction: string
  customerSafeStatus: string
  envKeys: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  findings: ReadonlyArray<SiteCompatibilityFinding>
  id: string
  needsD1: boolean
  needsPublicAuth: boolean
  needsR2: boolean
  needsWorkspaceAuth: boolean
  outputKind: SiteCompatibilityOutputKind
  outputPath: string | null
  packageManager: string | null
  siteId: string
  sourceKind: SiteCompatibilitySourceKind
  sourceRepository: AutopilotSiteSourceRepository | null
  status: SiteCompatibilityStatus
  warnings: ReadonlyArray<SiteCompatibilityFinding>
  workerModulePath: string | null
}>

type ReceiptRow = Readonly<{
  blockers_json: string
  build_command: string | null
  checked_by_user_id: string | null
  confidence: SiteCompatibilityConfidence
  created_at: string
  customer_safe_next_action: string
  customer_safe_status: string
  env_keys_json: string
  evidence_refs_json: string
  findings_json: string
  id: string
  needs_d1: number
  needs_public_auth: number
  needs_r2: number
  needs_workspace_auth: number
  output_kind: SiteCompatibilityOutputKind
  output_path: string | null
  package_manager: string | null
  site_id: string
  source_kind: SiteCompatibilitySourceKind
  source_repository_json: string | null
  status: SiteCompatibilityStatus
  warnings_json: string
  worker_module_path: string | null
}>

export type SiteCompatibilityRuntime = Readonly<{
  makeCheckId: () => string
  makeEventId: () => string
  nowIso: () => string
}>

export const systemSiteCompatibilityRuntime: SiteCompatibilityRuntime = {
  makeCheckId: () => compactRandomId('site_compatibility_check'),
  makeEventId: () => compactRandomId('site_event'),
  nowIso: currentIsoTimestamp,
}

export class SiteCompatibilityStorageError extends S.TaggedErrorClass<SiteCompatibilityStorageError>()(
  'SiteCompatibilityStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class SiteCompatibilityUnsafePayload extends S.TaggedErrorClass<SiteCompatibilityUnsafePayload>()(
  'SiteCompatibilityUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class SiteCompatibilityValidationError extends S.TaggedErrorClass<SiteCompatibilityValidationError>()(
  'SiteCompatibilityValidationError',
  {
    reason: S.String,
  },
) {}

export type SiteCompatibilityError =
  | SiteCompatibilityStorageError
  | SiteCompatibilityUnsafePayload
  | SiteCompatibilityValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SiteCompatibilityStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new SiteCompatibilityStorageError({ operation, error }),
  })

const finding = (
  severity: SiteCompatibilityFindingSeverity,
  code: string,
  message: string,
  evidence: ReadonlyArray<string>,
): SiteCompatibilityFinding => ({ code, evidence: [...evidence], message, severity })

const normalizePath = (path: string): string =>
  path.trim().replaceAll('\\', '/').replace(/^\.\/+/, '').toLowerCase()

const fileMap = (
  files: ReadonlyArray<SiteCompatibilityProjectFile>,
): Map<string, string> =>
  new Map(
    files
      .map(file => [normalizePath(file.path), file.text] as const)
      .filter(([path]) => path !== ''),
  )

const packageJson = (
  files: Map<string, string>,
): Record<string, unknown> | undefined => parseJsonRecord(files.get('package.json'))

const dependencies = (
  pkg: Record<string, unknown> | undefined,
): Set<string> => {
  const names = new Set<string>()

  for (const key of ['dependencies', 'devDependencies', 'peerDependencies']) {
    const deps = isRecord(pkg?.[key]) ? pkg?.[key] : undefined

    for (const name of Object.keys(deps ?? {})) {
      names.add(name)
    }
  }

  return names
}

const packageManager = (
  files: Map<string, string>,
  pkg: Record<string, unknown> | undefined,
): string | null => {
  const declared = optionalString(pkg?.packageManager)?.split('@')[0]

  if (declared !== undefined) {
    return declared
  }

  if (files.has('bun.lockb') || files.has('bun.lock')) {
    return 'bun'
  }

  if (files.has('pnpm-lock.yaml')) {
    return 'pnpm'
  }

  if (files.has('yarn.lock')) {
    return 'yarn'
  }

  if (files.has('package-lock.json')) {
    return 'npm'
  }

  return null
}

const buildCommand = (
  manager: string | null,
  pkg: Record<string, unknown> | undefined,
): string | null => {
  const scripts = isRecord(pkg?.scripts) ? pkg?.scripts : undefined
  const build = optionalString(scripts?.build)

  if (build === undefined) {
    return null
  }

  switch (manager) {
    case 'bun':
      return 'bun run build'
    case 'pnpm':
      return 'pnpm build'
    case 'yarn':
      return 'yarn build'
    case 'npm':
    case null:
      return 'npm run build'
    default:
      return `${manager} run build`
  }
}

const collectEnvKeys = (
  files: Map<string, string>,
): ReadonlyArray<string> => {
  const keys = new Set<string>()
  const keyPattern =
    /(?:process\.env|import\.meta\.env|env)\.([A-Z][A-Z0-9_]{1,80})/g

  for (const [path, text] of files.entries()) {
    if (path.endsWith('.env') || path.includes('.env')) {
      for (const line of text.split(/\r?\n/)) {
        const match = /^([A-Z][A-Z0-9_]{1,80})\s*=/.exec(line.trim())

        if (match?.[1] !== undefined) {
          keys.add(match[1])
        }
      }
    }

    for (const match of text.matchAll(keyPattern)) {
      if (match[1] !== undefined) {
        keys.add(match[1])
      }
    }
  }

  return [...keys].sort()
}

const wranglerMain = (files: Map<string, string>): string | null => {
  const toml = files.get('wrangler.toml')

  if (toml !== undefined) {
    return /^\s*main\s*=\s*["']([^"']+)["']/m.exec(toml)?.[1] ?? null
  }

  const json = parseJsonRecord(files.get('wrangler.json'))

  return optionalString(json?.main) ?? null
}

const hasAnyFile = (
  files: Map<string, string>,
  candidates: ReadonlyArray<string>,
): boolean => candidates.some(candidate => files.has(candidate))

const textIncludes = (
  files: Map<string, string>,
  needles: ReadonlyArray<string>,
): boolean => {
  const lowerNeedles = needles.map(needle => needle.toLowerCase())

  return [...files.values()].some(text => {
    const lower = text.toLowerCase()

    return lowerNeedles.some(needle => lower.includes(needle))
  })
}

const inferOutput = (
  files: Map<string, string>,
  deps: Set<string>,
): Readonly<{
  outputKind: SiteCompatibilityOutputKind
  outputPath: string | null
  workerModulePath: string | null
}> => {
  const main = wranglerMain(files)

  if (main !== null) {
    return {
      outputKind: 'worker_module',
      outputPath: null,
      workerModulePath: main,
    }
  }

  if (deps.has('next') || files.has('next.config.js') || files.has('next.config.mjs')) {
    return { outputKind: 'ssr', outputPath: '.next', workerModulePath: null }
  }

  if (deps.has('vite') || files.has('vite.config.ts') || files.has('vite.config.js')) {
    return { outputKind: 'static', outputPath: 'dist', workerModulePath: null }
  }

  if (hasAnyFile(files, ['index.html'])) {
    return { outputKind: 'static', outputPath: '.', workerModulePath: null }
  }

  return { outputKind: 'unknown', outputPath: null, workerModulePath: null }
}

const inspectCompatibility = (
  input: CheckSiteCompatibilityInput,
): SiteCompatibilityReceipt => {
  const files = fileMap(input.files)
  const pkg = packageJson(files)
  const deps = dependencies(pkg)
  const manager = packageManager(files, pkg)
  const command = buildCommand(manager, pkg)
  const output = inferOutput(files, deps)
  const evidenceRefs = [...files.keys()].sort()
  const findings: Array<SiteCompatibilityFinding> = []

  if (files.size === 0) {
    findings.push(
      finding('blocker', 'no_project_files', 'No project files were provided.', []),
    )
  }

  if (pkg === undefined && files.has('package.json')) {
    findings.push(
      finding(
        'blocker',
        'invalid_package_json',
        'package.json could not be parsed as an object.',
        ['package.json'],
      ),
    )
  }

  if (pkg !== undefined && command === null) {
    findings.push(
      finding(
        'warning',
        'missing_build_script',
        'package.json does not declare a build script.',
        ['package.json'],
      ),
    )
  }

  if (output.outputKind === 'worker_module') {
    findings.push(
      finding(
        'info',
        'worker_module_candidate',
        'Wrangler declares a Worker module candidate.',
        output.workerModulePath === null ? ['wrangler config'] : [output.workerModulePath],
      ),
    )
  }

  if (output.outputKind === 'static') {
    findings.push(
      finding(
        'info',
        'static_output_candidate',
        'Project appears to produce static output that can be wrapped by the Site runtime.',
        output.outputPath === null ? [] : [output.outputPath],
      ),
    )
  }

  if (output.outputKind === 'ssr') {
    findings.push(
      finding(
        'blocker',
        'unsupported_ssr_runtime',
        'Project appears to require an SSR runtime that is not yet validated for Sites.',
        ['package.json'],
      ),
    )
  }

  if (output.outputKind === 'unknown') {
    findings.push(
      finding(
        'warning',
        'unknown_output_shape',
        'Could not determine static output or Worker module entrypoint.',
        evidenceRefs.slice(0, 5),
      ),
    )
  }

  if (
    textIncludes(files, [
      'node:fs',
      "from 'fs'",
      'from "fs"',
      'require("fs")',
      "require('fs')",
      'child_process',
      'node:net',
      'node:http',
    ])
  ) {
    findings.push(
      finding(
        'blocker',
        'unsupported_node_runtime_api',
        'Project references Node runtime APIs that are not supported by default in Workers.',
        evidenceRefs.filter(path => path.endsWith('.js') || path.endsWith('.ts')),
      ),
    )
  }

  const needsD1 = textIncludes(files, ['D1Database', 'env.DB', 'drizzle-orm/d1'])
  const needsR2 = textIncludes(files, ['R2Bucket', 'env.R2', 'env.BUCKET'])
  const needsWorkspaceAuth = textIncludes(files, [
    'workspace user',
    'current workspace',
    'OPENAUTH_ISSUER_URL',
  ])
  const needsPublicAuth =
    deps.has('next-auth') ||
    deps.has('@auth/core') ||
    deps.has('@clerk/nextjs') ||
    deps.has('@auth0/nextjs-auth0')

  if (needsD1) {
    findings.push(
      finding('info', 'needs_d1', 'Project appears to need D1 structured storage.', []),
    )
  }

  if (needsR2) {
    findings.push(
      finding('info', 'needs_r2', 'Project appears to need R2 object storage.', []),
    )
  }

  if (needsWorkspaceAuth) {
    findings.push(
      finding(
        'info',
        'needs_workspace_auth',
        'Project appears to need workspace-authenticated user identity.',
        [],
      ),
    )
  }

  if (needsPublicAuth) {
    findings.push(
      finding(
        'warning',
        'needs_public_auth',
        'Project appears to need public or external identity-provider auth.',
        ['package.json'],
      ),
    )
  }

  const blockers = findings.filter(item => item.severity === 'blocker')
  const warnings = findings.filter(item => item.severity === 'warning')
  const status: SiteCompatibilityStatus =
    blockers.length > 0
      ? 'blocked'
      : output.outputKind === 'unknown'
        ? 'unknown'
        : warnings.length > 0
          ? 'warning'
          : 'ready'
  const confidence: SiteCompatibilityConfidence =
    output.outputKind === 'unknown' || files.size === 0
      ? 'low'
      : blockers.length > 0 || warnings.length > 0
        ? 'medium'
        : 'high'

  return {
    blockers,
    buildCommand: command,
    checkedByUserId: input.actorUserId ?? null,
    confidence,
    createdAt: '',
    customerSafeNextAction:
      status === 'blocked'
        ? 'Resolve compatibility blockers before building this Site.'
        : status === 'ready'
          ? 'This project is ready for Site build validation.'
          : 'Review compatibility warnings before build validation.',
    customerSafeStatus: status,
    envKeys: collectEnvKeys(files),
    evidenceRefs,
    findings,
    id: '',
    needsD1,
    needsPublicAuth,
    needsR2,
    needsWorkspaceAuth,
    outputKind: output.outputKind,
    outputPath: output.outputPath,
    packageManager: manager,
    siteId: input.site.id,
    sourceKind: input.sourceKind ?? 'github_import',
    sourceRepository: input.sourceRepository ?? input.site.sourceRepository,
    status,
    warnings,
    workerModulePath: output.workerModulePath,
  }
}

const jsonArray = (value: unknown): string => JSON.stringify(value)

const assertSafeReceipt = (
  receipt: SiteCompatibilityReceipt,
): Effect.Effect<void, SiteCompatibilityUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(receipt))
    ? Effect.fail(
        new SiteCompatibilityUnsafePayload({
          reason: 'Site compatibility receipt contains secret-shaped material.',
        }),
      )
    : Effect.void

const receiptFromRow = (row: ReceiptRow): SiteCompatibilityReceipt => ({
  blockers: parseJsonRecordArray(row.blockers_json) as Array<SiteCompatibilityFinding>,
  buildCommand: row.build_command,
  checkedByUserId: row.checked_by_user_id,
  confidence: row.confidence,
  createdAt: row.created_at,
  customerSafeNextAction: row.customer_safe_next_action,
  customerSafeStatus: row.customer_safe_status,
  envKeys: stringArrayFromUnknown(parseJsonValue(row.env_keys_json)),
  evidenceRefs: stringArrayFromUnknown(parseJsonValue(row.evidence_refs_json)),
  findings: parseJsonRecordArray(row.findings_json) as Array<SiteCompatibilityFinding>,
  id: row.id,
  needsD1: row.needs_d1 === 1,
  needsPublicAuth: row.needs_public_auth === 1,
  needsR2: row.needs_r2 === 1,
  needsWorkspaceAuth: row.needs_workspace_auth === 1,
  outputKind: row.output_kind,
  outputPath: row.output_path,
  packageManager: row.package_manager,
  siteId: row.site_id,
  sourceKind: row.source_kind,
  sourceRepository: sourceRepositoryFromJson(row.source_repository_json),
  status: row.status,
  warnings: parseJsonRecordArray(row.warnings_json) as Array<SiteCompatibilityFinding>,
  workerModulePath: row.worker_module_path,
})

const parseJsonValue = (value: string): unknown => {
  try {
    return parseJsonUnknown(value)
  } catch {
    return undefined
  }
}

const parseJsonRecordArray = (value: string): ReadonlyArray<Record<string, unknown>> => {
  const parsed = parseJsonValue(value)

  return Array.isArray(parsed)
    ? parsed.filter((item): item is Record<string, unknown> => isRecord(item))
    : []
}

const sourceRepositoryFromJson = (
  value: string | null,
): AutopilotSiteSourceRepository | null => {
  const parsed = parseJsonRecord(value)

  if (
    parsed?.provider === 'github' &&
    typeof parsed.owner === 'string' &&
    typeof parsed.name === 'string' &&
    typeof parsed.ref === 'string'
  ) {
    return {
      provider: 'github',
      owner: parsed.owner,
      name: parsed.name,
      ref: parsed.ref,
    }
  }

  return null
}

const insertReceipt = (
  db: D1Database,
  runtime: SiteCompatibilityRuntime,
  receipt: SiteCompatibilityReceipt,
): Effect.Effect<SiteCompatibilityReceipt, SiteCompatibilityError> =>
  Effect.gen(function* () {
    const id = runtime.makeCheckId()
    const now = runtime.nowIso()
    const saved = { ...receipt, createdAt: now, id }

    yield* assertSafeReceipt(saved)

    yield* d1Effect('siteCompatibilityChecks.insert', () =>
      db
        .prepare(
          `INSERT INTO site_compatibility_checks
             (id,
              site_id,
              source_kind,
              source_repository_json,
              status,
              confidence,
              package_manager,
              build_command,
              output_kind,
              output_path,
              worker_module_path,
              needs_d1,
              needs_r2,
              needs_workspace_auth,
              needs_public_auth,
              env_keys_json,
              findings_json,
              blockers_json,
              warnings_json,
              evidence_refs_json,
              customer_safe_status,
              customer_safe_next_action,
              checked_by_user_id,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          saved.id,
          saved.siteId,
          saved.sourceKind,
          saved.sourceRepository === null
            ? null
            : JSON.stringify(saved.sourceRepository),
          saved.status,
          saved.confidence,
          saved.packageManager,
          saved.buildCommand,
          saved.outputKind,
          saved.outputPath,
          saved.workerModulePath,
          saved.needsD1 ? 1 : 0,
          saved.needsR2 ? 1 : 0,
          saved.needsWorkspaceAuth ? 1 : 0,
          saved.needsPublicAuth ? 1 : 0,
          jsonArray(saved.envKeys),
          jsonArray(saved.findings),
          jsonArray(saved.blockers),
          jsonArray(saved.warnings),
          jsonArray(saved.evidenceRefs),
          saved.customerSafeStatus,
          saved.customerSafeNextAction,
          saved.checkedByUserId,
          saved.createdAt,
        )
        .run(),
    )

    yield* d1Effect('siteCompatibilityChecks.event.insert', () =>
      db
        .prepare(
          `INSERT INTO site_events
             (id,
              site_id,
              version_id,
              deployment_id,
              type,
              summary,
              actor_user_id,
              actor_run_id,
              payload_json,
              created_at)
           VALUES (?, ?, NULL, NULL, 'site_compatibility.checked', ?, ?, NULL, ?, ?)`,
        )
        .bind(
          runtime.makeEventId(),
          saved.siteId,
          `Checked Site compatibility: ${saved.status}.`,
          saved.checkedByUserId,
          JSON.stringify({
            checkId: saved.id,
            status: saved.status,
            blockers: saved.blockers.map(item => item.code),
            warnings: saved.warnings.map(item => item.code),
          }),
          saved.createdAt,
        )
        .run(),
    )

    return saved
  })

const latestReceipt = (
  db: D1Database,
  siteId: string,
): Effect.Effect<SiteCompatibilityReceipt | null, SiteCompatibilityError> =>
  d1Effect('siteCompatibilityChecks.latest.read', () =>
    db
      .prepare(
        `SELECT id,
                site_id,
                source_kind,
                source_repository_json,
                status,
                confidence,
                package_manager,
                build_command,
                output_kind,
                output_path,
                worker_module_path,
                needs_d1,
                needs_r2,
                needs_workspace_auth,
                needs_public_auth,
                env_keys_json,
                findings_json,
                blockers_json,
                warnings_json,
                evidence_refs_json,
                customer_safe_status,
                customer_safe_next_action,
                checked_by_user_id,
                created_at
           FROM site_compatibility_checks
          WHERE site_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(siteId)
      .first<ReceiptRow>(),
  ).pipe(Effect.map(row => (row === null ? null : receiptFromRow(row))))

const checkCompatibility = (
  db: D1Database,
  runtime: SiteCompatibilityRuntime,
  input: CheckSiteCompatibilityInput,
): Effect.Effect<SiteCompatibilityReceipt, SiteCompatibilityError> => {
  const receipt = inspectCompatibility(input)

  return insertReceipt(db, runtime, receipt)
}

export const makeSiteCompatibilityService = (
  db: D1Database,
  runtime: SiteCompatibilityRuntime = systemSiteCompatibilityRuntime,
) => ({
  checkCompatibility: Effect.fn('SiteCompatibilityService.check')(
    (input: CheckSiteCompatibilityInput) =>
      checkCompatibility(db, runtime, input),
  ),
  latestReceipt: Effect.fn('SiteCompatibilityService.latest')((siteId: string) =>
    latestReceipt(db, siteId),
  ),
})
