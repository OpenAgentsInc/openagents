#!/usr/bin/env bun
/**
 * SQ-1 / #8618 — Sarah quality scoreboard validator.
 * Playback-first: stills alone never make a take advance-eligible.
 */

import { existsSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const SCHEMA_VERSION = 'openagents.sarah.quality_scoreboard.v1'
const VERDICTS = new Set(['pending', 'pass', 'fail'])
const STT = new Set(['pass', 'fail', 'skip'])
const RISK = new Set(['low', 'med', 'high'])

const isRecord = (v) => typeof v === 'object' && v !== null && !Array.isArray(v)

/**
 * @param {unknown} raw
 * @returns {{ ok: boolean, errors: string[], warnings: string[], derivedEligible?: boolean }}
 */
export const validateQualityScoreboard = (raw) => {
  /** @type {string[]} */
  const errors = []
  /** @type {string[]} */
  const warnings = []

  if (!isRecord(raw)) {
    return { ok: false, errors: ['scoreboard must be a JSON object'], warnings }
  }
  if (raw.schemaVersion !== SCHEMA_VERSION) {
    errors.push(`schemaVersion must be ${SCHEMA_VERSION}`)
  }
  if (typeof raw.takeId !== 'string' || !raw.takeId.trim()) {
    errors.push('takeId is required')
  }
  if (!Array.isArray(raw.issueRefs) || raw.issueRefs.length === 0) {
    errors.push('issueRefs required')
  }

  if (!isRecord(raw.inputs)) {
    errors.push('inputs required')
  } else {
    for (const k of [
      'sourceClip',
      'script',
      'ttsRef',
      'renderCommand',
      'recipe',
    ]) {
      if (typeof raw.inputs[k] !== 'string' || !raw.inputs[k].trim()) {
        errors.push(`inputs.${k} required`)
      }
    }
    if (!isRecord(raw.inputs.modelVersions)) {
      errors.push('inputs.modelVersions required')
    }
    if (!Array.isArray(raw.inputs.commits) || raw.inputs.commits.length === 0) {
      errors.push('inputs.commits required (non-empty)')
    }
    if (
      !Array.isArray(raw.inputs.artifactUris) ||
      raw.inputs.artifactUris.length === 0
    ) {
      errors.push('inputs.artifactUris required (non-empty)')
    }
  }

  if (!isRecord(raw.audio)) {
    errors.push('audio required')
  } else {
    if (!STT.has(raw.audio.sttRoundTrip)) {
      errors.push('audio.sttRoundTrip must be pass|fail|skip')
    }
    if (!VERDICTS.has(raw.audio.prosodyVerdict)) {
      errors.push('audio.prosodyVerdict must be pending|pass|fail')
    }
    if (!RISK.has(raw.audio.initialismRisk)) {
      errors.push('audio.initialismRisk must be low|med|high')
    }
  }

  if (!isRecord(raw.video)) {
    errors.push('video required')
  } else {
    if (!VERDICTS.has(raw.video.playbackVerdict)) {
      errors.push('video.playbackVerdict must be pending|pass|fail')
    }
    if (!isRecord(raw.video.avSync)) {
      errors.push('video.avSync required')
    } else {
      for (const k of ['start', 'mid', 'end']) {
        if (!VERDICTS.has(raw.video.avSync[k])) {
          errors.push(`video.avSync.${k} must be pending|pass|fail`)
        }
      }
    }
    if (!Array.isArray(raw.video.badFrameExclusions)) {
      errors.push('video.badFrameExclusions must be an array')
    }
  }

  if (!isRecord(raw.ops)) {
    errors.push('ops required')
  } else {
    if (raw.ops.artifactExistenceOk !== true) {
      errors.push('ops.artifactExistenceOk must be true (pair with SQ-8 object_exists)')
    }
    if (
      typeof raw.ops.hostDisposition !== 'string' ||
      !raw.ops.hostDisposition.trim()
    ) {
      errors.push('ops.hostDisposition required')
    }
  }

  // Cultural law: derive eligibility; stills cannot advance alone
  let derivedEligible = false
  if (isRecord(raw.video) && isRecord(raw.audio) && isRecord(raw.ops)) {
    const playbackPass = raw.video.playbackVerdict === 'pass'
    const audioHardFail =
      raw.audio.sttRoundTrip === 'fail' || raw.audio.prosodyVerdict === 'fail'
    const artifactsOk = raw.ops.artifactExistenceOk === true
    derivedEligible = playbackPass && artifactsOk && !audioHardFail

    if (raw.video.stillsOk === true && raw.video.playbackVerdict !== 'pass') {
      // Explicit anti-pattern from the issue: enhanced take passed stills, failed playback
      if (isRecord(raw.advance) && raw.advance.eligible === true) {
        errors.push(
          'advance.eligible cannot be true when playbackVerdict is not pass (stills-only advances forbidden)',
        )
      }
    }
  }

  if (!isRecord(raw.advance)) {
    errors.push('advance required')
  } else if (typeof raw.advance.eligible !== 'boolean') {
    errors.push('advance.eligible must be boolean')
  } else if (raw.advance.eligible !== derivedEligible) {
    errors.push(
      `advance.eligible is ${raw.advance.eligible} but derived law yields ${derivedEligible} (playback+artifacts, no hard audio fail)`,
    )
  }

  return {
    ok: errors.length === 0,
    errors,
    warnings,
    derivedEligible,
  }
}

export const parseArgs = (argv) => {
  const out = { scoreboard: null, selfTest: false, help: false }
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--scoreboard' || a === '-s') out.scoreboard = argv[++i] ?? null
    else if (a === '--self-test') out.selfTest = true
    else if (a === '--help' || a === '-h') out.help = true
  }
  return out
}

const HERE = dirname(fileURLToPath(import.meta.url))
const EXAMPLE = resolve(HERE, '../fixtures/quality-scoreboard.example.json')

export const runSelfTest = () => {
  const example = JSON.parse(readFileSync(EXAMPLE, 'utf8'))
  const good = validateQualityScoreboard(example)
  if (!good.ok) {
    console.error('self-test FAIL', good.errors)
    return 1
  }
  const stillsOnly = structuredClone(example)
  stillsOnly.video.playbackVerdict = 'fail'
  stillsOnly.video.stillsOk = true
  stillsOnly.advance.eligible = true
  const bad = validateQualityScoreboard(stillsOnly)
  if (bad.ok) {
    console.error('self-test FAIL: stills-only advance should be rejected')
    return 1
  }
  console.log('self-test OK')
  return 0
}

const main = () => {
  const args = parseArgs(process.argv.slice(2))
  if (args.help) {
    console.log(
      'Usage: bun scripts/quality-scoreboard.mjs --scoreboard <path.json> | --self-test',
    )
    process.exit(0)
  }
  if (args.selfTest) process.exit(runSelfTest())
  if (!args.scoreboard) {
    console.error('error: --scoreboard required')
    process.exit(2)
  }
  const path = resolve(args.scoreboard)
  if (!existsSync(path)) {
    console.error(`error: not found ${path}`)
    process.exit(2)
  }
  const raw = JSON.parse(readFileSync(path, 'utf8'))
  const result = validateQualityScoreboard(raw)
  if (!result.ok) {
    console.error('SCOREBOARD INVALID')
    for (const e of result.errors) console.error(`  - ${e}`)
    process.exit(1)
  }
  console.log(
    `SCOREBOARD OK — takeId=${raw.takeId} advance.eligible=${raw.advance.eligible}`,
  )
  process.exit(0)
}

const isMain =
  typeof Bun !== 'undefined'
    ? Boolean(Bun.main) &&
      resolve(String(Bun.main)) === resolve(fileURLToPath(import.meta.url))
    : process.argv[1] &&
      resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))

if (isMain) main()
