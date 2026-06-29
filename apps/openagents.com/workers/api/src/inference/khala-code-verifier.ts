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
import {
  assessVerificationIntegrity,
  type VerificationChannelDefenses,
  type VerificationIntegrityReport,
  type VerificationPanelMember,
} from './verification-integrity'

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

// A single external library reference the pre-screen ALLOWED through (an
// allowlisted CDN host). `pinned` records whether it carries a concrete version
// token — we prefer pinned (a `@latest`/unversioned URL can change under us),
// but an allowlisted-yet-unpinned URL is allowed (execution is the authority).
export type KhalaCodeAllowedCdnLibrary = Readonly<{
  url: string
  host: string
  pinned: boolean
}>

export type KhalaCodePrescreen = Readonly<{
  // Whether the artifact is worth ATTEMPTING to execute (one self-contained HTML file
  // with a script + a game surface). This gates execution; it never verifies behavior.
  attemptExecution: boolean
  checks: ReadonlyArray<KhalaCodePrescreenCheck>
  html: string | undefined
  // The external library references the pre-screen allowed through (allowlisted
  // CDN hosts only). Empty for a fully self-contained artifact. Surfaced so the
  // receipt/runner can see exactly which pinned CDN libs an artifact pulled and
  // flag any that are unpinned.
  allowedCdnLibraries: ReadonlyArray<KhalaCodeAllowedCdnLibrary>
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
  integrity: VerificationIntegrityReport
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
  verifierPanel?: ReadonlyArray<VerificationPanelMember> | undefined
  channelDefenses?: VerificationChannelDefenses | undefined
  minimumEffectiveIndependentVotes?: number | undefined
  // The executed-acceptance verdict from the out-of-Worker headless runner, when
  // available. PRESENT => the verdict is derived from EXECUTION. ABSENT => honest
  // downgrade to `unverified` (we did not run it).
  acceptance?: AcceptanceVerdict | undefined
}>

// ALLOWLISTED CDN LIBRARY HOSTS (EPIC #6017 — pre-screen CDN allowance).
//
// WHY: the north-star prompt is "build a crossy road game WITH three.js". A
// FAITHFUL artifact loads three.js (and its standard addons) from a well-known
// CDN, so a blanket "any external <script src> => reject" gate fails a correct
// artifact at the cheap pre-screen and it never reaches the AUTHORITATIVE
// execution verifier. The fix is conservative: a small allowlist of pinned,
// well-known library CDN HOSTS for `<script src>` only — three.js + its
// addons load from these in practice. Everything else external is STILL
// rejected: an unknown/arbitrary script host, and ANY external stylesheet,
// image, audio, or video. The gate's real purpose (no random external assets)
// is preserved; the only relaxation is "a script tag pointing at a known
// library CDN is allowed through the pre-screen so the runner can execute it."
//
// This is NOT intent routing or retrieval selection — it is a bounded, exact,
// documented host allowlist over a static source gate (workspace semantic-routing
// rule §"Deterministic parsing is acceptable ... for bounded fields").
const ALLOWLISTED_CDN_HOSTS: ReadonlyArray<string> = [
  'unpkg.com',
  'cdn.jsdelivr.net',
  'cdnjs.cloudflare.com',
  'esm.sh',
  'esm.run',
  'ga.jspm.io',
]

// A host is allowlisted iff it exactly matches an entry or is a subdomain of one
// (e.g. `fastly.jsdelivr.net` for `cdn.jsdelivr.net`'s parent is NOT auto-allowed;
// only declared hosts and their subdomains). Case-insensitive.
const isAllowlistedCdnHost = (host: string): boolean => {
  const lower = host.toLowerCase()
  return ALLOWLISTED_CDN_HOSTS.some(
    allowed => lower === allowed || lower.endsWith(`.${allowed}`),
  )
}

// Heuristic "looks version-pinned" check over a CDN URL. We PREFER pinned
// versions (a stale `@latest` / unversioned path can silently change under us),
// so an allowlisted-but-unpinned URL is still allowed through the pre-screen
// (execution is the authority) but we record it. A URL is treated as pinned when
// it carries a concrete version token: `@1.2.3`, `/three.js/r128/`, `/0.160.0/`,
// `?v=...`, etc. Bounded + documented; never used to REJECT, only to annotate.
const looksVersionPinned = (url: string): boolean =>
  /@\d|\/(?:r?\d+(?:\.\d+)*)\//iu.test(url) || /[?&]v(?:er(?:sion)?)?=/iu.test(url)

// Pull the host out of an absolute or protocol-relative URL. Returns undefined
// for a relative/inline reference (which is never an EXTERNAL asset anyway).
const externalUrlHost = (url: string): string | undefined => {
  const normalized = url.startsWith('//') ? `https:${url}` : url
  try {
    return new URL(normalized).host
  } catch {
    return undefined
  }
}

// Match each `<script ... src="URL">` (classic script tag). `src` may be quoted
// or unquoted; the URL is captured for host classification.
const scriptSrcUrlPattern =
  /<script\b[^>]*\bsrc\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s">]+))/giu

// Any EXTERNAL stylesheet / image / audio / video, or an external `<link>` of
// any rel — NONE of these are allowlisted (the allowance is for library scripts
// only). Protocol-relative or absolute http(s).
const externalNonScriptAssetPattern =
  /<(?:link|img|audio|video)\b[^>]*(?:src|href)\s*=\s*["'](?:https?:)?\/\//iu

// An ES-module / importmap reference to an external URL inside a `<script>`
// body, e.g. `import * as THREE from 'https://.../three.module.js'` or an
// `<script type="importmap">{ "imports": { "three": "https://..." } }`. These
// are classified the same way as a classic `src` (allowlisted CDN host => ok).
const moduleImportUrlPattern =
  /(?:\bimport\b[^'"]*?|["'])((?:https?:)?\/\/[^\s'"]+)/giu

// Collect every EXTERNAL (protocol-relative/absolute) script `src` + module /
// importmap URL referenced by the HTML. Bare specifiers and relative paths are
// not external and are ignored.
const collectExternalScriptUrls = (html: string): ReadonlyArray<string> => {
  const urls: Array<string> = []

  for (const match of html.matchAll(scriptSrcUrlPattern)) {
    const url = match[1] ?? match[2] ?? match[3]
    if (url !== undefined && /^(?:https?:)?\/\//iu.test(url)) {
      urls.push(url)
    }
  }

  // Module imports / importmap URLs inside <script> bodies (e.g. esm.sh /
  // jsdelivr `three.module.js`).
  for (const match of html.matchAll(moduleImportUrlPattern)) {
    const url = match[1]
    if (url !== undefined && /^(?:https?:)?\/\//iu.test(url)) {
      urls.push(url)
    }
  }

  return urls
}

// Classify the external script/module URLs into (a) whether EVERY one resolves
// to an allowlisted CDN host — false on the FIRST non-allowlisted URL, so an
// unknown CDN / random script host is rejected — and (b) the allowed library
// references (host + pinned annotation) for the receipt/runner.
const classifyExternalScripts = (
  html: string,
): Readonly<{
  allAllowlisted: boolean
  allowed: ReadonlyArray<KhalaCodeAllowedCdnLibrary>
}> => {
  // Dedupe URLs — a classic `<script src>` URL is also caught as a quoted string
  // by the module-import pattern, so the same URL can appear twice.
  const urls = Array.from(new Set(collectExternalScriptUrls(html)))
  const allowed: Array<KhalaCodeAllowedCdnLibrary> = []
  let allAllowlisted = true

  for (const url of urls) {
    const host = externalUrlHost(url)
    if (host !== undefined && isAllowlistedCdnHost(host)) {
      allowed.push({ host, pinned: looksVersionPinned(url), url })
    } else {
      allAllowlisted = false
    }
  }

  return { allAllowlisted, allowed }
}

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

  // Classify external script/module refs once: are they ALL allowlisted CDN
  // libraries, and which allowed library refs (host + pinned) did we see?
  const externalScripts =
    html === undefined
      ? { allAllowlisted: true, allowed: [] as ReadonlyArray<KhalaCodeAllowedCdnLibrary> }
      : classifyExternalScripts(html)

  // SINGLE-FILE GATE (with the pinned-CDN-library allowance, EPIC #6017):
  //   - one `<html>...</html>` document, AND
  //   - NO external stylesheet/image/audio/video and NO external `<link>` of any
  //     kind (those must be inlined — the gate's real purpose), AND
  //   - every external `<script src>` / module-import URL resolves to an
  //     allowlisted, well-known library CDN host (three.js + addons load from
  //     these). An UNKNOWN/arbitrary external script host fails the gate.
  // A purely self-contained artifact (no external refs at all) still passes, as
  // before — `classifyExternalScripts` is vacuously `allAllowlisted` with no URLs.
  const hasSingleHtml =
    html !== undefined &&
    /<html\b/iu.test(html) &&
    /<\/html>/iu.test(html) &&
    !externalNonScriptAssetPattern.test(html) &&
    externalScripts.allAllowlisted

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
      label:
        'The delivered artifact is one self-contained HTML file (pinned, well-known CDN library scripts allowed).',
      passed: hasSingleHtml,
      ...(hasSingleHtml
        ? {}
        : {
            failureReason:
              'Expected one self-contained HTML file: inline all styles/media, and load scripts inline or from a pinned, well-known CDN (three.js + addons via unpkg/jsdelivr/cdnjs/esm.sh). No arbitrary external scripts, stylesheets, images, audio, or video.',
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
    allowedCdnLibraries: externalScripts.allowed,
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
  const integrity = assessVerificationIntegrity({
    channelDefenses: input.channelDefenses,
    minimumEffectiveIndependentVotes: input.minimumEffectiveIndependentVotes,
    panel: input.verifierPanel,
    workerModel: input.servedModel,
  })

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
  } else if (!integrity.passed) {
    verification = 'failed'
    verified = false
    scalarReward = 0
    executed = acceptance?.executed === true
    passedChecks = []
    failedChecks = integrity.blockerRefs
    rubricRef = KHALA_CODE_CROSSY_ROAD_RUBRIC_REF
    summary = `Verification integrity failed: ${integrity.blockerRefs.join(', ')}.`
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
    integrity,
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
      `verifier_independence:${integrity.effectiveIndependentVotes}`,
      ...integrity.blockerRefs,
    ],
    summary,
    verification,
    verified,
  }
}
