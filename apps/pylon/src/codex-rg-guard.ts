import { accessSync, chmodSync, constants, mkdirSync, writeFileSync } from "node:fs"
import { delimiter, join } from "node:path"
import { tmpdir } from "node:os"

export const CODEX_RG_GUARD_ENV = "OPENAGENTS_CODEX_RG_GUARD"
export const CODEX_RG_GUARD_DISABLED_ENV = "OPENAGENTS_CODEX_RG_GUARD_DISABLED"
export const CODEX_RG_GUARD_BIN_DIR_ENV = "OPENAGENTS_CODEX_RG_GUARD_BIN_DIR"
export const CODEX_REAL_RG_ENV = "OPENAGENTS_CODEX_REAL_RG"

export const CODEX_RG_GUARD_EXCLUDE_GLOBS = [
  "!node_modules/**",
  "!**/node_modules/**",
  "!.git/**",
  "!**/.git/**",
  "!dist/**",
  "!**/dist/**",
  "!build/**",
  "!**/build/**",
] as const

export interface CodexRipgrepGuardInstall {
  env: Record<string, string>
  installed: boolean
  binDir: string | null
  wrapperPath: string | null
  realRipgrepPath: string | null
}

export function isCodexRipgrepUnsafeArg(arg: string): boolean {
  return (
    /^-u+$/.test(arg) ||
    arg === "--unrestricted" ||
    arg === "--hidden" ||
    arg === "--follow" ||
    arg === "--no-ignore" ||
    arg.startsWith("--no-ignore-")
  )
}

export function sanitizeCodexRipgrepArgs(args: ReadonlyArray<string>): string[] {
  const sanitized: string[] = []
  let endOfOptions = false
  for (const arg of args) {
    if (endOfOptions) {
      sanitized.push(arg)
      continue
    }
    if (arg === "--") {
      sanitized.push(arg)
      endOfOptions = true
      continue
    }
    if (isCodexRipgrepUnsafeArg(arg)) continue
    sanitized.push(arg)
  }
  return sanitized
}

export function guardedCodexRipgrepArgs(args: ReadonlyArray<string>): string[] {
  const sanitized = sanitizeCodexRipgrepArgs(args)
  const endOfOptionsIndex = sanitized.indexOf("--")
  const guardArgs = CODEX_RG_GUARD_EXCLUDE_GLOBS.flatMap((glob) => ["--glob", glob])
  if (endOfOptionsIndex === -1) return [...sanitized, ...guardArgs]
  return [
    ...sanitized.slice(0, endOfOptionsIndex),
    ...guardArgs,
    ...sanitized.slice(endOfOptionsIndex),
  ]
}

function singleQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`
}

export function codexRipgrepGuardShell(realRipgrepPath: string): string {
  const guardGlobs = CODEX_RG_GUARD_EXCLUDE_GLOBS.map((glob) => singleQuote(glob)).join(" ")
  return `#!/usr/bin/env bash
set -euo pipefail
REAL_RG=${singleQuote(realRipgrepPath)}
guard_globs=(${guardGlobs})
before=()
after=()
seen_end=0
for arg in "$@"; do
  if [[ "$seen_end" == "1" ]]; then
    after+=("$arg")
    continue
  fi
  if [[ "$arg" == "--" ]]; then
    seen_end=1
    after+=("$arg")
    continue
  fi
  case "$arg" in
    -u|-uu|-uuu|--unrestricted|--hidden|--follow|--no-ignore|--no-ignore-*)
      continue
      ;;
  esac
  before+=("$arg")
done
guard_args=()
for glob in "\${guard_globs[@]}"; do
  guard_args+=(--glob "$glob")
done
exec "$REAL_RG" "\${before[@]}" "\${guard_args[@]}" "\${after[@]}"
`
}

function normalizedEnv(env: Record<string, string | undefined>): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) out[key] = value
  }
  return out
}

function findExecutableOnPath(name: string, pathValue: string | undefined): string | null {
  const entries = (pathValue ?? "").split(delimiter).filter((entry) => entry.length > 0)
  for (const entry of entries) {
    const candidate = join(entry, name)
    try {
      accessSync(candidate, constants.X_OK)
      return candidate
    } catch {
      // Continue scanning PATH.
    }
  }
  return null
}

function prependPathOnce(pathValue: string | undefined, binDir: string): string {
  const entries = (pathValue ?? "").split(delimiter).filter((entry) => entry.length > 0)
  if (entries.includes(binDir)) return pathValue ?? binDir
  return entries.length === 0 ? binDir : `${binDir}${delimiter}${entries.join(delimiter)}`
}

export function installCodexRipgrepGuard(input: {
  env: Record<string, string | undefined>
  binDir?: string
  realRipgrepPath?: string
}): CodexRipgrepGuardInstall {
  const env = normalizedEnv(input.env)
  if (env[CODEX_RG_GUARD_DISABLED_ENV] === "1" || process.platform === "win32") {
    return { env, installed: false, binDir: null, wrapperPath: null, realRipgrepPath: null }
  }

  const realRipgrepPath =
    input.realRipgrepPath ??
    env[CODEX_REAL_RG_ENV] ??
    findExecutableOnPath("rg", env.PATH ?? process.env.PATH)
  if (realRipgrepPath === null) {
    return { env, installed: false, binDir: null, wrapperPath: null, realRipgrepPath: null }
  }

  const binDir = input.binDir ?? env[CODEX_RG_GUARD_BIN_DIR_ENV] ?? join(tmpdir(), "openagents-codex-rg-guard", "bin")
  mkdirSync(binDir, { recursive: true })
  const wrapperPath = join(binDir, "rg")
  writeFileSync(wrapperPath, codexRipgrepGuardShell(realRipgrepPath), { mode: 0o755 })
  chmodSync(wrapperPath, 0o755)

  const next = {
    ...env,
    [CODEX_RG_GUARD_ENV]: "1",
    [CODEX_REAL_RG_ENV]: realRipgrepPath,
    PATH: prependPathOnce(env.PATH ?? process.env.PATH, binDir),
  }
  return { env: next, installed: true, binDir, wrapperPath, realRipgrepPath }
}
