// Pure helpers for omega-screen-control recording lifecycle (#9233).
// Keep Finder/Desktop out of the capture tail: hard-stop before quit,
// optional safe-tail freeze, and a light desktop/Finder heuristic trim.

/** Quit must be SIGTERM on the tracked pid — never a Cmd+Q keystroke. */
export const FORBIDDEN_QUIT_KEY_COMBOS = Object.freeze([
  'cmd+q',
  'command+q',
  'cmd + q',
  'command + q',
])

export function normalizeKeyCombo(combo) {
  return String(combo || '')
    .toLowerCase()
    .split('+')
    .map((p) => p.trim())
    .filter(Boolean)
    .join('+')
}

export function assertNotCmdQQuit(combo) {
  const norm = normalizeKeyCombo(combo)
  if (norm === 'cmd+q' || norm === 'command+q') {
    throw new Error(
      'Refusing Cmd+Q. Quit Omega with `omega-screen-control quit` (SIGTERM only).',
    )
  }
}

export function parseSafeTailSeconds(raw, fallback = 0) {
  if (raw == null || raw === '') return fallback
  const n = Number(raw)
  if (!Number.isFinite(n) || n < 0) {
    throw new Error('--safe-tail-seconds must be a number >= 0')
  }
  return n
}

/**
 * Plan post-capture tail handling.
 * - safeTailSeconds > 0: keep [0, duration - safeTail], freeze last clean
 *   frame for safeTailSeconds (replaces a dirty quit/teardown tail).
 * - finderTrimSeconds > 0: also drop that many seconds from the end before
 *   the freeze (heuristic-detected Desktop/Finder).
 */
export function computeSafeTailPlan({
  durationSec,
  safeTailSeconds = 0,
  finderTrimSeconds = 0,
}) {
  const duration = Number(durationSec)
  if (!Number.isFinite(duration) || duration <= 0) {
    throw new Error('durationSec must be a positive number')
  }
  const safe = Math.max(0, Number(safeTailSeconds) || 0)
  const finder = Math.max(0, Number(finderTrimSeconds) || 0)
  const trimTotal = Math.min(duration - 0.05, safe + finder)
  if (trimTotal <= 0) {
    return {
      action: 'none',
      keepUntilSec: duration,
      freezeFrameAtSec: null,
      freezeDurationSec: 0,
      trimmedSec: 0,
    }
  }
  const keepUntilSec = Math.max(0.05, duration - trimTotal)
  const freezeDurationSec = safe > 0 ? safe : 0
  return {
    action: freezeDurationSec > 0 ? 'freeze_clean_tail' : 'trim_only',
    keepUntilSec,
    freezeFrameAtSec: keepUntilSec,
    freezeDurationSec,
    trimmedSec: trimTotal,
  }
}

/**
 * Build an ffmpeg -vf chain that trims to keepUntil and optionally freezes
 * the last kept frame for freezeDurationSec.
 */
export function buildSafeTailVideoFilter(plan) {
  if (!plan || plan.action === 'none') return null
  const keep = Number(plan.keepUntilSec).toFixed(3)
  if (plan.action === 'trim_only' || plan.freezeDurationSec <= 0) {
    return `trim=0:${keep},setpts=PTS-STARTPTS`
  }
  const freeze = Number(plan.freezeDurationSec).toFixed(3)
  return `trim=0:${keep},setpts=PTS-STARTPTS,tpad=stop_mode=clone:stop_duration=${freeze}`
}

/**
 * Lightweight Desktop/Finder heuristic on sampled frame stats.
 * Expect referenceStats from mid-clip Omega UI; tailStats from the last ~1s.
 *
 * Signals (any strong hit → true):
 * - large mean-luma jump toward a brighter desktop
 * - much higher luma variance (wallpaper / Finder chrome)
 * - low "dark UI" ratio when reference was dark (Omega + pad)
 */
export function looksLikeDesktopOrFinderTail(tailStats, referenceStats, opts = {}) {
  const lumaJump = opts.lumaJump ?? 28
  const varianceRatio = opts.varianceRatio ?? 1.85
  const darkUiDrop = opts.darkUiDrop ?? 0.25

  if (!tailStats || !referenceStats) return false

  const refLuma = Number(referenceStats.meanLuma)
  const tailLuma = Number(tailStats.meanLuma)
  const refVar = Number(referenceStats.variance)
  const tailVar = Number(tailStats.variance)
  const refDark = Number(referenceStats.darkUiRatio ?? 0)
  const tailDark = Number(tailStats.darkUiRatio ?? 0)

  if (![refLuma, tailLuma, refVar, tailVar].every((n) => Number.isFinite(n))) {
    return false
  }

  if (tailLuma - refLuma >= lumaJump) return true
  if (refVar > 1 && tailVar / refVar >= varianceRatio && tailLuma > refLuma + 8) {
    return true
  }
  if (refDark >= 0.35 && refDark - tailDark >= darkUiDrop && tailLuma > refLuma + 10) {
    return true
  }
  return false
}

/** Estimate how many seconds of tail look like Desktop/Finder (0 if clean). */
export function estimateFinderTailSeconds({
  durationSec,
  sampleStepSec = 0.2,
  sampleStats = [],
  referenceStats,
  maxScanSec = 1.5,
}) {
  const duration = Number(durationSec)
  if (!Number.isFinite(duration) || duration <= 0) return 0
  if (!Array.isArray(sampleStats) || sampleStats.length === 0) return 0

  const scanFrom = Math.max(0, duration - maxScanSec)
  let firstBadAt = null
  for (const sample of sampleStats) {
    const t = Number(sample.t)
    if (!Number.isFinite(t) || t < scanFrom) continue
    if (looksLikeDesktopOrFinderTail(sample, referenceStats)) {
      if (firstBadAt == null || t < firstBadAt) firstBadAt = t
    }
  }
  if (firstBadAt == null) return 0
  // Trim from first bad sample to end, plus one sample step of slack.
  return Math.min(duration, duration - firstBadAt + sampleStepSec)
}

export function assertQuitIsSigtermOnly(quitSourceText) {
  const text = String(quitSourceText || '')
  if (/keystroke\s+["']q["'].*command|command down.*keystroke\s+["']q["']/i.test(text)) {
    throw new Error('quit path must not send Cmd+Q keystrokes')
  }
  if (!/SIGTERM/.test(text)) {
    throw new Error('quit path must signal SIGTERM')
  }
}
