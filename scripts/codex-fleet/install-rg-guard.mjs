#!/usr/bin/env node
import { chmodSync, mkdirSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { fileURLToPath } from "node:url"

export const CODEX_RG_GUARD_EXCLUDE_GLOBS = [
  "!node_modules/**",
  "!**/node_modules/**",
  "!.git/**",
  "!**/.git/**",
  "!dist/**",
  "!**/dist/**",
  "!build/**",
  "!**/build/**",
]

export function isUnsafeRipgrepArg(arg) {
  return (
    /^-u+$/.test(arg) ||
    arg === "--unrestricted" ||
    arg === "--hidden" ||
    arg === "--follow" ||
    arg === "--no-ignore" ||
    arg.startsWith("--no-ignore-")
  )
}

export function sanitizeRipgrepArgs(args) {
  const sanitized = []
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
    if (isUnsafeRipgrepArg(arg)) continue
    sanitized.push(arg)
  }
  return sanitized
}

export function guardedRipgrepArgs(args) {
  const sanitized = sanitizeRipgrepArgs(args)
  const endOfOptionsIndex = sanitized.indexOf("--")
  const guardArgs = CODEX_RG_GUARD_EXCLUDE_GLOBS.flatMap((glob) => ["--glob", glob])
  if (endOfOptionsIndex === -1) return [...sanitized, ...guardArgs]
  return [
    ...sanitized.slice(0, endOfOptionsIndex),
    ...guardArgs,
    ...sanitized.slice(endOfOptionsIndex),
  ]
}

function singleQuote(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`
}

export function rgGuardShell(realRg) {
  const guardGlobs = CODEX_RG_GUARD_EXCLUDE_GLOBS.map((glob) => singleQuote(glob)).join(" ")
  return `#!/usr/bin/env bash
set -euo pipefail
REAL_RG=${singleQuote(realRg)}
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

export function installRipgrepGuard({ binDir, realRg }) {
  if (!binDir) throw new Error("missing binDir")
  if (!realRg) throw new Error("missing realRg")
  mkdirSync(binDir, { recursive: true })
  const wrapperPath = join(binDir, "rg")
  writeFileSync(wrapperPath, rgGuardShell(realRg), { mode: 0o755 })
  chmodSync(wrapperPath, 0o755)
  return wrapperPath
}

function usage() {
  return "usage: install-rg-guard.mjs --bin-dir <dir> --real-rg <path>"
}

function parseArgs(argv) {
  const out = { binDir: "", realRg: "" }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === "--bin-dir" && argv[i + 1]) {
      out.binDir = argv[i + 1]
      i += 1
    } else if (arg === "--real-rg" && argv[i + 1]) {
      out.realRg = argv[i + 1]
      i += 1
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage())
      process.exit(0)
    } else {
      throw new Error(`unknown arg: ${arg}`)
    }
  }
  return out
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    const parsed = parseArgs(process.argv.slice(2))
    const wrapperPath = installRipgrepGuard(parsed)
    console.log(JSON.stringify({ ok: true, wrapperPath }))
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    console.error(usage())
    process.exit(2)
  }
}
