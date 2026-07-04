import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type {
  AutopilotSiteProject,
  AutopilotSiteSourceRepository,
  AutopilotSiteVersionSourceKind,
} from './sites'
import {
  isRecord,
  optionalString,
  parseJsonRecord,
  parseJsonUnknown,
  stringArrayFromUnknown,
} from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  SiteCompatibilityFinding,
  SiteCompatibilityOutputKind,
  SiteCompatibilityProjectFile,
  type SiteCompatibilityFindingSeverity,
} from './sites-compatibility'

export const SiteBuildValidationStatus = S.Literals([
  'passed',
  'warning',
  'failed',
  'blocked',
  'unknown',
])
export type SiteBuildValidationStatus =
  typeof SiteBuildValidationStatus.Type

export const SiteBuildValidationManifest = S.Struct({
  assets: S.Array(S.String),
  bindings: S.Struct({
    d1: S.Array(S.String),
    r2: S.Array(S.String),
  }),
  entrypoints: S.Array(S.String),
  envKeys: S.Array(S.String),
  migrations: S.Array(S.String),
})
export type SiteBuildValidationManifest =
  typeof SiteBuildValidationManifest.Type

export const SiteBuildValidationCompatibilityHint = S.Struct({
  blockers: S.optionalKey(S.Array(SiteCompatibilityFinding)),
  buildCommand: S.optionalKey(S.NullOr(S.String)),
  compatibilityCheckId: S.optionalKey(S.String),
  outputKind: S.optionalKey(SiteCompatibilityOutputKind),
  outputPath: S.optionalKey(S.NullOr(S.String)),
  packageManager: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(S.String),
  warnings: S.optionalKey(S.Array(SiteCompatibilityFinding)),
  workerModulePath: S.optionalKey(S.NullOr(S.String)),
})
export type SiteBuildValidationCompatibilityHint =
  typeof SiteBuildValidationCompatibilityHint.Type

export const SiteVisualAssetRequirement = S.Struct({
  kind: S.Literals(['image', 'screenshot', 'diagram', 'reference_image']),
  required: S.Boolean,
  source: S.Literals(['customer_request', 'operator_notes', 'task_packet']),
  summary: S.String,
})
export type SiteVisualAssetRequirement =
  typeof SiteVisualAssetRequirement.Type

export const ValidateSiteBuildInputSchema = S.Struct({
  actorUserId: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  compatibility: S.optionalKey(SiteBuildValidationCompatibilityHint),
  files: S.Array(SiteCompatibilityProjectFile),
  requestedBuildCommand: S.optionalKey(S.String),
  site: S.Unknown,
  sourceCommitSha: S.optionalKey(S.String),
  sourceKind: S.optionalKey(
    S.Literals(['autopilot_generated', 'github_import', 'operator_static']),
  ),
  sourceRepository: S.optionalKey(S.NullOr(S.Unknown)),
  visualAssetRequirements: S.optionalKey(S.Array(SiteVisualAssetRequirement)),
})
export type ValidateSiteBuildInput = Readonly<{
  actorUserId?: string | undefined
  buildLogText?: string | undefined
  compatibility?: SiteBuildValidationCompatibilityHint | undefined
  files: ReadonlyArray<SiteCompatibilityProjectFile>
  requestedBuildCommand?: string | undefined
  site: AutopilotSiteProject
  sourceCommitSha?: string | undefined
  sourceKind?: AutopilotSiteVersionSourceKind | undefined
  sourceRepository?: AutopilotSiteSourceRepository | null | undefined
  visualAssetRequirements?: ReadonlyArray<SiteVisualAssetRequirement> | undefined
}>

export type SiteBuildValidationReceipt = Readonly<{
  blockers: ReadonlyArray<SiteCompatibilityFinding>
  boundedLogs: ReadonlyArray<string>
  buildCommand: string | null
  compatibilityCheckId: string | null
  createdAt: string
  customerSafeNextAction: string
  customerSafeStatus: string
  evidenceRefs: ReadonlyArray<string>
  findings: ReadonlyArray<SiteCompatibilityFinding>
  id: string
  logLineCount: number
  logTruncated: boolean
  manifest: SiteBuildValidationManifest
  outputKind: SiteCompatibilityOutputKind
  outputPath: string | null
  packageManager: string | null
  requestedBuildCommand: string | null
  siteId: string
  sourceCommitSha: string | null
  sourceHash: string
  sourceKind: AutopilotSiteVersionSourceKind
  sourceRepository: AutopilotSiteSourceRepository | null
  status: SiteBuildValidationStatus
  validatedByUserId: string | null
  warnings: ReadonlyArray<SiteCompatibilityFinding>
  workerModulePath: string | null
}>

type ReceiptRow = Readonly<{
  blockers_json: string
  bounded_logs_json: string
  build_command: string | null
  compatibility_check_id: string | null
  created_at: string
  customer_safe_next_action: string
  customer_safe_status: string
  evidence_refs_json: string
  findings_json: string
  id: string
  log_line_count: number
  log_truncated: number
  manifest_json: string
  output_kind: SiteCompatibilityOutputKind
  output_path: string | null
  package_manager: string | null
  requested_build_command: string | null
  site_id: string
  source_commit_sha: string | null
  source_hash: string
  source_kind: AutopilotSiteVersionSourceKind
  source_repository_json: string | null
  status: SiteBuildValidationStatus
  validated_by_user_id: string | null
  warnings_json: string
  worker_module_path: string | null
}>

export type SiteBuildValidationRuntime = Readonly<{
  makeEventId: () => string
  makeValidationId: () => string
  nowIso: () => string
}>

export const systemSiteBuildValidationRuntime: SiteBuildValidationRuntime = {
  makeEventId: () => compactRandomId('site_event'),
  makeValidationId: () => compactRandomId('site_build_validation'),
  nowIso: currentIsoTimestamp,
}

export class SiteBuildValidationStorageError extends S.TaggedErrorClass<SiteBuildValidationStorageError>()(
  'SiteBuildValidationStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

export class SiteBuildValidationUnsafePayload extends S.TaggedErrorClass<SiteBuildValidationUnsafePayload>()(
  'SiteBuildValidationUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class SiteBuildValidationValidationError extends S.TaggedErrorClass<SiteBuildValidationValidationError>()(
  'SiteBuildValidationValidationError',
  {
    reason: S.String,
  },
) {}

export type SiteBuildValidationError =
  | SiteBuildValidationStorageError
  | SiteBuildValidationUnsafePayload
  | SiteBuildValidationValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SiteBuildValidationStorageError> =>
  Effect.tryPromise({
    catch: error => new SiteBuildValidationStorageError({ error, operation }),
    try: run,
  })

const finding = (
  severity: SiteCompatibilityFindingSeverity,
  code: string,
  message: string,
  evidence: ReadonlyArray<string>,
): SiteCompatibilityFinding => ({
  code,
  evidence: [...evidence],
  message,
  severity,
})

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

const inferPackageManager = (
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

const inferBuildCommand = (
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

const wranglerMain = (files: Map<string, string>): string | null => {
  const toml = files.get('wrangler.toml')

  if (toml !== undefined) {
    return /^\s*main\s*=\s*["']([^"']+)["']/m.exec(toml)?.[1] ?? null
  }

  const json = parseJsonRecord(files.get('wrangler.json'))
  const jsonMain = optionalString(json?.main)

  if (jsonMain !== undefined) {
    return jsonMain
  }

  const jsonc = files.get('wrangler.jsonc')

  return jsonc === undefined
    ? null
    : /["']main["']\s*:\s*["']([^"']+)["']/.exec(jsonc)?.[1] ?? null
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

  if (files.has('index.html')) {
    return { outputKind: 'static', outputPath: '.', workerModulePath: null }
  }

  return { outputKind: 'unknown', outputPath: null, workerModulePath: null }
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

const collectManifest = (
  files: Map<string, string>,
  output: Readonly<{
    outputKind: SiteCompatibilityOutputKind
    outputPath: string | null
    workerModulePath: string | null
  }>,
): SiteBuildValidationManifest => {
  const envKeys = collectEnvKeys(files)
  const d1 = envKeys.filter(key => key === 'DB' || key.endsWith('_DB'))
  const r2 = envKeys.filter(
    key => key === 'BUCKET' || key === 'R2' || key.endsWith('_BUCKET'),
  )
  const entrypoints =
    output.workerModulePath === null
      ? output.outputPath === null
        ? []
        : [output.outputPath]
      : [output.workerModulePath]
  const migrations = [...files.keys()].filter(
    path => path.startsWith('migrations/') || path.includes('/migrations/'),
  )
  const assets = [...files.keys()].filter(path =>
    /\.(css|gif|html|ico|jpeg|jpg|js|json|png|svg|webp)$/.test(path),
  )

  return {
    assets: assets.sort(),
    bindings: {
      d1,
      r2,
    },
    entrypoints,
    envKeys,
    migrations: migrations.sort(),
  }
}

const boundLogs = (
  text: string | undefined,
  status: SiteBuildValidationStatus,
  buildCommand: string | null,
): Readonly<{
  lineCount: number
  lines: ReadonlyArray<string>
  truncated: boolean
}> => {
  const source =
    text ??
    [
      `Build validation status: ${status}.`,
      buildCommand === null
        ? 'No build command was selected.'
        : `Selected build command: ${buildCommand}.`,
      'Live build execution is deferred to the hosted build runner issue.',
    ].join('\n')
  const rawLines = source.split(/\r?\n/)
  const maxLines = 80
  const maxBytes = 12_000
  const lines: Array<string> = []
  let bytes = 0
  let truncated = rawLines.length > maxLines

  for (const line of rawLines.slice(0, maxLines)) {
    const clean = line.slice(0, 500)
    const encoded = new TextEncoder().encode(clean).byteLength

    if (bytes + encoded > maxBytes) {
      truncated = true
      break
    }

    bytes += encoded
    lines.push(clean)
  }

  return {
    lineCount: rawLines.length,
    lines,
    truncated,
  }
}

const sourceHash = async (
  files: ReadonlyArray<SiteCompatibilityProjectFile>,
): Promise<string> => {
  const canonical = files
    .map(file => [normalizePath(file.path), file.text] as const)
    .filter(([path]) => path !== '')
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([path, text]) => `${path}\n${text}`)
    .join('\n---openagents-site-file---\n')
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(canonical),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

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

const visualRequestPattern =
  /\b(image|images|photo|photos|picture|pictures|screenshot|screenshots|visual asset|visual assets|reference image|reference images)\b/i

const diagramOnlyPattern =
  /\b(diagram|diagrams|schematic|schematics)\b/i

const imageFilePattern = /\.(avif|gif|jpeg|jpg|png|webp)$/i

const htmlImagePattern =
  /<img\b|<picture\b|<source\b[^>]*\bsrcset\s*=|\bbackground-image\s*:|url\(\s*['"]?(?:https?:\/\/|\/|\.\/|data:image\/)/i

export const inferSiteVisualAssetRequirements = (
  values: ReadonlyArray<
    Readonly<{ source: SiteVisualAssetRequirement['source']; text: string }>
  >,
): ReadonlyArray<SiteVisualAssetRequirement> =>
  values
    .filter(value => visualRequestPattern.test(value.text))
    .map(value => ({
      kind: 'image',
      required: true,
      source: value.source,
      summary:
        'Customer/operator request explicitly asks for image or visual media assets.',
    }))

const hasRealImageAsset = (
  files: Map<string, string>,
  manifestAssets: ReadonlyArray<string>,
): boolean =>
  [...files.keys(), ...manifestAssets].some(path => imageFilePattern.test(path)) ||
  [...files.values()].some(text => htmlImagePattern.test(text))

export const collectSiteVisualAssetFindings = (
  files: Map<string, string>,
  requirements: ReadonlyArray<SiteVisualAssetRequirement>,
  manifestAssets: ReadonlyArray<string>,
): ReadonlyArray<SiteCompatibilityFinding> => {
  const requiredImages = requirements.filter(
    requirement =>
      requirement.required &&
      (requirement.kind === 'image' ||
        requirement.kind === 'screenshot' ||
        requirement.kind === 'reference_image'),
  )

  if (requiredImages.length === 0) {
    return []
  }

  if (hasRealImageAsset(files, manifestAssets)) {
    return [
      finding(
        'info',
        'required_visual_asset_present',
        'Requested image media is present in the Site artifact.',
        [...files.keys(), ...manifestAssets]
          .filter(path => imageFilePattern.test(path))
          .slice(0, 10),
      ),
    ]
  }

  const diagramOnly = requirements.every(
    requirement =>
      requirement.kind === 'diagram' ||
      diagramOnlyPattern.test(requirement.summary),
  )

  if (diagramOnly) {
    return []
  }

  return [
    finding(
      'blocker',
      'missing_required_visual_asset',
      'The request requires image media, but the Site artifact only contains text, code, or CSS/SVG shapes.',
      requiredImages.map(requirement => requirement.source),
    ),
  ]
}

const inspectBuildValidation = async (
  input: ValidateSiteBuildInput,
): Promise<SiteBuildValidationReceipt> => {
  const files = fileMap(input.files)
  const pkg = packageJson(files)
  const deps = dependencies(pkg)
  const inferredManager = inferPackageManager(files, pkg)
  const packageManager =
    input.compatibility?.packageManager === undefined
      ? inferredManager
      : input.compatibility.packageManager
  const inferredCommand = inferBuildCommand(packageManager, pkg)
  const buildCommand =
    input.requestedBuildCommand ??
    input.compatibility?.buildCommand ??
    inferredCommand
  const inferredOutput = inferOutput(files, deps)
  const output = {
    outputKind: input.compatibility?.outputKind ?? inferredOutput.outputKind,
    outputPath:
      input.compatibility?.outputPath === undefined
        ? inferredOutput.outputPath
        : input.compatibility.outputPath,
    workerModulePath:
      input.compatibility?.workerModulePath === undefined
        ? inferredOutput.workerModulePath
        : input.compatibility.workerModulePath,
  }
  const evidenceRefs = [...files.keys()].sort()
  const manifest = collectManifest(files, output)
  const findings: Array<SiteCompatibilityFinding> = [
    ...(input.compatibility?.blockers ?? []),
    ...(input.compatibility?.warnings ?? []),
    ...collectSiteVisualAssetFindings(
      files,
      input.visualAssetRequirements ??
        inferSiteVisualAssetRequirements([
          { source: 'customer_request', text: input.site.prompt },
        ]),
      manifest.assets,
    ),
  ]

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

  if (output.outputKind === 'ssr') {
    findings.push(
      finding(
        'blocker',
        'unsupported_ssr_runtime',
        'SSR output is not yet deployable by the current Sites runtime.',
        ['package.json'],
      ),
    )
  }

  if (output.outputKind === 'unknown') {
    findings.push(
      finding(
        'warning',
        'unknown_output_shape',
        'Build validation could not determine a static output path or Worker module entrypoint.',
        evidenceRefs.slice(0, 5),
      ),
    )
  }

  if (buildCommand === null && pkg !== undefined && output.outputKind !== 'worker_module') {
    findings.push(
      finding(
        'blocker',
        'missing_build_command',
        'No build command was available for this build candidate.',
        ['package.json'],
      ),
    )
  }

  if (output.outputKind === 'static' && output.outputPath === null) {
    findings.push(
      finding(
        'blocker',
        'missing_static_output_path',
        'Static build candidates must include an output path.',
        evidenceRefs.slice(0, 5),
      ),
    )
  }

  if (output.outputKind === 'worker_module' && output.workerModulePath === null) {
    findings.push(
      finding(
        'blocker',
        'missing_worker_module_path',
        'Worker module build candidates must include an entrypoint path.',
        ['wrangler config'],
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

  if (output.outputKind === 'static') {
    findings.push(
      finding(
        'info',
        'static_manifest_ready',
        'Static output candidate has a manifest shape for review.',
        output.outputPath === null ? [] : [output.outputPath],
      ),
    )
  }

  if (output.outputKind === 'worker_module') {
    findings.push(
      finding(
        'info',
        'worker_manifest_ready',
        'Worker module candidate has an entrypoint for review.',
        output.workerModulePath === null ? [] : [output.workerModulePath],
      ),
    )
  }

  const blockers = findings.filter(item => item.severity === 'blocker')
  const warnings = findings.filter(item => item.severity === 'warning')
  const status: SiteBuildValidationStatus =
    blockers.length > 0
      ? input.compatibility?.status === 'blocked'
        ? 'blocked'
        : 'failed'
      : output.outputKind === 'unknown'
        ? 'unknown'
        : warnings.length > 0
          ? 'warning'
          : 'passed'
  const logs = boundLogs(input.buildLogText, status, buildCommand)

  return {
    blockers,
    boundedLogs: logs.lines,
    buildCommand,
    compatibilityCheckId: input.compatibility?.compatibilityCheckId ?? null,
    createdAt: '',
    customerSafeNextAction:
      status === 'passed'
        ? 'This Site build candidate is ready for version save/review.'
        : status === 'warning'
          ? 'Review build warnings before saving a version.'
          : 'Resolve build validation blockers before saving or deploying.',
    customerSafeStatus: status,
    evidenceRefs,
    findings,
    id: '',
    logLineCount: logs.lineCount,
    logTruncated: logs.truncated,
    manifest,
    outputKind: output.outputKind,
    outputPath: output.outputPath,
    packageManager,
    requestedBuildCommand: input.requestedBuildCommand ?? null,
    siteId: input.site.id,
    sourceCommitSha: input.sourceCommitSha ?? null,
    sourceHash: await sourceHash(input.files),
    sourceKind: input.sourceKind ?? 'github_import',
    sourceRepository: input.sourceRepository ?? input.site.sourceRepository,
    status,
    validatedByUserId: input.actorUserId ?? null,
    warnings,
    workerModulePath: output.workerModulePath,
  }
}

const jsonValue = (value: unknown): string => JSON.stringify(value)

const assertSafeInput = (
  input: ValidateSiteBuildInput,
): Effect.Effect<void, SiteBuildValidationUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(input.files)) ||
  containsProviderSecretMaterial(input.buildLogText ?? '')
    ? Effect.fail(
        new SiteBuildValidationUnsafePayload({
          reason: 'Site build validation input contains secret-shaped material.',
        }),
      )
    : Effect.void

const assertSafeReceipt = (
  receipt: SiteBuildValidationReceipt,
): Effect.Effect<void, SiteBuildValidationUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(receipt))
    ? Effect.fail(
        new SiteBuildValidationUnsafePayload({
          reason: 'Site build validation receipt contains secret-shaped material.',
        }),
      )
    : Effect.void

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

const manifestFromJson = (value: string): SiteBuildValidationManifest => {
  const parsed = parseJsonRecord(value)
  const bindings = isRecord(parsed?.bindings) ? parsed.bindings : undefined

  return {
    assets: stringArrayFromUnknown(parsed?.assets),
    bindings: {
      d1: stringArrayFromUnknown(bindings?.d1),
      r2: stringArrayFromUnknown(bindings?.r2),
    },
    entrypoints: stringArrayFromUnknown(parsed?.entrypoints),
    envKeys: stringArrayFromUnknown(parsed?.envKeys),
    migrations: stringArrayFromUnknown(parsed?.migrations),
  }
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

const receiptFromRow = (row: ReceiptRow): SiteBuildValidationReceipt => ({
  blockers: parseJsonRecordArray(row.blockers_json) as Array<SiteCompatibilityFinding>,
  boundedLogs: stringArrayFromUnknown(parseJsonValue(row.bounded_logs_json)),
  buildCommand: row.build_command,
  compatibilityCheckId: row.compatibility_check_id,
  createdAt: row.created_at,
  customerSafeNextAction: row.customer_safe_next_action,
  customerSafeStatus: row.customer_safe_status,
  evidenceRefs: stringArrayFromUnknown(parseJsonValue(row.evidence_refs_json)),
  findings: parseJsonRecordArray(row.findings_json) as Array<SiteCompatibilityFinding>,
  id: row.id,
  logLineCount: Number(row.log_line_count),
  logTruncated: row.log_truncated === 1,
  manifest: manifestFromJson(row.manifest_json),
  outputKind: row.output_kind,
  outputPath: row.output_path,
  packageManager: row.package_manager,
  requestedBuildCommand: row.requested_build_command,
  siteId: row.site_id,
  sourceCommitSha: row.source_commit_sha,
  sourceHash: row.source_hash,
  sourceKind: row.source_kind,
  sourceRepository: sourceRepositoryFromJson(row.source_repository_json),
  status: row.status,
  validatedByUserId: row.validated_by_user_id,
  warnings: parseJsonRecordArray(row.warnings_json) as Array<SiteCompatibilityFinding>,
  workerModulePath: row.worker_module_path,
})

const insertReceipt = (
  db: D1Database,
  runtime: SiteBuildValidationRuntime,
  receipt: SiteBuildValidationReceipt,
): Effect.Effect<SiteBuildValidationReceipt, SiteBuildValidationError> =>
  Effect.gen(function* () {
    const id = runtime.makeValidationId()
    const now = runtime.nowIso()
    const saved = { ...receipt, createdAt: now, id }

    yield* assertSafeReceipt(saved)

    yield* d1Effect('siteBuildValidations.insert', () =>
      db
        .prepare(
          `INSERT INTO site_build_validations
             (id,
              site_id,
              compatibility_check_id,
              source_kind,
              source_repository_json,
              source_commit_sha,
              source_hash,
              status,
              package_manager,
              requested_build_command,
              build_command,
              output_kind,
              output_path,
              worker_module_path,
              manifest_json,
              bounded_logs_json,
              log_line_count,
              log_truncated,
              findings_json,
              blockers_json,
              warnings_json,
              evidence_refs_json,
              customer_safe_status,
              customer_safe_next_action,
              validated_by_user_id,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          saved.id,
          saved.siteId,
          saved.compatibilityCheckId,
          saved.sourceKind,
          saved.sourceRepository === null
            ? null
            : JSON.stringify(saved.sourceRepository),
          saved.sourceCommitSha,
          saved.sourceHash,
          saved.status,
          saved.packageManager,
          saved.requestedBuildCommand,
          saved.buildCommand,
          saved.outputKind,
          saved.outputPath,
          saved.workerModulePath,
          jsonValue(saved.manifest),
          jsonValue(saved.boundedLogs),
          saved.logLineCount,
          saved.logTruncated ? 1 : 0,
          jsonValue(saved.findings),
          jsonValue(saved.blockers),
          jsonValue(saved.warnings),
          jsonValue(saved.evidenceRefs),
          saved.customerSafeStatus,
          saved.customerSafeNextAction,
          saved.validatedByUserId,
          saved.createdAt,
        )
        .run(),
    )

    yield* d1Effect('siteBuildValidations.event.insert', () =>
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
           VALUES (?, ?, NULL, NULL, 'site_build_validation.checked', ?, ?, NULL, ?, ?)`,
        )
        .bind(
          runtime.makeEventId(),
          saved.siteId,
          `Validated Site build candidate: ${saved.status}.`,
          saved.validatedByUserId,
          JSON.stringify({
            blockers: saved.blockers.map(item => item.code),
            sourceHash: saved.sourceHash,
            status: saved.status,
            validationId: saved.id,
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
): Effect.Effect<SiteBuildValidationReceipt | null, SiteBuildValidationError> =>
  d1Effect('siteBuildValidations.latest.read', () =>
    db
      .prepare(
        `SELECT id,
                site_id,
                compatibility_check_id,
                source_kind,
                source_repository_json,
                source_commit_sha,
                source_hash,
                status,
                package_manager,
                requested_build_command,
                build_command,
                output_kind,
                output_path,
                worker_module_path,
                manifest_json,
                bounded_logs_json,
                log_line_count,
                log_truncated,
                findings_json,
                blockers_json,
                warnings_json,
                evidence_refs_json,
                customer_safe_status,
                customer_safe_next_action,
                validated_by_user_id,
                created_at
           FROM site_build_validations
          WHERE site_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(siteId)
      .first<ReceiptRow>(),
  ).pipe(Effect.map(row => (row === null ? null : receiptFromRow(row))))

const validateBuild = (
  db: D1Database,
  runtime: SiteBuildValidationRuntime,
  input: ValidateSiteBuildInput,
): Effect.Effect<SiteBuildValidationReceipt, SiteBuildValidationError> =>
  Effect.gen(function* () {
    yield* assertSafeInput(input)
    const receipt = yield* Effect.tryPromise({
      catch: error =>
        new SiteBuildValidationValidationError({
          reason:
            error instanceof Error
              ? error.message
              : 'failed to inspect build candidate',
        }),
      try: () => inspectBuildValidation(input),
    })

    return yield* insertReceipt(db, runtime, receipt)
  })

export const makeSiteBuildValidationService = (
  db: D1Database,
  runtime: SiteBuildValidationRuntime = systemSiteBuildValidationRuntime,
) => ({
  latestReceipt: Effect.fn('SiteBuildValidationService.latest')((siteId: string) =>
    latestReceipt(db, siteId),
  ),
  validateBuild: Effect.fn('SiteBuildValidationService.validate')(
    (input: ValidateSiteBuildInput) => validateBuild(db, runtime, input),
  ),
})
