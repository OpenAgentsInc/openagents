/**
 * macOS Gatekeeper release oracles (DMG-1, #8786).
 *
 * Two live 2026-07-13 incidents define the failure classes these oracles
 * refuse (both cited in GUARANTEES.md):
 *
 * - T3 Code shipped a fully-notarized app inside an UNSIGNED, un-notarized
 *   DMG. macOS assesses the outermost quarantined artifact, so Gatekeeper
 *   showed the "damaged" dialog and the correct app inside was unreachable
 *   (`docs/teardowns/2026-07-13-t3-code-teardown.md`, night addendum:
 *   installed-artifact verification).
 * - ChatGPT's updater swapped a working app for one the machine refused to
 *   exec and never noticed
 *   (`docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md`).
 *
 * The rule encoded here, as oracles rather than checklist prose: notarize
 * the DMG itself (which covers the nested app), staple the ticket to BOTH
 * the `.dmg` and the `.app`, then gate release on
 *   codesign --verify --deep --strict            (app)
 *   spctl -a -t open --context context:primary-signature  (image)
 *   spctl -a -t exec                              (app)
 *   xcrun stapler validate                        (both)
 * and REFUSE the release lane entirely when the Developer ID identity or
 * notary credentials are absent — no unsigned fallback. The only escape
 * valve is explicit (`--allow-unsigned-dev` / `OA_ALLOW_UNSIGNED_DEV=1`)
 * and it renames the artifact `-UNSIGNED-DEV` so it can never be mistaken
 * for (or published as) a release.
 *
 * Every verdict is a PURE function over a recorded `CommandObservation`, so
 * the full oracle set is unit-testable against fixture outputs without the
 * owner's signing credentials (`tests/macos-gatekeeper.test.ts`). The thin
 * runners below gather real observations on a release machine. This module
 * is runtime-neutral (node:child_process only — it is imported by both the
 * Bun preflight CLI and the jiti-loaded `forge.config.ts`).
 */
import { spawnSync } from "node:child_process"
import { rmSync } from "node:fs"
import path from "node:path"

// ---------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------

/** A recorded command run — the ONLY input the pure interpreters see. */
export interface CommandObservation {
  readonly exitCode: number | null
  readonly stdout: string
  readonly stderr: string
}

export interface GatekeeperCheck {
  readonly id: string
  readonly ok: boolean
  readonly detail: string
}

const boundedDetail = (text: string): string => {
  const flat = text.replace(/\s+/gu, " ").trim()
  return flat.length <= 220 ? flat : `${flat.slice(0, 217)}...`
}

const combinedOutput = (observation: CommandObservation): string =>
  `${observation.stdout}\n${observation.stderr}`

// ---------------------------------------------------------------------------
// Pure interpreters — fail closed on ANYTHING but the exact green shape
// ---------------------------------------------------------------------------

/**
 * `codesign --verify --deep --strict --verbose=2 <app>` — green requires
 * exit 0 AND both success lines. An unsigned bundle reports "code object is
 * not signed at all" (the exact T3 DMG-container failure signature).
 */
export const interpretCodesignDeepStrict = (
  observation: CommandObservation,
  subject: string,
): GatekeeperCheck => {
  const output = combinedOutput(observation)
  const ok =
    observation.exitCode === 0 &&
    output.includes("valid on disk") &&
    output.includes("satisfies its Designated Requirement")
  return {
    id: "gatekeeper_codesign_deep_strict",
    ok,
    detail: ok
      ? `${path.basename(subject)}: deep-strict signature valid on disk`
      : boundedDetail(`codesign --verify --deep --strict failed (exit ${String(observation.exitCode)}): ${output}`),
  }
}

/**
 * `spctl -a -t open --context context:primary-signature -vv <dmg>` /
 * `spctl -a -t exec -vv <app>` — green requires exit 0, an explicit
 * `accepted`, AND a Notarized Developer ID source. A signed-but-unnotarized
 * artifact is still a Gatekeeper block on current macOS, so mere acceptance
 * by a non-notarized source is a RED row here.
 */
export const interpretSpctlAssessment = (
  observation: CommandObservation,
  kind: "open_image" | "exec_app",
  subject: string,
): GatekeeperCheck => {
  const output = combinedOutput(observation)
  const accepted = observation.exitCode === 0 && /(^|\s|:)accepted\b/u.test(output)
  const notarized = /source=Notarized Developer ID/u.test(output)
  const ok = accepted && notarized
  const id = kind === "open_image" ? "gatekeeper_spctl_image_notarized" : "gatekeeper_spctl_exec_notarized"
  return {
    id,
    ok,
    detail: ok
      ? `${path.basename(subject)}: spctl accepted (Notarized Developer ID)`
      : boundedDetail(
          accepted
            ? `spctl accepted ${subject} but not as Notarized Developer ID (fail closed): ${output}`
            : `spctl rejected ${subject} (exit ${String(observation.exitCode)}): ${output}`,
        ),
  }
}

/**
 * `xcrun stapler validate <path>` — green requires exit 0 and the explicit
 * success line, proving the notarization ticket travels WITH the bytes
 * (offline Gatekeeper assessment; no notary round trip on first launch).
 */
export const interpretStaplerValidate = (
  observation: CommandObservation,
  kind: "dmg" | "app",
  subject: string,
): GatekeeperCheck => {
  const output = combinedOutput(observation)
  const ok = observation.exitCode === 0 && output.includes("The validate action worked")
  return {
    id: kind === "dmg" ? "gatekeeper_stapler_dmg" : "gatekeeper_stapler_app",
    ok,
    detail: ok
      ? `${path.basename(subject)}: notarization ticket stapled`
      : boundedDetail(`stapler validate failed for ${subject} (exit ${String(observation.exitCode)}): ${output}`),
  }
}

// ---------------------------------------------------------------------------
// Credentials — the release lane refuses without them (no unsigned fallback)
// ---------------------------------------------------------------------------

export interface MacSigningCredentials {
  readonly developerIdApplication: string | undefined
  readonly ascApiPrivateKeyPath: string | undefined
  readonly ascApiKeyId: string | undefined
  readonly ascApiIssuerId: string | undefined
}

export const readMacSigningCredentials = (
  env: Record<string, string | undefined> = process.env,
): MacSigningCredentials => ({
  developerIdApplication: env.OA_DEVELOPER_ID_APPLICATION,
  ascApiPrivateKeyPath: env.ASC_API_PRIVATE_KEY_PATH,
  ascApiKeyId: env.ASC_API_KEY_ID,
  ascApiIssuerId: env.ASC_API_ISSUER_ID,
})

export const missingCredentialNames = (
  credentials: MacSigningCredentials,
): ReadonlyArray<string> => {
  const missing: Array<string> = []
  if (!credentials.developerIdApplication) missing.push("OA_DEVELOPER_ID_APPLICATION")
  if (!credentials.ascApiPrivateKeyPath) missing.push("ASC_API_PRIVATE_KEY_PATH")
  if (!credentials.ascApiKeyId) missing.push("ASC_API_KEY_ID")
  if (!credentials.ascApiIssuerId) missing.push("ASC_API_ISSUER_ID")
  return missing
}

/**
 * Fail-closed credentials oracle: the release lane REFUSES when the
 * Developer ID identity or notary credentials are absent. The only bypass
 * is the explicit dev escape valve, which is honest about producing a
 * non-releasable `-UNSIGNED-DEV` artifact.
 */
export const checkSigningCredentialsPresent = (
  credentials: MacSigningCredentials,
  allowUnsignedDev: boolean,
): GatekeeperCheck => {
  const missing = missingCredentialNames(credentials)
  if (missing.length === 0) {
    return {
      id: "signing_credentials_present",
      ok: true,
      detail: "Developer ID identity and notary credentials present",
    }
  }
  if (allowUnsignedDev) {
    return {
      id: "signing_credentials_present",
      ok: true,
      detail:
        `UNSIGNED-DEV escape valve engaged (missing ${missing.join(", ")}) — ` +
        "output is a dev-only artifact renamed -UNSIGNED-DEV and can NEVER be published",
    }
  }
  return {
    id: "signing_credentials_present",
    ok: false,
    detail:
      `release lane REFUSED: missing ${missing.join(", ")} — there is no unsigned release fallback ` +
      "(T3 Gatekeeper-dead DMG, docs/teardowns/2026-07-13-t3-code-teardown.md). " +
      "Pass --allow-unsigned-dev (preflight) / OA_ALLOW_UNSIGNED_DEV=1 (make) for a dev-only -UNSIGNED-DEV artifact.",
  }
}

// ---------------------------------------------------------------------------
// UNSIGNED-DEV artifact naming — dev output can never impersonate a release
// ---------------------------------------------------------------------------

export const UNSIGNED_DEV_MARKER = "-UNSIGNED-DEV"

export const unsignedDevArtifactName = (fileName: string): string => {
  if (fileName.includes(UNSIGNED_DEV_MARKER)) return fileName
  const dot = fileName.lastIndexOf(".")
  return dot <= 0
    ? `${fileName}${UNSIGNED_DEV_MARKER}`
    : `${fileName.slice(0, dot)}${UNSIGNED_DEV_MARKER}${fileName.slice(dot)}`
}

export const isUnsignedDevArtifactName = (fileName: string): boolean =>
  fileName.includes(UNSIGNED_DEV_MARKER)

/** An `-UNSIGNED-DEV` artifact NEVER passes release preflight or publish. */
export const checkArtifactNotUnsignedDev = (fileName: string): GatekeeperCheck => {
  const unsignedDev = isUnsignedDevArtifactName(path.basename(fileName))
  return {
    id: "artifact_not_unsigned_dev",
    ok: !unsignedDev,
    detail: unsignedDev
      ? `${path.basename(fileName)} is an -UNSIGNED-DEV dev artifact — refused for release unconditionally`
      : `${path.basename(fileName)} carries no dev-escape marker`,
  }
}

// ---------------------------------------------------------------------------
// Runners — thin observation gatherers around the pure interpreters
// ---------------------------------------------------------------------------

export const observeCommand = (
  command: string,
  args: ReadonlyArray<string>,
): CommandObservation => {
  const result = spawnSync(command, [...args], { encoding: "utf8" })
  if (result.error !== undefined) {
    return { exitCode: null, stdout: "", stderr: String(result.error) }
  }
  return { exitCode: result.status, stdout: result.stdout ?? "", stderr: result.stderr ?? "" }
}

/** The three app-side Gatekeeper oracles against a real `.app` bundle. */
export const gatekeeperAppChecks = (appPath: string): ReadonlyArray<GatekeeperCheck> => [
  interpretCodesignDeepStrict(
    observeCommand("codesign", ["--verify", "--deep", "--strict", "--verbose=2", appPath]),
    appPath,
  ),
  interpretSpctlAssessment(
    observeCommand("spctl", ["-a", "-t", "exec", "-vv", appPath]),
    "exec_app",
    appPath,
  ),
  interpretStaplerValidate(observeCommand("xcrun", ["stapler", "validate", appPath]), "app", appPath),
]

/** The two image-side Gatekeeper oracles against a real `.dmg`. */
export const gatekeeperImageChecks = (dmgPath: string): ReadonlyArray<GatekeeperCheck> => [
  interpretSpctlAssessment(
    observeCommand("spctl", ["-a", "-t", "open", "--context", "context:primary-signature", "-vv", dmgPath]),
    "open_image",
    dmgPath,
  ),
  interpretStaplerValidate(observeCommand("xcrun", ["stapler", "validate", dmgPath]), "dmg", dmgPath),
]

// ---------------------------------------------------------------------------
// Notarize + staple the DMG itself (make pipeline; covers the nested app)
// ---------------------------------------------------------------------------

export interface NotaryCredentials {
  readonly appleApiKey: string
  readonly appleApiKeyId: string
  readonly appleApiIssuer: string
}

const runInherit = (command: string, args: ReadonlyArray<string>, label: string): void => {
  const result = spawnSync(command, [...args], { stdio: "inherit" })
  if (result.error !== undefined || result.status !== 0) {
    throw new Error(`${label} failed (exit ${String(result.status)}${result.error ? `; ${String(result.error)}` : ""})`)
  }
}

/**
 * Notarize and staple the signed app BEFORE any maker snapshots it. A ticket
 * stapled to the out/ app after MakerDMG runs does not alter the app already
 * captured inside the immutable DMG.
 */
export const notarizeAndStapleApp = (
  appPath: string,
  credentials: NotaryCredentials,
): void => {
  const archivePath = `${appPath}.notarization.zip`
  rmSync(archivePath, { force: true })
  try {
    runInherit("/usr/bin/ditto", ["-c", "-k", "--keepParent", appPath, archivePath], `archive ${path.basename(appPath)} for notarization`)
    runInherit(
      "xcrun",
      [
        "notarytool",
        "submit",
        archivePath,
        "--key",
        credentials.appleApiKey,
        "--key-id",
        credentials.appleApiKeyId,
        "--issuer",
        credentials.appleApiIssuer,
        "--wait",
      ],
      `notarytool submit ${path.basename(appPath)}`,
    )
    runInherit("xcrun", ["stapler", "staple", appPath], `stapler staple ${path.basename(appPath)}`)
  } finally {
    rmSync(archivePath, { force: true })
  }
}

/** Submit and staple the finished DMG after it captured the stapled app. */
export const notarizeAndStapleDmg = (
  dmgPath: string,
  credentials: NotaryCredentials,
): void => {
  runInherit(
    "xcrun",
    [
      "notarytool",
      "submit",
      dmgPath,
      "--key",
      credentials.appleApiKey,
      "--key-id",
      credentials.appleApiKeyId,
      "--issuer",
      credentials.appleApiIssuer,
      "--wait",
    ],
    `notarytool submit ${path.basename(dmgPath)}`,
  )
  runInherit("xcrun", ["stapler", "staple", dmgPath], `stapler staple ${path.basename(dmgPath)}`)
}

/** Throw (fail the make/publish lane closed) when any oracle row is red. */
export const assertGatekeeperGreen = (checks: ReadonlyArray<GatekeeperCheck>): void => {
  const failures = checks.filter((check) => !check.ok)
  if (failures.length > 0) {
    throw new Error(
      `Gatekeeper oracle RED: ${failures.map((check) => `${check.id}: ${check.detail}`).join("; ")}`,
    )
  }
}
