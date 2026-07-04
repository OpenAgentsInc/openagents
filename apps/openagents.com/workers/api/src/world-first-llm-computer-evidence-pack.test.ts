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
const DEFINITION_DOC =
  'docs/launch/2026-06-20-llm-computer-training-run-definition.md'
const EVIDENCE_PACK_DOC =
  'docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md'

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

    // State is untouched by the evidence work. (The record sat red until
    // registry 2026-07-04.8 — commit 71c80fe9c4 — demoted out-of-focus
    // non-green records to planned as an owner-directed refocus; this guard
    // tracks the registry's owner-directed state and keeps asserting the
    // evidence work itself never flips the record green.)
    expect(promise.state).toBe('planned')

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
})
