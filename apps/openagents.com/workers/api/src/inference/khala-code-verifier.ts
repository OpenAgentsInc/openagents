// Khala coding-lane verifier substrate (#6010, EPIC #6017).
//
// HONEST DOWNGRADE (docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md):
// This module USED to return `verification: 'test_passed'` / `verified: true` /
// `scalarReward: 1` from REGEX over the HTML source — it never ran the artifact. That
// certified a crossy-road game that crashed on load, had a dead PLAY button, a 100x
// camera, and stopped generating world. `verified:true` must mean "we ran it and it
// did what the user asked." So:
//
//   - The regex checks remain ONLY as a cheap PRE-SCREEN: a gate to decide whether the
//     artifact is even worth attempting to execute (is it one self-contained HTML file
//     with a script + game surface?). They are NOT the verification verdict.
//   - With no executed-acceptance result, the verdict is `unverified`, `verified:false`,
//     `scalarReward:0`. Better to say "we didn't run it" than to falsely certify.
//   - When the out-of-Worker headless acceptance runner (`acceptance-runner/`) HAS run
//     the artifact, the route passes its `AcceptanceVerdict` in and the verdict is
//     derived from EXECUTION: `verified` = all acceptance tests passed; `scalarReward` =
//     fraction passing.
//
// The hot Worker route cannot launch a browser, so the actual execution happens out of
// the Worker (a sandbox / Pylon) via the runner; this module stays pure + receipt-shaped
// and accepts the runner's verdict.

import type { AcceptanceVerdict } from './acceptance-runner/verdict'

export const KHALA_CODE_CROSSY_ROAD_RUBRIC_REF =
  'rubric.khala_code.crossy_road.single_html.v1'

export const KHALA_CODE_HEADLESS_COMMAND_REF =
  'command.khala_code.crossy_road.headless_html_probe.v1'

export const KHALA_CODE_VERIFIER_WORKER_ID = 'khala-code-crossy-road-verifier'

// The verification verdict states. `unverified` is the HONEST default for an
// executable artifact we have NOT actually run yet (the prescreen passed but no runner
// executed it). `test_passed` is reserved for an EXECUTED acceptance suite that fully
// passed. `failed` is an executed suite that did not fully pass, or an artifact that
// failed the prescreen (not even worth attempting to run).
export type KhalaCodeVerification = 'test_passed' | 'unverified' | 'failed'

// The prescreen check ids — a cheap source gate, NOT the verification verdict.
export type KhalaCodePrescreenCheckId =
  | 'single_html_file'
  | 'has_runnable_surface'

export type KhalaCodePrescreenCheck = Readonly<{
  id: KhalaCodePrescreenCheckId
  label: string
  passed: boolean
  failureReason?: string | undefined
}>

export type KhalaCodePrescreen = Readonly<{
  // Whether the artifact is worth ATTEMPTING to execute (one self-contained HTML file
  // with a script + a game surface). This gates execution; it never verifies behavior.
  attemptExecution: boolean
  checks: ReadonlyArray<KhalaCodePrescreenCheck>
  html: string | undefined
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
  // The cheap source pre-screen (gate-to-attempt only).
  prescreen: KhalaCodePrescreen
  command: KhalaCodeVerificationCommand
  // Per-acceptance-check ids when an executed suite ran; otherwise empty.
  failedChecks: ReadonlyArray<string>
  passedChecks: ReadonlyArray<string>
  // Whether a real headless acceptance run produced this verdict.
  executed: boolean
  receiptRef: string
  reward: Readonly<{
    handoffRef: string
    scalar: number
  }>
  rubricRef: string
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
  // The executed-acceptance verdict from the out-of-Worker headless runner, when
  // available. PRESENT => the verdict is derived from EXECUTION. ABSENT => honest
  // downgrade to `unverified` (we did not run it).
  acceptance?: AcceptanceVerdict | undefined
}>

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

// CHEAP PRE-SCREEN ONLY. Decides whether an artifact is worth ATTEMPTING to execute:
// is it one self-contained HTML file with a script + a plausible game surface and no
// obvious fatal marker? This NEVER verifies behavior — it only gates execution.
export const prescreenKhalaCodeArtifact = (
  content: string,
): KhalaCodePrescreen => {
  const html = extractSingleHtmlArtifact(content)
  const artifactText = html ?? content.trim()
  const lower = artifactText.toLowerCase()

  const hasSingleHtml =
    html !== undefined &&
    /<html\b/iu.test(html) &&
    /<\/html>/iu.test(html) &&
    !externalAssetPattern.test(html) &&
    !scriptSrcPattern.test(html) &&
    !linkedStylesheetPattern.test(html)

  const hasRunnableSurface =
    hasSingleHtml &&
    /<script\b/iu.test(artifactText) &&
    (/<canvas\b/iu.test(lower) ||
      /\bid=["']game["']/iu.test(lower) ||
      /\bgame\b/iu.test(lower)) &&
    !/throw\s+new\s+error/iu.test(lower) &&
    !/todo:\s*not\s+implemented/iu.test(lower)

  const checks: ReadonlyArray<KhalaCodePrescreenCheck> = [
    {
      id: 'single_html_file',
      label: 'The delivered artifact is one self-contained HTML file.',
      passed: hasSingleHtml,
      ...(hasSingleHtml
        ? {}
        : {
            failureReason:
              'Expected one self-contained HTML file with no external script/style/media dependencies.',
          }),
    },
    {
      id: 'has_runnable_surface',
      label:
        'The artifact has a script and a plausible game surface to execute.',
      passed: hasRunnableSurface,
      ...(hasRunnableSurface
        ? {}
        : {
            failureReason:
              'Expected a <script> plus a canvas/game surface and no obvious fatal marker.',
          }),
    },
  ]

  return {
    attemptExecution: hasSingleHtml && hasRunnableSurface,
    checks,
    html,
  }
}

export const verifyKhalaCodeCompletion = (
  input: KhalaCodeVerifierInput,
): KhalaCodeVerificationVerdict => {
  const prescreen = prescreenKhalaCodeArtifact(input.content)
  const artifactText = prescreen.html ?? input.content.trim()
  const fingerprint = stableFingerprint(artifactText)
  const receiptRef = khalaCodeVerificationReceiptRef({
    artifactFingerprint: fingerprint,
    requestId: input.requestId,
  })

  const acceptance = input.acceptance

  // VERDICT DERIVATION.
  //   - Prescreen failed => `failed` (not even worth running). scalarReward 0.
  //   - Executed acceptance present => derive from EXECUTION.
  //   - Otherwise => HONEST DOWNGRADE to `unverified` (we did not run it).
  let verification: KhalaCodeVerification
  let verified: boolean
  let scalarReward: number
  let executed: boolean
  let passedChecks: ReadonlyArray<string>
  let failedChecks: ReadonlyArray<string>
  let rubricRef: string
  let summary: string

  if (!prescreen.attemptExecution) {
    verification = 'failed'
    verified = false
    scalarReward = 0
    executed = false
    passedChecks = []
    failedChecks = prescreen.checks
      .filter(check => !check.passed)
      .map(check => check.id)
    rubricRef = KHALA_CODE_CROSSY_ROAD_RUBRIC_REF
    summary =
      'Artifact failed the cheap pre-screen (not a runnable single-file HTML game); not executed.'
  } else if (acceptance !== undefined && acceptance.executed) {
    verified = acceptance.verified
    scalarReward = acceptance.scalarReward
    verification = verified ? 'test_passed' : 'failed'
    executed = true
    passedChecks = acceptance.passedChecks
    failedChecks = acceptance.failedChecks
    rubricRef = acceptance.rubricRef
    summary = verified
      ? 'Crossy-road artifact PASSED every executed acceptance check (ran in a real headless browser).'
      : `Crossy-road artifact FAILED ${acceptance.failedChecks.length} executed acceptance check(s).`
  } else {
    // HONEST DOWNGRADE: prescreen passed, but no execution happened.
    verification = 'unverified'
    verified = false
    scalarReward = 0
    executed = false
    passedChecks = []
    failedChecks = []
    rubricRef = KHALA_CODE_CROSSY_ROAD_RUBRIC_REF
    summary =
      'Artifact passed the pre-screen but was NOT executed; verification is pending the headless acceptance runner. Not certified.'
  }

  return {
    artifact: {
      bytes: artifactText.length,
      fingerprint,
      kind: 'single_html',
    },
    command: discoverKhalaCodeVerificationCommand(),
    executed,
    failedChecks,
    passedChecks,
    prescreen,
    receiptRef,
    reward: {
      handoffRef: khalaCodeAcceptedOutcomeHandoffRef(receiptRef),
      scalar: scalarReward,
    },
    rubricRef,
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
    summary,
    verification,
    verified,
  }
}
