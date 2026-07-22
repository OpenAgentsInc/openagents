import { chmodSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { findExecutableOnPath, probeHarnessBinary } from "./harness-binary-probe.ts"

/**
 * Seven-agents Part 2 (#9183): the shared host-run harness binary probe is a
 * READ-ONLY detection — it walks a supplied PATH, never mutates it, never runs
 * an install. These tests use a throwaway fixture directory so nothing depends
 * on the machine actually having goose/opencode installed.
 */

const makeFixtureBin = (name: string, versionLine: string): string => {
  const dir = mkdtempSync(join(tmpdir(), "oa-harness-probe-"))
  const file = join(dir, name)
  writeFileSync(file, `#!/bin/sh\necho "${versionLine}"\n`, { encoding: "utf8" })
  chmodSync(file, 0o755)
  return dir
}

describe("harness binary probe", () => {
  test("findExecutableOnPath resolves an executable on the supplied PATH and never mutates process.env", () => {
    const before = process.env.PATH
    const dir = makeFixtureBin("goose", "goose 1.2.3")
    return findExecutableOnPath("goose", dir).then((resolved) => {
      expect(resolved).toBe(join(dir, "goose"))
      // The probe reads PATH; it must never write it.
      expect(process.env.PATH).toBe(before)
    })
  })

  test("findExecutableOnPath returns null when the binary is absent from the supplied PATH", async () => {
    const dir = mkdtempSync(join(tmpdir(), "oa-harness-empty-"))
    expect(await findExecutableOnPath("opencode", dir)).toBeNull()
  })

  test("probeHarnessBinary detects an installed CLI and reports its version", async () => {
    const dir = makeFixtureBin("opencode", "opencode/0.9.9")
    const probe = await probeHarnessBinary({
      executable: "opencode",
      displayName: "OpenCode CLI",
      versionArgs: [],
      environment: { PATH: dir },
    })
    expect(probe.state).toBe("detected")
    if (probe.state === "detected") {
      expect(probe.resolvedPath).toBe(join(dir, "opencode"))
      expect(probe.reportedVersion).toContain("opencode/0.9.9")
    }
  })

  test("probeHarnessBinary reports an honest reason when the CLI is not installed", async () => {
    const dir = mkdtempSync(join(tmpdir(), "oa-harness-empty-"))
    const probe = await probeHarnessBinary({
      executable: "goose",
      displayName: "Goose CLI",
      versionArgs: ["--version"],
      environment: { PATH: dir },
    })
    expect(probe.state).toBe("not_detected")
    if (probe.state === "not_detected") {
      expect(probe.reason).toContain("Goose CLI")
      expect(probe.reason.toLowerCase()).toContain("not installed")
    }
  })
})
