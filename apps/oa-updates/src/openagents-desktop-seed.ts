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

export const seedOpenAgentsDesktopRelease = async (input: Readonly<{
  server: UpdatesServer
  distDir: string
  readFile?: (path: string) => Promise<Uint8Array>
}>): Promise<void> => {
  const readFile = input.readFile ?? (async path => new Uint8Array(await nodeReadFile(path)))
  const descriptorRaw = JSON.parse(new TextDecoder().decode(
    await readFile(join(input.distDir, "openagents-desktop-release.json")),
  )) as Partial<Descriptor>
  const manifestPath = safeRelativeFile(descriptorRaw.manifestPath)
  const signaturePath = safeRelativeFile(descriptorRaw.signaturePath)
  if (manifestPath === null || signaturePath === null ||
    typeof descriptorRaw.artifactUrl !== "string") {
    throw new Error("OpenAgents Desktop release descriptor rejected")
  }
  const manifestBytes = await readFile(join(input.distDir, manifestPath))
  const signature = JSON.parse(new TextDecoder().decode(
    await readFile(join(input.distDir, signaturePath)),
  )) as unknown
  input.server.registerOpenAgentsDesktopRelease(admitOpenAgentsDesktopRelease({
    manifestBytes,
    signature,
    artifactUrl: descriptorRaw.artifactUrl,
  }))
}
