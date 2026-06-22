// Deterministic verifier substrate for the Khala coding lane (#6010).
//
// The hot Worker route cannot launch a browser, so this module keeps the
// request-time gate pure and receipt-shaped while exposing a stable headless
// command contract for the runner that executes generated HTML artifacts outside
// the Worker. The current crossy-road rubric is deliberately narrow and fixture
// backed: one single-file HTML artifact, bounded controls/camera/difficulty/
// restart checks, and an explicit scalar reward for the later Psion handoff.

export const KHALA_CODE_CROSSY_ROAD_RUBRIC_REF =
  'rubric.khala_code.crossy_road.single_html.v1'

export const KHALA_CODE_HEADLESS_COMMAND_REF =
  'command.khala_code.crossy_road.headless_html_probe.v1'

export const KHALA_CODE_VERIFIER_WORKER_ID = 'khala-code-crossy-road-verifier'

export type KhalaCodeVerification = 'test_passed' | 'failed'

export type KhalaCodeRubricCheckId =
  | 'single_html_file'
  | 'loads_and_runs_headless'
  | 'direction_controls'
  | 'sane_follow_camera'
  | 'difficulty_ramps_with_progress'
  | 'restart_resets_character'

export type KhalaCodeRubricCheck = Readonly<{
  id: KhalaCodeRubricCheckId
  label: string
  passed: boolean
  evidenceRefs: ReadonlyArray<string>
  failureReason?: string | undefined
}>

export type KhalaCodeVerificationCommand = Readonly<{
  commandRef: typeof KHALA_CODE_HEADLESS_COMMAND_REF
  kind: 'headless_html_probe'
  rubricRef: typeof KHALA_CODE_CROSSY_ROAD_RUBRIC_REF
  target: 'crossy-road-single-html'
}>

export type KhalaCodeVerificationVerdict = Readonly<{
  artifact: Readonly<{
    bytes: number
    fingerprint: string
    kind: 'single_html'
  }>
  checks: ReadonlyArray<KhalaCodeRubricCheck>
  command: KhalaCodeVerificationCommand
  failedChecks: ReadonlyArray<KhalaCodeRubricCheckId>
  passedChecks: ReadonlyArray<KhalaCodeRubricCheckId>
  receiptRef: string
  reward: Readonly<{
    handoffRef: string
    scalar: number
  }>
  rubricRef: typeof KHALA_CODE_CROSSY_ROAD_RUBRIC_REF
  scalarReward: number
  sourceRefs: ReadonlyArray<string>
  summary: string
  verification: KhalaCodeVerification
  verified: boolean
}>

export type KhalaCodeVerifierInput = Readonly<{
  content: string
  meteringReceiptRef?: string | null | undefined
  requestId: string
  servedModel: string
  worker: string
}>

const CHECK_LABELS: Readonly<Record<KhalaCodeRubricCheckId, string>> = {
  direction_controls:
    'Arrow/WASD direction controls are wired to character movement.',
  difficulty_ramps_with_progress:
    'Difficulty or traffic speed ramps as score/progress increases.',
  loads_and_runs_headless:
    'The artifact has a runnable game surface and animation/update loop.',
  restart_resets_character:
    'Restart/reset places the character back at the starting position.',
  sane_follow_camera:
    'The camera or viewport follows the character with explicit framing.',
  single_html_file: 'The delivered artifact is one self-contained HTML file.',
}

const check = (
  id: KhalaCodeRubricCheckId,
  passed: boolean,
  evidenceRefs: ReadonlyArray<string>,
  failureReason: string,
): KhalaCodeRubricCheck => ({
  id,
  label: CHECK_LABELS[id],
  passed,
  evidenceRefs,
  ...(passed ? {} : { failureReason }),
})

const includesAll = (value: string, needles: ReadonlyArray<string>): boolean =>
  needles.every(needle => value.includes(needle.toLowerCase()))

const hasAny = (value: string, patterns: ReadonlyArray<RegExp>): boolean =>
  patterns.some(pattern => pattern.test(value))

const externalAssetPattern =
  /<(?:script|link|img|audio|video)\b[^>]*(?:src|href)\s*=\s*["'](?:https?:)?\/\//iu

const scriptSrcPattern = /<script\b[^>]*\bsrc\s*=/iu
const linkedStylesheetPattern =
  /<link\b[^>]*\brel\s*=\s*["'][^"']*stylesheet[^"']*["'][^>]*\bhref\s*=/iu

export const extractSingleHtmlArtifact = (
  content: string,
): string | undefined => {
  const trimmed = content.trim()
  if (/^(?:<!doctype\s+html>|<html\b)/iu.test(trimmed)) {
    return trimmed
  }

  const fenced = /```(?:html)?\s*([\s\S]*?)```/iu.exec(content)
  const candidate = fenced?.[1]?.trim()
  if (
    candidate !== undefined &&
    /^(?:<!doctype\s+html>|<html\b)/iu.test(candidate)
  ) {
    return candidate
  }

  return undefined
}

const stableFingerprint = (value: string): string => {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193) >>> 0
  }
  return `fnv1a32:${hash.toString(16).padStart(8, '0')}`
}

const safeReceiptPart = (value: string): string => {
  const cleaned = value.replace(/[^A-Za-z0-9_.-]+/gu, '_').slice(0, 80)
  return cleaned === '' ? 'unknown' : cleaned
}

export const khalaCodeVerificationReceiptRef = (
  input: Readonly<{ artifactFingerprint: string; requestId: string }>,
): string =>
  `receipt.inference.khala_code.verification.${safeReceiptPart(
    input.requestId,
  )}.${safeReceiptPart(input.artifactFingerprint.replace(':', '.'))}`

export const khalaCodeAcceptedOutcomeHandoffRef = (
  receiptRef: string,
): string =>
  `accepted_outcome.khala_code.crossy_road.${safeReceiptPart(receiptRef)}`

export const discoverKhalaCodeVerificationCommand =
  (): KhalaCodeVerificationCommand => ({
    commandRef: KHALA_CODE_HEADLESS_COMMAND_REF,
    kind: 'headless_html_probe',
    rubricRef: KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
    target: 'crossy-road-single-html',
  })

export const verifyKhalaCodeCompletion = (
  input: KhalaCodeVerifierInput,
): KhalaCodeVerificationVerdict => {
  const html = extractSingleHtmlArtifact(input.content)
  const artifactText = html ?? input.content.trim()
  const lower = artifactText.toLowerCase()
  const difficultyBlock =
    /function\s+(?:rampDifficulty|updateDifficulty|increaseDifficulty)\s*\([^)]*\)\s*\{([\s\S]*?)\}/iu
      .exec(artifactText)?.[1]
      ?.toLowerCase() ?? lower
  const restartBlock =
    /function\s+(?:restartGame|restart|resetGame|startOver)\s*\([^)]*\)\s*\{([\s\S]*?)\}/iu
      .exec(artifactText)?.[1]
      ?.toLowerCase() ?? lower
  const fingerprint = stableFingerprint(artifactText)
  const evidencePrefix = `artifact:${fingerprint}`
  const receiptRef = khalaCodeVerificationReceiptRef({
    artifactFingerprint: fingerprint,
    requestId: input.requestId,
  })

  const hasSingleHtml =
    html !== undefined &&
    /<html\b/iu.test(html) &&
    /<\/html>/iu.test(html) &&
    !externalAssetPattern.test(html) &&
    !scriptSrcPattern.test(html) &&
    !linkedStylesheetPattern.test(html)

  const loadsAndRuns =
    hasSingleHtml &&
    /<script\b/iu.test(artifactText) &&
    hasAny(lower, [/<canvas\b/iu, /\bid=["']game["']/iu, /\bgame\b/iu]) &&
    hasAny(lower, [
      /requestanimationframe/iu,
      /\bsetinterval\s*\(/iu,
      /\bfunction\s+(?:loop|tick|update|animate)\b/iu,
    ]) &&
    !hasAny(lower, [/throw\s+new\s+error/iu, /todo:\s*not\s+implemented/iu])

  const hasDirectionControls =
    includesAll(lower, ['arrowup', 'arrowdown', 'arrowleft', 'arrowright']) &&
    includesAll(lower, ["'w'", "'a'", "'s'", "'d'"]) &&
    hasAny(lower, [
      /addEventListener\s*\(\s*["']keydown/iu,
      /onkeydown/iu,
      /keyboard/iu,
    ]) &&
    hasAny(lower, [/\bmove\s*\(/iu, /\bdirection\b/iu, /\bturn\b/iu])

  const hasSaneCamera =
    hasAny(lower, [
      /\bcamera\b/iu,
      /\bviewport\b/iu,
      /\bfollow\b/iu,
      /\bisometric\b/iu,
      /\bthird[_ -]?person\b/iu,
    ]) &&
    hasAny(lower, [/\blookat\b/iu, /\boffset\b/iu, /\btrack/iu, /\bplayer\b/iu])

  const hasDifficultyRamp =
    hasAny(lower, [
      /\bdifficulty\b/iu,
      /\bspeed\b/iu,
      /\bspawnrate\b/iu,
      /\btraffic\b/iu,
      /\blevel\b/iu,
    ]) &&
    hasAny(lower, [/\bprogress\b/iu, /\bscore\b/iu, /\bdistance\b/iu]) &&
    hasAny(difficultyBlock, [
      /\bprogress\b/iu,
      /\bscore\b/iu,
      /\bdistance\b/iu,
    ]) &&
    hasAny(difficultyBlock, [
      /\+=/iu,
      /math\.min/iu,
      /math\.max/iu,
      /\bdifficulty\s*=/iu,
      /\bspeed\s*=/iu,
    ])

  const hasRestartReset =
    hasAny(lower, [/\brestart/iu, /\bresetgame\b/iu, /\bstartover\b/iu]) &&
    hasAny(restartBlock, [
      /\bstart(?:x|y|z|position)\b/iu,
      /\bspawn(?:x|y|z|position)\b/iu,
      /\binitial(?:x|y|z|position)\b/iu,
      /\bplayer\.(?:x|y|z)\s*=\s*0\b/iu,
    ]) &&
    hasAny(restartBlock, [/\bprogress\s*=\s*0\b/iu, /\bscore\s*=\s*0\b/iu])

  const checks: ReadonlyArray<KhalaCodeRubricCheck> = [
    check(
      'single_html_file',
      hasSingleHtml,
      [`${evidencePrefix}:single_html_file`],
      'Expected one self-contained HTML file with no external script/style/media dependencies.',
    ),
    check(
      'loads_and_runs_headless',
      loadsAndRuns,
      [`${evidencePrefix}:headless_load_probe`],
      'Expected a runnable game surface with an animation/update loop and no obvious fatal marker.',
    ),
    check(
      'direction_controls',
      hasDirectionControls,
      [`${evidencePrefix}:keyboard_probe`],
      'Expected Arrow and WASD direction controls wired through a key handler.',
    ),
    check(
      'sane_follow_camera',
      hasSaneCamera,
      [`${evidencePrefix}:camera_probe`],
      'Expected explicit camera/viewport follow framing for the character.',
    ),
    check(
      'difficulty_ramps_with_progress',
      hasDifficultyRamp,
      [`${evidencePrefix}:difficulty_probe`],
      'Expected difficulty/speed/traffic to update from score, distance, or progress.',
    ),
    check(
      'restart_resets_character',
      hasRestartReset,
      [`${evidencePrefix}:restart_probe`],
      'Expected restart/reset to restore character position and progress or score.',
    ),
  ]

  const passedChecks = checks.filter(item => item.passed).map(item => item.id)
  const failedChecks = checks.filter(item => !item.passed).map(item => item.id)
  const scalarReward = passedChecks.length / checks.length
  const verified = failedChecks.length === 0
  const verification: KhalaCodeVerification = verified
    ? 'test_passed'
    : 'failed'

  return {
    artifact: {
      bytes: artifactText.length,
      fingerprint,
      kind: 'single_html',
    },
    checks,
    command: discoverKhalaCodeVerificationCommand(),
    failedChecks,
    passedChecks,
    receiptRef,
    reward: {
      handoffRef: khalaCodeAcceptedOutcomeHandoffRef(receiptRef),
      scalar: scalarReward,
    },
    rubricRef: KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
    scalarReward,
    sourceRefs: [
      KHALA_CODE_CROSSY_ROAD_RUBRIC_REF,
      KHALA_CODE_HEADLESS_COMMAND_REF,
      receiptRef,
      ...(input.meteringReceiptRef === undefined ||
      input.meteringReceiptRef === null
        ? []
        : [input.meteringReceiptRef]),
      `model:${input.servedModel}`,
      `worker:${input.worker}`,
    ],
    summary: verified
      ? 'Crossy-road single-file HTML artifact passed every deterministic rubric check.'
      : `Crossy-road artifact failed ${failedChecks.length} deterministic rubric check(s).`,
    verification,
    verified,
  }
}
