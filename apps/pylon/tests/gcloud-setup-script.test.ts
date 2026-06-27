import { describe, expect, test } from "bun:test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

const script = join(import.meta.dir, "..", "deploy", "gcloud", "setup-pylon.sh")

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

  test("dry-run copies account archive and enables own-capacity supervisors", () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-gcloud-setup-test-"))
    const envFile = join(dir, "pylon.env")
    const archive = join(dir, "accounts.tgz")
    writeFileSync(envFile, "OPENAGENTS_AGENT_TOKEN=oa_agent_fleet_secret\n")
    writeFileSync(archive, "fixture archive bytes")

    const result = spawnSync(
      "bash",
      [
        script,
        "--dry-run",
        "--instance",
        "oa-codex-control-1",
        "--project",
        "openagentsgemini",
        "--zone",
        "us-central1-a",
        "--account-archive",
        archive,
        "--enable-codex-supervisor",
        "--enable-claude-supervisor",
        "--env-file",
        envFile,
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("gcloud compute scp")
    expect(output).toContain("openagents-pylon-accounts.tgz")
    expect(output).toContain("openagents-pylon-codex-supervisor")
    expect(output).toContain("openagents-pylon-claude-supervisor")
    expect(output).not.toContain("oa_agent_fleet_secret")
  })
})
