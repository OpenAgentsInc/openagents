#!/usr/bin/env bun
/**
 * Retained supervised daily-driver proof runner (#4847 Claude lane,
 * #4860 Codex lane).
 *
 * Drives one real coding task through the same composer execution path the
 * TUI uses (`runCodexComposerStream` / `runClaudeComposerStream`) in the
 * active source checkout, strictly in local_bounded mode, then runs the
 * focused dev-check loop and retains a typed, redaction-scanned proof
 * artifact under apps/pylon/docs/proofs/.
 *
 * The retained artifact carries refs and digests only: hashed session refs,
 * command digests, repo/commit context from the dev doctor. Raw session
 * ids, prompts, credentials, and local absolute paths never reach the file.
 *
 * Usage:
 *   bun apps/pylon/scripts/dev-proof-run.ts \
 *     --adapter codex|claude_agent \
 *     --objective "<public objective summary>" \
 *     [--prompt-file <path>] [--issue <ref>]... [--timeout-seconds <n>] \
 *     [--no-network] -- <verification argv...>
 */
import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  publicPylonAccountSelection,
  pylonAccountEnvironment,
  resolvePylonAccountSelection,
  type PublicPylonAccountSelection,
} from "../src/account-registry"
import { loadClaudeAgentConfig } from "../src/claude-agent"
import { runClaudeComposerStream } from "../src/claude-composer"
import { loadCodexAgentConfig, type PylonComposerAdapter } from "../src/codex-agent"
import { runCodexComposerStream } from "../src/codex-composer"
import { collectPylonDevDoctor, type PylonDevDoctorProjection } from "../src/dev-doctor"
import {
  recordPylonDevCodexRun,
  runPylonDevCheck,
  type PylonDevCheckProjection,
} from "../src/dev-loop"
import {
  PROOF_REDACTION_PATTERN_REFS,
  scanProofSerialization,
} from "../src/proof-redaction"
import { assertPublicProjectionSafe } from "../src/state"

export const PYLON_DEV_PROOF_SCHEMA = "openagents.pylon.dev_proof_run.v0.1"

type ProofBoundary =
  | {
      adapter: "codex"
      sandboxMode: "read-only" | "workspace-write"
      approvalPolicy: "never"
      networkAccessEnabled: boolean
    }
  | {
      adapter: "claude_agent"
      permissionMode: "acceptEdits"
      allowedToolsProfile: "composer_default"
      settingSourcesExcluded: true
    }

type ProofExecutorSummary = {
  executionPathRef: "composer.run_stream"
  executionMode: "local_bounded"
  boundary: ProofBoundary
  outcome: "completed"
  eventCount: number
  turnCount: number | null
  commandCount: number
  editedFileCount: number
  totalTokens: number
  sessionRef: string | null
  responseDigestRef: string | null
}

export type RetainedDailyDriverProof = {
  schema: typeof PYLON_DEV_PROOF_SCHEMA
  observedAt: string
  completedAt: string
  adapter: PylonComposerAdapter
  supervision: {
    label: "agent_supervised_owner_directed_session"
    ownerDirected: true
    sourceCheckout: true
    executionMode: "local_bounded"
  }
  doctor: PylonDevDoctorProjection
  task: {
    objective: string
    promptDigestRef: string
    issueRefs: string[]
  }
  account: PublicPylonAccountSelection | null
  executor: ProofExecutorSummary
  devCheck: PylonDevCheckProjection
  redactionScan: {
    state: "clean"
    patternRefs: string[]
  }
  deviations: string[]
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

/**
 * Scans any serialized retained projection against the house redaction
 * patterns. Exported so sibling proof harnesses (e.g. the M10 overnight
 * runner) reuse the exact same gate instead of reinventing it.
 */
export { scanProofSerialization }

function scanRetainedProof(proof: RetainedDailyDriverProof): string[] {
  return scanProofSerialization(JSON.stringify(proof))
}

export type ProofRunArgs = {
  adapter: PylonComposerAdapter
  objective: string
  promptFile: string | null
  issueRefs: string[]
  accountRef: string | null
  codexHome: string | null
  claudeConfigDir: string | null
  proofOutput: string | null
  timeoutSeconds: number
  verificationArgv: string[]
  networkAccessEnabled?: boolean
  /** Task repo for the composer run; defaults to process.cwd(). */
  cwd?: string
}

export function parseProofRunArgs(argv: string[]): ProofRunArgs {
  const usage =
    'usage: dev-proof-run.ts --adapter codex|claude_agent --objective "<text>" [--prompt-file <path>] [--issue <ref>]... [--account-ref <ref>] [--codex-home <dir>|--claude-config-dir <dir>] [--proof-output <path>] [--timeout-seconds <n>] [--no-network] -- <verification argv...>'
  let adapter: PylonComposerAdapter | null = null
  let objective: string | null = null
  let promptFile: string | null = null
  let accountRef: string | null = null
  let codexHome: string | null = null
  let claudeConfigDir: string | null = null
  let proofOutput: string | null = null
  const issueRefs: string[] = []
  let timeoutSeconds = 600
  let verificationArgv: string[] = []
  let networkAccessEnabled = true
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--") {
      verificationArgv = argv.slice(index + 1)
      break
    }
    const value = argv[index + 1]
    if (arg === "--adapter" && (value === "codex" || value === "claude_agent")) {
      adapter = value
      index += 1
    } else if (arg === "--objective" && typeof value === "string") {
      objective = value
      index += 1
    } else if (arg === "--prompt-file" && typeof value === "string") {
      promptFile = value
      index += 1
    } else if (arg === "--issue" && typeof value === "string") {
      issueRefs.push(value)
      index += 1
    } else if (arg === "--account-ref" && typeof value === "string") {
      accountRef = value
      index += 1
    } else if (arg === "--codex-home" && typeof value === "string") {
      codexHome = value
      index += 1
    } else if (arg === "--claude-config-dir" && typeof value === "string") {
      claudeConfigDir = value
      index += 1
    } else if (arg === "--proof-output" && typeof value === "string") {
      proofOutput = value
      index += 1
    } else if (arg === "--timeout-seconds" && typeof value === "string") {
      const parsed = Number(value)
      if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 1200) throw new Error(usage)
      timeoutSeconds = Math.floor(parsed)
      index += 1
    } else if (arg === "--no-network") {
      networkAccessEnabled = false
    } else {
      throw new Error(usage)
    }
  }
  if (adapter === null || objective === null || verificationArgv.length === 0) {
    throw new Error(usage)
  }
  if (adapter === "codex" && claudeConfigDir !== null) throw new Error(usage)
  if (adapter === "claude_agent" && codexHome !== null) throw new Error(usage)
  return {
    adapter,
    objective,
    promptFile,
    issueRefs,
    accountRef,
    codexHome,
    claudeConfigDir,
    proofOutput,
    timeoutSeconds,
    verificationArgv,
    networkAccessEnabled,
  }
}

export async function runProof(args: ProofRunArgs): Promise<RetainedDailyDriverProof> {
  const baseEnv = Bun.env as Record<string, string | undefined>
  const cwd = args.cwd ?? process.cwd()
  const observedAt = new Date().toISOString()
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), Bun.env)
  const account = await resolvePylonAccountSelection(summary, {
    provider: args.adapter === "codex" ? "codex" : "claude_agent",
    ...(args.accountRef === null ? {} : { accountRef: args.accountRef }),
    ...(args.adapter === "codex" && args.codexHome !== null
      ? { accountHome: args.codexHome }
      : {}),
    ...(args.adapter === "claude_agent" && args.claudeConfigDir !== null
      ? { accountHome: args.claudeConfigDir }
      : {}),
  })
  const env = pylonAccountEnvironment(baseEnv, account)

  // 1. Doctor context: typed, public-safe repo/readiness projection.
  const doctor = await collectPylonDevDoctor({ cwd, env, summary })
  const readinessState =
    args.adapter === "codex" ? doctor.codex.sdkReadiness.state : doctor.claudeAgent.readiness.state
  if (readinessState !== "ready") {
    throw new Error(`adapter ${args.adapter} is not ready on this device (${readinessState})`)
  }
  const executionMode =
    args.adapter === "codex" ? doctor.codex.executionMode : doctor.claudeAgent.executionMode
  if (executionMode !== "local_bounded") {
    throw new Error(`proof runs require local_bounded execution mode, got ${executionMode}`)
  }

  // 2. Execute the real task through the same composer path the TUI uses.
  const prompt =
    args.promptFile === null ? args.objective : await readFile(resolve(args.promptFile), "utf8")
  const timeoutMs = args.timeoutSeconds * 1000
  const networkAccessEnabled = args.networkAccessEnabled ?? true
  let executor: ProofExecutorSummary
  if (args.adapter === "codex") {
    const config = await loadCodexAgentConfig(summary)
    const sandboxMode = doctor.codex.sandboxMode
    if (sandboxMode === "danger-full-access") {
      throw new Error("proof runs never use danger-full-access")
    }
    const result = await runCodexComposerStream(prompt, {
      approvalPolicy: "never",
      config,
      cwd,
      account,
      env,
      executionMode: "local_bounded",
      ...(config.model === undefined ? {} : { model: config.model }),
      networkAccessEnabled,
      sandboxMode,
      timeoutMs,
    })
    await recordPylonDevCodexRun(
      {
        commandCount: result.commandCount,
        cwd,
        editedFileCount: result.editedFileCount,
        eventCount: result.eventCount,
        executionMode: "local_bounded",
        sandboxMode,
        totalTokens: result.totalTokens,
      },
      { cwd, env, summary },
    )
    executor = {
      executionPathRef: "composer.run_stream",
      executionMode: "local_bounded",
      boundary: {
        adapter: "codex",
        sandboxMode,
        approvalPolicy: "never",
        networkAccessEnabled,
      },
      outcome: "completed",
      eventCount: result.eventCount,
      turnCount: null,
      commandCount: result.commandCount,
      editedFileCount: result.editedFileCount,
      totalTokens: result.totalTokens,
      sessionRef:
        result.threadId === null ? null : stableRef("session.pylon.codex_composer", result.threadId),
      responseDigestRef:
        result.text.length === 0 ? null : stableRef("digest.composer.response", result.text),
    }
  } else {
    const config = await loadClaudeAgentConfig(summary)
    if (doctor.claudeAgent.permissionMode !== "acceptEdits") {
      throw new Error("proof runs require the acceptEdits permission mode")
    }
    const result = await runClaudeComposerStream(prompt, {
      config,
      cwd,
      account,
      env,
      executionMode: "local_bounded",
      ...(config.model === undefined ? {} : { model: config.model }),
      permissionMode: "acceptEdits",
      timeoutMs,
    })
    executor = {
      executionPathRef: "composer.run_stream",
      executionMode: "local_bounded",
      boundary: {
        adapter: "claude_agent",
        permissionMode: "acceptEdits",
        allowedToolsProfile: "composer_default",
        settingSourcesExcluded: true,
      },
      outcome: "completed",
      eventCount: result.eventCount,
      turnCount: result.turnCount,
      commandCount: result.commandCount,
      editedFileCount: result.editedFileCount,
      totalTokens: result.totalTokens,
      sessionRef: result.sessionRef,
      responseDigestRef:
        result.text.length === 0 ? null : stableRef("digest.composer.response", result.text),
    }
  }

  // 3. Focused dev check over the agent's change (real loop, allow dirty).
  const devCheck = await runPylonDevCheck({
    allowDirty: true,
    // Proof runs verify isolated worktrees that are legitimately detached
    // (e.g. materialized from a pinned commit). The run never touches the
    // branch/commit, so a detached HEAD must not block verification.
    allowDetached: true,
    commands: [
      { argv: args.verificationArgv, cwd, reasonRef: "check.pylon.proof_verification" },
    ],
    cwd,
    env,
    summary,
  })

  const proof: RetainedDailyDriverProof = {
    schema: PYLON_DEV_PROOF_SCHEMA,
    observedAt,
    completedAt: new Date().toISOString(),
    adapter: args.adapter,
    supervision: {
      label: "agent_supervised_owner_directed_session",
      ownerDirected: true,
      sourceCheckout: true,
      executionMode: "local_bounded",
    },
    doctor,
    task: {
      objective: args.objective,
      promptDigestRef: stableRef("digest.composer.prompt", prompt),
      issueRefs: args.issueRefs,
    },
    account: publicPylonAccountSelection(account),
    executor,
    devCheck,
    redactionScan: {
      state: "clean",
      patternRefs: PROOF_REDACTION_PATTERN_REFS,
    },
    deviations: [],
  }

  // 4. Redaction gate: typed projection guard plus pattern scan. The proof
  // is only written when both pass.
  assertPublicProjectionSafe(proof)
  const hits = scanRetainedProof(proof)
  if (hits.length > 0) {
    throw new Error(`retained proof failed redaction scan: ${hits.join(", ")}`)
  }
  return proof
}

async function main() {
  const args = parseProofRunArgs(Bun.argv.slice(2))
  const proof = await runProof(args)
  const proofsDir = join(dirname(fileURLToPath(import.meta.url)), "..", "docs", "proofs")
  await mkdir(args.proofOutput === null ? proofsDir : dirname(resolve(args.proofOutput)), { recursive: true })
  const fileName = `${proof.observedAt.slice(0, 10)}-${args.adapter.replace("_", "-")}-daily-driver-proof.json`
  const proofPath = args.proofOutput === null ? join(proofsDir, fileName) : resolve(args.proofOutput)
  await writeFile(proofPath, `${JSON.stringify(proof, null, 2)}\n`, "utf8")
  process.stdout.write(
    `${JSON.stringify(
      {
        adapter: proof.adapter,
        devCheckState: proof.devCheck.state,
        editedFileCount: proof.executor.editedFileCount,
        proofFile: args.proofOutput === null ? `apps/pylon/docs/proofs/${fileName}` : proofPath,
        redactionScan: proof.redactionScan.state,
        sessionRef: proof.executor.sessionRef,
      },
      null,
      2,
    )}\n`,
  )
  if (proof.devCheck.state !== "passed") {
    process.stderr.write(`dev check did not pass: ${proof.devCheck.state}\n`)
    process.exitCode = 1
  }
}

if (import.meta.main) {
  await main()
}
