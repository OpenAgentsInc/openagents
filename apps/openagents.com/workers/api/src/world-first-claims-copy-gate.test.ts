import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'

import { describe, expect, test } from 'vitest'

import { publicProductPromisesDocument } from './product-promises'

const AUDIT_DOC =
  'docs/promises/2026-06-29-world-first-claims-blocker-audit.md'

const WORLD_FIRST_PROMISE_IDS = [
  'claims.world_first_ai_training_paid_bitcoin.v1',
  'claims.world_first_public_llm_computer_training_run.v1',
  'claims.pursued_world_first_largest_agentic_sales_force.v1',
  'claims.pursued_world_first_largest_sales_force.v1',
] as const

const repoFile = (relPath: string): URL =>
  new URL(`../../../../../${relPath}`, import.meta.url)

const readRepoFile = (relPath: string): string =>
  readFileSync(repoFile(relPath), 'utf8')

const productCopyRoots = [
  'apps/openagents.com/apps/web/src',
  'apps/openagents.com/workers/api/src',
] as const

const skippedRuntimeCopyFiles = new Set([
  'apps/openagents.com/workers/api/src/product-promises.ts',
  'apps/openagents.com/workers/api/src/world-first-claims-copy-gate.test.ts',
  'apps/openagents.com/workers/api/src/world-first-llm-computer-evidence-pack.test.ts',
])

const runtimeSourceFiles = (relDir: string): ReadonlyArray<string> => {
  const entries = readdirSync(repoFile(relDir), { withFileTypes: true })
  return entries.flatMap(entry => {
    const relPath = `${relDir}/${entry.name}`
    if (entry.name === 'node_modules' || entry.name === 'dist') {
      return []
    }
    if (entry.isDirectory()) {
      return runtimeSourceFiles(relPath)
    }
    if (!entry.isFile() || !/\.(ts|tsx|js|jsx|html)$/.test(entry.name)) {
      return []
    }
    if (entry.name.endsWith('.test.ts') || skippedRuntimeCopyFiles.has(relPath)) {
      return []
    }
    return [relPath]
  })
}

describe('world-first public-claim copy gate', () => {
  test('the dated #7027 audit exists and carries the refuse list', () => {
    expect(existsSync(repoFile(AUDIT_DOC))).toBe(true)

    const audit = readRepoFile(AUDIT_DOC)
    expect(audit).toContain('Date: 2026-06-29')
    expect(audit).toContain('## Refuse List')
    expect(audit).toContain('Bare "world first" without the full qualifiers')
    expect(audit).toContain('OpenAgents has the largest agentic sales force')
    expect(audit).toContain('OpenAgents has the largest sales force')
  })

  test('each world-first promise cites the audit and stays blocked', () => {
    const document = publicProductPromisesDocument()

    for (const promiseId of WORLD_FIRST_PROMISE_IDS) {
      const promise = document.promises.find(
        candidate => candidate.promiseId === promiseId,
      )
      expect(promise, promiseId).toBeDefined()
      expect(promise?.evidenceRefs).toContain(AUDIT_DOC)
      expect(promise?.blockerRefs).toContain(
        'blocker.product_promises.world_first_owner_signed_upgrade_missing',
      )
      expect(promise?.verification).toContain('2026-06-29')
      expect(promise?.verification).toContain('#7027')
    }

    expect(
      document.promises.find(
        promise =>
          promise.promiseId ===
          'claims.world_first_ai_training_paid_bitcoin.v1',
      )?.state,
    ).toBe('red')
    expect(
      document.promises.find(
        promise =>
          promise.promiseId ===
          'claims.world_first_public_llm_computer_training_run.v1',
      )?.state,
    ).toBe('red')
    expect(
      document.promises.find(
        promise =>
          promise.promiseId ===
          'claims.pursued_world_first_largest_agentic_sales_force.v1',
      )?.state,
    ).toBe('planned')
    expect(
      document.promises.find(
        promise =>
          promise.promiseId ===
          'claims.pursued_world_first_largest_sales_force.v1',
      )?.state,
    ).toBe('planned')
  })

  test('runtime product copy does not carry unqualified world-first claims', () => {
    const forbiddenPatterns = [
      /\bworld first\b/i,
      /\blargest agentic sales force\b/i,
      /\blargest sales force\b/i,
      /\bseven million agents\b/i,
      /\bworld record\b/i,
    ]

    const files = productCopyRoots.flatMap(runtimeSourceFiles)
    expect(files.length).toBeGreaterThan(0)

    for (const file of files) {
      const stat = statSync(repoFile(file))
      expect(stat.isFile()).toBe(true)

      const text = readRepoFile(file)
      for (const pattern of forbiddenPatterns) {
        expect({
          file,
          pattern: String(pattern),
          matched: pattern.test(text),
        }).toEqual({
          file,
          pattern: String(pattern),
          matched: false,
        })
      }
    }
  })
})
