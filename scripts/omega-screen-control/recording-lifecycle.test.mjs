import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import {
  assertNotCmdQQuit,
  assertQuitIsSigtermOnly,
  buildSafeTailVideoFilter,
  computeSafeTailPlan,
  estimateFinderTailSeconds,
  looksLikeDesktopOrFinderTail,
  normalizeKeyCombo,
  parseSafeTailSeconds,
} from './recording-lifecycle.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const cliSource = readFileSync(join(here, 'omega-screen-control.mjs'), 'utf8')

test('normalizeKeyCombo collapses whitespace and case', () => {
  assert.equal(normalizeKeyCombo('Cmd + Q'), 'cmd+q')
  assert.equal(normalizeKeyCombo('command+enter'), 'command+enter')
})

test('assertNotCmdQQuit refuses Cmd+Q variants', () => {
  assert.throws(() => assertNotCmdQQuit('cmd+q'), /SIGTERM only/)
  assert.throws(() => assertNotCmdQQuit('command+q'), /SIGTERM only/)
  assert.throws(() => assertNotCmdQQuit('Cmd + Q'), /SIGTERM only/)
  assert.doesNotThrow(() => assertNotCmdQQuit('cmd+enter'))
})

test('parseSafeTailSeconds validates and defaults', () => {
  assert.equal(parseSafeTailSeconds(undefined, 1), 1)
  assert.equal(parseSafeTailSeconds('0.9'), 0.9)
  assert.equal(parseSafeTailSeconds('0'), 0)
  assert.throws(() => parseSafeTailSeconds('-1'), />= 0/)
  assert.throws(() => parseSafeTailSeconds('nope'), />= 0/)
})

test('computeSafeTailPlan freezes clean frame over dirty tail', () => {
  const plan = computeSafeTailPlan({
    durationSec: 10,
    safeTailSeconds: 1,
    finderTrimSeconds: 0.9,
  })
  assert.equal(plan.action, 'freeze_clean_tail')
  assert.ok(Math.abs(plan.keepUntilSec - 8.1) < 0.001)
  assert.equal(plan.freezeDurationSec, 1)
  assert.ok(plan.trimmedSec > 1.8)
})

test('computeSafeTailPlan trim-only when safe-tail is zero', () => {
  const plan = computeSafeTailPlan({
    durationSec: 10,
    safeTailSeconds: 0,
    finderTrimSeconds: 0.9,
  })
  assert.equal(plan.action, 'trim_only')
  assert.equal(plan.freezeDurationSec, 0)
  assert.match(buildSafeTailVideoFilter(plan), /^trim=0:/)
})

test('computeSafeTailPlan none when no trim requested', () => {
  const plan = computeSafeTailPlan({
    durationSec: 10,
    safeTailSeconds: 0,
    finderTrimSeconds: 0,
  })
  assert.equal(plan.action, 'none')
  assert.equal(buildSafeTailVideoFilter(plan), null)
})

test('buildSafeTailVideoFilter freezes with tpad clone', () => {
  const plan = computeSafeTailPlan({
    durationSec: 28,
    safeTailSeconds: 1,
    finderTrimSeconds: 0,
  })
  const vf = buildSafeTailVideoFilter(plan)
  assert.match(vf, /tpad=stop_mode=clone:stop_duration=1\.000/)
  assert.match(vf, /trim=0:27\.000/)
})

test('looksLikeDesktopOrFinderTail detects bright high-variance jump', () => {
  const reference = { meanLuma: 32, variance: 40, darkUiRatio: 0.55 }
  const desktop = { meanLuma: 110, variance: 220, darkUiRatio: 0.05 }
  assert.equal(looksLikeDesktopOrFinderTail(desktop, reference), true)
  assert.equal(
    looksLikeDesktopOrFinderTail(
      { meanLuma: 34, variance: 42, darkUiRatio: 0.54 },
      reference,
    ),
    false,
  )
})

test('estimateFinderTailSeconds returns seconds from first bad sample', () => {
  const referenceStats = { meanLuma: 30, variance: 35, darkUiRatio: 0.5 }
  const sampleStats = [
    { t: 9.0, meanLuma: 31, variance: 36, darkUiRatio: 0.49 },
    { t: 9.2, meanLuma: 95, variance: 200, darkUiRatio: 0.08 },
    { t: 9.4, meanLuma: 100, variance: 210, darkUiRatio: 0.05 },
  ]
  const trim = estimateFinderTailSeconds({
    durationSec: 10,
    sampleStepSec: 0.2,
    sampleStats,
    referenceStats,
    maxScanSec: 1.5,
  })
  assert.ok(trim >= 0.8 && trim <= 1.2, `unexpected trim ${trim}`)
})

test('CLI source hard-stops before quit and never documents Cmd+Q as quit', () => {
  assert.match(cliSource, /hard-stop/i)
  assert.match(cliSource, /safe-tail-seconds/)
  assert.match(cliSource, /assertNotCmdQQuit/)
  assert.match(cliSource, /SIGTERM/)
  assert.match(cliSource, /Refusing quit while screen recording is still active/)
  // Quit must not osascript a Cmd+Q keystroke.
  assert.doesNotMatch(
    cliSource,
    /keystroke\s+"q".*command down|command down.*keystroke\s+"q"/i,
  )
  assertQuitIsSigtermOnly(cliSource)
})
