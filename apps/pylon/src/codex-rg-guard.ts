import { accessSync, chmodSync, constants, mkdirSync, writeFileSync } from "node:fs"
import { delimiter, isAbsolute, join, resolve } from "node:path"
import { tmpdir } from "node:os"

export const CODEX_RG_GUARD_ENV = "OPENAGENTS_CODEX_RG_GUARD"
export const CODEX_RG_GUARD_DISABLED_ENV = "OPENAGENTS_CODEX_RG_GUARD_DISABLED"
export const CODEX_RG_GUARD_BIN_DIR_ENV = "OPENAGENTS_CODEX_RG_GUARD_BIN_DIR"
export const CODEX_REAL_RG_ENV = "OPENAGENTS_CODEX_REAL_RG"
export const CODEX_REAL_FIND_ENV = "OPENAGENTS_CODEX_REAL_FIND"
export const CODEX_WORKSPACE_ROOT_ENV = "OPENAGENTS_CODEX_WORKSPACE_ROOT"

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

function isWithinWorkspace(path: string, workspaceRoot: string): boolean {
  const root = resolve(workspaceRoot)
  const resolved = isAbsolute(path) ? resolve(path) : resolve(root, path)
  return resolved === root || resolved.startsWith(`${root}/`)
}

function isPathLikeArg(arg: string): boolean {
  return arg === "." ||
    arg === ".." ||
    arg.startsWith("./") ||
    arg.startsWith("../") ||
    arg.startsWith("/") ||
    arg.startsWith("~/")
}

function workspaceRelativePathArg(arg: string, workspaceRoot: string): string {
  if (arg === ".") return "."
  if (isWithinWorkspace(arg, workspaceRoot)) return arg
  return "."
}

export function scopeCodexRipgrepArgsToWorkspace(
  args: ReadonlyArray<string>,
  workspaceRoot: string,
): string[] {
  const scoped: string[] = []
  let endOfOptions = false
  let sawPattern = false
  let optionValuePending: "pattern" | "other" | null = null
  for (const arg of args) {
    if (endOfOptions) {
      scoped.push(arg)
      continue
    }
    if (optionValuePending) {
      scoped.push(arg)
      if (optionValuePending === "pattern") sawPattern = true
      optionValuePending = null
      continue
    }
    if (arg === "--") {
      scoped.push(arg)
      endOfOptions = true
      continue
    }
    if (!sawPattern && arg.startsWith("-")) {
      scoped.push(arg)
      if (
        !arg.includes("=") &&
        ["-e", "-f", "-g", "--regexp", "--file", "--glob", "--type", "-t", "--type-not", "-T"].includes(arg)
      ) {
        optionValuePending = ["-e", "-f", "--regexp", "--file"].includes(arg) ? "pattern" : "other"
      }
      continue
    }
    if (!sawPattern) {
      sawPattern = true
      scoped.push(arg)
      continue
    }
    scoped.push(isPathLikeArg(arg) ? workspaceRelativePathArg(arg, workspaceRoot) : arg)
  }
  return scoped
}

export function scopeCodexFindArgsToWorkspace(
  args: ReadonlyArray<string>,
  workspaceRoot: string,
): string[] {
  const scoped: string[] = []
  let sawExpression = false
  for (const arg of args) {
    if (sawExpression) {
      scoped.push(arg)
      continue
    }
    if (arg.startsWith("-") || arg === "(" || arg === "!" || arg === "\\(") {
      sawExpression = true
      scoped.push(arg)
      continue
    }
    scoped.push(isPathLikeArg(arg) ? workspaceRelativePathArg(arg, workspaceRoot) : arg)
  }
  return scoped.length === 0 ? ["."] : scoped
}

function singleQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`
}

export function codexRipgrepGuardShell(realRipgrepPath: string): string {
  const guardGlobs = CODEX_RG_GUARD_EXCLUDE_GLOBS.map((glob) => singleQuote(glob)).join(" ")
  return `#!/usr/bin/env bash
set -euo pipefail
REAL_RG=${singleQuote(realRipgrepPath)}
workspace_root="\${${CODEX_WORKSPACE_ROOT_ENV}:-}"
guard_globs=(${guardGlobs})
before=()
after=()
seen_end=0
saw_pattern=0
option_value_pending=none
is_within_workspace() {
  local candidate="$1"
  [[ -z "$workspace_root" ]] && return 0
  python3 - "$workspace_root" "$candidate" <<'PY'
import os, sys
root = os.path.abspath(sys.argv[1])
candidate = sys.argv[2]
path = os.path.abspath(candidate if os.path.isabs(candidate) else os.path.join(root, candidate))
sys.exit(0 if path == root or path.startswith(root + os.sep) else 1)
PY
}
root_scoped_arg() {
  local candidate="$1"
  if [[ "$candidate" == "." ]] || is_within_workspace "$candidate"; then
    printf '%s\\n' "$candidate"
  else
    printf '.\\n'
  fi
}
for arg in "$@"; do
  if [[ "$seen_end" == "1" ]]; then
    after+=("$arg")
    continue
  fi
  if [[ "$option_value_pending" != "none" ]]; then
    before+=("$arg")
    if [[ "$option_value_pending" == "pattern" ]]; then
      saw_pattern=1
    fi
    option_value_pending=none
    continue
  fi
  if [[ "$arg" == "--" ]]; then
    seen_end=1
    after+=("$arg")
    continue
  fi
  if [[ "$saw_pattern" == "0" ]]; then
    case "$arg" in
      -u|-uu|-uuu|--unrestricted|--hidden|--follow|--no-ignore|--no-ignore-*)
        continue
        ;;
      -e|-f|--regexp|--file)
        before+=("$arg")
        option_value_pending=pattern
        continue
        ;;
      -g|--glob|--type|-t|--type-not|-T)
        before+=("$arg")
        option_value_pending=other
        continue
        ;;
      -*)
        before+=("$arg")
        continue
        ;;
    esac
    saw_pattern=1
    before+=("$arg")
    continue
  fi
  case "$arg" in
    /*|~/*|../*|..|./*|.)
      before+=("$(root_scoped_arg "$arg")")
      continue
      ;;
  esac
  before+=("$arg")
done
guard_args=()
for glob in "\${guard_globs[@]}"; do
  guard_args+=(--glob "$glob")
done
cd "\${workspace_root:-.}"
exec "$REAL_RG" "\${before[@]}" "\${guard_args[@]}" "\${after[@]}"
`
}

export function codexFindGuardShell(realFindPath: string): string {
  return `#!/usr/bin/env bash
set -euo pipefail
REAL_FIND=${singleQuote(realFindPath)}
workspace_root="\${${CODEX_WORKSPACE_ROOT_ENV}:-}"
args=()
saw_expression=0
is_within_workspace() {
  local candidate="$1"
  [[ -z "$workspace_root" ]] && return 0
  python3 - "$workspace_root" "$candidate" <<'PY'
import os, sys
root = os.path.abspath(sys.argv[1])
candidate = sys.argv[2]
path = os.path.abspath(candidate if os.path.isabs(candidate) else os.path.join(root, candidate))
sys.exit(0 if path == root or path.startswith(root + os.sep) else 1)
PY
}
root_scoped_arg() {
  local candidate="$1"
  if [[ "$candidate" == "." ]] || is_within_workspace "$candidate"; then
    printf '%s\\n' "$candidate"
  else
    printf '.\\n'
  fi
}
for arg in "$@"; do
  if [[ "$saw_expression" == "1" ]]; then
    args+=("$arg")
    continue
  fi
  case "$arg" in
    -\\(*|-*|\\(|!)
      saw_expression=1
      args+=("$arg")
      continue
      ;;
    /*|~/*|../*|..|./*|.)
      args+=("$(root_scoped_arg "$arg")")
      continue
      ;;
  esac
  args+=("$arg")
done
if [[ "\${#args[@]}" == "0" ]]; then
  args=(.)
fi
cd "\${workspace_root:-.}"
exec "$REAL_FIND" "\${args[@]}"
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
  workspaceRoot?: string
  realFindPath?: string
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
  const realFindPath = input.realFindPath ?? env[CODEX_REAL_FIND_ENV] ?? findExecutableOnPath("find", env.PATH ?? process.env.PATH)
  if (realFindPath !== null) {
    const findWrapperPath = join(binDir, "find")
    writeFileSync(findWrapperPath, codexFindGuardShell(realFindPath), { mode: 0o755 })
    chmodSync(findWrapperPath, 0o755)
  }

  const next = {
    ...env,
    [CODEX_RG_GUARD_ENV]: "1",
    [CODEX_REAL_RG_ENV]: realRipgrepPath,
    ...(realFindPath === null ? {} : { [CODEX_REAL_FIND_ENV]: realFindPath }),
    ...(input.workspaceRoot === undefined ? {} : { [CODEX_WORKSPACE_ROOT_ENV]: resolve(input.workspaceRoot) }),
    PATH: prependPathOnce(env.PATH ?? process.env.PATH, binDir),
  }
  return { env: next, installed: true, binDir, wrapperPath, realRipgrepPath }
}
