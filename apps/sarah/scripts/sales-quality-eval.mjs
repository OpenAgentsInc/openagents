#!/usr/bin/env bun
/**
 * SQ-5 / #8622 — deterministic sales-quality eval pack.
 * Hard oracles only (no LLM judge). Complements S-12 safety fixtures.
 */

import { readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = dirname(fileURLToPath(import.meta.url))
const FIXTURES = join(HERE, '../evals/sales-quality-fixtures.json')

const PRODUCT_WORDS =
  /\b(khala|openagents|pylon|credits|subscription|plan|pricing|package)\b/i
const CATALOG_DUMP =
  /\b(and also|as well as|plus our|we also offer|suite of|full suite)\b/i
const QUESTION = /\?/
const OPTIONAL_ACCOUNT =
  /\b(if useful|no rush|when you(?:'re| are) ready|optional|later)\b/i
const HANDOFF = /\b(human|team member|colleague|specialist|brief them|hand ?off)\b/i

export const wordCount = (text) =>
  text
    .trim()
    .split(/\s+/)
    .filter(Boolean).length

export const countQuestions = (text) => (text.match(/\?/g) ?? []).length

export const loadSalesQualityFixtures = (path = FIXTURES) =>
  JSON.parse(readFileSync(path, 'utf8'))

/**
 * @param {Record<string, unknown>} testCase
 */
export const evaluateSalesQualityCase = (testCase) => {
  const id = String(testCase.id)
  /** @type {Record<string, unknown>} */
  const evidence = { signals: [] }

  const fail = (reason) => ({
    id,
    status: /** @type {const} */ ('REFUTED'),
    evidence,
    refutedReason: reason,
  })
  const pass = () => ({
    id,
    status: /** @type {const} */ ('CONFIRMED'),
    evidence,
    refutedReason: null,
  })

  switch (id) {
    case 'pain_hunting_first_two_turns': {
      const turns = /** @type {string[]} */ (testCase.assistantTurns ?? [])
      if (turns.length < 2) return fail('need two assistant turns')
      for (const [i, turn] of turns.entries()) {
        if (countQuestions(turn) !== 1) {
          return fail(`turn ${i} must contain exactly one question`)
        }
        if (PRODUCT_WORDS.test(turn)) {
          return fail(`turn ${i} pitches product too early`)
        }
      }
      evidence.signals = ['single_question_per_turn', 'no_product_pitch_early']
      return pass()
    }
    case 'mirroring_before_pitch': {
      const transcript = /** @type {Array<{role:string,text:string}>} */ (
        testCase.transcript ?? []
      )
      const user = transcript.find((t) => t.role === 'user')
      const asst = transcript.find((t) => t.role === 'assistant')
      if (!user || !asst) return fail('need user+assistant turns')
      const userTokens = user.text
        .toLowerCase()
        .split(/\W+/)
        .filter((w) => w.length > 4)
      const mirrored = userTokens.some((w) => asst.text.toLowerCase().includes(w))
      if (!mirrored) return fail('pain not restated')
      if (!QUESTION.test(asst.text)) return fail('no follow-up question')
      evidence.signals = ['pain_restated', 'question_after_mirror']
      return pass()
    }
    case 'one_product_strike': {
      const text = String(testCase.assistantText ?? '')
      const productHits =
        text.match(/\b(Khala Code|Khala|OpenAgents|Pylon|Forum)\b/g) ?? []
      const unique = new Set(productHits.map((p) => p.toLowerCase()))
      if (productHits.length === 0) {
        return fail('expected a product family mention')
      }
      if (CATALOG_DUMP.test(text)) return fail('catalog dump language')
      if (unique.size > 2) return fail('too many product names')
      evidence.signals = ['single_product_mention', 'no_catalog_dump']
      return pass()
    }
    case 'momentum_ends_with_cta': {
      const text = String(testCase.assistantText ?? '').trim()
      if (!text.endsWith('?')) return fail('v1 requires ending with ?')
      evidence.signals = ['ends_with_question_or_cta']
      return pass()
    }
    case 'voice_length_avatar_safe': {
      const text = String(testCase.assistantText ?? '')
      const max = Number(testCase.maxWords ?? 45)
      const n = wordCount(text)
      evidence.wordCount = n
      evidence.maxWords = max
      if (n > max) return fail(`word count ${n} > ${max}`)
      evidence.signals = ['within_word_budget']
      return pass()
    }
    case 'non_pushy_account_funding': {
      const text = String(testCase.assistantText ?? '')
      if (!OPTIONAL_ACCOUNT.test(text)) {
        return fail('missing optional/no-rush account language')
      }
      if (
        /\b(must|required|immediately)\b.*\b(account|pay|card)\b/i.test(text)
      ) {
        return fail('pushy account/funding language')
      }
      if (!QUESTION.test(text)) return fail('should continue qualification')
      evidence.signals = ['optional_account_language', 'continues_qualification']
      return pass()
    }
    case 'human_handoff_enterprise': {
      const asst = String(testCase.assistantText ?? '')
      if (!HANDOFF.test(asst)) return fail('missing human handoff language')
      if (!/\b(seat|msa|timeline|decision|volume|discount)\b/i.test(asst)) {
        return fail('handoff brief missing concrete fields')
      }
      evidence.signals = ['human_handoff', 'brief_fields_present']
      return pass()
    }
    default:
      return fail(`unknown case ${id}`)
  }
}

export const runSalesQualityPack = (fixtures = loadSalesQualityFixtures()) => {
  const results = fixtures.cases.map((c) => evaluateSalesQualityCase(c))
  return {
    schema: 'sarah.sales_quality_run.v1',
    generatedAt: new Date().toISOString(),
    fixtureSchema: fixtures.schema,
    sourceRefs: fixtures.sourceRefs,
    results,
    summary: {
      confirmed: results.filter((r) => r.status === 'CONFIRMED').length,
      refuted: results.filter((r) => r.status === 'REFUTED').length,
      total: results.length,
    },
  }
}

const isMain =
  typeof Bun !== 'undefined'
    ? Boolean(Bun.main) &&
      resolve(String(Bun.main)) === resolve(fileURLToPath(import.meta.url))
    : process.argv[1] &&
      resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))

if (isMain) {
  const artifact = runSalesQualityPack()
  for (const r of artifact.results) {
    console.log(`${r.status} ${r.id}`)
    if (r.refutedReason) console.log(`  ${r.refutedReason}`)
  }
  console.log(
    `Sales quality: ${artifact.summary.confirmed}/${artifact.summary.total} confirmed`,
  )
  if (artifact.summary.refuted > 0) process.exit(1)
}
