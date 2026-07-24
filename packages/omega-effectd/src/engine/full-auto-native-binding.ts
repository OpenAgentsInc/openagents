/**
 * FA-06: native Zed/Omega project + worktree binding for Full Auto runs.
 *
 * Zed owns project/worktree/buffer/Git truth. OpenAgents admits run completion.
 * This module stores the join refs only — it never mutates buffers or worktrees.
 */

import { createHash } from "node:crypto"
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

export const FULL_AUTO_NATIVE_BINDING_SCHEMA =
  "openagents.omega.full_auto_native_binding.v1" as const

export type FullAutoNativeBinding = Readonly<{
  runRef: string
  workspaceRef: string
  projectRef: string
  worktreeRef: string
  /** SHA-256 of the absolute worktree path — never the raw path in public receipts. */
  worktreePathDigest: string | null
  gitHead: string | null
  rebaseUnsafe: boolean
  boundAt: string
}>

export type FullAutoNativeEvidence = Readonly<{
  projectRef: string
  worktreeRef: string
  worktreePathDigest: string | null
  gitHead: string | null
}>

export type FullAutoNativeBoundaryAssessment =
  | Readonly<{ ok: true; evidence: FullAutoNativeEvidence }>
  | Readonly<{
      ok: false
      reason:
        | "missing_binding"
        | "workspace_mismatch"
        | "rebase_unsafe"
        | "stale_worktree"
      message: string
    }>

const digestPath = (absolutePath: string): string =>
  createHash("sha256").update(absolutePath).digest("hex")

export const buildFullAutoNativeBinding = (input: Readonly<{
  runRef: string
  workspaceRef: string
  projectRef: string
  worktreeRef: string
  worktreeAbsolutePath?: string
  gitHead?: string
  rebaseUnsafe?: boolean
  now?: () => Date
}>): FullAutoNativeBinding => ({
  runRef: input.runRef,
  workspaceRef: input.workspaceRef,
  projectRef: input.projectRef,
  worktreeRef: input.worktreeRef,
  worktreePathDigest:
    typeof input.worktreeAbsolutePath === "string" && input.worktreeAbsolutePath.length > 0
      ? digestPath(input.worktreeAbsolutePath)
      : null,
  gitHead: input.gitHead ?? null,
  rebaseUnsafe: input.rebaseUnsafe === true,
  boundAt: (input.now ?? (() => new Date))().toISOString(),
})

export const projectFullAutoNativeEvidence = (
  binding: FullAutoNativeBinding,
): FullAutoNativeEvidence => ({
  projectRef: binding.projectRef,
  worktreeRef: binding.worktreeRef,
  worktreePathDigest: binding.worktreePathDigest,
  gitHead: binding.gitHead,
})

export const assessFullAutoNativeBoundary = (input: Readonly<{
  binding: FullAutoNativeBinding | null
  expectedWorkspaceRef: string
  currentWorktreePathDigest?: string | null
}>): FullAutoNativeBoundaryAssessment => {
  if (input.binding === null) {
    return {
      ok: false,
      reason: "missing_binding",
      message: "No native project/worktree binding exists for this Full Auto run.",
    }
  }
  if (input.binding.workspaceRef !== input.expectedWorkspaceRef) {
    return {
      ok: false,
      reason: "workspace_mismatch",
      message: "The bound workspace does not match the currently resolved workspace.",
    }
  }
  if (input.binding.rebaseUnsafe) {
    return {
      ok: false,
      reason: "rebase_unsafe",
      message: "The bound worktree is rebase-unsafe; Full Auto refuses to continue.",
    }
  }
  if (
    typeof input.currentWorktreePathDigest === "string" &&
    input.binding.worktreePathDigest !== null &&
    input.currentWorktreePathDigest !== input.binding.worktreePathDigest
  ) {
    return {
      ok: false,
      reason: "stale_worktree",
      message: "The worktree path digest no longer matches the binding recorded at start.",
    }
  }
  return { ok: true, evidence: projectFullAutoNativeEvidence(input.binding) }
}

type BindingFile = Readonly<{
  schema: typeof FULL_AUTO_NATIVE_BINDING_SCHEMA
  bindings: ReadonlyArray<FullAutoNativeBinding>
}>

export type FullAutoNativeBindingStore = Readonly<{
  get: (runRef: string) => FullAutoNativeBinding | null
  put: (binding: FullAutoNativeBinding) => FullAutoNativeBinding
  list: () => ReadonlyArray<FullAutoNativeBinding>
}>

export const openFullAutoNativeBindingStore = (file: string): FullAutoNativeBindingStore => {
  const filePath = path.resolve(file)
  const parent = path.dirname(filePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })

  let bindings: FullAutoNativeBinding[] = []
  if (existsSync(filePath)) {
    try {
      const parsed = JSON.parse(readFileSync(filePath, "utf8")) as BindingFile
      if (parsed.schema === FULL_AUTO_NATIVE_BINDING_SCHEMA && Array.isArray(parsed.bindings)) {
        bindings = [...parsed.bindings]
      }
    } catch {
      bindings = []
    }
  }

  const persist = (): void => {
    const tmp = `${filePath}.${process.pid}.tmp`
    writeFileSync(
      tmp,
      JSON.stringify(
        { schema: FULL_AUTO_NATIVE_BINDING_SCHEMA, bindings } satisfies BindingFile,
        null,
        2,
      ),
      { mode: 0o600 },
    )
    renameSync(tmp, filePath)
  }

  return {
    get: runRef => bindings.find(entry => entry.runRef === runRef) ?? null,
    put: binding => {
      const index = bindings.findIndex(entry => entry.runRef === binding.runRef)
      if (index === -1) bindings.push(binding)
      else bindings[index] = binding
      persist()
      return binding
    },
    list: () => [...bindings],
  }
}
