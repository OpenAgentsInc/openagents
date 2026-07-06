import { describe, expect, test } from "bun:test"
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

const script = join(
  import.meta.dir,
  "..",
  "deploy",
  "cloud-gcp-worker",
  "bootstrap.sh",
)
const scriptContents = readFileSync(script, "utf8")

// Runs `command` after sourcing bootstrap.sh, via a real wrapper script file
// (not `bash -c "<inline string>"`) so this matches the file-argv invocation
// pattern the sibling gcloud-setup-script.test.ts already uses. Uses `printf`
// rather than `echo` for probe output within the wrapper.
// Ambient ops env vars (e.g. a real OPENAGENTS_AGENT_TOKEN exported for
// unrelated fleet/dispatch work in the calling shell) must never leak into
// the "missing metadata" test cases below, or a genuinely-missing value would
// look present. Strip the metadata-shaped keys bootstrap.sh reads and let
// each test opt specific ones back in via `env`.
const AMBIENT_KEYS_TO_STRIP = [
  "OPENAGENTS_ADMIN_TOKEN",
  "OPENAGENTS_AGENT_TOKEN",
  "OPENAGENTS_PIN_REF",
  "OPENAGENTS_BASE_URL",
]

const runSourced = (command: string, env: Record<string, string> = {}) => {
  const dir = mkdtempSync(join(tmpdir(), "cloud-gcp-worker-bootstrap-test-"))
  const wrapper = join(dir, "run.sh")
  writeFileSync(
    wrapper,
    `#!/usr/bin/env bash\nset -euo pipefail\nsource '${script}'\n${command}\n`,
  )
  const baseEnv = { ...process.env }
  for (const key of AMBIENT_KEYS_TO_STRIP) delete baseEnv[key]
  return spawnSync("bash", [wrapper], {
    encoding: "utf8",
    env: { ...baseEnv, ...env },
  })
}

describe("cloud-gcp worker bootstrap.sh", () => {
  test("has valid bash syntax", () => {
    const result = spawnSync("bash", ["-n", script], { encoding: "utf8" })
    expect(result.status).toBe(0)
  })

  test("is only executed directly, not on source (main is guarded)", () => {
    // Sourcing must not attempt network access (install_bun/sync_repo/etc.)
    // — it should return instantly with no output beyond what we ask for.
    const result = runSourced("printf 'sourced-ok'")
    expect(result.status).toBe(0)
    expect(result.stdout.trim()).toBe("sourced-ok")
  })

  test("metadata_attr prefers an env var override over the metadata server", () => {
    const result = runSourced("metadata_attr OPENAGENTS_PIN_REF fallback-value", {
      OPENAGENTS_PIN_REF: "issue-8503-pin",
    })
    expect(result.status).toBe(0)
    expect(result.stdout.trim()).toBe("issue-8503-pin")
  })

  test("metadata_attr falls back to the provided default when no metadata server and no env override", () => {
    const result = runSourced("metadata_attr OPENAGENTS_PIN_REF fallback-value")
    expect(result.status).toBe(0)
    expect(result.stdout.trim()).toBe("fallback-value")
  })

  test("require_metadata fails closed with a clear message when the value is missing", () => {
    const result = runSourced("require_metadata OPENAGENTS_AGENT_TOKEN")
    expect(result.status).toBe(2)
    expect(result.stderr).toContain(
      "missing required instance metadata attribute: OPENAGENTS_AGENT_TOKEN",
    )
  })

  test("require_metadata returns the env-overridden value without leaking it to stderr", () => {
    const result = runSourced("require_metadata OPENAGENTS_AGENT_TOKEN", {
      OPENAGENTS_AGENT_TOKEN: "oa_agent_test_secret_value",
    })
    expect(result.status).toBe(0)
    expect(result.stdout.trim()).toBe("oa_agent_test_secret_value")
    expect(result.stderr).not.toContain("oa_agent_test_secret_value")
  })

  test("never selects owner_local mode: run_supervisor's bun invocation omits --owner-user-id", () => {
    expect(scriptContents).toContain("runtime-intent-supervisor.ts")
    expect(scriptContents).not.toContain("--owner-user-id")
  })

  test("reads required credentials from instance metadata, never hardcoded", () => {
    expect(scriptContents).toContain("require_metadata OPENAGENTS_ADMIN_TOKEN")
    expect(scriptContents).toContain("require_metadata OPENAGENTS_AGENT_TOKEN")
    expect(scriptContents).not.toMatch(/oa_agent_[a-zA-Z0-9]{10,}/)
  })
})
