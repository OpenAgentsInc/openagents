#!/usr/bin/env bun
// Stage a Pylon RC build (output of apps/pylon/scripts/build-rc-binaries.sh) into
// the oa-updates pylon-dist/ tree the server seeds from. Reads the per-platform
// signed binaries + .sig.json, copies each into assets/<hash>, and writes
// pylon-releases.json (channel/version/platform/signature/kid/rollout). Signatures
// are NOT minted here — they come from the build's sign step (the pinned key).
//
// Usage:
//   bun apps/oa-updates/scripts/publish-pylon-release.ts \
//     --build-dir apps/pylon/dist/rc/1.0.0-rc.1 \
//     [--channel rc] [--rollout 100] [--min-version 1.0.0-rc.1] \
//     [--out apps/oa-updates/pylon-dist]
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { join, resolve } from "node:path"

import { assetKeyFromBytes } from "../src/asset-store.ts"
import {
  normalizePylonPlatform,
  sha256Hex,
  type PylonPlatform,
} from "../src/pylon-release.ts"
import {
  normalizePylonReleaseList,
  type PylonReleaseSeed,
} from "../src/pylon-seed.ts"

const repoRoot = resolve(import.meta.dir, "../../..")
const defaultOutDir = join(repoRoot, "apps/oa-updates/pylon-dist")

type BuildManifest = {
  version: string
  channel: string
  platforms: Record<string, { file: string; sha256: string; signature: string; kid: string }>
}

async function main(): Promise<void> {
  const args = parseArgs(Bun.argv.slice(2))
  const manifest = JSON.parse(
    await readFile(join(args.buildDir, "manifest.json"), "utf8"),
  ) as BuildManifest

  const channel = args.channel ?? manifest.channel ?? "rc"
  const version = manifest.version
  const createdAt = new Date().toISOString()
  await mkdir(join(args.outDir, "assets"), { recursive: true })

  const seeds: PylonReleaseSeed[] = []
  for (const [platformRaw, entry] of Object.entries(manifest.platforms)) {
    const platform: PylonPlatform = normalizePylonPlatform(platformRaw)
    const binBytes = new Uint8Array(await readFile(join(args.buildDir, entry.file)))
    // Re-derive sha256 + re-read the detached signature so we publish exactly
    // what was signed (and catch a corrupted build dir before it ships).
    const sha = sha256Hex(binBytes)
    if (sha !== entry.sha256) {
      throw new Error(`sha256 mismatch for ${entry.file}: build manifest says ${entry.sha256}, bytes are ${sha}`)
    }
    const hash = assetKeyFromBytes(binBytes)
    const artifactPath = `assets/${hash}`
    await writeFile(join(args.outDir, artifactPath), binBytes)

    seeds.push({
      channel,
      version,
      platform,
      artifactPath,
      sha256: entry.sha256,
      signature: entry.signature,
      kid: entry.kid,
      createdAt,
      ...(args.rollout !== undefined ? { rolloutPercent: args.rollout } : {}),
      ...(args.minVersion ? { minVersion: args.minVersion } : {}),
    })
  }

  // Merge with any existing releases (keep prior versions in the feed).
  const existing = await readExisting(args.outDir)
  const next = [
    ...seeds,
    ...existing.filter(
      (candidate) =>
        !seeds.some(
          (seed) =>
            seed.channel === candidate.channel &&
            seed.version === candidate.version &&
            seed.platform === candidate.platform,
        ),
    ),
  ]
  await writeFile(
    join(args.outDir, "pylon-releases.json"),
    `${JSON.stringify({ releases: next }, null, 2)}\n`,
  )

  console.log(`pylon ${version} staged for channel=${channel} (${seeds.length} platforms)`) // eslint-disable-line no-console
  for (const seed of seeds) console.log(`  ${seed.platform} kid=${seed.kid} ${seed.artifactPath}`)
  console.log("deploy with:")
  console.log("  OA_PYLON_RELEASES_DIST=/app/pylon-dist bash apps/oa-updates/scripts/deploy-cloudrun.sh")
}

async function readExisting(outDir: string): Promise<PylonReleaseSeed[]> {
  try {
    const raw = JSON.parse(await readFile(join(outDir, "pylon-releases.json"), "utf8")) as unknown
    return normalizePylonReleaseList(raw)
  } catch (error) {
    if (isNodeError(error) && error.code === "ENOENT") return []
    throw error
  }
}

type Args = {
  buildDir: string
  outDir: string
  channel?: string
  rollout?: number
  minVersion?: string
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
  const buildDir = values.get("build-dir")
  if (!buildDir) throw new Error(usage())
  return {
    buildDir: resolve(buildDir),
    outDir: resolve(values.get("out") ?? defaultOutDir),
    ...(values.get("channel") ? { channel: values.get("channel") } : {}),
    ...(values.get("rollout") ? { rollout: Number(values.get("rollout")) } : {}),
    ...(values.get("min-version") ? { minVersion: values.get("min-version") } : {}),
  }
}

function usage(): string {
  return [
    "Usage:",
    "  bun apps/oa-updates/scripts/publish-pylon-release.ts",
    "    --build-dir apps/pylon/dist/rc/1.0.0-rc.1",
    "    [--channel rc] [--rollout 100] [--min-version 1.0.0-rc.1]",
    "    [--out apps/oa-updates/pylon-dist]",
  ].join("\n")
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return typeof error === "object" && error !== null && "code" in error
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
