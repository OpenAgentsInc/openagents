import { existsSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import { relative, resolve, sep } from "node:path"
import {
  PROVIDER_COMPLIANT_USAGE_LABOR_POLICY_REF,
  type LaborJobRequest,
  type LaborJobType,
} from "@openagentsinc/nip90"
import { assertPublicProjectionSafe, ensureStateDirectories, type PylonPaths } from "./state.js"

export const PYLON_LABOR_CAPABILITY_REF = "capability.public.pylon.labor.local_agent.v0.3"
export const PYLON_LABOR_APPROVAL_POLICY_REF = "policy.public.pylon.labor.first_run_operator_approval.v0.3"
export const PYLON_LABOR_SANDBOX_POLICY_REF = "policy.public.pylon.labor.bounded_workspace.v0.3"

export type LaborAdmissionBlocker =
  | "labor_first_run_approval_required"
  | "labor_auth_exfiltration_blocked"
  | "labor_policy_mismatch"
  | "labor_workspace_out_of_bounds"

export type LaborLocalAgentKind = "codex" | "opencode" | "claude_code" | "test_fixture"

export type PylonLaborApprovalRecord = {
  approvedAt: string
  approvedByRef: string
  jobType: LaborJobType
  policyRef: string
}

export type PylonLaborApprovalStore = {
  schema: "openagents.pylon.labor_approval_state.v0.3"
  approvals: Record<string, PylonLaborApprovalRecord>
}

export type LaborWorkspace = {
  absolutePath: string
  relativePath: string
  root: string
}

export type LaborRunInput = {
  agentKind: LaborLocalAgentKind
  request: LaborJobRequest
  requestEventId: string
  workspace: LaborWorkspace
  // Resolved, public-safe task detail for the opaque `objectiveRef`. The
  // NIP-LBR kind-5934 request is strictly ref-only (content must be empty), so
  // the provider resolves the public objective text out-of-band (e.g. the
  // openagents.com work-request API) and passes it here. When present it gives
  // the local agent an actionable, self-contained task instead of opaque refs.
  objectiveDetail?: string
}

export type LaborRunCompletion = {
  artifactRefs: string[]
  content: string
  model: string
  receiptRefs: string[]
}

export type LaborRuntime = {
  runLabor(input: LaborRunInput): Promise<LaborRunCompletion>
}

export function laborApprovalStatePath(paths: PylonPaths) {
  return `${paths.home}/labor-approval-state.json`
}

export async function loadLaborApprovalStore(paths: PylonPaths): Promise<PylonLaborApprovalStore> {
  await ensureStateDirectories(paths)
  const path = laborApprovalStatePath(paths)
  if (!existsSync(path)) {
    return { schema: "openagents.pylon.labor_approval_state.v0.3", approvals: {} }
  }
  const parsed = JSON.parse(await readFile(path, "utf8")) as PylonLaborApprovalStore
  return {
    schema: "openagents.pylon.labor_approval_state.v0.3",
    approvals: parsed.approvals ?? {},
  }
}

export async function writeLaborApprovalStore(paths: PylonPaths, store: PylonLaborApprovalStore) {
  assertPublicProjectionSafe(store)
  await ensureStateDirectories(paths)
  await writeFile(laborApprovalStatePath(paths), `${JSON.stringify(store, null, 2)}\n`, { mode: 0o600 })
}

export async function approveLaborFirstRun(input: {
  paths: PylonPaths
  approvedByRef: string
  jobType?: LaborJobType
  now?: Date
  policyRef?: string
}) {
  const jobType = input.jobType ?? "code_task"
  const policyRef = input.policyRef ?? PROVIDER_COMPLIANT_USAGE_LABOR_POLICY_REF
  const store = await loadLaborApprovalStore(input.paths)
  store.approvals[laborApprovalKey(jobType, policyRef)] = {
    approvedAt: (input.now ?? new Date()).toISOString(),
    approvedByRef: input.approvedByRef,
    jobType,
    policyRef,
  }
  await writeLaborApprovalStore(input.paths, store)
  return {
    ok: true,
    approvalRef: stableRef("approval.public.pylon.labor.first_run", `${jobType}:${policyRef}:${input.approvedByRef}`),
    policyRef: PYLON_LABOR_APPROVAL_POLICY_REF,
  }
}

export async function hasLaborFirstRunApproval(paths: PylonPaths, request: LaborJobRequest) {
  const store = await loadLaborApprovalStore(paths)
  return store.approvals[laborApprovalKey(request.jobType, request.policyRef)] !== undefined
}

export function evaluateLaborRequestSafety(request: LaborJobRequest): LaborAdmissionBlocker[] {
  const blockers = new Set<LaborAdmissionBlocker>()
  if (request.policyRef !== PROVIDER_COMPLIANT_USAGE_LABOR_POLICY_REF) {
    blockers.add("labor_policy_mismatch")
  }
  if (containsProviderAuthMaterial(request)) {
    blockers.add("labor_auth_exfiltration_blocked")
  }
  return [...blockers]
}

export function resolveLaborWorkspace(input: {
  root: string
  requestedPath?: string
}): LaborWorkspace | undefined {
  const root = resolve(input.root)
  const requestedPath = input.requestedPath?.trim() || "."
  const absolutePath = resolve(root, requestedPath)
  const relativePath = relative(root, absolutePath) || "."
  if (
    requestedPath.includes("\0") ||
    relativePath.startsWith("..") ||
    relativePath.split(sep).includes("..") ||
    relativePath.split(sep).includes(".git")
  ) {
    return undefined
  }
  return { absolutePath, relativePath, root }
}

export function requestedLaborWorkspacePath(request: LaborJobRequest): string | undefined {
  return request.request.params.find((param) => param.key === "workspace")?.value
}

export function laborPrompt(request: LaborJobRequest, objectiveDetail?: string): string {
  const lines = [
    `OpenAgents labor job: ${request.jobType}`,
    `Policy: ${request.policyRef}`,
  ]
  const detail = objectiveDetail?.trim()
  if (detail) {
    lines.push("", "Objective:", detail)
  }
  lines.push(
    "",
    "Inputs:",
    ...request.inputRefs.map((ref) => `- ${ref}`),
    "",
    "Acceptance criteria:",
    ...request.acceptanceCriteria.map((criterion) => `- ${criterion}`),
  )
  if (request.expectedArtifacts.length > 0) {
    lines.push("", "Expected artifacts:")
    for (const artifact of request.expectedArtifacts) {
      lines.push(`- ${artifact.artifactType}: ${artifact.ref}`)
    }
  }
  if (request.request.content.trim()) {
    lines.push("", "Task detail:", request.request.content.trim())
  }
  lines.push(
    "",
    "Do not print, copy, summarize, or exfiltrate provider credentials, sessions, tokens, wallet material, or private account data.",
    "Return public-safe artifact refs and a concise summary only.",
  )
  return lines.join("\n")
}

export function laborResultContent(input: {
  agentKind: LaborLocalAgentKind
  request: LaborJobRequest
  artifactRefs: readonly string[]
  receiptRefs?: readonly string[]
  summary: string
  workspace: LaborWorkspace
}) {
  const content = JSON.stringify({
    schema: "openagents.pylon.labor_result.v0.3",
    agentKind: input.agentKind,
    artifactRefs: [...input.artifactRefs],
    jobType: input.request.jobType,
    policyRef: input.request.policyRef,
    receiptRefs: [...(input.receiptRefs ?? [])],
    sandbox: {
      policyRef: PYLON_LABOR_SANDBOX_POLICY_REF,
      workspaceRelativePath: input.workspace.relativePath,
    },
    summary: input.summary,
  })
  assertLaborPublicSafe({ content, artifactRefs: input.artifactRefs, receiptRefs: input.receiptRefs ?? [] })
  return content
}

export function assertLaborPublicSafe(value: unknown): void {
  assertPublicProjectionSafe(value)
  if (containsProviderAuthMaterial(value)) {
    throw new Error("labor projection contains provider auth material")
  }
}

export function detectConfiguredLaborAgent(input: {
  env?: Readonly<Record<string, string | undefined>>
  which?: (name: string) => string | null
} = {}): LaborLocalAgentKind | null {
  const env = input.env ?? process.env
  const configured = env.PYLON_LABOR_AGENT?.trim().toLowerCase()
  if (configured === "codex" || configured === "opencode" || configured === "claude_code") return configured
  const which = input.which ?? ((name: string) => Bun.which(name))
  if (which("codex")) return "codex"
  if (which("opencode")) return "opencode"
  if (which("claude")) return "claude_code"
  return null
}

export function makeConfiguredLaborRuntime(input: {
  env?: Readonly<Record<string, string | undefined>>
  spawn?: typeof Bun.spawn
  which?: (name: string) => string | null
} = {}): LaborRuntime {
  const env = input.env ?? process.env
  const spawn = input.spawn ?? Bun.spawn
  return {
    async runLabor(run) {
      const agentKind = run.agentKind
      const prompt = laborPrompt(run.request, run.objectiveDetail)
      const command = laborCommand(agentKind, prompt, env, input.which)
      if (command === null) {
        throw new Error("no configured local labor agent found")
      }
      await mkdir(run.workspace.absolutePath, { recursive: true })
      const proc = spawn(command, {
        cwd: run.workspace.absolutePath,
        stdout: "pipe",
        stderr: "pipe",
      })
      const [stdout, stderr, exitCode] = await Promise.all([
        new Response(proc.stdout).text(),
        new Response(proc.stderr).text(),
        proc.exited,
      ])
      if (exitCode !== 0) {
        throw new Error(`local labor agent failed: ${stderr.trim() || `exit ${exitCode}`}`)
      }
      const artifactRef = stableRef("artifact.public.pylon.labor", `${run.requestEventId}:${stdout}`)
      return {
        artifactRefs: [artifactRef],
        content: laborResultContent({
          agentKind,
          request: run.request,
          artifactRefs: [artifactRef],
          receiptRefs: [stableRef("receipt.public.pylon.labor.agent", `${agentKind}:${run.requestEventId}`)],
          summary: stdout.trim() || "Local labor agent completed without text output.",
          workspace: run.workspace,
        }),
        model: agentKind,
        receiptRefs: [stableRef("receipt.public.pylon.labor.agent", `${agentKind}:${run.requestEventId}`)],
      }
    },
  }
}

function laborCommand(
  agentKind: LaborLocalAgentKind,
  prompt: string,
  env: Readonly<Record<string, string | undefined>>,
  which?: (name: string) => string | null,
): string[] | null {
  const configuredCommand = env.PYLON_LABOR_AGENT_COMMAND?.trim()
  if (configuredCommand) {
    return configuredCommand.split(/\s+/).concat([prompt])
  }
  const find = which ?? ((name: string) => Bun.which(name))
  if (agentKind === "codex") {
    const path = find("codex")
    // `codex exec` in the bounded labor workspace is a fresh non-git temp dir,
    // so bare `codex exec` blocks on the interactive git-repo-check and never
    // returns under the provider loop. `--skip-git-repo-check` clears that;
    // `-s workspace-write` keeps codex sandboxed to the workspace (it must NOT
    // run untrusted requester work with `--dangerously-bypass-...`); and
    // `network_access=false` denies the sandbox network so an untrusted job
    // cannot clone/fetch a repo into the workspace (observed: codex otherwise
    // cloned the target repo, polluting the sandbox so `bun test` ran the whole
    // suite). Output-only labor stays self-contained in the workspace.
    return path
      ? [
          path,
          "exec",
          "--skip-git-repo-check",
          "-s",
          "workspace-write",
          "-c",
          "sandbox_workspace_write.network_access=false",
          prompt,
        ]
      : null
  }
  if (agentKind === "opencode") {
    const path = find("opencode")
    return path ? [path, "run", prompt, "--format", "json"] : null
  }
  if (agentKind === "claude_code") {
    const path = find("claude")
    return path ? [path, "-p", prompt] : null
  }
  return null
}

function laborApprovalKey(jobType: LaborJobType, policyRef: string) {
  return `${jobType}:${policyRef}`
}

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

function containsProviderAuthMaterial(value: unknown): boolean {
  const serialized = typeof value === "string" ? value : JSON.stringify(value)
  return /(\bBearer\s+[A-Za-z0-9._~+/-]{10,}|sk-[A-Za-z0-9_-]{20,}|AIza[A-Za-z0-9_-]{20,}|OPENAI_API_KEY|ANTHROPIC_API_KEY|BREEZ_API_KEY|refresh_token|access_token|id_token|session_token|auth\.json|provider_auth|provider_secret|api_key|client_secret)/i.test(
    serialized,
  )
}
