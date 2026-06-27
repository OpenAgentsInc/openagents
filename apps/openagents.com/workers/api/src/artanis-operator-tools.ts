// Artanis operator TOOLS (#6365 repo-read, #6366 Codex dispatch) for the bounded
// tool-calling loop in `artanis-operator.ts` (#6364).
//
// These are the concrete owner-scoped tools Artanis can invoke during a turn:
//
//   - #6365 read_repo_file / list_repo_dir — READ tools over the PUBLIC
//     `OpenAgentsInc/openagents` repo via the GitHub contents APIs. Bounded
//     size, public repo only, secret-path denylist. This is what lets Artanis
//     read e.g. `docs/khala/2026-06-26-khala-open-issues-master-roadmap.md`
//     himself and reason over its real contents.
//   - #6366 dispatch_codex_task — a RISKY tool (`pylon_job_dispatch`). It NEVER
//     executes in the loop; its `plan(args)` returns the EXACT public-safe
//     Khala -> Pylon -> Codex dispatch it WOULD run (per the AGENTS.md runbook),
//     and the loop frames that as requiring owner approval. No new spend
//     authority is granted here; live dispatch stays gated behind
//     `artanis-approval-gates`.
//
// Authority discipline: read tools are public-state-only and side-effect-free;
// the dispatch tool is plan-only. No secrets, tokens, mnemonics, payout targets,
// wallet material, raw prompts, or private-repo content ever enter a tool's
// inputs or outputs.

import { Effect } from 'effect'

import type {
  ArtanisOperatorReadTool,
  ArtanisOperatorRiskyTool,
  ArtanisOperatorTool,
} from './artanis-operator'

// ---------------------------------------------------------------------------
// #6365 — repo-read tools (public OpenAgentsInc/openagents only).
// ---------------------------------------------------------------------------

// The public repo the read tools are scoped to. Fixed: these tools NEVER read a
// private repo or an arbitrary owner/repo the model might name.
export const ARTANIS_REPO_READ_OWNER = 'OpenAgentsInc'
export const ARTANIS_REPO_READ_REPO = 'openagents'
export const ARTANIS_REPO_READ_REF = 'main'

// Max bytes returned from a single file read. Larger files are truncated with an
// explicit marker so the model knows the read was bounded.
export const ARTANIS_REPO_READ_MAX_BYTES = 256 * 1024

export type ArtanisRepoReadConfig = Readonly<{
  owner?: string | undefined
  repo?: string | undefined
  ref?: string | undefined
  maxBytes?: number | undefined
  // Injected for testability; defaults to the global fetch.
  fetchImpl?: typeof fetch | undefined
}>

// Deny paths that could point at secrets/credentials/local state even though the
// public repo should not contain them. Mirrors the redaction discipline in
// `artanis-approval-gates.ts`. Honest absence: a blocked path returns a clear
// "(blocked …)" string, never a fabricated file.
const DENIED_REPO_PATH_PATTERN =
  /(^|\/)(\.secrets|\.env|\.git|node_modules|target|dist)(\/|$)|secret|mnemonic|wallet|auth\.json|\.pem$|\.key$|id_rsa|credentials/i

// A safe repo path: a relative POSIX-ish path with no traversal, no leading
// slash, no NUL, only conservative characters.
export const isSafeArtanisRepoPath = (path: string): boolean => {
  if (typeof path !== 'string') return false
  const trimmed = path.trim()
  if (trimmed === '') return false
  if (trimmed.startsWith('/')) return false
  if (trimmed.includes('..')) return false
  if (trimmed.includes('\0')) return false
  if (trimmed.includes('//')) return false
  if (DENIED_REPO_PATH_PATTERN.test(trimmed)) return false
  return /^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(trimmed)
}

// Pull a string `path` argument out of model-produced tool args.
const readPathArg = (args: unknown): string | undefined => {
  if (typeof args !== 'object' || args === null) return undefined
  const value = (args as Record<string, unknown>).path
  return typeof value === 'string' ? value : undefined
}

const resolveRepoReadConfig = (config: ArtanisRepoReadConfig) => ({
  fetchImpl: config.fetchImpl ?? globalThis.fetch,
  maxBytes: config.maxBytes ?? ARTANIS_REPO_READ_MAX_BYTES,
  owner: config.owner ?? ARTANIS_REPO_READ_OWNER,
  ref: config.ref ?? ARTANIS_REPO_READ_REF,
  repo: config.repo ?? ARTANIS_REPO_READ_REPO,
})

// read_repo_file(path) — reads a file from the public repo via
// raw.githubusercontent.com. Bounded, public-only, secret-path-denied.
export const makeArtanisReadRepoFileTool = (
  config: ArtanisRepoReadConfig = {},
): ArtanisOperatorReadTool => {
  const { fetchImpl, maxBytes, owner, ref, repo } =
    resolveRepoReadConfig(config)

  return {
    definition: {
      description: `Read a UTF-8 text file from the PUBLIC ${owner}/${repo} repo (branch ${ref}). Use for docs and source, e.g. "docs/khala/2026-06-26-khala-open-issues-master-roadmap.md". Public repo only; bounded size; secret paths are blocked.`,
      name: 'read_repo_file',
      parameters: {
        additionalProperties: false,
        properties: {
          path: {
            description:
              'Repo-relative path to the file, e.g. "docs/khala/2026-06-26-khala-open-issues-master-roadmap.md". No leading slash, no "..".',
            type: 'string',
          },
        },
        required: ['path'],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        const path = readPathArg(args)
        if (path === undefined) {
          return '(invalid arguments: a string "path" is required)'
        }
        if (!isSafeArtanisRepoPath(path)) {
          return `(blocked: "${path}" is not an allowed public-repo path)`
        }

        const url = `https://raw.githubusercontent.com/${owner}/${repo}/${ref}/${path}`
        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, {
            headers: { 'User-Agent': 'artanis-operator' },
          }),
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (response === undefined) {
          return `(could not fetch "${path}")`
        }
        if (response.status === 404) {
          return `(file not found: "${path}")`
        }
        if (!response.ok) {
          return `(read failed for "${path}": status ${response.status})`
        }

        const text = yield* Effect.tryPromise(() => response.text()).pipe(
          Effect.orElseSucceed(() => ''),
        )
        if (text.length > maxBytes) {
          return `${text.slice(0, maxBytes)}\n\n(…truncated at ${maxBytes} bytes; the file is longer)`
        }
        if (text === '') {
          return `(empty file: "${path}")`
        }
        return text
      }),
    kind: 'read',
  }
}

type GitHubContentsEntry = Readonly<{
  name?: unknown
  type?: unknown
}>

// list_repo_dir(path) — lists a directory in the public repo via the GitHub
// contents API. Returns one "name (type)" line per entry.
export const makeArtanisListRepoDirTool = (
  config: ArtanisRepoReadConfig = {},
): ArtanisOperatorReadTool => {
  const { fetchImpl, owner, ref, repo } = resolveRepoReadConfig(config)

  return {
    definition: {
      description: `List a directory in the PUBLIC ${owner}/${repo} repo (branch ${ref}). Pass "" or "." for the repo root. Returns each entry as "name (file|dir)".`,
      name: 'list_repo_dir',
      parameters: {
        additionalProperties: false,
        properties: {
          path: {
            description:
              'Repo-relative directory path, e.g. "docs/khala". Use "" for the repo root. No leading slash, no "..".',
            type: 'string',
          },
        },
        required: ['path'],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        const raw = readPathArg(args) ?? ''
        const path = raw.trim() === '.' ? '' : raw.trim()
        if (path !== '' && !isSafeArtanisRepoPath(path)) {
          return `(blocked: "${path}" is not an allowed public-repo path)`
        }

        const url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}?ref=${ref}`
        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, {
            headers: {
              Accept: 'application/vnd.github+json',
              'User-Agent': 'artanis-operator',
            },
          }),
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (response === undefined) {
          return `(could not list "${path}")`
        }
        if (response.status === 404) {
          return `(directory not found: "${path}")`
        }
        if (!response.ok) {
          return `(list failed for "${path}": status ${response.status})`
        }

        const body = yield* Effect.tryPromise(
          () => response.json() as Promise<unknown>,
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (!Array.isArray(body)) {
          return `("${path}" is not a directory)`
        }
        const entries = (body as ReadonlyArray<GitHubContentsEntry>)
          .map(entry => {
            const name = typeof entry.name === 'string' ? entry.name : null
            const type = entry.type === 'dir' ? 'dir' : 'file'
            return name === null ? null : `${name} (${type})`
          })
          .filter((line): line is string => line !== null)
          .sort()

        if (entries.length === 0) {
          return `(empty directory: "${path === '' ? '.' : path}")`
        }
        return [
          `Contents of ${owner}/${repo}/${path === '' ? '.' : path}:`,
          ...entries.map(line => `- ${line}`),
        ].join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// #6366 — dispatch_codex_task (RISKY: pylon_job_dispatch; plan-only).
// ---------------------------------------------------------------------------

const asString = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const asStringArray = (value: unknown): ReadonlyArray<string> =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : []

// Reject obviously private/unsafe material in a dispatch field, mirroring the
// approval-gate redaction discipline. The dispatch tool is public-safe-only.
const DISPATCH_UNSAFE_PATTERN =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|ghp_[A-Za-z0-9_]+|gho_[A-Za-z0-9_]+|lnbc|lntb|lnurl|macaroon|mnemonic|oauth|payout|preimage|private[_-]?key|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const dispatchFieldIsSafe = (value: string): boolean =>
  !DISPATCH_UNSAFE_PATTERN.test(value)

// dispatch_codex_task — a risky (pylon_job_dispatch) tool. It does NOT execute;
// `plan` returns the exact, public-safe Khala -> Pylon -> Codex dispatch it
// WOULD run so the owner can approve the scope. Live execution stays gated.
export const makeArtanisDispatchCodexTaskTool = (
  config: Readonly<{ defaultBranch?: string | undefined }> = {},
): ArtanisOperatorRiskyTool => {
  const defaultBranch = config.defaultBranch ?? 'main'

  return {
    definition: {
      description:
        'Plan a parallel Codex coding task dispatch through the Khala -> Pylon -> Codex burndown loop against the owner\'s linked Codex accounts. This is a gated (pylon_job_dispatch) action: it is NOT executed; it returns the exact public-safe dispatch that would run, pending owner approval. Inputs MUST be public-safe (public issue numbers, public file paths, public verification commands) — no secrets, tokens, or private content.',
      name: 'dispatch_codex_task',
      parameters: {
        additionalProperties: false,
        properties: {
          branch: {
            description: `Git branch to pin (default "${defaultBranch}").`,
            type: 'string',
          },
          filePaths: {
            description: 'Public repo-relative file paths the task should touch.',
            items: { type: 'string' },
            type: 'array',
          },
          issue: {
            description: 'Public GitHub issue number to implement, e.g. 6320.',
            type: 'number',
          },
          objective: {
            description:
              'Short PUBLIC-SAFE objective summary of the coding task. No secrets, prompts, or private content.',
            type: 'string',
          },
          verify: {
            description:
              'Public verification command the Pylon/Codex runner must pass, e.g. "bun run --cwd apps/openagents.com/workers/api test -- src/foo.test.ts".',
            type: 'string',
          },
        },
        required: ['objective'],
        type: 'object',
      },
    },
    kind: 'risky',
    plan: (args: unknown) =>
      Effect.sync(() => {
        const record =
          typeof args === 'object' && args !== null
            ? (args as Record<string, unknown>)
            : {}
        const objective = asString(record.objective)
        if (objective === undefined) {
          return '(invalid arguments: a public-safe "objective" string is required)'
        }
        const branch = asString(record.branch) ?? defaultBranch
        const verify = asString(record.verify)
        const issue =
          typeof record.issue === 'number' && Number.isFinite(record.issue)
            ? Math.trunc(record.issue)
            : undefined
        const filePaths = asStringArray(record.filePaths)

        // Public-safety gate over every text field.
        const unsafe = [objective, branch, verify ?? '', ...filePaths].find(
          field => field !== '' && !dispatchFieldIsSafe(field),
        )
        if (unsafe !== undefined) {
          return '(blocked: a dispatch field contained non-public-safe material; rephrase with public issue numbers, public file paths, and public verification commands only)'
        }
        const badPath = filePaths.find(path => !isSafeArtanisRepoPath(path))
        if (badPath !== undefined) {
          return `(blocked: "${badPath}" is not an allowed public repo path)`
        }

        const promptParts = [objective]
        if (issue !== undefined) {
          promptParts.unshift(`Implement public issue #${issue}.`)
        }
        if (filePaths.length > 0) {
          promptParts.push(`Files: ${filePaths.join(', ')}.`)
        }
        if (verify !== undefined) {
          promptParts.push(`Run the named verification.`)
        }
        const prompt = promptParts.join(' ')

        const lines: Array<string> = [
          'Planned Khala -> Pylon -> Codex dispatch (own-capacity only, no spend):',
          '',
          '  pylon khala request \\',
          `    --prompt "${prompt}" \\`,
          '    --workflow codex_agent_task \\',
          '    --pylon-ref "<owner pylon ref>" \\',
          `    --repo ${ARTANIS_REPO_READ_OWNER}/${ARTANIS_REPO_READ_REPO} \\`,
          `    --branch ${branch} \\`,
          '    --commit "<current origin/main sha>" \\',
        ]
        if (verify !== undefined) {
          lines.push(`    --verify "${verify}" \\`)
        }
        lines.push('    --json')
        lines.push('')
        lines.push(
          '  Then execute locally with no spend: pylon assignment run-no-spend --json',
        )
        lines.push('')
        lines.push(
          'For parallel burndown, set OPENAGENTS_PYLON_CODEX_CONCURRENCY=N and publish capacity (pylon presence heartbeat), then run-no-spend per assignment ref. Verify each closeout against the exact token_usage_events + agent_traces rows before merging non-spend code.',
        )
        if (issue !== undefined) {
          lines.push(`Targets public issue #${issue}.`)
        }
        return lines.join('\n')
      }),
    riskyActionKind: 'pylon_job_dispatch',
  }
}

// ---------------------------------------------------------------------------
// The default owner-scoped operator tool table.
// ---------------------------------------------------------------------------

// Build the standard Artanis operator tool set: the two public repo-read tools
// (#6365) plus the plan-only Codex dispatch tool (#6366). The route wires this
// into `artanisOperatorTurn`.
export const makeArtanisOperatorTools = (
  config: Readonly<{
    repoRead?: ArtanisRepoReadConfig | undefined
    defaultBranch?: string | undefined
  }> = {},
): ReadonlyArray<ArtanisOperatorTool> => [
  makeArtanisReadRepoFileTool(config.repoRead),
  makeArtanisListRepoDirTool(config.repoRead),
  makeArtanisDispatchCodexTaskTool({ defaultBranch: config.defaultBranch }),
]
