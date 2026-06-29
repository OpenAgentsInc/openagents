import { describe, expect, test } from "bun:test"
import {
  decodeForgeDispatchWorkItem,
  type ForgeDispatchVerificationCommand,
} from "@openagentsinc/forge-protocol"
import {
  FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF,
  planForgeDockerVerificationCommand,
  planForgeDockerVerificationForWorkItem,
  runForgeDockerVerification,
  type ForgeDockerCommandRunner,
} from "./forge-verification-runner.js"

const workspacePath = "/tmp/forge/worktree"
const command = (): ForgeDispatchVerificationCommand => ({
  command_ref: "verification-command.forge.6752",
  runner_ref: FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF,
  working_directory: "packages/forge-protocol",
  args: ["bun", "test", "src/index.test.ts"],
  timeout_seconds: 120,
})

const valueAfter = (args: string[], flag: string): string => {
  const index = args.indexOf(flag)
  expect(index).toBeGreaterThanOrEqual(0)
  return args[index + 1] ?? ""
}

describe("forge Docker verification runner", () => {
  test("plans no-network read-only Docker execution for bun verification", () => {
    const plan = planForgeDockerVerificationCommand({
      workspacePath,
      command: command(),
      limits: {
        cpus: 2,
        memory: "1536m",
        pidsLimit: 128,
        tmpfsSize: "128m",
      },
    })

    expect(plan.runnerRef).toBe(FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF)
    expect(plan.timeoutMs).toBe(120_000)
    expect(valueAfter(plan.dockerArgs, "--network")).toBe("none")
    expect(valueAfter(plan.dockerArgs, "--cpus")).toBe("2")
    expect(valueAfter(plan.dockerArgs, "--memory")).toBe("1536m")
    expect(valueAfter(plan.dockerArgs, "--memory-swap")).toBe("1536m")
    expect(valueAfter(plan.dockerArgs, "--pids-limit")).toBe("128")
    expect(valueAfter(plan.dockerArgs, "--workdir")).toBe(
      "/workspace/packages/forge-protocol",
    )
    expect(plan.dockerArgs).toContain("--pull=never")
    expect(plan.dockerArgs).toContain("--read-only")
    expect(plan.dockerArgs).toContain("--cap-drop")
    expect(plan.dockerArgs).toContain("ALL")
    expect(plan.dockerArgs).toContain("--security-opt")
    expect(plan.dockerArgs).toContain("no-new-privileges")
    expect(plan.dockerArgs).toContain("/tmp:rw,noexec,nosuid,nodev,size=128m")
    expect(plan.dockerArgs).toContain(
      `type=bind,src=${workspacePath},dst=/workspace,readonly`,
    )
    expect(plan.dockerArgs.slice(-4)).toEqual([
      "oven/bun:1.3.11",
      "bun",
      "test",
      "src/index.test.ts",
    ])
    expect(plan.dockerArgs).not.toContain("--privileged")
  })

  test("plans from a Forge dispatch work item verification command", () => {
    const item = decodeForgeDispatchWorkItem({
      schema: "openagents.forge.dispatch.work_item.v0.1",
      tenant_ref: "tenant.openagents",
      dispatch_ref: "dispatch.forge.6752",
      work_ref: "work.forge.6752",
      issue_ref: "issue.forge.6752",
      objective_ref: "objective.forge.6752",
      objective_summary: "Implement isolated Docker verification",
      work_class: "codex_agent_task",
      payment_mode: "no-spend",
      capability_refs: ["capability.codex_cli"],
      git: {
        repository_ref: "repo.openagents.openagents",
        remote_url: "https://forge.openagents.com/openagents/openagents.git",
        base_ref: "refs/heads/main",
        base_head: "8e0c9b2eaf84c821caf555cae233a0d27e94d4ab",
        branch_ref: "refs/heads/forge/work/6752",
        receive_pack_ref: "receive-pack.forge.6752",
        git_access: {
          token_ref: "forge_git_token.6752",
          token_prefix: "oa_forge_git_visible",
          scopes: ["git:receive-pack"],
          expires_at: "2026-06-28T19:00:00.000Z",
          delivery: "out_of_band",
        },
      },
      verification_command: command(),
      lease_ref: "lease.forge.6752",
      expires_at: "2026-06-28T19:00:00.000Z",
      created_at: "2026-06-28T18:00:00.000Z",
      source_refs: ["github:OpenAgentsInc/openagents#6752"],
    })

    const plan = planForgeDockerVerificationForWorkItem({ item, workspacePath })

    expect(plan.commandRef).toBe("verification-command.forge.6752")
    expect(plan.verificationRef).toStartWith("verification.forge.docker_bun.")
    expect(plan.workspaceRef).toStartWith("workspace.forge.verify.")
  })

  test("rejects unsafe runner, workspace, working-directory, and argv shapes", () => {
    expect(() =>
      planForgeDockerVerificationCommand({
        workspacePath,
        command: { ...command(), runner_ref: "other.runner" },
      }),
    ).toThrow("targets a different runner")
    expect(() =>
      planForgeDockerVerificationCommand({
        workspacePath: "relative/worktree",
        command: command(),
      }),
    ).toThrow("workspace path must be absolute")
    expect(() =>
      planForgeDockerVerificationCommand({
        workspacePath,
        command: { ...command(), working_directory: "../outside" },
      }),
    ).toThrow("working directory escapes")
    expect(() =>
      planForgeDockerVerificationCommand({
        workspacePath,
        command: { ...command(), args: ["bash", "-lc", "bun test"] },
      }),
    ).toThrow("only accepts bun test/run argv")
  })

  test("runs through an injectable executor and returns a redacted receipt", async () => {
    const calls: Parameters<ForgeDockerCommandRunner>[0][] = []
    const runner: ForgeDockerCommandRunner = async (input) => {
      calls.push(input)
      return {
        exitCode: 0,
        stdout: "all tests passed",
        stderr: "",
        timedOut: false,
      }
    }
    const times = [
      new Date("2026-06-28T18:00:00.000Z"),
      new Date("2026-06-28T18:00:02.000Z"),
    ]

    const result = await runForgeDockerVerification({
      workspacePath,
      command: command(),
      runner,
      now: () => times.shift() ?? new Date("2026-06-28T18:00:02.000Z"),
    })

    expect(calls).toHaveLength(1)
    expect(calls[0]?.timeoutMs).toBe(120_000)
    expect(calls[0]?.args).toContain("--network")
    expect(calls[0]?.args).toContain("none")
    expect(result).toMatchObject({
      schema: "openagents.forge.verification.docker_bun.result.v0.1",
      runnerRef: FORGE_DOCKER_BUN_VERIFICATION_RUNNER_REF,
      commandRef: "verification-command.forge.6752",
      status: "passed",
      exitCode: 0,
      stdoutBytes: 16,
      stderrBytes: 0,
      stderrDigestRef: null,
      network: "none",
      readOnlyRootFilesystem: true,
      workspaceMountReadOnly: true,
      redacted: true,
      observedAt: "2026-06-28T18:00:00.000Z",
      completedAt: "2026-06-28T18:00:02.000Z",
    })
    expect(result.stdoutDigestRef ?? "").toStartWith("verification.stdout.")
    expect(JSON.stringify(result)).not.toContain("all tests passed")
  })

  test("marks timed-out Docker runs without raw stderr", async () => {
    const result = await runForgeDockerVerification({
      workspacePath,
      command: command(),
      runner: async () => ({
        exitCode: null,
        stdout: "",
        stderr: "container timed out",
        timedOut: true,
      }),
      now: () => new Date("2026-06-28T18:00:00.000Z"),
    })

    expect(result.status).toBe("timed_out")
    expect(result.exitCode).toBeNull()
    expect(result.stderrDigestRef ?? "").toStartWith("verification.stderr.")
    expect(JSON.stringify(result)).not.toContain("container timed out")
  })

  test("turns host runner errors into redacted error receipts", async () => {
    const result = await runForgeDockerVerification({
      workspacePath,
      command: command(),
      runner: async () => {
        throw new Error("docker executable unavailable")
      },
      now: () => new Date("2026-06-28T18:00:00.000Z"),
    })

    expect(result.status).toBe("error")
    expect(result.exitCode).toBeNull()
    expect(result.stderrBytes).toBeGreaterThan(0)
    expect(result.stderrDigestRef ?? "").toStartWith("verification.stderr.")
    expect(JSON.stringify(result)).not.toContain("docker executable unavailable")
  })
})
