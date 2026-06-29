// Consumer-install platform-support classifier + claim guard.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.windows_wsl_consumer_install_coverage_missing
//
// The Episode 238 core promise is phrased "anybody can plug in consumer compute
// and get paid Bitcoin". Current install evidence is macOS/Linux only; native
// Windows and WSL are a DELIBERATE owner scope-out (apps/pylon/docs/platform-support.md,
// registry decision 2026-06-10). Per the promise verification text, the honest
// path for this blocker is NOT to build Windows support — it is to keep the
// public copy/claim narrowed to the platforms actually proven (macOS/Linux) and
// to prevent it drifting back to an unqualified "anybody on any platform" or a
// "Windows/WSL covered" claim.
//
// This module supplies that as a machine-checkable gate WITHOUT changing any
// live behavior:
//
//   - `classifyConsumerInstallPlatform` is a pure classifier giving a public-safe
//     disposition for a `NodeJS.Platform`: `supported` (darwin/linux) vs
//     `out-of-scope` (win32 and everything else), with honest guidance refs. It
//     mirrors the existing bootstrap supported-target set; it never emits machine
//     identifiers, paths, usernames, or any private material.
//   - `classifyConsumerInstallHost` extends that to WSL. WSL reports
//     `process.platform === "linux"`, so a platform-only check would mis-classify
//     a WSL host as `supported` — silently contradicting the documented
//     macOS/Linux-only scope-out. The host classifier takes an explicit WSL signal
//     (derived by `detectWslHost`) and classifies a WSL host `out-of-scope`.
//   - `verifyConsumerInstallPlatformClaim` audits an untrusted stated
//     platform-support claim and flags over-promises: a supported set that is not
//     exactly {darwin, linux}, any inclusion of windows/win32/wsl, a
//     windows-in-scope or wsl-in-scope flag, or an "any platform" claim. This is
//     the regression guard a reviewer runs so launch copy cannot silently claim
//     coverage the evidence does not support.
//
// Nothing here installs, probes a host, or flips a promise state.

import { isSupportedPlatform, type SupportedPlatform } from "./bootstrap.js"
// WSL detection lives in a dependency-free leaf module so it can be shared with
// `bootstrap.ts` (the runtime install path) without a circular import. Re-exported
// here to preserve this module's public surface.
import { detectWslHost, WSL_ENV_SIGNALS } from "./wsl-host-detect.js"

export { detectWslHost, WSL_ENV_SIGNALS }

export const WINDOWS_WSL_BLOCKER_REF =
  "blocker.product_promises.windows_wsl_consumer_install_coverage_missing" as const

// The exact, authoritative supported-target set for the self-serve consumer
// install path. Single source of truth shared with `bootstrap.isSupportedPlatform`.
export const CONSUMER_INSTALL_SUPPORTED_TARGETS: readonly SupportedPlatform[] = [
  "darwin",
  "linux",
] as const

export type ConsumerInstallPlatformDisposition = "supported" | "out-of-scope"

export type ConsumerInstallPlatformSupport = {
  schema: "openagents.pylon.consumer_install_platform_support.v0.1"
  platform: NodeJS.Platform
  disposition: ConsumerInstallPlatformDisposition
  supportedTargets: SupportedPlatform[]
  // Public-safe reason label for the disposition. Never a raw machine identifier.
  reasonRef: string
  // Public-safe next-step / documentation refs for an out-of-scope platform.
  guidanceRefs: string[]
  // The blocker this disposition relates to, present only when out-of-scope.
  blockerRefs: string[]
  contentRedacted: true
}

/**
 * Public-safe host signals for the self-serve consumer-install classifier.
 *
 * `platform` is the reported `NodeJS.Platform`. `wsl` is an explicit
 * "running under WSL" signal — WSL reports `platform === "linux"`, so it cannot
 * be inferred from `platform` alone; derive it with `detectWslHost`.
 */
export type ConsumerInstallHostSignals = {
  platform?: NodeJS.Platform
  wsl?: boolean
}

/**
 * Classify the self-serve consumer-install disposition for a host, accounting
 * for WSL.
 *
 * Pure and side-effect-free. A WSL host (`wsl === true`, which reports
 * `platform === "linux"`) is `out-of-scope` per the documented macOS/Linux-only
 * scope decision — even though its raw platform string is the supported `linux`.
 * Otherwise `supported` exactly when the bootstrap supported-target set covers
 * the platform (macOS/Linux); native Windows (`win32`) and everything else are
 * `out-of-scope` with honest guidance.
 */
export function classifyConsumerInstallHost(
  signals: ConsumerInstallHostSignals = {},
): ConsumerInstallPlatformSupport {
  const platform = signals.platform ?? process.platform
  const supportedTargets = [...CONSUMER_INSTALL_SUPPORTED_TARGETS]
  const base = {
    schema: "openagents.pylon.consumer_install_platform_support.v0.1",
    platform,
    supportedTargets,
    contentRedacted: true,
  } as const

  // WSL reports `platform === "linux"`; without this guard a platform-only check
  // would mis-classify a WSL host as `supported`, silently contradicting the
  // documented scope-out. The WSL signal is only meaningful on linux.
  if (signals.wsl === true && platform === "linux") {
    return {
      ...base,
      disposition: "out-of-scope",
      reasonRef: "reason.platform.wsl_out_of_scope",
      guidanceRefs: [
        "doc.pylon.platform_support",
        "guidance.platform.use_native_macos_or_linux_host_not_wsl",
      ],
      blockerRefs: [WINDOWS_WSL_BLOCKER_REF],
    }
  }

  if (isSupportedPlatform(platform)) {
    return {
      ...base,
      disposition: "supported",
      reasonRef: "reason.platform.supported_target",
      guidanceRefs: [],
      blockerRefs: [],
    }
  }

  if (platform === "win32") {
    return {
      ...base,
      disposition: "out-of-scope",
      reasonRef: "reason.platform.windows_out_of_scope",
      guidanceRefs: [
        "doc.pylon.platform_support",
        "guidance.platform.use_supported_macos_or_linux_host",
      ],
      blockerRefs: [WINDOWS_WSL_BLOCKER_REF],
    }
  }

  return {
    ...base,
    disposition: "out-of-scope",
    reasonRef: "reason.platform.unsupported_target",
    guidanceRefs: [
      "doc.pylon.platform_support",
      "guidance.platform.use_supported_macos_or_linux_host",
    ],
    blockerRefs: [WINDOWS_WSL_BLOCKER_REF],
  }
}

/**
 * Classify the self-serve consumer-install disposition for a `NodeJS.Platform`.
 *
 * Pure and side-effect-free. Thin wrapper over `classifyConsumerInstallHost`
 * with no WSL signal: a bare platform string cannot reveal WSL (WSL reports
 * `linux`), so callers that want WSL handling must use `classifyConsumerInstallHost`
 * with `detectWslHost`. `supported` exactly for macOS/Linux; native Windows
 * (`win32`) and everything else are `out-of-scope`.
 */
export function classifyConsumerInstallPlatform(
  platform: NodeJS.Platform = process.platform,
): ConsumerInstallPlatformSupport {
  return classifyConsumerInstallHost({ platform })
}

/**
 * A stated platform-support claim, e.g. parsed from launch copy or a docs
 * front-matter fixture, to be audited before publication.
 */
export type ConsumerInstallPlatformClaim = {
  schema: "openagents.pylon.consumer_install_platform_claim.v0.1"
  // The platforms the copy asserts are supported.
  supportedTargets: string[]
  // Whether the copy asserts native Windows is in scope.
  windowsInScope: boolean
  // Whether the copy asserts WSL is in scope.
  wslInScope: boolean
  // Whether the copy makes an unqualified "anybody on any platform" claim.
  anyPlatformClaimed: boolean
}

export type ConsumerInstallPlatformClaimVerification = {
  // The candidate is a well-formed claim object (correct schema/types, closed
  // key set).
  valid: boolean
  // The claim asserts coverage the macOS/Linux evidence does not support
  // (Windows, WSL, or "any platform"), or its supported set is not exactly
  // {darwin, linux}. True is the FAIL signal for launch copy.
  overpromises: boolean
  reasons: string[]
}

// Closed allowlist of claim keys. An unexpected key fails the audit so a copy
// fixture cannot smuggle an unreviewed assertion past the guard.
const ALLOWED_CLAIM_KEYS: ReadonlySet<string> = new Set<string>([
  "schema",
  "supportedTargets",
  "windowsInScope",
  "wslInScope",
  "anyPlatformClaimed",
])

// Tokens that, if present in a stated supported-target list, mean the copy is
// claiming Windows/WSL coverage that is deliberately out of scope.
const OUT_OF_SCOPE_TARGET_TOKENS: ReadonlySet<string> = new Set<string>([
  "win32",
  "windows",
  "wsl",
])

/**
 * Audit a stated platform-support claim for over-promising.
 *
 * Pure and side-effect-free. `valid` reflects well-formedness; `overpromises`
 * is the reviewer-facing fail signal — true when the claim asserts Windows, WSL,
 * or any-platform coverage, or when its supported set is not exactly the proven
 * {darwin, linux}. This does NOT clear the blocker; it keeps copy honest.
 */
export function verifyConsumerInstallPlatformClaim(
  candidate: unknown,
): ConsumerInstallPlatformClaimVerification {
  const reasons: string[] = []

  if (typeof candidate !== "object" || candidate === null || Array.isArray(candidate)) {
    return { valid: false, overpromises: true, reasons: ["not-an-object"] }
  }

  const record = candidate as Record<string, unknown>

  for (const key of Object.keys(record)) {
    if (!ALLOWED_CLAIM_KEYS.has(key)) {
      reasons.push(`unexpected-key:${key}`)
    }
  }

  if (record.schema !== "openagents.pylon.consumer_install_platform_claim.v0.1") {
    reasons.push("bad-schema")
  }

  const targets = record.supportedTargets
  let normalizedTargets: string[] | null = null
  if (!Array.isArray(targets) || !targets.every((t) => typeof t === "string")) {
    reasons.push("bad-supported-targets")
  } else {
    normalizedTargets = (targets as string[]).map((t) => t.toLowerCase())
  }

  for (const flag of ["windowsInScope", "wslInScope", "anyPlatformClaimed"] as const) {
    if (typeof record[flag] !== "boolean") {
      reasons.push(`bad-${flag}`)
    }
  }

  // Well-formedness decided before evaluating over-promise content.
  const valid = reasons.length === 0

  const overpromiseReasons: string[] = []

  if (normalizedTargets) {
    for (const token of normalizedTargets) {
      if (OUT_OF_SCOPE_TARGET_TOKENS.has(token)) {
        overpromiseReasons.push(`out-of-scope-target:${token}`)
      }
    }
    const expected = new Set<string>(CONSUMER_INSTALL_SUPPORTED_TARGETS)
    const actual = new Set<string>(normalizedTargets)
    const missing = [...expected].filter((t) => !actual.has(t))
    const extra = [...actual].filter(
      (t) => !expected.has(t) && !OUT_OF_SCOPE_TARGET_TOKENS.has(t),
    )
    for (const t of missing) overpromiseReasons.push(`missing-required-target:${t}`)
    for (const t of extra) overpromiseReasons.push(`unexpected-extra-target:${t}`)
  }

  if (record.windowsInScope === true) overpromiseReasons.push("windows-claimed-in-scope")
  if (record.wslInScope === true) overpromiseReasons.push("wsl-claimed-in-scope")
  if (record.anyPlatformClaimed === true) overpromiseReasons.push("any-platform-claimed")

  const overpromises = !valid || overpromiseReasons.length > 0

  return {
    valid,
    overpromises,
    reasons: [...reasons, ...overpromiseReasons],
  }
}

// ---------------------------------------------------------------------------
// Applied guard: bind the verifier to the ACTUAL shipped README copy.
//
// `verifyConsumerInstallPlatformClaim` above audits a structured claim, but a
// synthetic claim object cannot regress when someone edits the real consumer-
// facing copy. `auditReadmePlatformCopy` closes that gap: it derives the claim
// from the README text itself and runs the verifier, so a future copy edit that
// drops the narrowing sentence or reintroduces a Windows/WSL/any-platform claim
// fails in the suite — not just in a hypothetical fixture.
// ---------------------------------------------------------------------------

// The canonical, source-of-truth narrowing sentence the README must keep. Stored
// with single spaces; the audit normalizes whitespace before matching so a line
// wrap in the file does not defeat the check.
export const README_NARROWED_PLATFORM_SENTENCE =
  "Initial supported operator platforms are macOS and Linux. No other operator platforms are in scope for the first v1.0 launch path."

// Coverage verbs/phrases that, paired with a platform token, mean the copy is
// claiming that platform is supported. Matched in EITHER order relative to the
// platform token (see `coverageNear`), because the verb commonly appears BEFORE
// the platform ("works on Windows", "runs on WSL", "we support Windows") just as
// often as after it ("Windows is supported"). The earlier verb-after-only
// patterns silently let the verb-first phrasings drift through the guard.
const COVERAGE_VERB = "(?:supports?|supported|in scope|works?|runs?|covered)"

// Build a bidirectional detector for one platform token: the coverage verb may
// sit within 40 non-sentence-breaking chars before OR after the platform word.
function coverageNear(platformToken: string): RegExp {
  return new RegExp(
    `\\b${platformToken}\\b[^.\n]{0,40}\\b${COVERAGE_VERB}\\b` +
      `|\\b${COVERAGE_VERB}\\b[^.\n]{0,40}\\b${platformToken}\\b`,
    "i",
  )
}

// Public-safe over-promise phrase detectors. Each names a drift class and a
// pattern that, if it appears in the copy, means the README is claiming coverage
// the macOS/Linux evidence does not support.
export const OVERPROMISE_COPY_PATTERNS: ReadonlyArray<{
  ref: string
  pattern: RegExp
}> = [
  {
    ref: "any-platform-copy",
    // Catches "any/all/every/whatever platform(s)" and the obvious synonyms,
    // singular or plural — "runs on all platforms" is the same over-promise as
    // "runs on any platform" and was previously uncaught.
    pattern:
      /\b(?:any|all|every|whatever) (?:platforms?|os(?:es)?|machines?|computers?|devices?|laptops?)\b/i,
  },
  {
    ref: "windows-supported-copy",
    pattern: coverageNear("windows"),
  },
  {
    ref: "wsl-supported-copy",
    pattern: coverageNear("wsl"),
  },
]

export type ReadmePlatformCopyAudit = {
  schema: "openagents.pylon.readme_platform_copy_audit.v0.1"
  // The canonical narrowing sentence is present (whitespace-normalized).
  narrowedClaimPresent: boolean
  // Refs of any over-promise phrases detected in the copy.
  overpromisePhraseRefs: string[]
  // The structured claim derived from the actual copy.
  derivedClaim: ConsumerInstallPlatformClaim
  // The verifier verdict over the derived claim.
  claimVerification: ConsumerInstallPlatformClaimVerification
  // True only when the narrowing sentence is present, no over-promise phrase was
  // detected, and the derived claim does not over-promise. This is the
  // reviewer/CI signal that the shipped copy is still honest.
  copyHonest: boolean
}

/**
 * Audit the actual README platform copy for honesty.
 *
 * Pure and side-effect-free (the caller supplies the README text). Derives a
 * structured claim from the copy: the supported set stays the proven
 * `{darwin, linux}`, and each over-promise phrase flips the matching scope flag
 * so the existing verifier flags it. The README is honest only when the
 * narrowing sentence is present, no over-promise phrase matched, and the derived
 * claim does not over-promise.
 *
 * This does NOT clear the blocker or change any promise state; it locks the
 * owner copy-narrowing decision as an enforceable regression against real copy.
 */
export function auditReadmePlatformCopy(readmeText: string): ReadmePlatformCopyAudit {
  const normalized = readmeText.replace(/\s+/g, " ")
  const narrowedClaimPresent = normalized.includes(README_NARROWED_PLATFORM_SENTENCE)

  const overpromisePhraseRefs: string[] = []
  for (const { ref, pattern } of OVERPROMISE_COPY_PATTERNS) {
    if (pattern.test(readmeText)) overpromisePhraseRefs.push(ref)
  }

  const derivedClaim: ConsumerInstallPlatformClaim = {
    schema: "openagents.pylon.consumer_install_platform_claim.v0.1",
    supportedTargets: [...CONSUMER_INSTALL_SUPPORTED_TARGETS],
    windowsInScope: overpromisePhraseRefs.includes("windows-supported-copy"),
    wslInScope: overpromisePhraseRefs.includes("wsl-supported-copy"),
    anyPlatformClaimed: overpromisePhraseRefs.includes("any-platform-copy"),
  }

  const claimVerification = verifyConsumerInstallPlatformClaim(derivedClaim)

  return {
    schema: "openagents.pylon.readme_platform_copy_audit.v0.1",
    narrowedClaimPresent,
    overpromisePhraseRefs,
    derivedClaim,
    claimVerification,
    copyHonest:
      narrowedClaimPresent &&
      overpromisePhraseRefs.length === 0 &&
      !claimVerification.overpromises,
  }
}
