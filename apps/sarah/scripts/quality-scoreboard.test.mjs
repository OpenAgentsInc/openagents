import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  validateQualityScoreboard,
  runSelfTest,
} from './quality-scoreboard.mjs'

const EXAMPLE = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../fixtures/quality-scoreboard.example.json',
)
const load = () => JSON.parse(readFileSync(EXAMPLE, 'utf8'))

describe('SQ-1 quality scoreboard (#8618)', () => {
  test('example validates and is advance-eligible', () => {
    const r = validateQualityScoreboard(load())
    expect(r.ok).toBe(true)
    expect(r.derivedEligible).toBe(true)
  })

  test('stills-only cannot advance (playback fail + eligible true rejected)', () => {
    const bad = load()
    bad.video.playbackVerdict = 'fail'
    bad.video.stillsOk = true
    bad.advance.eligible = true
    const r = validateQualityScoreboard(bad)
    expect(r.ok).toBe(false)
    expect(r.errors.some((e) => e.includes('stills') || e.includes('playback'))).toBe(
      true,
    )
  })

  test('playback fail correctly forces eligible false', () => {
    const s = load()
    s.video.playbackVerdict = 'fail'
    s.advance.eligible = false
    const r = validateQualityScoreboard(s)
    expect(r.ok).toBe(true)
    expect(r.derivedEligible).toBe(false)
  })

  test('hard audio fail blocks advance', () => {
    const s = load()
    s.audio.prosodyVerdict = 'fail'
    s.advance.eligible = false
    const r = validateQualityScoreboard(s)
    expect(r.ok).toBe(true)
    expect(r.derivedEligible).toBe(false)
  })

  test('missing artifactExistenceOk fails', () => {
    const s = load()
    s.ops.artifactExistenceOk = false
    s.advance.eligible = false
    const r = validateQualityScoreboard(s)
    expect(r.ok).toBe(false)
  })

  test('self-test passes', () => {
    expect(runSelfTest()).toBe(0)
  })
})
