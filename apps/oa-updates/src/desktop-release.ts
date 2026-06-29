import { createHash } from "node:crypto"

import type { AssetStore } from "./asset-store.ts"

export type DesktopReleaseSeed = {
  readonly channel: string
  readonly version: string
  readonly artifactPath: string
  readonly artifactContentType?: string
  readonly createdAt?: string
  readonly bsdiffFromVersion?: string
  readonly bsdiffPath?: string
}

export type DesktopUpdateManifest = {
  readonly version: string
  readonly artifactUrl: string
  readonly sha256: string
  readonly createdAt?: string
  readonly bsdiffFromVersion?: string
  readonly bsdiffUrl?: string
  readonly bsdiffSha256?: string
}

export type BuildDesktopUpdateInput = {
  readonly version: string
  readonly artifactBytes: Uint8Array
  readonly baseUrl: string
  readonly store: AssetStore
  readonly artifactContentType?: string
  readonly createdAt?: string
  readonly bsdiffFromVersion?: string
  readonly bsdiffBytes?: Uint8Array
}

export type BuildDesktopUpdateResult = {
  readonly manifest: DesktopUpdateManifest
  readonly artifactHash: string
  readonly bsdiffHash?: string
}

export const DEFAULT_DESKTOP_ARTIFACT_CONTENT_TYPE = "application/zip"
export const BSDIFF_CONTENT_TYPE = "application/octet-stream"

export function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex")
}

export async function buildDesktopUpdateManifest(
  input: BuildDesktopUpdateInput,
): Promise<BuildDesktopUpdateResult> {
  assertNonEmpty(input.version, "version")
  const artifact = await input.store.put(input.artifactBytes)
  const manifest: DesktopUpdateManifest = {
    version: input.version.trim(),
    artifactUrl: assetUrl(input.baseUrl, artifact.hash),
    sha256: sha256Hex(input.artifactBytes),
    ...(input.createdAt ? { createdAt: input.createdAt.trim() } : {}),
  }

  if (input.bsdiffBytes !== undefined) {
    assertNonEmpty(input.bsdiffFromVersion, "bsdiffFromVersion")
    const bsdiff = await input.store.put(input.bsdiffBytes)

    return {
      manifest: {
        ...manifest,
        bsdiffFromVersion: input.bsdiffFromVersion?.trim(),
        bsdiffUrl: assetUrl(input.baseUrl, bsdiff.hash),
        bsdiffSha256: sha256Hex(input.bsdiffBytes),
      },
      artifactHash: artifact.hash,
      bsdiffHash: bsdiff.hash,
    }
  }

  return {
    manifest,
    artifactHash: artifact.hash,
  }
}

export function normalizeDesktopReleaseSeed(value: unknown): DesktopReleaseSeed {
  if (!isRecord(value)) {
    throw new Error("Desktop release seed must be an object")
  }

  const channel = readRequiredString(value, "channel")
  const version = readRequiredString(value, "version")
  const artifactPath = readRequiredString(value, "artifactPath")
  const artifactContentType = readOptionalString(value, "artifactContentType")
  const createdAt = readOptionalString(value, "createdAt")
  const bsdiffFromVersion = readOptionalString(value, "bsdiffFromVersion")
  const bsdiffPath = readOptionalString(value, "bsdiffPath")

  if ((bsdiffFromVersion === undefined) !== (bsdiffPath === undefined)) {
    throw new Error(
      "Desktop release seeds must set bsdiffFromVersion and bsdiffPath together",
    )
  }

  return {
    channel,
    version,
    artifactPath,
    ...(artifactContentType ? { artifactContentType } : {}),
    ...(createdAt ? { createdAt } : {}),
    ...(bsdiffFromVersion ? { bsdiffFromVersion } : {}),
    ...(bsdiffPath ? { bsdiffPath } : {}),
  }
}

export function sortDesktopFeed(
  feed: ReadonlyArray<DesktopUpdateManifest>,
): DesktopUpdateManifest[] {
  return [...feed].sort((left, right) =>
    compareVersions(right.version, left.version),
  )
}

function assetUrl(baseUrl: string, hash: string): string {
  return `${baseUrl.replace(/\/+$/, "")}/assets/${hash}`
}

function assertNonEmpty(value: string | undefined, field: string): void {
  if (value === undefined || value.trim().length === 0) {
    throw new Error(`Desktop update ${field} is required`)
  }
}

function readRequiredString(record: Record<string, unknown>, key: string): string {
  const value = readOptionalString(record, key)

  if (value === undefined) {
    throw new Error(`Desktop release seed ${key} is required`)
  }

  return value
}

function readOptionalString(
  record: Record<string, unknown>,
  key: string,
): string | undefined {
  const value = record[key]

  if (value === undefined) return undefined
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`Desktop release seed ${key} must be a non-empty string`)
  }

  return value.trim()
}

function compareVersions(left: string, right: string): number {
  const leftParts = versionParts(left)
  const rightParts = versionParts(right)
  const partCount = Math.max(leftParts.length, rightParts.length)

  for (let index = 0; index < partCount; index += 1) {
    const leftPart = leftParts[index] ?? 0
    const rightPart = rightParts[index] ?? 0
    if (leftPart !== rightPart) return leftPart - rightPart
  }

  return 0
}

function versionParts(version: string): number[] {
  return version.split(".").map((part) => {
    const value = Number.parseInt(part, 10)
    return Number.isFinite(value) ? value : 0
  })
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
