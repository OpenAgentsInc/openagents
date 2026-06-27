import { describe, expect, test } from "bun:test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

const script = join(import.meta.dir, "..", "deploy", "gcloud", "setup-pylon.sh")
const installer = join(import.meta.dir, "..", "scripts", "install-cloud-node.sh")

describe("GCloud Pylon setup script", () => {
  test("dry-run does not print env-file secrets", () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-gcloud-setup-test-"))
    const envFile = join(dir, "pylon.env")
    writeFileSync(envFile, "OPENAGENTS_AGENT_TOKEN=oa_agent_secret_for_test\nANTHROPIC_API_KEY=sk-ant-secret\n")

    const result = spawnSync(
      "bash",
      [
        script,
        "--dry-run",
        "--instance",
        "pylon-gcloud-test",
        "--project",
        "openagentsgemini",
        "--zone",
        "us-central1-a",
        "--env-file",
        envFile,
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("gcloud compute instances create pylon-gcloud-test")
    expect(output).toContain("gcloud compute scp")
    expect(output).toContain("gcloud compute ssh")
    expect(output).not.toContain("oa_agent_secret_for_test")
    expect(output).not.toContain("sk-ant-secret")
  })

  test("rejects malformed accelerator syntax before doing work", () => {
    const result = spawnSync(
      "bash",
      [script, "--dry-run", "--instance", "pylon-gcloud-test", "--accelerator", "nvidia-l4"],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(2)
    expect(result.stderr).toContain("--accelerator must use type=count")
  })

  test("dry-run accepts repurpose flags without printing secrets", () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-gcloud-setup-test-"))
    const envFile = join(dir, "pylon.env")
    writeFileSync(envFile, "OPENAGENTS_AGENT_TOKEN=oa_agent_repurpose_secret\n")

    const result = spawnSync(
      "bash",
      [
        script,
        "--dry-run",
        "--instance",
        "existing-l4-host",
        "--project",
        "openagentsgemini",
        "--zone",
        "us-central1-b",
        "--clear-startup-script",
        "--tags",
        "psion-swarm-contributor-host,pylon-hosted,openagents-pylon",
        "--env-file",
        envFile,
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("--tags psion-swarm-contributor-host\\,pylon-hosted\\,openagents-pylon")
    expect(output).toContain("gcloud compute ssh existing-l4-host")
    expect(output).not.toContain("oa_agent_repurpose_secret")
  })

  test("dry-run copies isolated Pylon home archive and requests supervisor install", () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-gcloud-setup-test-"))
    const envFile = join(dir, "pylon.env")
    const archive = join(dir, "pylon-home.tar.gz")
    writeFileSync(envFile, "OPENAGENTS_AGENT_TOKEN=oa_agent_archive_secret\n")
    writeFileSync(archive, "not-a-real-tarball-for-dry-run")

    const result = spawnSync(
      "bash",
      [
        script,
        "--dry-run",
        "--instance",
        "oa-codex-control-1",
        "--env-file",
        envFile,
        "--pylon-home-archive",
        archive,
        "--supervisor",
        "codex",
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("gcloud compute scp")
    expect(output).toContain("pylon-home.tar.gz")
    expect(output).toContain("PYLON_ENABLE_CODEX_SUPERVISOR")
    expect(output).toContain("openagents-codex-supervisor")
    expect(output).not.toContain("oa_agent_archive_secret")
  })

  test("installer dry-run renders Codex and Claude systemd supervisors", () => {
    const result = spawnSync("bash", [installer, "--dry-run"], {
      encoding: "utf8",
      env: {
        ...process.env,
        PYLON_ENABLE_CODEX_SUPERVISOR: "1",
        PYLON_ENABLE_CLAUDE_SUPERVISOR: "1",
        PYLON_HOME_ARCHIVE: "/tmp/pylon-home.tar.gz",
        OPENAGENTS_AGENT_TOKEN: "oa_agent_installer_secret",
      },
    })

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("openagents-codex-supervisor.service")
    expect(output).toContain("openagents-claude-supervisor.service")
    expect(output).toContain("capability.pylon.local_codex")
    expect(output).toContain("tar+ -xzf+ /tmp/pylon-home.tar.gz")
    expect(output).toContain("OPENAGENTS_AGENT_TOKEN=<redacted>")
    expect(output).not.toContain("oa_agent_installer_secret")
  })
})
