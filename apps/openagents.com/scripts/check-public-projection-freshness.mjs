#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join, relative } from 'node:path'

export const DEFAULT_ROOT = new URL('..', import.meta.url).pathname
export const DEFAULT_ALLOWLIST_PATH = 'scripts/public-projection-freshness-allowlist.json'

const productionTs = path =>
  path.endsWith('.ts') &&
  !path.endsWith('.test.ts') &&
  !path.endsWith('.test-support.ts') &&
  !path.includes('/test/')

const listFiles = dir =>
  readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name)

    return entry.isDirectory() ? listFiles(path) : [path]
  })

const lineForOffset = (text, offset) => text.slice(0, offset).split('\n').length

const unique = values => Array.from(new Set(values))

const routePattern = /['"`]\/(api\/(?:public|forum)[^'"`?#)]*)/g
const publicProjectionPattern = /\bpublicProjection\s*:\s*\{/g
const freshnessPattern = /\b(?:generatedAt|lastRebuiltAt)\b/
const stalenessPattern = /\b(?:maxStalenessSeconds|staleness|stalenessContract|stalenessPolicy|maxStaleness)\b/

export const inventorySource = (path, text) => {
  const routes = unique(
    Array.from(text.matchAll(routePattern), match => `/${match[1]}`),
  )
  const hasPublicRoute = routes.length > 0
  const hasForumProjection = text.includes('publicProjection')
  const surfaces = []

  if (hasPublicRoute) {
    routes.forEach(route =>
      surfaces.push({
        id: `${path}::route::${route}`,
        kind: route.startsWith('/api/forum') ? 'forum_route' : 'public_route',
        line: lineForOffset(text, text.indexOf(route)),
        path,
        route,
        text,
      }),
    )
  }

  Array.from(text.matchAll(publicProjectionPattern)).forEach(match =>
    surfaces.push({
      id: `${path}::publicProjection::${lineForOffset(text, match.index ?? 0)}`,
      kind: 'forum_public_projection_shape',
      line: lineForOffset(text, match.index ?? 0),
      path,
      route: routes[0] ?? null,
      text,
    }),
  )

  return surfaces
}

export const inventoryProjectionSurfaces = root => {
  const sourceRoot = join(root, 'workers/api/src')

  return listFiles(sourceRoot)
    .map(path => relative(root, path))
    .filter(productionTs)
    .flatMap(path => inventorySource(path, readFileSync(join(root, path), 'utf8')))
}

export const readAllowlist = (root, allowlistPath = DEFAULT_ALLOWLIST_PATH) => {
  const absolutePath = join(root, allowlistPath)

  if (!existsSync(absolutePath)) {
    return new Map()
  }

  const parsed = JSON.parse(readFileSync(absolutePath, 'utf8'))
  const entries = Array.isArray(parsed.entries) ? parsed.entries : []

  return new Map(entries.map(entry => [entry.id, entry]))
}

export const evaluateProjectionFreshness = (surfaces, allowlist) => {
  const findings = surfaces.map(surface => {
    const hasFreshnessTimestamp = freshnessPattern.test(surface.text)
    const hasStalenessDeclaration = stalenessPattern.test(surface.text)
    const allowlistEntry = allowlist.get(surface.id)
    const passed = hasFreshnessTimestamp && hasStalenessDeclaration
    const allowlistValid =
      allowlistEntry !== undefined && /^#\d+$/.test(allowlistEntry.issueRef ?? '')

    return {
      ...surface,
      allowlistEntry,
      hasFreshnessTimestamp,
      hasStalenessDeclaration,
      passed: passed || allowlistValid,
      reason:
        passed || allowlistValid
          ? null
          : [
              hasFreshnessTimestamp ? null : 'missing generatedAt/lastRebuiltAt',
              hasStalenessDeclaration
                ? null
                : 'missing maxStalenessSeconds/staleness declaration',
            ]
              .filter(Boolean)
              .join('; '),
    }
  })

  return {
    findings,
    failures: findings.filter(finding => !finding.passed),
    grandfathered: findings.filter(
      finding => finding.allowlistEntry !== undefined && !(
        finding.hasFreshnessTimestamp && finding.hasStalenessDeclaration
      ),
    ),
    inventoryCount: findings.length,
  }
}

export const formatReport = result => {
  const lines = [
    'Public projection freshness check',
    `inventory: ${result.inventoryCount}`,
    `grandfathered: ${result.grandfathered.length}`,
    `failures: ${result.failures.length}`,
    'policy: public projection surfaces must declare generatedAt/lastRebuiltAt plus maxStalenessSeconds/staleness, or carry a temporary issue-ref allowlist entry.',
  ]

  if (result.failures.length > 0) {
    lines.push('', 'Failures:')
    result.failures.forEach(failure => {
      lines.push(
        `- ${failure.id} (${failure.kind}, line ${failure.line}): ${failure.reason}`,
      )
    })
  }

  return lines.join('\n')
}

export const runProjectionFreshnessCheck = (root = DEFAULT_ROOT) => {
  const surfaces = inventoryProjectionSurfaces(root)
  const allowlist = readAllowlist(root)

  return evaluateProjectionFreshness(surfaces, allowlist)
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const result = runProjectionFreshnessCheck(process.cwd())
  console.log(formatReport(result))

  if (result.failures.length > 0) {
    process.exit(1)
  }
}
