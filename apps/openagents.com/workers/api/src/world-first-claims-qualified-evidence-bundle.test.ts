import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, test } from 'vitest'

import { publicProductPromisesDocument } from './product-promises'

const repoFile = (relPath: string): URL =>
  new URL(`../../../../../${relPath}`, import.meta.url)

const repoPath = (relPath: string): string => repoFile(relPath).pathname

const repoFileExists = (relPath: string): boolean =>
  existsSync(repoFile(relPath))

const readRepoFile = (relPath: string): string =>
  readFileSync(repoFile(relPath), 'utf8')

const BUNDLE_DOC =
  'docs/launch/2026-06-29-world-first-claims-qualified-evidence-bundle.md'

const PROMISE_IDS = [
  'claims.world_first_ai_training_paid_bitcoin.v1',
  'claims.world_first_public_llm_computer_training_run.v1',
  'claims.pursued_world_first_largest_agentic_sales_force.v1',
  'claims.pursued_world_first_largest_sales_force.v1',
] as const

const docRefs = (markdown: string): ReadonlyArray<string> => [
  ...new Set(markdown.match(/docs\/[A-Za-z0-9._/-]+\.md/g) ?? []),
]

const productSourceFiles = (dir: string): ReadonlyArray<string> =>
  readdirSync(dir).flatMap(entry => {
    const path = join(dir, entry)
    const stat = statSync(path)

    if (stat.isDirectory()) {
      return productSourceFiles(path)
    }

    if (
      !path.endsWith('.ts') &&
      !path.endsWith('.tsx') &&
      !path.endsWith('.html') &&
      !path.endsWith('.md')
    ) {
      return []
    }

    if (path.endsWith('.test.ts') || path.endsWith('.story.test.ts')) {
      return []
    }

    return [path]
  })

const promiseById = (promiseId: string) => {
  const document = publicProductPromisesDocument()
  const promise = document.promises.find(
    candidate => candidate.promiseId === promiseId,
  )
  if (!promise) {
    throw new Error(`promise ${promiseId} not found in registry`)
  }
  return { document, promise }
}

describe('world-first claims qualified evidence bundle', () => {
  test('the unified bundle is registry evidence for all #7027 promises', () => {
    for (const promiseId of PROMISE_IDS) {
      const { promise } = promiseById(promiseId)

      expect(promise.evidenceRefs).toContain(BUNDLE_DOC)
      expect(promise.evidenceRefs).toContain(
        'apps/openagents.com/workers/api/src/world-first-claims-qualified-evidence-bundle.test.ts',
      )
      expect(repoFileExists(BUNDLE_DOC)).toBe(true)
    }
  })

  test('bundle doc refs and promise refs are dereferenceable', () => {
    const { document } = promiseById(PROMISE_IDS[0])
    const knownPromiseIds = new Set(
      document.promises.map(candidate => candidate.promiseId),
    )
    const bundle = readRepoFile(BUNDLE_DOC)

    for (const ref of docRefs(bundle)) {
      expect({ ref, exists: repoFileExists(ref) }).toEqual({
        ref,
        exists: true,
      })
    }

    for (const promiseId of PROMISE_IDS) {
      const { promise } = promiseById(promiseId)
      const promiseRefs = promise.evidenceRefs
        .filter(ref => ref.startsWith('promise:'))
        .map(ref => ref.slice('promise:'.length))

      expect(promiseRefs.length).toBeGreaterThan(0)
      for (const ref of promiseRefs) {
        expect({ ref, known: knownPromiseIds.has(ref) }).toEqual({
          ref,
          known: true,
        })
      }
    }
  })

  test('states remain red or planned and owner-signed gates remain explicit', () => {
    const claim1 = promiseById(
      'claims.world_first_ai_training_paid_bitcoin.v1',
    ).promise
    const claim2 = promiseById(
      'claims.world_first_public_llm_computer_training_run.v1',
    ).promise
    const agenticForce = promiseById(
      'claims.pursued_world_first_largest_agentic_sales_force.v1',
    ).promise
    const largestForce = promiseById(
      'claims.pursued_world_first_largest_sales_force.v1',
    ).promise

    expect(claim1.state).toBe('red')
    expect(claim1.blockerRefs).not.toContain(
      'blocker.product_promises.world_first_evidence_pack_missing',
    )
    expect(claim1.blockerRefs).toContain(
      'blocker.product_promises.world_first_owner_signed_upgrade_missing',
    )

    expect(claim2.state).toBe('red')
    expect(claim2.blockerRefs).toContain(
      'blocker.product_promises.world_first_owner_signed_upgrade_missing',
    )

    expect(agenticForce.state).toBe('planned')
    expect(agenticForce.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.product_promises.world_first_agentic_sales_force_not_achieved',
        'blocker.product_promises.world_first_agentic_sales_force_no_sized_verifiable_force',
        'blocker.product_promises.world_first_owner_signed_upgrade_missing',
      ]),
    )

    expect(largestForce.state).toBe('planned')
    expect(largestForce.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.product_promises.world_first_largest_sales_force_not_achieved',
        'blocker.product_promises.world_first_largest_sales_force_seven_million_bar_unmet',
        'blocker.product_promises.world_first_owner_signed_upgrade_missing',
      ]),
    )
  })

  test('bundle keeps exact qualifiers, dated blockers, and refuse-list wording', () => {
    const bundle = readRepoFile(BUNDLE_DOC)

    expect(bundle).toContain('Bitcoin')
    expect(bundle).toContain('replay-verified training compute')
    expect(bundle).toContain('own consumer devices')
    expect(bundle).toContain('Percepta')
    expect(bundle).toContain('Dated blocker note')
    expect(bundle).toContain('refuse-list')
    expect(bundle).toContain('OpenAgents has the largest sales force')
    expect(bundle).toContain('First to pay Bitcoin for AI')
  })

  test('current live product copy does not carry unqualified world-first or largest-force claims', () => {
    const sourceRoots = [
      repoPath('apps/openagents.com/apps/web/src'),
      repoPath('apps/openagents.com/workers/api/src'),
    ]
    const allowlistedFiles = new Set([
      repoPath('apps/openagents.com/workers/api/src/product-promises.ts'),
    ])
    const forbiddenPatterns = [
      /\b(?:we|openagents)\s+(?:have|has|hold|holds|achieved|verified)\s+(?:the\s+)?largest\s+(?:agentic\s+)?sales\s+force\b/i,
      /\bworld\s*first\b(?![^"'`\n]{0,180}\b(?:qualified|qualifier|pursu|aspiration|aspirational|not claiming|do not|owner-signed|red|planned|refuse-list|gated|pending|Percepta|Bitcoin|replay-verified|own consumer devices|open-contributor)\b)/i,
      /\bfirst\s+to\s+pay\s+bitcoin\s+for\s+ai\b/i,
      /\bwe\s+invented\s+the\s+llm-computer\b/i,
      /\bseven[- ]million[- ]agent\s+bar\s+is\s+met\b/i,
    ]

    const violations = sourceRoots.flatMap(root =>
      productSourceFiles(root).flatMap(path => {
        if (allowlistedFiles.has(path)) {
          return []
        }

        const text = readFileSync(path, 'utf8')
        return forbiddenPatterns.flatMap(pattern =>
          pattern.test(text) ? [`${path} matches ${pattern.source}`] : [],
        )
      }),
    )

    expect(violations).toEqual([])
  })
})
