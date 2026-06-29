import { existsSync, readFileSync } from 'node:fs'

import { describe, expect, test } from 'vitest'

import { publicProductPromisesDocument } from './product-promises'

// Repo root relative to this file: src -> api -> workers -> openagents.com -> apps -> root
const repoFile = (relPath: string): URL =>
  new URL(`../../../../../${relPath}`, import.meta.url)

const repoFileExists = (relPath: string): boolean =>
  existsSync(repoFile(relPath))

const readRepoFile = (relPath: string): string =>
  readFileSync(repoFile(relPath), 'utf8')

const PROMISE_ID = 'claims.world_first_public_llm_computer_training_run.v1'
const PAID_BITCOIN_PROMISE_ID =
  'claims.world_first_ai_training_paid_bitcoin.v1'
const AGENTIC_SALES_FORCE_PROMISE_ID =
  'claims.pursued_world_first_largest_agentic_sales_force.v1'
const SALES_FORCE_PROMISE_ID =
  'claims.pursued_world_first_largest_sales_force.v1'
const DEFINITION_DOC =
  'docs/launch/2026-06-20-llm-computer-training-run-definition.md'
const EVIDENCE_PACK_DOC =
  'docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md'
const WORLD_FIRST_AUDIT_DOC =
  'docs/promises/2026-06-29-world-first-claims-qualified-evidence-audit.md'

const docRefs = (markdown: string): ReadonlyArray<string> => [
  ...new Set(markdown.match(/docs\/[A-Za-z0-9._/-]+\.md/g) ?? []),
]

const claim2Promise = () => {
  const document = publicProductPromisesDocument()
  const promise = document.promises.find(
    candidate => candidate.promiseId === PROMISE_ID,
  )
  if (!promise) {
    throw new Error(`promise ${PROMISE_ID} not found in registry`)
  }
  return { document, promise }
}

const getPromise = (promiseId: string) => {
  const document = publicProductPromisesDocument()
  const promise = document.promises.find(
    candidate => candidate.promiseId === promiseId,
  )
  if (!promise) {
    throw new Error(`promise ${promiseId} not found in registry`)
  }
  return promise
}

describe('world-first LLM-computer evidence pack dereferenceability', () => {
  test('the definition + evidence-pack docs are registry evidence and exist on disk', () => {
    const { promise } = claim2Promise()

    for (const doc of [DEFINITION_DOC, EVIDENCE_PACK_DOC]) {
      // Cited by the registry record as load-bearing evidence...
      expect(promise.evidenceRefs).toContain(doc)
      // ...and actually dereferenceable (the whole point of an "evidence pack").
      expect(repoFileExists(doc)).toBe(true)
    }
  })

  test('every repo-relative doc the pack/definition cite actually resolves', () => {
    // An evidence pack that links to missing files is not dereferenceable.
    // Guard the pack (and its companion definition) against link bit-rot.
    for (const doc of [DEFINITION_DOC, EVIDENCE_PACK_DOC]) {
      const refs = docRefs(readRepoFile(doc))
      // Each doc must cite at least its companion context, not be a dead end.
      expect(refs.length).toBeGreaterThan(0)
      for (const ref of refs) {
        expect({ doc, ref, exists: repoFileExists(ref) }).toEqual({
          doc,
          ref,
          exists: true,
        })
      }
    }
  })

  test('promise: evidence refs resolve to real registry promiseIds', () => {
    const { document, promise } = claim2Promise()
    const knownPromiseIds = new Set(
      document.promises.map(candidate => candidate.promiseId),
    )

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
  })

  test('clearing the evidence-pack blocker did not flip state or re-open siblings', () => {
    const { promise } = claim2Promise()

    // State is untouched by the evidence work.
    expect(promise.state).toBe('red')

    // The two buildable blockers are genuinely cleared (docs exist + wired in).
    expect(promise.blockerRefs).not.toContain(
      'blocker.product_promises.world_first_evidence_pack_missing',
    )
    expect(promise.blockerRefs).not.toContain(
      'blocker.product_promises.llm_computer_training_run_definition_missing',
    )

    // The honest remaining gate still stands.
    expect(promise.blockerRefs).toContain(
      'blocker.product_promises.world_first_owner_signed_upgrade_missing',
    )
  })

  test('the pack keeps its refuse-list and the registry copy stays qualified', () => {
    const { promise } = claim2Promise()
    const pack = readRepoFile(EVIDENCE_PACK_DOC)

    // The pack must keep an explicit overclaim refuse-list.
    expect(pack).toContain('refuse-list')
    expect(pack).toContain('we invented the LLM-computer')
    expect(pack).toContain('Percepta')

    // Public copy must never authorize a bare "world first" claim.
    expect(promise.unsafeCopy).toContain('world first')
    expect(promise.safeCopy).toContain('Percepta')
  })

  test('the #7027 world-first audit is wired into every gated world-first claim', () => {
    expect(repoFileExists(WORLD_FIRST_AUDIT_DOC)).toBe(true)

    const promises = [
      getPromise(PAID_BITCOIN_PROMISE_ID),
      getPromise(PROMISE_ID),
      getPromise(AGENTIC_SALES_FORCE_PROMISE_ID),
      getPromise(SALES_FORCE_PROMISE_ID),
    ]

    for (const promise of promises) {
      expect(promise.evidenceRefs).toContain(WORLD_FIRST_AUDIT_DOC)
      expect(promise.verification).toContain('2026-06-29 #7027 audit')
    }
  })

  test('the #7027 audit keeps red/planned states and owner-signed blockers', () => {
    expect(getPromise(PAID_BITCOIN_PROMISE_ID)).toEqual(
      expect.objectContaining({
        state: 'red',
        blockerRefs: expect.arrayContaining([
          'blocker.product_promises.world_first_evidence_pack_missing',
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ]),
      }),
    )
    expect(getPromise(PROMISE_ID)).toEqual(
      expect.objectContaining({
        state: 'red',
        blockerRefs: expect.arrayContaining([
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ]),
      }),
    )
    expect(getPromise(AGENTIC_SALES_FORCE_PROMISE_ID)).toEqual(
      expect.objectContaining({
        state: 'planned',
        blockerRefs: expect.arrayContaining([
          'blocker.product_promises.world_first_agentic_sales_force_not_achieved',
          'blocker.product_promises.world_first_agentic_sales_force_no_sized_verifiable_force',
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ]),
      }),
    )
    expect(getPromise(SALES_FORCE_PROMISE_ID)).toEqual(
      expect.objectContaining({
        state: 'planned',
        blockerRefs: expect.arrayContaining([
          'blocker.product_promises.world_first_largest_sales_force_not_achieved',
          'blocker.product_promises.world_first_largest_sales_force_seven_million_bar_unmet',
          'blocker.product_promises.world_first_owner_signed_upgrade_missing',
        ]),
      }),
    )
  })

  test('the #7027 audit carries a refuse-list for unqualified public copy', () => {
    const audit = readRepoFile(WORLD_FIRST_AUDIT_DOC)

    expect(audit).toContain('Refuse-List')
    expect(audit).toContain('bare "world first" framing')
    expect(audit).toContain('OpenAgents has the largest agentic sales force')
    expect(audit).toContain('has met the seven-million-agent bar')
    expect(audit).toContain('owner-signed transition receipt')
  })
})
