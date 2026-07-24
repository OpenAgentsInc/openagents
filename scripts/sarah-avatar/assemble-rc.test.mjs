import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import {
  buildAssembleFilterComplex,
  buildCutawayInventory,
  checkTranscriptLock,
  DEFAULT_T2_FRACTION,
  deriveT1FromSilenceIntervals,
  evaluateCutPointQc,
  extractSpokenBody,
  normalizeSpokenText,
  parseQcOffsetsFlag,
  parseSilenceDetectLog,
  planCutPointQcTimes,
  planMidBeats,
  resolveCutTimes,
} from './assemble-rc-lib.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const cliSource = readFileSync(join(here, 'assemble-rc.mjs'), 'utf8')

test('parseSilenceDetectLog reads ffmpeg silencedetect lines', () => {
  const log = `
[silencedetect @ 0x1] silence_start: 16.430
[silencedetect @ 0x1] silence_end: 17.310 | silence_duration: 0.880
[silencedetect @ 0x1] silence_start: 40.000
[silencedetect @ 0x1] silence_end: 40.200 | silence_duration: 0.200
`
  const intervals = parseSilenceDetectLog(log)
  assert.equal(intervals.length, 2)
  assert.ok(Math.abs(intervals[0].start - 16.43) < 0.001)
  assert.ok(Math.abs(intervals[0].end - 17.31) < 0.001)
  assert.ok(Math.abs(intervals[0].duration - 0.88) < 0.001)
})

test('deriveT1FromSilenceIntervals picks post-zed pause (Ep262 shape)', () => {
  const derived = deriveT1FromSilenceIntervals(
    [
      { start: 16.43, end: 17.31, duration: 0.88 },
      { start: 18.0, end: 18.15, duration: 0.15 },
    ],
    { fromSec: 14, toSec: 20, minSilenceSec: 0.25, padAfterSilenceSec: 0.2 },
  )
  assert.equal(derived.ok, true)
  assert.equal(derived.t1, 17.51)
})

test('deriveT1FromSilenceIntervals fails when no pause in window', () => {
  const derived = deriveT1FromSilenceIntervals(
    [{ start: 2, end: 2.5, duration: 0.5 }],
    { fromSec: 14, toSec: 20 },
  )
  assert.equal(derived.ok, false)
})

test('resolveCutTimes defaults T2 to 0.78*D and rejects folklore-blind bad T1', () => {
  const cuts = resolveCutTimes({ durationSec: 44.64, t1: 17.5 })
  assert.equal(cuts.t2FromFraction, true)
  assert.equal(cuts.t2Fraction, DEFAULT_T2_FRACTION)
  assert.ok(Math.abs(cuts.T2 - 44.64 * 0.78) < 0.001)
  assert.throws(() => resolveCutTimes({ durationSec: 44.64, t1: 0 }), /T1/)
  assert.throws(
    () => resolveCutTimes({ durationSec: 44.64, t1: 17.5, t2: 17.6 }),
    /T2/,
  )
})

test('planMidBeats supports second mid cutaway still', () => {
  const plan = planMidBeats({
    durB: 17.3192,
    cutawaySeconds: 6.3192,
    screenshareDurationSec: 22,
  })
  assert.equal(plan.beats.length, 2)
  assert.equal(plan.beats[0].kind, 'screenshare')
  assert.equal(plan.beats[1].kind, 'cutaway')
  assert.ok(Math.abs(plan.screenshareUseSec - 11) < 0.001)
  assert.ok(Math.abs(plan.cutawayUseSec - 6.3192) < 0.001)
})

test('planMidBeats rejects cutaway that starves screenshare', () => {
  assert.throws(
    () => planMidBeats({ durB: 10, cutawaySeconds: 9.9 }),
    /0\.25s/,
  )
})

test('planMidBeats chains multiple cutaway beats for multi-beat RCs (#9237)', () => {
  const plan = planMidBeats({
    durB: 20,
    cutaways: [3, 4, 2],
    screenshareDurationSec: 30,
  })
  assert.equal(plan.beats.length, 4)
  assert.deepEqual(
    plan.beats.map((b) => b.kind),
    ['screenshare', 'cutaway', 'cutaway', 'cutaway'],
  )
  assert.deepEqual(
    plan.beats.slice(1).map((b) => b.durationSec),
    [3, 4, 2],
  )
  assert.ok(Math.abs(plan.screenshareUseSec - 11) < 0.001)
  assert.ok(Math.abs(plan.cutawayUseSec - 9) < 0.001)
})

test('planMidBeats rejects a multi-beat cutaway total that starves screenshare', () => {
  assert.throws(
    () => planMidBeats({ durB: 10, cutaways: [5, 4.9] }),
    /0\.25s/,
  )
})

test('planCutPointQcTimes samples mid tail near T2 (not Sarah return)', () => {
  const times = planCutPointQcTimes(34.8192)
  assert.deepEqual(
    times.map((t) => t.label),
    ['T2-1', 'T2-0.5', 'T2-1frame'],
  )
  assert.ok(Math.abs(times[0].atSec - 33.8192) < 0.001)
  assert.ok(times[2].atSec < 34.8192)
  assert.ok(times[2].atSec > 34.75)
})

test('evaluateCutPointQc fails on Finder-like frames near T2', () => {
  const referenceStats = { meanLuma: 32, variance: 40, darkUiRatio: 0.55 }
  const qc = evaluateCutPointQc({
    referenceStats,
    samples: [
      {
        label: 'T2-1',
        atSec: 33.8,
        stats: { meanLuma: 34, variance: 42, darkUiRatio: 0.54 },
      },
      {
        label: 'T2',
        atSec: 34.8,
        stats: { meanLuma: 110, variance: 220, darkUiRatio: 0.05 },
      },
    ],
  })
  assert.equal(qc.ok, false)
  assert.equal(qc.failures.length, 1)
  assert.match(qc.failures[0].reason, /Finder/)
})

test('evaluateCutPointQc passes clean product frames', () => {
  const referenceStats = { meanLuma: 32, variance: 40, darkUiRatio: 0.55 }
  const qc = evaluateCutPointQc({
    referenceStats,
    samples: [
      {
        label: 'T2',
        atSec: 34.8,
        stats: { meanLuma: 33, variance: 41, darkUiRatio: 0.53 },
      },
    ],
  })
  assert.equal(qc.ok, true)
})

test('transcript lock matches Desktop paste to repo spoken body', () => {
  const desktop = `
Hello again. Let's talk strategy.

So we take a page from the Cursor playbook. They forked VS Code. We fork zed.
`
  const repo = `# Episode 262

Status: final script.

---

**Sarah:** Hello again. Let's talk strategy.

So we take a page from the Cursor playbook. They forked VS Code. We fork zed.
`
  const lock = checkTranscriptLock({ desktopText: desktop, repoText: repo })
  assert.equal(lock.ok, true)
  assert.equal(
    normalizeSpokenText(desktop),
    extractSpokenBody(repo),
  )
})

test('transcript lock detects drift', () => {
  const lock = checkTranscriptLock({
    desktopText: 'Hello again. We fork zed.',
    repoText: '**Sarah:** Hello again. We fork Zed and more.',
  })
  assert.equal(lock.ok, false)
  assert.match(lock.reason, /drift/)
})

test('buildAssembleFilterComplex keeps continuous Sarah audio and cutaway concat', () => {
  const midPlan = planMidBeats({ durB: 17.3192, cutawaySeconds: 6.3192 })
  const fc = buildAssembleFilterComplex({
    T1: 17.5,
    T2: 34.8192,
    D: 44.64,
    midPlan,
    hasCutaway: true,
  })
  assert.match(fc, /\[0:a\]atrim=0:44\.6400/)
  assert.match(fc, /\[0:v\]trim=0:17\.5000/)
  assert.match(fc, /concat=n=2:v=1:a=0\[vb\]/)
  assert.match(fc, /\[0:v\]trim=34\.8192:44\.6400/)
  assert.match(fc, /concat=n=3:v=1:a=0\[vout\]/)
})

test('buildAssembleFilterComplex chains N cutaway inputs for multi-beat RCs (#9237)', () => {
  const midPlan = planMidBeats({ durB: 20, cutaways: [3, 4, 2] })
  const fc = buildAssembleFilterComplex({
    T1: 10,
    T2: 30,
    D: 40,
    midPlan,
    hasCutaway: true,
    cutawayLabels: ['2', '3', '4'],
  })
  assert.match(fc, /\[1:v\]trim=0:11\.0000,setpts=PTS-STARTPTS\[vb1\]/)
  assert.match(fc, /\[2:v\]trim=0:3\.0000,setpts=PTS-STARTPTS,fps=24,format=yuv420p\[vb2\]/)
  assert.match(fc, /\[3:v\]trim=0:4\.0000,setpts=PTS-STARTPTS,fps=24,format=yuv420p\[vb3\]/)
  assert.match(fc, /\[4:v\]trim=0:2\.0000,setpts=PTS-STARTPTS,fps=24,format=yuv420p\[vb4\]/)
  assert.match(fc, /\[vb1\]\[vb2\]\[vb3\]\[vb4\]concat=n=4:v=1:a=0\[vb\]/)
  assert.match(fc, /concat=n=3:v=1:a=0\[vout\]/)
})

test('buildCutawayInventory records one entry per cutaway beat, including multi-beat RCs (#9237)', () => {
  const midPlan = planMidBeats({ durB: 20, cutaways: [3, 4, 2] })
  const inventory = buildCutawayInventory({
    episode: '262',
    midPlan,
    cutaways: [
      { sourcePath: '/tmp/a.jpg', archivedPath: '/repo/.artifacts/episode-262/cutaways/1-a.jpg' },
      { sourcePath: '/tmp/b.jpg', archivedPath: '/repo/.artifacts/episode-262/cutaways/2-b.jpg' },
      { sourcePath: '/tmp/c.mp4', archivedPath: '/repo/.artifacts/episode-262/cutaways/3-c.mp4' },
    ],
  })
  assert.equal(inventory.beatCount, 3)
  assert.equal(inventory.cutaways.length, 3)
  assert.deepEqual(
    inventory.cutaways.map((c) => c.durationSec),
    [3, 4, 2],
  )
  assert.equal(inventory.cutaways[1].sourcePath, '/tmp/b.jpg')
  assert.equal(inventory.episode, '262')
})

test('buildCutawayInventory is populated (empty list, not missing) with zero cutaway beats', () => {
  const midPlan = planMidBeats({ durB: 20 })
  const inventory = buildCutawayInventory({ episode: null, midPlan, cutaways: [] })
  assert.equal(inventory.beatCount, 0)
  assert.deepEqual(inventory.cutaways, [])
})

test('buildCutawayInventory throws on a beat/source count mismatch', () => {
  const midPlan = planMidBeats({ durB: 20, cutaways: [3, 4] })
  assert.throws(
    () => buildCutawayInventory({ midPlan, cutaways: [{ sourcePath: '/tmp/a.jpg' }] }),
    /mismatch/,
  )
})

test('buildAssembleFilterComplex without cutaway uses screenshare only for B', () => {
  const midPlan = planMidBeats({ durB: 10, cutawaySeconds: 0 })
  const fc = buildAssembleFilterComplex({
    T1: 5,
    T2: 15,
    D: 20,
    midPlan,
    hasCutaway: false,
  })
  assert.match(fc, /\[1:v\]trim=0:10\.0000,setpts=PTS-STARTPTS\[vb\]/)
  assert.doesNotMatch(fc, /concat=n=2:v=1/)
})

test('parseQcOffsetsFlag validates list', () => {
  assert.deepEqual(parseQcOffsetsFlag('-1,-0.5,0'), [-1, -0.5, 0])
  assert.throws(() => parseQcOffsetsFlag('a,b'), /qc-offsets/)
})

test('CLI documents one-command assemble + cut QC + silence T1', () => {
  assert.match(cliSource, /assemble-rc/)
  assert.match(cliSource, /t1-auto|silencedetect|deriveT1FromSilenceIntervals/)
  assert.match(cliSource, /cutaway-seconds/)
  assert.match(cliSource, /require-transcript-lock|transcript-lock/)
  assert.match(cliSource, /Finder\/Desktop/)
  assert.match(cliSource, /rc-no-music/)
})

test('CLI documents multi-beat cutaway inventory (#9237)', () => {
  assert.match(cliSource, /buildCutawayInventory/)
  assert.match(cliSource, /cutaway-inventory\.json/)
  assert.match(cliSource, /flagAll/)
})
