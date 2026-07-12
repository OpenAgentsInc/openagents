import { readFile as nodeReadFile } from "node:fs/promises"
import { join } from "node:path"

import { admitOpenAgentsDesktopRelease } from "./openagents-desktop-release.ts"
import type { UpdatesServer } from "./server.ts"

type Descriptor = Readonly<{
  manifestPath: string
  signaturePath: string
  artifactUrl: string
}>

const safeRelativeFile = (value: unknown): string | null =>
  typeof value === "string" && /^[A-Za-z0-9][A-Za-z0-9._-]{0,119}$/.test(value)
    ? value : null

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

/**
 * The descriptor is either the original flat single-release shape or the
 * publisher's multi-channel `{ releases: [...] }` list (one latest release
 * per channel, written by
 * `apps/openagents-desktop/scripts/publish-release.ts`). Both are bounded;
 * anything else is rejected (fail closed).
 */
const descriptorEntries = (raw: unknown): readonly Descriptor[] => {
  const rows = isRecord(raw) && Array.isArray(raw.releases) ? raw.releases : [raw]
  if (rows.length === 0 || rows.length > 8) {
    throw new Error("OpenAgents Desktop release descriptor rejected")
  }
  return rows.map((row) => {
    if (!isRecord(row)) {
      throw new Error("OpenAgents Desktop release descriptor rejected")
    }
    const manifestPath = safeRelativeFile(row.manifestPath)
    const signaturePath = safeRelativeFile(row.signaturePath)
    if (manifestPath === null || signaturePath === null ||
      typeof row.artifactUrl !== "string") {
      throw new Error("OpenAgents Desktop release descriptor rejected")
    }
    return { manifestPath, signaturePath, artifactUrl: row.artifactUrl }
  })
}

export const seedOpenAgentsDesktopRelease = async (input: Readonly<{
  server: UpdatesServer
  distDir: string
  readFile?: (path: string) => Promise<Uint8Array>
}>): Promise<void> => {
  const readFile = input.readFile ?? (async path => new Uint8Array(await nodeReadFile(path)))
  const descriptorRaw = JSON.parse(new TextDecoder().decode(
    await readFile(join(input.distDir, "openagents-desktop-release.json")),
  )) as unknown

  const seenChannels = new Set<string>()
  for (const entry of descriptorEntries(descriptorRaw)) {
    const manifestBytes = await readFile(join(input.distDir, entry.manifestPath))
    const signature = JSON.parse(new TextDecoder().decode(
      await readFile(join(input.distDir, entry.signaturePath)),
    )) as unknown
    const release = admitOpenAgentsDesktopRelease({
      manifestBytes,
      signature,
      artifactUrl: entry.artifactUrl,
    })
    // One latest release per channel — a duplicate means a malformed
    // publish; refuse instead of silently letting the last row win.
    if (seenChannels.has(release.channel)) {
      throw new Error(
        `OpenAgents Desktop release descriptor has duplicate channel ${release.channel}`,
      )
    }
    seenChannels.add(release.channel)
    input.server.registerOpenAgentsDesktopRelease(release)
  }
}
