import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"
import { createUpdatesServer } from "./server.ts"
import { seedOpenAgentsDesktopRelease } from "./openagents-desktop-seed.ts"
import { sha256 } from "./openagents-desktop-release.ts"

describe("OpenAgents Desktop release seed", () => {
  test("release image copies an independent seed directory", () => {
    const dockerfile = readFileSync(path.resolve(import.meta.dir, "../Dockerfile"), "utf8")
    expect(dockerfile).toContain("COPY openagents-desktop-dist ./openagents-desktop-dist")
  })
  test("registers exact manifest/signature bytes from a bounded descriptor", async () => {
    const manifestBytes = new TextEncoder().encode(JSON.stringify({
      schema: "openagents.desktop.update_manifest.v1",
      app: "openagents-desktop",
      channel: "rc",
      version: "0.1.0-rc.1",
      artifactName: "OpenAgents-0.1.0-rc.1-arm64.zip",
      artifactSha256: "a".repeat(64),
      artifactByteLength: 123,
      releasedAt: "2026-07-12T06:00:00.000Z",
    }))
    const files = new Map<string, Uint8Array>([
      ["openagents-desktop-release.json", new TextEncoder().encode(JSON.stringify({
        manifestPath: "manifest.json",
        signaturePath: "manifest.sig.json",
        artifactUrl: "https://storage.googleapis.com/openagents-releases/artifact.zip",
      }))],
      ["manifest.json", manifestBytes],
      ["manifest.sig.json", new TextEncoder().encode(JSON.stringify({
        alg: "ed25519", kid: "release.1", sha256: sha256(manifestBytes), signature: "fixture",
      }))],
    ])
    const server = createUpdatesServer()
    await seedOpenAgentsDesktopRelease({
      server,
      distDir: "/release",
      readFile: async path => files.get(path.split("/").at(-1)!)!,
    })
    const response = await server.fetch(new Request(
      "https://updates.openagents.com/desktop/openagents/rc/manifest.json",
    ))
    expect(new Uint8Array(await response.arrayBuffer())).toEqual(manifestBytes)
  })

  test("rejects traversal paths before reading release files", async () => {
    const server = createUpdatesServer()
    await expect(seedOpenAgentsDesktopRelease({
      server,
      distDir: "/release",
      readFile: async () => new TextEncoder().encode(JSON.stringify({
        manifestPath: "../manifest.json",
        signaturePath: "manifest.sig.json",
        artifactUrl: "https://updates.openagents.com/a.zip",
      })),
    })).rejects.toThrow("descriptor rejected")
  })
})
