#!/usr/bin/env bun
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { basename, extname, join, resolve } from "node:path"
import { tmpdir } from "node:os"

import { assetKeyFromBytes } from "../src/asset-store.ts"
import {
  DEFAULT_DESKTOP_ARTIFACT_CONTENT_TYPE,
  DEFAULT_DESKTOP_RELEASE_PRODUCT,
  normalizeDesktopReleaseProduct,
  normalizeDesktopReleaseSeed,
  sha256Hex,
  type DesktopReleaseProduct,
  type DesktopReleaseSeed,
} from "../src/desktop-release.ts"

type Args = {
  readonly product: DesktopReleaseProduct
  readonly channel: string
  readonly version: string
  readonly artifact: string
  readonly previousVersion?: string
  readonly previousArtifact?: string
  readonly outDir: string
}

const repoRoot = resolve(import.meta.dir, "../../..")
const defaultOutDir = join(repoRoot, "apps/oa-updates/desktop-dist")

async function main(): Promise<void> {
  const args = parseArgs(Bun.argv.slice(2))
  const artifactBytes = await readFile(args.artifact)
  const artifactHash = assetKeyFromBytes(new Uint8Array(artifactBytes))
  const artifactPath = `assets/${artifactHash}${normalizedExt(args.artifact)}`
  let bsdiffPath: string | undefined
  let bsdiffBytes: Uint8Array | undefined

  await mkdir(join(args.outDir, "assets"), { recursive: true })
  await writeFile(join(args.outDir, artifactPath), artifactBytes)

  if (args.previousArtifact !== undefined) {
    const patchPath = join(
      tmpdir(),
      `openagents-${args.previousVersion}-${args.version}-${Date.now()}.bsdiff`,
    )
    await runBsdiff(args.previousArtifact, args.artifact, patchPath)
    bsdiffBytes = new Uint8Array(await readFile(patchPath))
    const bsdiffHash = assetKeyFromBytes(bsdiffBytes)
    bsdiffPath = `assets/${bsdiffHash}.bsdiff`
    await writeFile(join(args.outDir, bsdiffPath), bsdiffBytes)
  }

  const release: DesktopReleaseSeed = normalizeDesktopReleaseSeed({
    product: args.product,
    channel: args.channel,
    version: args.version,
    artifactPath,
    artifactContentType: inferArtifactContentType(args.artifact),
    createdAt: new Date().toISOString(),
    ...(args.previousVersion && bsdiffPath
      ? {
        bsdiffFromVersion: args.previousVersion,
        bsdiffPath,
      }
      : {}),
  })
  const releases = await readExistingReleases(args.outDir)
  const next = [
    release,
    ...releases.filter(
      (candidate) =>
        candidate.channel !== release.channel ||
        candidate.version !== release.version,
    ),
  ]

  await writeFile(
    join(args.outDir, "releases.json"),
    `${JSON.stringify({ releases: next }, null, 2)}\n`,
  )

  console.log(
    `desktop release ${args.product} ${args.version} staged for ${args.channel}`,
  )
  console.log(`artifact: ${artifactPath} sha256=${sha256Hex(artifactBytes)}`)
  if (bsdiffBytes !== undefined && bsdiffPath !== undefined) {
    console.log(
      `bsdiff: ${args.previousVersion}->${args.version} ${bsdiffPath} sha256=${sha256Hex(bsdiffBytes)}`,
    )
  }
  console.log("deploy with:")
  console.log(
    `  OA_DESKTOP_RELEASES_DIST=/app/desktop-dist bash apps/oa-updates/scripts/deploy-cloudrun.sh`,
  )
}

function parseArgs(argv: string[]): Args {
  const values = new Map<string, string>()

  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    const value = argv[index + 1]

    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      throw new Error(usage())
    }

    values.set(key.slice(2), value)
    index += 1
  }

  const channel = values.get("channel")
  const product = normalizeDesktopReleaseProduct(
    values.get("product") ?? DEFAULT_DESKTOP_RELEASE_PRODUCT,
  )
  const version = values.get("version")
  const artifact = values.get("artifact")
  const previousVersion = values.get("previous-version")
  const previousArtifact = values.get("previous-artifact")

  if (!channel || !version || !artifact) {
    throw new Error(usage())
  }

  if ((previousVersion === undefined) !== (previousArtifact === undefined)) {
    throw new Error("--previous-version and --previous-artifact must be set together")
  }

  return {
    product,
    channel,
    version,
    artifact: resolve(artifact),
    ...(previousVersion ? { previousVersion } : {}),
    ...(previousArtifact ? { previousArtifact: resolve(previousArtifact) } : {}),
    outDir: resolve(values.get("out") ?? defaultOutDir),
  }
}

async function readExistingReleases(outDir: string): Promise<DesktopReleaseSeed[]> {
  try {
    const bytes = await readFile(join(outDir, "releases.json"))
    const raw = JSON.parse(new TextDecoder().decode(bytes)) as unknown
    const rows = Array.isArray(raw)
      ? raw
      : isRecord(raw) && Array.isArray(raw.releases)
        ? raw.releases
        : []

    return rows.map(normalizeDesktopReleaseSeed)
  } catch (error) {
    if (isNodeError(error) && error.code === "ENOENT") return []
    throw error
  }
}

async function runBsdiff(
  previousArtifact: string,
  artifact: string,
  patchPath: string,
): Promise<void> {
  const proc = Bun.spawn(["bsdiff", previousArtifact, artifact, patchPath], {
    stdout: "pipe",
    stderr: "pipe",
  })
  const [exitCode, stderr] = await Promise.all([
    proc.exited,
    new Response(proc.stderr).text(),
  ])

  if (exitCode !== 0) {
    throw new Error(
      `bsdiff failed (${exitCode}). Install bsdiff and retry. ${stderr.trim()}`,
    )
  }
}

function inferArtifactContentType(path: string): string {
  switch (extname(path).toLowerCase()) {
    case ".zip":
      return "application/zip"
    case ".dmg":
      return "application/x-apple-diskimage"
    default:
      return DEFAULT_DESKTOP_ARTIFACT_CONTENT_TYPE
  }
}

function normalizedExt(path: string): string {
  const ext = extname(basename(path)).toLowerCase()
  return ext.length > 0 ? ext : ".bin"
}

function usage(): string {
  return [
    "Usage:",
    "  bun apps/oa-updates/scripts/publish-desktop-release.ts",
    "    --product autopilot-desktop --channel stable --version 1.2.0 --artifact ./AutopilotDesktop.zip",
    "    [--previous-version 1.1.0 --previous-artifact ./AutopilotDesktop-1.1.0.zip]",
    "    [--out apps/oa-updates/desktop-dist]",
  ].join("\n")
}

function isRecord(value: unknown): value is { readonly releases?: unknown } {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return typeof error === "object" && error !== null && "code" in error
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
