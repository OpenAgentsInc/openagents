#!/usr/bin/env node
import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Scripted desktop release publish flow (CUT-26, #8706).
 *
 * Takes one packaged artifact (the notarized `.dmg` or `.zip` from
 * `bun run make:mac`), produces the signed
 * `openagents.desktop.update_manifest.v1` + detached ed25519 signature, and
 * stages them into the dist-dir shape the deployed `apps/oa-updates`
 * serving seam consumes (`openagents-desktop-release.json` descriptor +
 * versioned manifest/signature files, seeded via
 * `OA_OPENAGENTS_DESKTOP_RELEASE_DIST` and served at
 * `/desktop/openagents/<channel>/manifest.json` / `manifest.sig.json` /
 * `release.json`). Version monotonicity and channel rules are enforced by
 * the landed update contract BEFORE anything is written; the signed
 * manifest is self-verified through the exact client verification seam
 * before it is staged.
 *
 * This script does NOT deploy and does NOT upload artifact bytes. The
 * artifact is served from a credential-free HTTPS URL (GCS) — the script
 * prints the exact upload + deploy commands as the documented next steps.
 *
 * SIGNING KEY SEAM (the only ways a key enters, per
 * apps/oa-updates/docs/release-signing-runbook.md):
 *   1) env  OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D  +
 *           OPENAGENTS_RELEASE_SIGNING_KID            (GCP mounts / CI / tests)
 *   2) env  OPENAGENTS_RELEASE_SECRETS_PATH — an env-format file holding the
 *           same two variables (the owner's local `.secrets` custody).
 * The key value is NEVER printed, logged, or embedded in errors, and this
 * script never shells out to fetch it. Tests use fixture keypairs only.
 */
import { mkdir, readFile, rename, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { basename, join, resolve } from "node:path"
import {
  PRODUCTION_RELEASE_KEY_PIN,
  updateChannels,
  type UpdateChannel,
} from "../src/update-contract.ts"
import {
  RELEASE_DESCRIPTOR_FILE,
  artifactExtension,
  computeDesktopReleasePublish,
  decodeReleaseDescriptor,
  decodeUpdateManifest,
  type ReleaseDescriptorEntry,
  type ReleaseSigningKey,
} from "../src/release-publish.ts"
import { UNSIGNED_DEV_MARKER, isUnsignedDevArtifactName } from "./macos-gatekeeper.ts"

interface Args {
  readonly channel: UpdateChannel
  readonly version: string
  readonly artifact: string
  readonly distDir: string
  readonly artifactUrl?: string
  readonly artifactBaseUrl: string
  readonly notesRef?: string
  readonly releasedAt: string
  readonly dryRun: boolean
}

const appRoot = resolve(import.meta.dirname, "..")
const defaultDistDir = resolve(appRoot, "../oa-updates/openagents-desktop-dist")
/** Public GCS mirror of gs://openagentsgemini-oa-updates (see DEPLOYMENT.md). */
const DEFAULT_ARTIFACT_BASE_URL =
  "https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/openagents-desktop/"

const usage = (): string =>
  [
    "Usage:",
    "  bun apps/openagents-desktop/scripts/publish-release.ts",
    "    --channel <stable|rc> --version <X.Y.Z[-rc.N]> --artifact <path/to/OpenAgents.dmg|.zip>",
    "    [--dist-dir <dir>] [--artifact-url <https-url> | --artifact-base-url <https-base>]",
    "    [--notes-ref <public-safe-ref>] [--released-at <ISO-8601 Z>] [--dry-run]",
    "",
    "Signing key: OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D + OPENAGENTS_RELEASE_SIGNING_KID",
    "in the environment, or OPENAGENTS_RELEASE_SECRETS_PATH pointing at an env file",
    "holding them (see apps/oa-updates/docs/release-signing-runbook.md).",
  ].join("\n")

const parseArgs = (argv: readonly string[]): Args => {
  const values = new Map<string, string>()
  const flags = new Set<string>()
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    if (key === "--dry-run") {
      flags.add("dry-run")
      continue
    }
    const value = argv[index + 1]
    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      throw new Error(usage())
    }
    values.set(key.slice(2), value)
    index += 1
  }

  const channel = values.get("channel")
  const version = values.get("version")
  const artifact = values.get("artifact")
  if (!channel || !version || !artifact) throw new Error(usage())
  if (!(updateChannels as readonly string[]).includes(channel)) {
    throw new Error(`--channel must be one of: ${updateChannels.join(", ")}`)
  }

  return {
    channel: channel as UpdateChannel,
    version,
    artifact: resolve(artifact),
    distDir: resolve(values.get("dist-dir") ?? defaultDistDir),
    ...(values.get("artifact-url") === undefined ? {} : { artifactUrl: values.get("artifact-url")! }),
    artifactBaseUrl: values.get("artifact-base-url") ?? DEFAULT_ARTIFACT_BASE_URL,
    ...(values.get("notes-ref") === undefined ? {} : { notesRef: values.get("notes-ref")! }),
    releasedAt: values.get("released-at") ?? new Date().toISOString(),
    dryRun: flags.has("dry-run"),
  }
}

const parseEnvFile = (text: string): Record<string, string> =>
  Object.fromEntries(
    text
      .split("\n")
      .filter((line) => line.length > 0 && !line.startsWith("#") && line.includes("="))
      .map((line) => {
        const separator = line.indexOf("=")
        return [line.slice(0, separator).trim(), line.slice(separator + 1).trim()]
      }),
  )

/** Load the signing key from the documented seam ONLY. Never printed. */
const loadSigningKey = async (): Promise<ReleaseSigningKey> => {
  const envD = process.env.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D
  const envKid = process.env.OPENAGENTS_RELEASE_SIGNING_KID
  if (envD !== undefined && envD.length > 0) {
    if (envKid === undefined || envKid.length === 0) {
      throw new Error(
        "OPENAGENTS_RELEASE_SIGNING_KID is required alongside OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D",
      )
    }
    return { d: envD, kid: envKid }
  }

  const secretsPath = process.env.OPENAGENTS_RELEASE_SECRETS_PATH
  if (secretsPath !== undefined && secretsPath.length > 0 && existsSync(secretsPath)) {
    const parsed = parseEnvFile(await readFile(secretsPath, "utf8"))
    const d = parsed.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D
    const kid = parsed.OPENAGENTS_RELEASE_SIGNING_KID
    if (d !== undefined && d.length > 0 && kid !== undefined && kid.length > 0) {
      return { d, kid }
    }
    throw new Error(
      "signing secrets file is missing OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D / OPENAGENTS_RELEASE_SIGNING_KID",
    )
  }

  throw new Error(
    "no release signing key: set OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D + OPENAGENTS_RELEASE_SIGNING_KID, " +
      "or OPENAGENTS_RELEASE_SECRETS_PATH (see apps/oa-updates/docs/release-signing-runbook.md)",
  )
}

const readJsonIfExists = async (path: string): Promise<unknown | null> => {
  try {
    return JSON.parse(await readFile(path, "utf8")) as unknown
  } catch (error) {
    if (typeof error === "object" && error !== null && "code" in error && error.code === "ENOENT") {
      return null
    }
    throw error
  }
}

/**
 * Resolve the current descriptor into (a) the entry for this channel (whose
 * manifest gates monotonicity) and (b) the preserved entries for the OTHER
 * channels. Any undecodable descriptor/manifest fails closed.
 */
const resolveExistingReleases = async (
  distDir: string,
  channel: UpdateChannel,
): Promise<{
  readonly existingManifest: unknown | null
  readonly preservedEntries: readonly ReleaseDescriptorEntry[]
}> => {
  const raw = await readJsonIfExists(join(distDir, RELEASE_DESCRIPTOR_FILE))
  if (raw === null) return { existingManifest: null, preservedEntries: [] }

  const descriptor = decodeReleaseDescriptor(raw)
  if (descriptor === null) {
    throw new Error(`existing ${RELEASE_DESCRIPTOR_FILE} does not decode; refusing to publish over it`)
  }

  let existingManifest: unknown | null = null
  const preservedEntries: ReleaseDescriptorEntry[] = []
  for (const entry of descriptor.releases) {
    const manifestRaw = JSON.parse(await readFile(join(distDir, entry.manifestPath), "utf8")) as unknown
    const manifest = decodeUpdateManifest(manifestRaw)
    if (manifest === null) {
      throw new Error(`existing manifest ${entry.manifestPath} does not decode; refusing to publish over it`)
    }
    if (manifest.channel === channel) {
      if (existingManifest !== null) {
        throw new Error(`existing descriptor has duplicate ${channel} entries; refusing to publish over it`)
      }
      existingManifest = manifestRaw
    } else {
      preservedEntries.push(entry)
    }
  }
  return { existingManifest, preservedEntries }
}

const main = async (): Promise<void> => {
  const args = parseArgs(Runtime.argv.slice(2))

  const artifactName = basename(args.artifact)
  const extension = artifactExtension(artifactName)
  if (extension === null) {
    throw new Error("--artifact must be a .dmg or .zip produced by the packaging lane")
  }
  // Gatekeeper release oracle (#8786): the -UNSIGNED-DEV escape valve exists
  // ONLY for local dev artifacts. Publishing one is refused unconditionally —
  // an unsigned outer artifact is Gatekeeper-dead on arrival
  // (docs/teardowns/2026-07-13-t3-code-teardown.md, T3 DMG incident).
  if (isUnsignedDevArtifactName(artifactName)) {
    throw new Error(
      `refusing to publish ${artifactName}: ${UNSIGNED_DEV_MARKER} artifacts are dev-only and never releasable ` +
        "(rebuild with the Developer ID identity + notary credentials; see docs/deploy/openagents-desktop-production-release.md)",
    )
  }
  const artifactBytes = new Uint8Array(await readFile(args.artifact))

  const key = await loadSigningKey()
  const { existingManifest, preservedEntries } = await resolveExistingReleases(
    args.distDir,
    args.channel,
  )

  const publish = computeDesktopReleasePublish({
    existingManifest,
    channel: args.channel,
    version: args.version,
    artifactName,
    artifactBytes,
    releasedAt: args.releasedAt,
    ...(args.notesRef === undefined ? {} : { notesRef: args.notesRef }),
    key,
    // Default object name is the version-bearing artifact file name under
    // the release bucket base. The URL is transport only — the client gates
    // the download on the SIGNED sha256/byteLength, never the URL.
    artifactUrl:
      args.artifactUrl ??
      new URL(
        artifactName,
        args.artifactBaseUrl.endsWith("/") ? args.artifactBaseUrl : `${args.artifactBaseUrl}/`,
      ).toString(),
  })

  const productionKey = publish.pin.kid === PRODUCTION_RELEASE_KEY_PIN.kid
  const summary = [
    `channel:        ${args.channel}`,
    `version:        ${publish.manifest.version}`,
    `artifact:       ${publish.manifest.artifactName}`,
    `artifactSha256: ${publish.manifest.artifactSha256}`,
    `artifactBytes:  ${publish.manifest.artifactByteLength}`,
    `artifactUrl:    ${publish.descriptorEntry.artifactUrl}`,
    `releasedAt:     ${publish.manifest.releasedAt}`,
    `signing kid:    ${publish.pin.kid}${productionKey ? " (PRODUCTION pin verified)" : " (non-production key)"}`,
  ].join("\n")

  if (args.dryRun) {
    console.log("dry run — nothing written")
    console.log(summary)
    return
  }

  await mkdir(args.distDir, { recursive: true })
  await writeFile(join(args.distDir, publish.manifestFileName), publish.payloadBytes)
  await writeFile(
    join(args.distDir, publish.signatureFileName),
    `${JSON.stringify(publish.envelope, null, 2)}\n`,
  )
  // Atomic descriptor replace: a crash mid-write can never truncate the
  // live descriptor. This channel's entry first, other channels preserved.
  const descriptor = { releases: [publish.descriptorEntry, ...preservedEntries] }
  const descriptorPath = join(args.distDir, RELEASE_DESCRIPTOR_FILE)
  const descriptorTmpPath = `${descriptorPath}.tmp-${process.pid}`
  await writeFile(descriptorTmpPath, `${JSON.stringify(descriptor, null, 2)}\n`)
  await rename(descriptorTmpPath, descriptorPath)

  console.log(`staged OpenAgents Desktop ${publish.manifest.version} for ${args.channel}`)
  console.log(summary)
  console.log("")
  console.log("Staged only — nothing deployed. Documented next steps:")
  console.log(`  1. upload the artifact bytes to the exact URL above, e.g.`)
  console.log(
    `     gsutil cp "${args.artifact}" "${gsUriForHttpsUrl(publish.descriptorEntry.artifactUrl) ?? "<matching gs:// URI>"}"`,
  )
  console.log("  2. deploy the feed service:")
  console.log("     OA_OPENAGENTS_DESKTOP_RELEASE_DIST=/app/openagents-desktop-dist \\")
  console.log("       bash apps/oa-updates/scripts/deploy-cloudrun.sh")
}

const gsUriForHttpsUrl = (url: string): string | null => {
  const match = url.match(/^https:\/\/storage\.googleapis\.com\/([^/]+)\/(.+)$/)
  return match === null ? null : `gs://${match[1]}/${match[2]}`
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
