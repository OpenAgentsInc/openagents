import { describe, expect, test } from "bun:test"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

const script = join(import.meta.dir, "..", "deploy", "gcloud", "setup-pylon.sh")
const agentComputerScript = join(
  import.meta.dir,
  "..",
  "deploy",
  "agent-computer",
  "setup-gce-host.sh",
)

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
})

describe("Agent Computer GCE host setup script", () => {
  test("dry-run creates a nested-virtualization host and verifies /dev/kvm", () => {
    const result = spawnSync(
      "bash",
      [
        agentComputerScript,
        "--dry-run",
        "--instance",
        "agent-computer-test",
        "--project",
        "openagentsgemini",
        "--zone",
        "us-central1-a",
        "--machine-type",
        "n2-standard-4",
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("gcloud compute instances create agent-computer-test")
    expect(output).toContain("--enable-nested-virtualization")
    expect(output).toContain("--no-address")
    expect(output).toContain("gcloud compute ssh agent-computer-test")
    expect(output).toContain("test\\ -c\\ /dev/kvm")
    expect(output).toContain("/var/lib/openagents/agent-computers")
    expect(output).not.toContain("OA_CLOUD_CONTROL_TOKEN")
    expect(output).not.toContain("OPENAGENTS_AGENT_TOKEN")
  })

  test("rejects machine families that do not support the planned nested-virt lane", () => {
    const result = spawnSync(
      "bash",
      [
        agentComputerScript,
        "--dry-run",
        "--instance",
        "agent-computer-test",
        "--machine-type",
        "e2-standard-4",
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(2)
    expect(result.stderr).toContain("machine type must be n2-* or n1-*")
  })

  test("uses Haswell minimum CPU platform when n1 hosts are requested", () => {
    const result = spawnSync(
      "bash",
      [
        agentComputerScript,
        "--dry-run",
        "--instance",
        "agent-computer-n1-test",
        "--machine-type",
        "n1-standard-4",
      ],
      { encoding: "utf8" },
    )

    expect(result.status).toBe(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("--min-cpu-platform Intel\\ Haswell")
  })
})
