#!/usr/bin/env node

/**
 * VP-1 static retirement guard (#8795).
 *
 * This is intentionally narrower than a keyword sweep. It protects the
 * executable deployment and discovery seams that could restore the retired
 * Money/Sites graph while allowing immutable migrations, historical evidence,
 * the typed 410 compatibility contract, and explicit no-spend policy fields.
 */

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { extname, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const RETIRED_SERVICE_TREE =
  /^apps\/openagents\.com\/services\/(?:mdk-(?:sidecar|treasury|tips-buffer)|sites?(?:-|\/|$)|payments?(?:-|\/|$)|checkout(?:-|\/|$)|billing(?:-|\/|$)|stripe(?:-|\/|$)|treasury(?:-|\/|$)|wallet(?:-|\/|$))/i

const RETIRED_CLOUD_RUN_SERVICE = /\boa-mdk-(?:sidecar|treasury|tips-buffer)(?:-staging)?\b/i
const RETIRED_CONTAINER_BINDING = /\bMDK_(?:SIDECAR|TREASURY|TIPS_BUFFER)\b/
const RETIRED_RUNTIME_AUTHORITY =
  /\b(?:Mdk(?:Sidecar|Treasury|TipsBuffer)Container|MDK_(?:SIDECAR|TREASURY|TIPS_BUFFER)_(?:ACCESS_TOKEN|MNEMONIC|SERVICE_TOKEN)|SPARK_TREASURY_MNEMONIC)\b/
const RETIRED_SECRET_MOUNT =
  /\b(?:MDK_(?:SIDECAR|TREASURY|TIPS_BUFFER)_(?:ACCESS_TOKEN|MNEMONIC|SERVICE_TOKEN)|SPARK_TREASURY_MNEMONIC|STRIPE_(?:API_KEY|WEBHOOK_SECRET)|REVENUECAT_WEBHOOK_SECRET|KHALA_SYNC_(?:BILLING|TREASURY)_(?:READS|WRITES))\b|\b(?:mdk-(?:sidecar|treasury|tips-buffer)-(?:access-token|mnemonic|service-token)|openagents-stripe-api-key-(?:live|test))\b/i

const RETIRED_CLIENT_MONEY_REQUEST =
  /["'`](?:\/api\/(?:admin\/credits(?:\/|["'`])|mobile\/credits(?:\/|["'`])|treasury(?:\/|["'`])|sites(?:\/|["'`])|forum\/tip(?:[^"'`]*))|\/treasury(?:[/?"'`]|$))|\/tips\/ladder["'`]/i

const OPENAPI_PATH = 'apps/openagents.com/workers/api/src/openagents-openapi.ts'
const CAPABILITY_PATH =
  'apps/openagents.com/workers/api/src/openagents-capability-manifest.ts'
const WORKER_INDEX_PATH = 'apps/openagents.com/workers/api/src/index.ts'

const SELF_PATHS = new Set([
  'scripts/vp1-retired-money-surface-guard.mjs',
  'scripts/vp1-retired-money-surface-guard.test.mjs',
])

const TEXT_EXTENSIONS = new Set([
  '',
  '.cjs',
  '.cts',
  '.js',
  '.json',
  '.jsonc',
  '.jsx',
  '.mjs',
  '.mts',
  '.sh',
  '.tf',
  '.tfvars',
  '.toml',
  '.ts',
  '.tsx',
  '.yaml',
  '.yml',
])

const normalizePath = value => value.replaceAll('\\', '/').replace(/^\.\//, '')

export const isVp1RetirementException = path => {
  const normalized = normalizePath(path)
  return (
    SELF_PATHS.has(normalized) ||
    /(?:^|\/)migrations\/[^/]+\.sql$/i.test(normalized) ||
    /(?:^|\/)docs\/(?:archive|archived|historical|receipts?)(?:\/|$)/i.test(
      normalized,
    ) ||
    /^docs\/sol\/(?:receipts\/)?/i.test(normalized) ||
    /^docs\/ops\/2026-07-14-vp1-treasury-wallet-recovery-runbook\.md$/i.test(
      normalized,
    ) ||
    /^apps\/openagents\.com\/workers\/api\/src\/money-surface-retirement(?:\.test)?\.ts$/i.test(
      normalized,
    )
  )
}

const isIgnoredPath = path =>
  /(?:^|\/)(?:\.git|\.wrangler|build|coverage|dist|node_modules)(?:\/|$)/.test(
    path,
  )

const isTestOrFixture = path =>
  /\.(?:test|spec)\.[cm]?[jt]sx?$/i.test(path) ||
  /(?:^|\/)(?:__tests__|fixtures)(?:\/|$)/.test(path)

const isActiveProductionFile = path => {
  if (isVp1RetirementException(path) || isIgnoredPath(path) || isTestOrFixture(path)) {
    return false
  }
  return /^(?:apps|clients|infra|packages|scripts)\//.test(path)
}

const isClientRuntimeFile = path =>
  /^(?:apps\/(?:aiur|openagents\.com\/apps\/(?:start|web))|clients\/khala-mobile)\/src\//.test(
    path,
  ) &&
  !/(?:^|\/)(?:contracts|demo|qa)(?:\/|$)/.test(path)

const isDeploymentOrConfigFile = path =>
  /(?:^|\/)(?:Dockerfile|package\.json|wrangler(?:\.[^/]+)?\.jsonc?|[^/]+\.(?:jsonc?|ya?ml|toml|sh|tf|tfvars))$/i.test(
    path,
  ) || /(?:^|\/)(?:deploy|release|bootstrap)[^/]*$/i.test(path)

const uncomment = text => {
  let inBlock = false
  return text
    .split(/\r?\n/)
    .map(line => {
      let value = line
      if (inBlock) {
        const end = value.indexOf('*/')
        if (end < 0) return ''
        value = value.slice(end + 2)
        inBlock = false
      }
      for (;;) {
        const start = value.indexOf('/*')
        if (start < 0) break
        const end = value.indexOf('*/', start + 2)
        if (end < 0) {
          value = value.slice(0, start)
          inBlock = true
          break
        }
        value = `${value.slice(0, start)} ${value.slice(end + 2)}`
      }
      const lineComment = value.indexOf('//')
      return lineComment < 0 ? value : value.slice(0, lineComment)
    })
    .join('\n')
}

const lineForOffset = (text, offset) => text.slice(0, offset).split('\n').length

const patternFinding = (path, text, category, pattern) => {
  const match = pattern.exec(text)
  pattern.lastIndex = 0
  return match === null
    ? []
    : [{ category, line: lineForOffset(text, match.index), path }]
}

const discoveryFindings = (path, text) => {
  if (path === OPENAPI_PATH) {
    const requirements = [
      ['openapi-retirement-filter-missing', /const\s+retiredDiscoveryPathPattern\s*=/],
      ['openapi-retirement-filter-missing', /Object\.entries\(paths\(\)\)\.filter/],
      ['openapi-retirement-filter-missing', /retiredDiscoveryPathPattern\.test\(path\)/],
      ['openapi-retirement-filter-missing', /paths:\s*activeOpenApiPaths\(\)/],
    ]
    return requirements.flatMap(([category, pattern]) =>
      pattern.test(text) ? [] : [{ category, line: 1, path }],
    )
  }

  if (path === CAPABILITY_PATH) {
    const requirements = [
      ['capability-retirement-filter-missing', /const\s+retiredCapabilityEntryPattern\s*=/],
      ['capability-retirement-filter-missing', /actions:\s*manifest\.actions\.filter/],
      ['capability-retirement-filter-missing', /resources:\s*manifest\.resources\.filter/],
      ['capability-retirement-filter-missing', /!advertisesRetiredCapability\(entry\)/],
      ['capability-retirement-filter-missing', /recovery:\s*manifest\.rateLimits\.public\.recovery\.filter/],
      ['capability-retirement-filter-missing', /recovery:\s*manifest\.rateLimits\.authenticated\.recovery\.filter/],
    ]
    return requirements.flatMap(([category, pattern]) =>
      pattern.test(text) ? [] : [{ category, line: 1, path }],
    )
  }

  if (path === WORKER_INDEX_PATH) {
    const requirements = [
      ['worker-retirement-gate-missing', /isRetiredMoneySurfaceRequest\(request\.method, url\.pathname\)/],
      ['paid-capacity-retirement-missing', /continuation\.skipped\.paid_capacity_retired/],
      ['run-billing-retirement-missing', /makeBillingAwareOmniRunStore[\s\S]*?makeOmniRunStoreForEnv\(env\)/],
    ]
    const forbidden = [
      ['retired-money-scheduler-restored', /TipsBuffer\.reconcileForwarding/],
      ['retired-money-scheduler-restored', /TreasuryTransactions\.reconcilePending/],
      ['retired-paid-dispatch-restored', /AutopilotScheduledLaunches\.dispatchDue/],
    ]
    return [
      ...requirements.flatMap(([category, pattern]) =>
        pattern.test(text) ? [] : [{ category, line: 1, path }],
      ),
      ...forbidden.flatMap(([category, pattern]) =>
        patternFinding(path, uncomment(text), category, pattern),
      ),
    ]
  }

  return []
}

export const scanVp1RetiredMoneySurfaces = ({ files, readText }) => {
  const findings = []

  for (const rawPath of [...files].map(normalizePath).sort()) {
    if (isIgnoredPath(rawPath) || isTestOrFixture(rawPath)) continue

    if (RETIRED_SERVICE_TREE.test(rawPath) && !isVp1RetirementException(rawPath)) {
      findings.push({ category: 'retired-service-tree', line: 1, path: rawPath })
      continue
    }

    if (!isActiveProductionFile(rawPath) || !TEXT_EXTENSIONS.has(extname(rawPath))) {
      continue
    }

    const source = readText(rawPath)
    if (source === undefined) continue
    const activeText = uncomment(source)

    if (isClientRuntimeFile(rawPath)) {
      findings.push(
        ...patternFinding(
          rawPath,
          activeText,
          'retired-client-money-request',
          RETIRED_CLIENT_MONEY_REQUEST,
        ),
      )
    }

    // Discovery checks look for required executable structure rather than
    // forbidden tokens, so inspect the source verbatim. Large schema files can
    // legitimately contain comment-shaped text inside strings or regexes.
    findings.push(...discoveryFindings(rawPath, source))
    findings.push(
      ...patternFinding(
        rawPath,
        activeText,
        'retired-cloud-run-service',
        RETIRED_CLOUD_RUN_SERVICE,
      ),
    )
    findings.push(
      ...patternFinding(
        rawPath,
        activeText,
        'retired-money-container-binding',
        RETIRED_CONTAINER_BINDING,
      ),
    )
    // Wrangler must retain the applied Durable Object creation/deletion class
    // names as immutable migration history. Active bindings use the MDK_* form
    // above and remain forbidden.
    if (!/\/wrangler(?:\.[^/]+)?\.jsonc?$/i.test(rawPath)) {
      findings.push(
        ...patternFinding(
          rawPath,
          activeText,
          'retired-money-runtime-authority',
          RETIRED_RUNTIME_AUTHORITY,
        ),
      )
    }
    if (isDeploymentOrConfigFile(rawPath)) {
      findings.push(
        ...patternFinding(
          rawPath,
          activeText,
          'retired-money-secret-mount',
          RETIRED_SECRET_MOUNT,
        ),
      )
    }
  }

  return findings
}

const walk = (root, directory = root) => {
  const result = []
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const absolute = resolve(directory, entry.name)
    const path = normalizePath(relative(root, absolute))
    if (isIgnoredPath(path)) continue
    if (entry.isDirectory()) result.push(...walk(root, absolute))
    else if (entry.isFile()) result.push(path)
  }
  return result
}

const repoFiles = root => {
  try {
    return execFileSync(
      'git',
      ['ls-files', '--cached', '--others', '--exclude-standard', '-z'],
      { cwd: root, encoding: 'utf8' },
    )
      .split('\0')
      .filter(Boolean)
      .filter(path => existsSync(resolve(root, path)))
  } catch {
    return walk(root)
  }
}

export const scanVp1RetiredMoneyRepo = root => {
  const absoluteRoot = resolve(root)
  return scanVp1RetiredMoneySurfaces({
    files: repoFiles(absoluteRoot),
    readText: path => {
      const absolute = resolve(absoluteRoot, path)
      if (!existsSync(absolute) || !statSync(absolute).isFile()) return undefined
      try {
        return readFileSync(absolute, 'utf8')
      } catch {
        return undefined
      }
    },
  })
}

const isMain = process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)

if (isMain) {
  const root = resolve(process.argv[2] ?? process.cwd())
  const findings = scanVp1RetiredMoneyRepo(root)
  console.log(`VP-1 retired-money static guard: ${findings.length} violation(s)`)
  for (const finding of findings) {
    console.log(`${finding.category} ${finding.path}:${finding.line}`)
  }
  process.exitCode = findings.length === 0 ? 0 : 1
}
