import { describe, expect, test } from "bun:test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

const script = join(import.meta.dir, "..", "scripts", "install-cloud-node.sh")

describe("cloud node installer script", () => {
  test("dry-run restores account archive and writes supervisor units", () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-cloud-install-test-"))
    const archive = join(dir, "accounts.tgz")
    writeFileSync(archive, "fixture archive bytes")

    const result = spawnSync("bash", [script, "--dry-run"], {
      encoding: "utf8",
      env: {
        ...process.env,
        PYLON_INSTALL_DIR: "/opt/openagents-pylon",
        PYLON_HOME: "/var/lib/openagents-pylon",
        PYLON_REF: "gcloud.oa-codex-control-1",
        PYLON_ACCOUNT_ARCHIVE: archive,
        PYLON_ENABLE_CODEX_SUPERVISOR: "1",
        PYLON_ENABLE_CLAUDE_SUPERVISOR: "1",
        OPENAGENTS_AGENT_TOKEN: "oa_agent_installer_secret",
        ANTHROPIC_API_KEY: "sk-ant-installer-secret",
      },
    })

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain(`+ validate account archive ${archive}`)
    expect(output).toContain(`+ tar -xzf ${archive} -C /var/lib/openagents-pylon`)
    expect(output).toContain("capability.pylon.local_codex")
    expect(output).toContain("Description=OpenAgents Pylon codex own-capacity supervisor")
    expect(output).toContain("apps/pylon/scripts/codex-supervisor/codex-supervisor.sh")
    expect(output).toContain("Description=OpenAgents Pylon claude own-capacity supervisor")
    expect(output).toContain("apps/pylon/scripts/claude-supervisor/claude-supervisor.sh")
    expect(output).toContain("systemctl+ enable+ --now+ openagents-pylon-codex-supervisor")
    expect(output).toContain("systemctl+ enable+ --now+ openagents-pylon-claude-supervisor")
    expect(output).toContain("OPENAGENTS_AGENT_TOKEN=<redacted>")
    expect(output).toContain("ANTHROPIC_API_KEY=<redacted>")
    expect(output).not.toContain("oa_agent_installer_secret")
    expect(output).not.toContain("sk-ant-installer-secret")
  })
})
