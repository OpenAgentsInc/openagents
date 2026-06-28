// Artanis operator TOOLS (#6365 repo-read, read_github_issue, #6366 Codex
// dispatch) for the bounded tool-calling loop in `artanis-operator.ts` (#6364).
//
// These are the concrete owner-scoped tools Artanis can invoke during a turn:
//
//   - #6365 read_repo_file / list_repo_dir — READ tools over the PUBLIC
//     `OpenAgentsInc/openagents` repo via the GitHub contents APIs. Bounded
//     size, public repo only, secret-path denylist. This is what lets Artanis
//     read e.g. `docs/khala/2026-06-26-khala-open-issues-master-roadmap.md`
//     himself and reason over its real contents.
//   - read_github_issue — a READ tool over the PUBLIC `OpenAgentsInc/openagents`
//     issues via the GitHub issues API. Given a numeric issue number it returns
//     the title, state, body, and a bounded set of comments so Artanis can pull
//     the exact requirements, acceptance criteria, API contracts, and fresh
//     user/dev feedback from public issues (e.g. #6311, #6320, #6359) BEFORE he
//     drafts a `dispatch_codex_task` plan. The master roadmap only gives him
//     one-line epic summaries; this lets the burndown dispatch hit the mark on
//     the first run. Public repo only; bounded; non-numeric/private input is
//     blocked; a missing issue degrades to "(issue not found: #N)".
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
  ArtanisOperatorGatedResult,
  ArtanisOperatorGatedTool,
  ArtanisOperatorReadTool,
  ArtanisOperatorRiskyTool,
  ArtanisOperatorTool,
  ArtanisOperatorWriteTool,
  ArtanisRiskyActionKind,
} from './artanis-operator'
import {
  type ArtanisGetNetworkStatsConfig,
  type ArtanisNetworkStats,
  fetchArtanisNetworkStats,
  formatArtanisTokenPaceLine,
} from './artanis-token-pace'

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

type GitHubContentsFile = Readonly<{
  content?: unknown
  encoding?: unknown
  size?: unknown
  type?: unknown
}>

const repoContentsUrl = (
  owner: string,
  repo: string,
  path: string,
  ref: string,
): string => {
  const encodedPath = path
    .split('/')
    .map(part => encodeURIComponent(part))
    .join('/')
  return `https://api.github.com/repos/${owner}/${repo}/contents/${encodedPath}?ref=${encodeURIComponent(ref)}`
}

const decodeGitHubBase64Utf8 = (content: string): string | undefined => {
  try {
    const binary = atob(content.replace(/\s+/g, ''))
    const bytes = Uint8Array.from(binary, character => character.charCodeAt(0))
    return new TextDecoder().decode(bytes)
  } catch {
    return undefined
  }
}

// read_repo_file(path) — reads a file from the public repo via the GitHub
// contents API. Bounded, public-only, secret-path-denied.
export const makeArtanisReadRepoFileTool = (
  config: ArtanisRepoReadConfig = {},
): ArtanisOperatorReadTool => {
  const { fetchImpl, maxBytes, owner, ref, repo } =
    resolveRepoReadConfig(config)

  return {
    definition: {
      description: `Read a UTF-8 text file from the PUBLIC ${owner}/${repo} repo (branch ${ref}) via the GitHub contents API. Use for docs and source, e.g. "docs/khala/2026-06-26-khala-open-issues-master-roadmap.md". Public repo only; bounded size; secret paths are blocked.`,
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

        const url = repoContentsUrl(owner, repo, path, ref)
        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, {
            headers: {
              Accept: 'application/vnd.github+json',
              'User-Agent': 'artanis-operator',
            },
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

        const body = yield* Effect.tryPromise(
          () => response.json() as Promise<unknown>,
        ).pipe(Effect.orElseSucceed(() => undefined))
        if (typeof body !== 'object' || body === null || Array.isArray(body)) {
          return `("${path}" is not a file)`
        }

        const file = body as GitHubContentsFile
        if (file.type !== 'file') {
          return `("${path}" is not a file)`
        }
        if (typeof file.size === 'number' && file.size > maxBytes) {
          return `(file too large: "${path}" is ${file.size} bytes; max is ${maxBytes})`
        }
        if (file.encoding !== 'base64' || typeof file.content !== 'string') {
          return `(read failed for "${path}": expected base64 file content from GitHub contents API)`
        }

        const text = decodeGitHubBase64Utf8(file.content)
        if (text === undefined) {
          return `(read failed for "${path}": invalid base64 file content)`
        }
        if (new TextEncoder().encode(text).byteLength > maxBytes) {
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

        const url = repoContentsUrl(owner, repo, path, ref)
        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, {
            headers: {
              Accept: 'application/vnd.github+json',
              'User-Agent': 'artanis-operator',
            },
          }),
        )
          .pipe(Effect.orElseSucceed(() => undefined))

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
// read_github_issue — read a PUBLIC OpenAgentsInc/openagents issue (title,
// state, body, bounded comments). Side-effect-free, public-state-only.
// ---------------------------------------------------------------------------

// Max characters returned from the issue body and from each comment body. Issue
// threads can be huge; we bound both so a single read stays small and bounded.
export const ARTANIS_ISSUE_BODY_MAX_CHARS = 16 * 1024
export const ARTANIS_ISSUE_COMMENT_MAX_CHARS = 4 * 1024
// Max comments fetched/rendered for a single issue read.
export const ARTANIS_ISSUE_MAX_COMMENTS = 20

export type ArtanisIssueReadConfig = Readonly<{
  owner?: string | undefined
  repo?: string | undefined
  maxComments?: number | undefined
  fetchImpl?: typeof fetch | undefined
}>

// The parse outcome for a model-produced issue argument. We distinguish ABSENT
// (no issue field at all -> honest "invalid arguments") from INVALID (a present
// but non-numeric/private value -> honest "(blocked …)"), per the acceptance.
type ArtanisIssueArg =
  | Readonly<{ kind: 'number'; value: number }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'invalid'; raw: string }>

// Coerce a model-produced issue argument into a positive integer. Accepts a
// number, or a bare/`#`-prefixed numeric string. Anything else (a path, a URL,
// a word, a float, a negative, an object) is INVALID and gets blocked — only a
// public issue NUMBER is ever turned into a request.
export const parseArtanisIssueNumber = (args: unknown): ArtanisIssueArg => {
  if (typeof args !== 'object' || args === null) return { kind: 'absent' }
  const record = args as Record<string, unknown>
  const raw =
    record.issue_number ?? record.issueNumber ?? record.issue ?? record.number
  if (raw === undefined || raw === null) return { kind: 'absent' }
  if (typeof raw === 'number') {
    return Number.isInteger(raw) && raw > 0
      ? { kind: 'number', value: raw }
      : { kind: 'invalid', raw: String(raw) }
  }
  if (typeof raw === 'string') {
    const trimmed = raw.trim()
    if (/^#?\d+$/.test(trimmed)) {
      const value = Number.parseInt(trimmed.replace(/^#/, ''), 10)
      return value > 0
        ? { kind: 'number', value }
        : { kind: 'invalid', raw: trimmed }
    }
    return { kind: 'invalid', raw: trimmed }
  }
  return { kind: 'invalid', raw: String(raw) }
}

const resolveIssueReadConfig = (config: ArtanisIssueReadConfig) => ({
  fetchImpl: config.fetchImpl ?? globalThis.fetch,
  maxComments: config.maxComments ?? ARTANIS_ISSUE_MAX_COMMENTS,
  owner: config.owner ?? ARTANIS_REPO_READ_OWNER,
  repo: config.repo ?? ARTANIS_REPO_READ_REPO,
})

const boundText = (value: string, max: number): string =>
  value.length > max
    ? `${value.slice(0, max)}\n(…truncated at ${max} chars)`
    : value

type GitHubIssueBody = Readonly<{
  title?: unknown
  state?: unknown
  body?: unknown
  comments?: unknown
  pull_request?: unknown
}>

type GitHubIssueComment = Readonly<{
  body?: unknown
  user?: unknown
  created_at?: unknown
}>

const commentAuthor = (comment: GitHubIssueComment): string => {
  const user = comment.user
  if (typeof user === 'object' && user !== null) {
    const login = (user as Record<string, unknown>).login
    if (typeof login === 'string' && login.trim() !== '') return login
  }
  return 'unknown'
}

// read_github_issue(issue_number) — reads a PUBLIC issue from the fixed repo via
// the GitHub issues API. Returns the title, state, body, and a bounded set of
// comments. Public repo only; non-numeric/private input is blocked; a missing
// issue degrades to "(issue not found: #N)". Side-effect-free.
export const makeArtanisReadGithubIssueTool = (
  config: ArtanisIssueReadConfig = {},
): ArtanisOperatorReadTool => {
  const { fetchImpl, maxComments, owner, repo } = resolveIssueReadConfig(config)

  return {
    definition: {
      description: `Read a PUBLIC GitHub issue from ${owner}/${repo} by its number. Returns the issue title, state, body, and up to ${maxComments} comments so you can pull the exact requirements, acceptance criteria, API contracts, and recent user/dev feedback BEFORE drafting a dispatch_codex_task plan, e.g. issue 6311, 6320, or 6359. Public repo only; a non-numeric/invalid issue is blocked; an unknown issue reads as "(issue not found: #N)".`,
      name: 'read_github_issue',
      parameters: {
        additionalProperties: false,
        properties: {
          issue_number: {
            description:
              'The PUBLIC GitHub issue number to read, e.g. 6311. A positive integer (a leading "#" is tolerated). Non-numeric values are rejected.',
            type: 'number',
          },
        },
        required: ['issue_number'],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        const parsed = parseArtanisIssueNumber(args)
        if (parsed.kind === 'absent') {
          return '(invalid arguments: a numeric "issue_number" is required)'
        }
        if (parsed.kind === 'invalid') {
          return `(blocked: "${parsed.raw}" is not a valid public issue number; pass a positive integer like 6311)`
        }
        const number = parsed.value

        const issueUrl = `https://api.github.com/repos/${owner}/${repo}/issues/${number}`
        const issueResponse = yield* Effect.tryPromise(() =>
          fetchImpl(issueUrl, {
            headers: {
              Accept: 'application/vnd.github+json',
              'User-Agent': 'artanis-operator',
            },
          }),
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (issueResponse === undefined) {
          return `(could not fetch issue #${number})`
        }
        if (issueResponse.status === 404) {
          return `(issue not found: #${number})`
        }
        if (!issueResponse.ok) {
          return `(read failed for issue #${number}: status ${issueResponse.status})`
        }

        const issue = yield* Effect.tryPromise(
          () => issueResponse.json() as Promise<unknown>,
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (typeof issue !== 'object' || issue === null) {
          return `(read failed for issue #${number}: unexpected response shape)`
        }
        const issueBody = issue as GitHubIssueBody
        const title =
          typeof issueBody.title === 'string' ? issueBody.title : '(no title)'
        const state =
          typeof issueBody.state === 'string' ? issueBody.state : 'unknown'
        const body =
          typeof issueBody.body === 'string' && issueBody.body.trim() !== ''
            ? boundText(issueBody.body, ARTANIS_ISSUE_BODY_MAX_CHARS)
            : '(no description)'
        const kind =
          typeof issueBody.pull_request === 'object' &&
          issueBody.pull_request !== null
            ? 'Pull request'
            : 'Issue'
        const commentCount =
          typeof issueBody.comments === 'number' ? issueBody.comments : 0

        const lines: Array<string> = [
          `${kind} #${number}: ${title}`,
          `State: ${state}`,
          `URL: https://github.com/${owner}/${repo}/issues/${number}`,
          '',
          body,
        ]

        // Bounded comment fetch. Comments are best-effort: a failure here still
        // returns the issue itself with an honest note, never a thrown turn.
        if (commentCount > 0) {
          const commentsUrl = `${issueUrl}/comments?per_page=${maxComments}`
          const commentsResponse = yield* Effect.tryPromise(() =>
            fetchImpl(commentsUrl, {
              headers: {
                Accept: 'application/vnd.github+json',
                'User-Agent': 'artanis-operator',
              },
            }),
          ).pipe(Effect.orElseSucceed(() => undefined))

          const commentsJson =
            commentsResponse !== undefined && commentsResponse.ok
              ? yield* Effect.tryPromise(
                  () => commentsResponse.json() as Promise<unknown>,
                ).pipe(Effect.orElseSucceed(() => undefined))
              : undefined

          if (Array.isArray(commentsJson)) {
            const rendered = (commentsJson as ReadonlyArray<GitHubIssueComment>)
              .slice(0, maxComments)
              .map((comment, index) => {
                const author = commentAuthor(comment)
                const at =
                  typeof comment.created_at === 'string'
                    ? comment.created_at
                    : ''
                const text =
                  typeof comment.body === 'string' && comment.body.trim() !== ''
                    ? boundText(comment.body, ARTANIS_ISSUE_COMMENT_MAX_CHARS)
                    : '(empty comment)'
                return `[comment ${index + 1}] @${author}${at ? ` (${at})` : ''}:\n${text}`
              })
            lines.push('')
            lines.push(
              `Comments (showing ${rendered.length} of ${commentCount}):`,
            )
            for (const block of rendered) {
              lines.push('')
              lines.push(block)
            }
          } else {
            lines.push('')
            lines.push(
              `(${commentCount} comment(s) exist but could not be fetched)`,
            )
          }
        } else {
          lines.push('')
          lines.push('Comments: (none)')
        }

        return lines.join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// list_github_issues — read a bounded, filterable LIST of issues in the PUBLIC
// OpenAgentsInc/openagents repo (title, number, state, labels). Side-effect-free,
// public-state-only.
//
// This is Artanis's iteration-10 self-improvement capability. read_github_issue
// (above) pulls ONE issue he already knows the number of; this gives him the
// CANDIDATE SET to triage. A read-only, filterable list of open issues lets him
// programmatically identify high-priority bugs/feature gaps to fan out across the
// parallel Khala -> Pylon -> Codex burndown — the core lever for flipping product
// promises green and 10x-ing Khala token usage. He can filter by state (default
// open) and by labels (e.g. "khala") to scope the triage.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It only GETs the public GitHub issues API for
//     the fixed public repo; it never writes, mutates, dispatches, or spends. It
//     grants no new authority and fits the existing read-tool boundary.
//   - PUBLIC REPO ONLY. The owner/repo are fixed to OpenAgentsInc/openagents like
//     the other GitHub read tools; the model cannot name an arbitrary repo.
//   - FILTERS OUT PULL REQUESTS. The GitHub issues endpoint returns PRs too (they
//     carry a `pull_request` field); those are dropped so the list is real issues.
//   - BOUNDED + FAIL-SOFT. A single read returns at most `maxLimit` issues, each
//     title truncated, so a busy repo stays cheap. A fetch failure degrades to an
//     honest "(could not list issues)"; an empty result reads "(no … issues …)".
// ---------------------------------------------------------------------------

// The default and max number of issues returned for one list read, so a single
// read stays bounded even when the repo has many open issues.
export const ARTANIS_ISSUE_LIST_DEFAULT_LIMIT = 30
export const ARTANIS_ISSUE_LIST_MAX_LIMIT = 100

// Max characters of a single issue title surfaced into Artanis's context; a
// longer title is truncated with an explicit marker so one essay-length title
// cannot blow the context budget.
export const ARTANIS_ISSUE_LIST_TITLE_MAX_CHARS = 200

// Max distinct label filters honored for one read; extra labels are ignored so a
// model typo cannot build an unboundedly long query string.
export const ARTANIS_ISSUE_LIST_MAX_LABELS = 10

// The bounded set of issue states a model may filter on. Anything else falls back
// to 'open' (the high-leverage default for burndown triage).
export const ARTANIS_ISSUE_LIST_STATES = ['open', 'closed', 'all'] as const
export type ArtanisIssueListState = (typeof ARTANIS_ISSUE_LIST_STATES)[number]

export type ArtanisIssueListConfig = Readonly<{
  owner?: string | undefined
  repo?: string | undefined
  defaultLimit?: number | undefined
  maxLimit?: number | undefined
  // Injected for testability; defaults to the global fetch.
  fetchImpl?: typeof fetch | undefined
}>

const resolveIssueListConfig = (config: ArtanisIssueListConfig) => {
  const maxLimit = config.maxLimit ?? ARTANIS_ISSUE_LIST_MAX_LIMIT
  return {
    // The default is itself bounded by maxLimit so an absent `limit` arg can
    // never request more than the configured ceiling.
    defaultLimit: Math.min(
      config.defaultLimit ?? ARTANIS_ISSUE_LIST_DEFAULT_LIMIT,
      maxLimit,
    ),
    fetchImpl: config.fetchImpl ?? globalThis.fetch,
    maxLimit,
    owner: config.owner ?? ARTANIS_REPO_READ_OWNER,
    repo: config.repo ?? ARTANIS_REPO_READ_REPO,
  }
}

// Coerce a model-produced `state` arg into a bounded GitHub issue state; an
// absent/invalid value falls back to 'open'.
const parseIssueListState = (args: unknown): ArtanisIssueListState => {
  if (typeof args !== 'object' || args === null) return 'open'
  const raw = (args as Record<string, unknown>).state
  if (typeof raw !== 'string') return 'open'
  const trimmed = raw.trim().toLowerCase()
  return (ARTANIS_ISSUE_LIST_STATES as ReadonlyArray<string>).includes(trimmed)
    ? (trimmed as ArtanisIssueListState)
    : 'open'
}

// A safe label filter token: GitHub labels are short tags. We keep a
// conservative character set (letters, digits, space, and a few separators) so
// the query string can never carry traversal/control material. Unsafe labels are
// dropped rather than fetched.
const isSafeIssueLabel = (label: string): boolean =>
  label.length > 0 &&
  label.length <= 80 &&
  /^[A-Za-z0-9][A-Za-z0-9 ._:/-]*$/.test(label)

// Coerce a model-produced `labels`/`label` arg (a comma-separated string or an
// array of strings) into a bounded, deduped, safe label list.
const parseIssueListLabels = (args: unknown): ReadonlyArray<string> => {
  if (typeof args !== 'object' || args === null) return []
  const record = args as Record<string, unknown>
  const raw = record.labels ?? record.label
  const parts: Array<string> =
    typeof raw === 'string'
      ? raw.split(',')
      : Array.isArray(raw)
        ? raw.filter((part): part is string => typeof part === 'string')
        : []
  const seen = new Set<string>()
  const labels: Array<string> = []
  for (const part of parts) {
    const trimmed = part.trim()
    if (!isSafeIssueLabel(trimmed)) continue
    if (seen.has(trimmed)) continue
    seen.add(trimmed)
    labels.push(trimmed)
    if (labels.length >= ARTANIS_ISSUE_LIST_MAX_LABELS) break
  }
  return labels
}

// Coerce a model-produced `limit`/`count`/`max` arg into a bounded positive
// integer, clamped into [1, maxLimit]; an absent/invalid value falls back to the
// default. A model typo can never request an unboundedly large list.
const parseIssueListLimit = (
  args: unknown,
  defaultLimit: number,
  maxLimit: number,
): number => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}
  const raw = record.limit ?? record.count ?? record.max
  let value: number | undefined
  if (typeof raw === 'number' && Number.isInteger(raw)) {
    value = raw
  } else if (typeof raw === 'string' && /^\d+$/.test(raw.trim())) {
    value = Number.parseInt(raw.trim(), 10)
  }
  if (value === undefined || value < 1) {
    return defaultLimit
  }
  return Math.min(value, maxLimit)
}

type GitHubIssueListEntry = Readonly<{
  number?: unknown
  title?: unknown
  state?: unknown
  pull_request?: unknown
  labels?: unknown
}>

// True when a list entry is a pull request, not an issue. The GitHub issues
// endpoint returns BOTH; PRs carry a `pull_request` object. We drop them so the
// list is real issues only.
const isPullRequestEntry = (entry: GitHubIssueListEntry): boolean =>
  typeof entry.pull_request === 'object' && entry.pull_request !== null

// Extract the public-safe label names from a list entry (each label is a string
// or an object with a string `name`). Unsafe label values are dropped.
const issueEntryLabelNames = (
  entry: GitHubIssueListEntry,
): ReadonlyArray<string> => {
  const labels = entry.labels
  if (!Array.isArray(labels)) return []
  return labels
    .map(label => {
      if (typeof label === 'string') return label
      if (typeof label === 'object' && label !== null) {
        const name = (label as Record<string, unknown>).name
        return typeof name === 'string' ? name : null
      }
      return null
    })
    .filter(
      (name): name is string => name !== null && isSafeIssueLabel(name),
    )
}

// Format ONE issue as a bounded, public-safe one-line entry: number, state,
// (truncated) title, and its labels.
const formatIssueListLine = (entry: GitHubIssueListEntry): string => {
  const number =
    typeof entry.number === 'number' && Number.isInteger(entry.number)
      ? entry.number
      : null
  if (number === null) return ''
  const state =
    typeof entry.state === 'string' && entry.state.trim() !== ''
      ? entry.state
      : 'unknown'
  const rawTitle =
    typeof entry.title === 'string' && entry.title.trim() !== ''
      ? entry.title.trim()
      : '(no title)'
  const title = boundText(rawTitle, ARTANIS_ISSUE_LIST_TITLE_MAX_CHARS).replace(
    /\s+/g,
    ' ',
  )
  const labels = issueEntryLabelNames(entry)
  const labelSuffix =
    labels.length > 0 ? ` (labels: ${labels.join(', ')})` : ''
  return `- #${number} [${state}] ${title}${labelSuffix}`
}

// list_github_issues(state?, labels?, limit?) — reads a bounded, filterable LIST
// of issues from the fixed PUBLIC repo via the GitHub issues API. Returns one
// public-safe line per issue (number + state + title + labels), with pull
// requests filtered out and the count bounded. Honest absence: an empty result
// reads "(no <state> issues found …)"; a fetch failure reads "(could not list
// issues)". Public repo only; side-effect-free.
export const makeArtanisListGithubIssuesTool = (
  config: ArtanisIssueListConfig = {},
): ArtanisOperatorReadTool => {
  const { defaultLimit, fetchImpl, maxLimit, owner, repo } =
    resolveIssueListConfig(config)

  return {
    definition: {
      description: `List issues in the PUBLIC ${owner}/${repo} repo so you can triage the candidate set (vs. read_github_issue, which reads ONE issue you already know the number of). Returns one bounded line per issue with its number, state, title, and labels; pull requests are filtered out. Use this to scan the open backlog, spot high-priority bugs and feature gaps, and decide which issues to fan out across the Khala -> Pylon -> Codex burndown, then call read_github_issue on a specific number for the full requirements. Optional "state" ("open" (default), "closed", or "all"), "labels" (comma-separated, e.g. "khala"), and "limit" (default ${defaultLimit}, max ${maxLimit}).`,
      name: 'list_github_issues',
      parameters: {
        additionalProperties: false,
        properties: {
          labels: {
            description:
              'Optional comma-separated label filter, e.g. "khala" or "bug,khala". Only issues carrying ALL named labels are returned. Omit to list all labels.',
            type: 'string',
          },
          limit: {
            description: `Max number of issues to return, a positive integer up to ${maxLimit} (default ${defaultLimit}).`,
            type: 'number',
          },
          state: {
            description:
              'Issue state filter: "open" (default), "closed", or "all".',
            type: 'string',
          },
        },
        required: [],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        const state = parseIssueListState(args)
        const labels = parseIssueListLabels(args)
        const limit = parseIssueListLimit(args, defaultLimit, maxLimit)

        const params = new URLSearchParams()
        params.set('state', state)
        params.set('per_page', String(Math.min(limit, 100)))
        params.set('sort', 'created')
        params.set('direction', 'desc')
        if (labels.length > 0) {
          params.set('labels', labels.join(','))
        }
        const url = `https://api.github.com/repos/${owner}/${repo}/issues?${params.toString()}`

        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, {
            headers: {
              Accept: 'application/vnd.github+json',
              'User-Agent': 'artanis-operator',
            },
          }),
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (response === undefined) {
          return '(could not list issues)'
        }
        if (response.status === 404) {
          return `(repo not found: ${owner}/${repo})`
        }
        if (!response.ok) {
          return `(list failed: status ${response.status})`
        }

        const body = yield* Effect.tryPromise(
          () => response.json() as Promise<unknown>,
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (!Array.isArray(body)) {
          return '(list failed: unexpected response shape)'
        }

        const labelLabel =
          labels.length > 0 ? ` with labels [${labels.join(', ')}]` : ''
        const lines = (body as ReadonlyArray<GitHubIssueListEntry>)
          .filter(entry => !isPullRequestEntry(entry))
          .slice(0, limit)
          .map(formatIssueListLine)
          .filter(line => line !== '')

        if (lines.length === 0) {
          return `(no ${state} issues found in ${owner}/${repo}${labelLabel})`
        }

        const stateLabel =
          state === 'all'
            ? 'Issues'
            : state === 'open'
              ? 'Open issues'
              : 'Closed issues'
        const header = `${stateLabel} in ${owner}/${repo}${labelLabel} (${lines.length}):`
        return [header, ...lines].join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// get_pylon_job_status — read the public-safe closeout/proof status for ONE
// owner-scoped Pylon/Codex assignment ref. Side-effect-free, owner-only,
// public-state-only. This is Artanis's iteration-3 self-improvement capability:
// it closes the loop on his parallel dispatch. He can already DRAFT a dispatch
// (dispatch_codex_task) and see a broad recent-assignment LIST in situational
// awareness, but he could not pull the closeout state, proof/verify result, and
// redacted failure summary for ONE specific assignment ref on demand during a
// turn. With it he can verify whether a delegated burndown task actually passed,
// read the failing check, and iterate the next dispatch autonomously.
// ---------------------------------------------------------------------------

// Max refs of each kind rendered for one status read, so a single read stays
// bounded even for a chatty assignment.
export const ARTANIS_JOB_STATUS_MAX_REFS = 10

// The verify/proof verdict derived from the assignment's real closeout state.
// 'pass' = an accepted closeout with retained proof/artifacts and no blockers;
// 'fail' = a rejected/blocked closeout (or a blocked progress event); 'unknown'
// = still in progress (offered/accepted/running/proof-submitted, not yet closed
// out). Honest absence over invention: a state we cannot resolve reads 'unknown'.
export type ArtanisPylonJobVerifyResult = 'pass' | 'fail' | 'unknown'

// The public-safe status of ONE Pylon/Codex assignment. Every field is a
// public-safe projection of real D1 state (the assignment record + its events);
// it never carries raw prompts, shell output, credentials, wallet material, or
// local paths. The production reader builds this from the Pylon API store; tests
// inject a fake reader.
export type ArtanisPylonJobStatus = Readonly<{
  assignmentRef: string
  // The job kind, e.g. 'codex_agent_task'.
  jobKind: string
  // The assignment lifecycle state, e.g. 'closeout_submitted' | 'running' |
  // 'rejected' | 'blocked'.
  state: string
  // The lease state: 'active' | 'expired' | 'terminal'.
  leaseState: string
  // True once a worker closeout has been submitted (state 'closeout_submitted').
  closeoutSubmitted: boolean
  // True once artifact/proof metadata has been observed for the assignment.
  proofObserved: boolean
  // The derived verify/proof verdict.
  verifyResult: ArtanisPylonJobVerifyResult
  // A short, public-safe, redacted failure summary when the work failed/blocked,
  // else null. Built from public-safe rejection/blocker refs only.
  failureSummary: string | null
  // Public-safe evidence refs (already store-projected; bounded by the tool).
  artifactRefs: ReadonlyArray<string>
  proofRefs: ReadonlyArray<string>
  closeoutRefs: ReadonlyArray<string>
  rejectionRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  // Public-safe "last updated" display string (not a raw timestamp).
  updatedAt: string
}>

// The injected, owner-scoped reader seam. Given an assignment ref it resolves
// the public-safe status, or `null` when no assignment matches FOR THIS OWNER
// (honest absence — it must never return another owner's assignment). A thrown
// rejection is treated by the tool as a soft read failure, never a fabricated
// status. The production reader (`artanis-operator-pylon-job-status.ts`) reads
// the owner's own linked-Pylon assignments from the Pylon API store.
export type ArtanisPylonJobStatusReader = (
  assignmentRef: string,
) => Promise<ArtanisPylonJobStatus | null>

export type ArtanisPylonJobStatusConfig = Readonly<{
  reader?: ArtanisPylonJobStatusReader | undefined
  maxRefs?: number | undefined
}>

// A safe assignment ref: a public-safe ref-shaped token with no traversal, no
// secret material. Mirrors the proof-trace ref discipline; only such a ref is
// ever turned into a store lookup.
export const isSafeArtanisAssignmentRef = (ref: string): boolean => {
  if (typeof ref !== 'string') return false
  const trimmed = ref.trim()
  if (trimmed === '') return false
  if (trimmed.length > 300) return false
  if (trimmed.includes('..')) return false
  if (!dispatchFieldIsSafe(trimmed)) return false
  return /^[A-Za-z0-9][A-Za-z0-9_.:/#-]*$/.test(trimmed)
}

// The parse outcome for a model-produced assignment-ref argument. ABSENT (no
// ref field at all -> honest "invalid arguments"); INVALID (a present but
// unsafe/empty value -> honest "(blocked …)"); VALID (a safe ref).
type ArtanisAssignmentRefArg =
  | Readonly<{ kind: 'ref'; value: string }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'invalid'; raw: string }>

// Coerce a model-produced argument into a safe assignment ref. Accepts the
// `assignmentRef` / `assignment_ref` / `assignment` / `ref` keys as a string.
export const parseArtanisAssignmentRef = (
  args: unknown,
): ArtanisAssignmentRefArg => {
  if (typeof args !== 'object' || args === null) return { kind: 'absent' }
  const record = args as Record<string, unknown>
  const raw =
    record.assignmentRef ??
    record.assignment_ref ??
    record.assignment ??
    record.ref
  if (raw === undefined || raw === null) return { kind: 'absent' }
  if (typeof raw !== 'string') return { kind: 'invalid', raw: String(raw) }
  const trimmed = raw.trim()
  if (trimmed === '') return { kind: 'absent' }
  return isSafeArtanisAssignmentRef(trimmed)
    ? { kind: 'ref', value: trimmed }
    : { kind: 'invalid', raw: trimmed }
}

// Drop any ref that fails the public-safety gate, then bound the list. The store
// projection should already be public-safe; this is a defensive second pass so a
// regression upstream can never leak private material into Artanis's context.
const boundedSafeRefs = (
  refs: ReadonlyArray<string>,
  maxRefs: number,
): ReadonlyArray<string> =>
  refs.filter(ref => dispatchFieldIsSafe(ref)).slice(0, maxRefs)

const formatRefLine = (
  label: string,
  refs: ReadonlyArray<string>,
): string =>
  refs.length === 0 ? `${label}: (none)` : `${label}: ${refs.join(', ')}`

const verifyResultLabel = (
  result: ArtanisPylonJobVerifyResult,
): string =>
  result === 'pass'
    ? 'PASS (accepted closeout, proof retained, no blockers)'
    : result === 'fail'
      ? 'FAIL (rejected/blocked closeout)'
      : 'in progress / not yet closed out (unknown)'

// get_pylon_job_status(assignmentRef) — reads the public-safe closeout/proof
// status for ONE owner-scoped assignment via the injected reader. Returns a
// bounded text block: state, lease state, closeout state, verify/proof verdict,
// a redacted failure summary, and public-safe evidence refs. Honest absence: a
// missing/other-owner assignment reads "(no assignment found …)"; a read failure
// reads "(could not read status …)"; an absent/unsafe ref is blocked. Never
// invents a status. Side-effect-free.
export const makeArtanisGetPylonJobStatusTool = (
  config: ArtanisPylonJobStatusConfig = {},
): ArtanisOperatorReadTool => {
  const reader = config.reader
  const maxRefs = config.maxRefs ?? ARTANIS_JOB_STATUS_MAX_REFS

  return {
    definition: {
      description:
        'Read the public-safe closeout/proof STATUS of ONE Pylon/Codex assignment by its assignment ref. Returns the lifecycle state, lease state, whether a worker closeout was submitted, the verify/proof verdict (PASS/FAIL/in-progress), a redacted failure summary when it failed, and public-safe evidence refs. Use this to verify whether a delegated burndown task you dispatched actually passed, read the failing check, and decide the next dispatch. Owner-scoped (only your own linked-Pylon assignments); a missing/other-owner assignment reads as "(no assignment found …)".',
      name: 'get_pylon_job_status',
      parameters: {
        additionalProperties: false,
        properties: {
          assignmentRef: {
            description:
              'The assignment ref to read, e.g. "assignment.public.pylon_api.…". A public-safe ref string; no secrets or local paths.',
            type: 'string',
          },
        },
        required: ['assignmentRef'],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        const parsed = parseArtanisAssignmentRef(args)
        if (parsed.kind === 'absent') {
          return '(invalid arguments: a string "assignmentRef" is required)'
        }
        if (parsed.kind === 'invalid') {
          return `(blocked: "${parsed.raw}" is not an allowed public-safe assignment ref)`
        }
        const ref = parsed.value

        if (reader === undefined) {
          return `(could not read status for "${ref}": no status reader is wired)`
        }

        const exit = yield* Effect.exit(Effect.tryPromise(() => reader(ref)))
        if (exit._tag === 'Failure') {
          return `(could not read status for "${ref}")`
        }
        const status = exit.value
        if (status === null) {
          return `(no assignment found for "${ref}")`
        }

        const lines: Array<string> = [
          `Pylon job status for ${status.assignmentRef}:`,
          `- Job kind: ${status.jobKind}`,
          `- State: ${status.state} (lease: ${status.leaseState})`,
          `- Closeout: ${status.closeoutSubmitted ? 'submitted' : 'not yet submitted'}`,
          `- Proof observed: ${status.proofObserved ? 'yes' : 'no'}`,
          `- Verify/proof: ${verifyResultLabel(status.verifyResult)}`,
        ]
        if (status.failureSummary !== null && status.failureSummary !== '') {
          // The summary is built from public-safe refs only; defensively gate it.
          lines.push(
            `- Failure summary: ${
              dispatchFieldIsSafe(status.failureSummary)
                ? status.failureSummary
                : '(redacted)'
            }`,
          )
        }
        lines.push(
          formatRefLine(
            '- Artifact refs',
            boundedSafeRefs(status.artifactRefs, maxRefs),
          ),
          formatRefLine(
            '- Proof refs',
            boundedSafeRefs(status.proofRefs, maxRefs),
          ),
          formatRefLine(
            '- Closeout refs',
            boundedSafeRefs(status.closeoutRefs, maxRefs),
          ),
          formatRefLine(
            '- Rejection refs',
            boundedSafeRefs(status.rejectionRefs, maxRefs),
          ),
          formatRefLine(
            '- Blocker refs',
            boundedSafeRefs(status.blockerRefs, maxRefs),
          ),
          `- Last updated: ${status.updatedAt}`,
        )
        return lines.join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// list_pylon_assignments — read a bounded, public-safe LIST of the owner's
// active/recent Khala -> Pylon -> Codex assignments in ONE call.
// Side-effect-free, owner-only, public-state-only. This is Artanis's iteration-5
// self-improvement capability. get_pylon_job_status (iteration-3) inspects ONE
// assignment by ref; this scans ALL of the owner's recent assignments at once so
// he can instantly spot failed/stalled runs and queue parallel retries, keeping
// the Codex burndown loop saturated (more concurrent delegated coding work =
// more metered openagents/khala tokens, the 10x-usage goal). It grants no new
// spend/execution authority and fits the existing read-tool authority boundary.
// ---------------------------------------------------------------------------

// The default and max number of assignment summaries returned for one list read,
// so a single read stays bounded even when the owner has many assignments.
export const ARTANIS_ASSIGNMENTS_LIST_DEFAULT_LIMIT = 25
export const ARTANIS_ASSIGNMENTS_LIST_MAX_LIMIT = 100

// A public-safe one-line summary of ONE owner-scoped assignment. Every field is
// a public-safe projection of real D1 state (the assignment record); it never
// carries raw prompts, shell output, credentials, wallet material, or local
// paths. The production lister builds this from the Pylon API store; tests inject
// a fake lister.
export type ArtanisPylonAssignmentSummary = Readonly<{
  assignmentRef: string
  // The job kind, e.g. 'codex_agent_task'.
  jobKind: string
  // The assignment lifecycle state, e.g. 'accepted' | 'closeout_submitted' |
  // 'rejected'.
  state: string
  // The lease state: 'active' | 'expired' | 'terminal'.
  leaseState: string
  // A short public-safe phase label for where the run is, e.g. 'accepted' |
  // 'proof-ready' | 'closeout_submitted' | 'rejected'.
  phase: string
  // The derived coarse verify/proof verdict (bulk-list granularity; for the full
  // proof/failure detail use get_pylon_job_status on the ref).
  verifyResult: ArtanisPylonJobVerifyResult
  // Public-safe "last updated" display string (not a raw timestamp).
  updatedAt: string
}>

// The injected, owner-scoped lister seam. Given a max count it resolves the
// owner's own active/recent assignments as public-safe summaries, newest first.
// Returns `[]` when the owner has none (honest absence — it must never return
// another owner's assignments). A thrown rejection is treated by the tool as a
// soft read failure, never a fabricated list. The production lister
// (`artanis-operator-pylon-job-status.ts`) reads the owner's own linked-Pylon
// assignments from the Pylon API store.
export type ArtanisPylonAssignmentsLister = (
  limit: number,
) => Promise<ReadonlyArray<ArtanisPylonAssignmentSummary>>

export type ArtanisPylonAssignmentsConfig = Readonly<{
  lister?: ArtanisPylonAssignmentsLister | undefined
  defaultLimit?: number | undefined
  maxLimit?: number | undefined
}>

// Coerce a model-produced `limit`/`count`/`max` arg into a bounded positive
// integer, clamped into [1, maxLimit]; an absent/invalid value falls back to the
// default. A model typo can never request an unboundedly large list.
const parseAssignmentsLimit = (
  args: unknown,
  defaultLimit: number,
  maxLimit: number,
): number => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}
  const raw = record.limit ?? record.count ?? record.max
  let value: number | undefined
  if (typeof raw === 'number' && Number.isInteger(raw)) {
    value = raw
  } else if (typeof raw === 'string' && /^\d+$/.test(raw.trim())) {
    value = Number.parseInt(raw.trim(), 10)
  }
  if (value === undefined || value < 1) {
    return defaultLimit
  }
  return Math.min(value, maxLimit)
}

// Pull an optional bounded `state`/`phase` filter out of model-produced args. A
// bounded enum-style equality filter (applied AFTER the model already selected
// this tool) over lifecycle states; non-string/empty/unsafe values are ignored.
const parseAssignmentsStateFilter = (args: unknown): string | undefined => {
  if (typeof args !== 'object' || args === null) return undefined
  const record = args as Record<string, unknown>
  const raw = record.state ?? record.phase ?? record.status
  if (typeof raw !== 'string') return undefined
  const trimmed = raw.trim().toLowerCase()
  if (trimmed === '' || !dispatchFieldIsSafe(trimmed)) return undefined
  return trimmed
}

const assignmentVerifyShortLabel = (
  result: ArtanisPylonJobVerifyResult,
): string =>
  result === 'pass' ? 'PASS' : result === 'fail' ? 'FAIL' : 'in-progress'

// Format ONE assignment summary as a bounded, public-safe one-line entry
// carrying the ref, state, and phase (plus job kind, verify verdict, and last
// update). No secrets, prompts, or private material.
const formatAssignmentSummaryLine = (
  summary: ArtanisPylonAssignmentSummary,
): string =>
  `- ${summary.assignmentRef} | ${summary.jobKind} | state=${summary.state} | phase=${summary.phase} | verify=${assignmentVerifyShortLabel(
    summary.verifyResult,
  )} | updated ${summary.updatedAt}`

// A summary row is renderable only if every public-facing field passes the
// public-safety gate and the ref is ref-shaped. The store projection should
// already be public-safe; this is a defensive second pass so an upstream
// regression can never leak private material into Artanis's context.
const assignmentSummaryIsSafe = (
  summary: ArtanisPylonAssignmentSummary,
): boolean =>
  isSafeArtanisAssignmentRef(summary.assignmentRef) &&
  dispatchFieldIsSafe(summary.jobKind) &&
  dispatchFieldIsSafe(summary.state) &&
  dispatchFieldIsSafe(summary.phase)

// list_pylon_assignments(limit?, state?) — reads a bounded, public-safe LIST of
// the owner's active/recent assignments via the injected lister. Returns one
// public-safe summary line per assignment (ref + state + phase + verdict). Honest
// absence: an owner with no assignments reads "(no recent Pylon assignments …)";
// a read failure reads "(could not list assignments)"; with no lister wired it is
// honest rather than inventive. Side-effect-free, owner-scoped, no spend.
export const makeArtanisListPylonAssignmentsTool = (
  config: ArtanisPylonAssignmentsConfig = {},
): ArtanisOperatorReadTool => {
  const lister = config.lister
  const defaultLimit =
    config.defaultLimit ?? ARTANIS_ASSIGNMENTS_LIST_DEFAULT_LIMIT
  const maxLimit = config.maxLimit ?? ARTANIS_ASSIGNMENTS_LIST_MAX_LIMIT

  return {
    definition: {
      description:
        'List the public-safe STATUS of ALL your active/recent Pylon/Codex assignments in one call (vs. get_pylon_job_status, which inspects one ref). Returns one bounded line per assignment with its ref, job kind, lifecycle state, phase, and a coarse verify verdict (PASS/FAIL/in-progress). Use this to scan the whole burndown at a glance, instantly spot failed or stalled runs, and decide which to retry or dispatch in parallel; then call get_pylon_job_status on a specific ref for the full proof/failure detail. Owner-scoped (only your own linked-Pylon assignments). Optional "limit" bounds the count; optional "state" filters to one lifecycle state.',
      name: 'list_pylon_assignments',
      parameters: {
        additionalProperties: false,
        properties: {
          limit: {
            description: `Max number of assignments to return, a positive integer up to ${maxLimit} (default ${defaultLimit}).`,
            type: 'number',
          },
          state: {
            description:
              'Optional lifecycle-state filter, e.g. "accepted", "closeout_submitted", or "rejected". Omit to list all recent states.',
            type: 'string',
          },
        },
        required: [],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        if (lister === undefined) {
          return '(could not list assignments: no assignments lister is wired)'
        }
        const limit = parseAssignmentsLimit(args, defaultLimit, maxLimit)
        const stateFilter = parseAssignmentsStateFilter(args)

        const exit = yield* Effect.exit(Effect.tryPromise(() => lister(limit)))
        if (exit._tag === 'Failure') {
          return '(could not list assignments)'
        }

        const filtered =
          stateFilter === undefined
            ? exit.value
            : exit.value.filter(
                summary =>
                  summary.state.toLowerCase() === stateFilter ||
                  summary.phase.toLowerCase() === stateFilter,
              )
        const safe = filtered
          .filter(assignmentSummaryIsSafe)
          .slice(0, limit)

        if (safe.length === 0) {
          return stateFilter === undefined
            ? '(no recent Pylon assignments found for you)'
            : `(no recent Pylon assignments found with state "${stateFilter}")`
        }

        const header =
          stateFilter === undefined
            ? `Recent Pylon/Codex assignments (${safe.length}):`
            : `Recent Pylon/Codex assignments with state "${stateFilter}" (${safe.length}):`
        return [header, ...safe.map(formatAssignmentSummaryLine)].join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// get_khala_feedback — read a bounded, public-safe LIST of the most recent
// user feedback submitted through the Khala CLI `/feedback` command.
//
// This is Artanis's iteration-6 self-improvement capability. The Khala CLI lets
// users submit frustration/quality notes through `/feedback`; those land in the
// already-existing `khala_feedback` store (`khala-feedback-routes.ts`). This tool
// lets Artanis READ that stream himself so he can spot capability gaps, bugs, and
// style preferences and immediately triage them (route to unsupported-requests
// #6357 or plan a Codex fix), closing the user-feedback -> fix loop that drives
// adoption and 10x daily Khala token usage.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It never writes, mutates, dispatches, or
//     spends. It grants no new authority and fits the existing read-tool boundary.
//   - OWNER-SCOPED. It is only ever wired into the owner-authenticated operator
//     chat (the same admin-gated path as the other read tools); the production
//     reader is backed by the same admin-gated `khala_feedback` store the
//     `GET /api/operator/khala/feedback` route already uses.
//   - BOUNDED + FAIL-SOFT. A single read returns at most `maxLimit` records, each
//     feedback body truncated, so a chatty stream stays cheap. A reader rejection
//     degrades to an honest "(could not read feedback)" string; no reader wired is
//     honest rather than inventive; an empty store reads "(no recent ... feedback)".
// ---------------------------------------------------------------------------

// The default and max number of feedback records returned for one read, so a
// single read stays bounded even when there are many feedback rows.
export const ARTANIS_KHALA_FEEDBACK_DEFAULT_LIMIT = 10
export const ARTANIS_KHALA_FEEDBACK_MAX_LIMIT = 50

// Max characters of a single feedback body surfaced into Artanis's context; a
// longer body is truncated with an explicit marker so one essay cannot blow the
// context budget. The structured fields (ref/source) stay public-safe-gated.
export const ARTANIS_KHALA_FEEDBACK_TEXT_MAX_CHARS = 600

// A public-safe one-record projection of ONE Khala CLI feedback submission. The
// production reader builds this from the `khala_feedback` store; tests inject a
// fake reader. The free-text `feedback` is the value the owner is entitled to
// read, so it is kept verbatim (only truncated); the STRUCTURED fields are
// defensively public-safety-gated.
export type ArtanisKhalaFeedbackRecord = Readonly<{
  // The opaque feedback ref, e.g. `khala_feedback:fb_...`.
  feedbackRef: string
  // The user-submitted feedback body (kept verbatim; truncated if long).
  feedback: string
  // The submission source, e.g. `khala-cli`.
  source: string
  // The optional client version string, or null.
  clientVersion: string | null
  // Public-safe "submitted at" display string.
  createdAt: string
}>

// The injected, owner-scoped reader seam. Given a max count it resolves the most
// recent feedback records as public-safe projections, newest first. Returns `[]`
// when there is none (honest absence). A thrown rejection is treated by the tool
// as a soft read failure, never a fabricated list. The production reader
// (`artanis-operator-khala-feedback.ts`) reads the admin-gated `khala_feedback`
// store.
export type ArtanisKhalaFeedbackReader = (
  limit: number,
) => Promise<ReadonlyArray<ArtanisKhalaFeedbackRecord>>

export type ArtanisKhalaFeedbackConfig = Readonly<{
  reader?: ArtanisKhalaFeedbackReader | undefined
  defaultLimit?: number | undefined
  maxLimit?: number | undefined
  maxTextChars?: number | undefined
}>

// Coerce a model-produced `limit`/`count`/`max` arg into a bounded positive
// integer, clamped into [1, maxLimit]; an absent/invalid value falls back to the
// default. A model typo can never request an unboundedly large read.
const parseFeedbackLimit = (
  args: unknown,
  defaultLimit: number,
  maxLimit: number,
): number => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}
  const raw = record.limit ?? record.count ?? record.max
  let value: number | undefined
  if (typeof raw === 'number' && Number.isInteger(raw)) {
    value = raw
  } else if (typeof raw === 'string' && /^\d+$/.test(raw.trim())) {
    value = Number.parseInt(raw.trim(), 10)
  }
  if (value === undefined || value < 1) {
    return defaultLimit
  }
  return Math.min(value, maxLimit)
}

// Collapse internal whitespace and truncate a feedback body to a bounded length
// with an explicit marker, so one long submission stays cheap and renders as a
// single readable line.
const truncateFeedbackText = (text: string, maxChars: number): string => {
  const collapsed = text.replace(/\s+/g, ' ').trim()
  if (collapsed.length <= maxChars) return collapsed
  return `${collapsed.slice(0, maxChars)}...[truncated]`
}

// Format ONE feedback record as a bounded, public-safe entry carrying the
// submitted-at display, source, ref, and the (truncated) feedback body.
const formatFeedbackRecordLine = (
  record: ArtanisKhalaFeedbackRecord,
  maxTextChars: number,
): string => {
  const version =
    record.clientVersion !== null && dispatchFieldIsSafe(record.clientVersion)
      ? ` v${record.clientVersion}`
      : ''
  return `- [${record.createdAt}] (${record.source}${version}) ${record.feedbackRef}: "${truncateFeedbackText(
    record.feedback,
    maxTextChars,
  )}"`
}

// A feedback record is renderable only if its STRUCTURED fields pass the
// public-safety gate: the ref is ref-shaped and the source is safe. The free-text
// `feedback` body is intentionally NOT gated (the owner is entitled to read it);
// it is only truncated. This is a defensive second pass so an upstream store
// regression cannot leak private material into a structured ref/source field.
const feedbackRecordIsSafe = (record: ArtanisKhalaFeedbackRecord): boolean =>
  isSafeArtanisAssignmentRef(record.feedbackRef) &&
  dispatchFieldIsSafe(record.source) &&
  typeof record.feedback === 'string' &&
  record.feedback.trim() !== ''

// get_khala_feedback(limit?) — reads a bounded, public-safe LIST of the most
// recent Khala CLI `/feedback` submissions via the injected reader. Returns one
// entry per record (submitted-at + source + ref + truncated body). Honest
// absence: an empty store reads "(no recent Khala CLI feedback ...)"; a read
// failure reads "(could not read feedback)"; with no reader wired it is honest
// rather than inventive. Side-effect-free, owner-scoped, no spend.
export const makeArtanisGetKhalaFeedbackTool = (
  config: ArtanisKhalaFeedbackConfig = {},
): ArtanisOperatorReadTool => {
  const reader = config.reader
  const defaultLimit =
    config.defaultLimit ?? ARTANIS_KHALA_FEEDBACK_DEFAULT_LIMIT
  const maxLimit = config.maxLimit ?? ARTANIS_KHALA_FEEDBACK_MAX_LIMIT
  const maxTextChars =
    config.maxTextChars ?? ARTANIS_KHALA_FEEDBACK_TEXT_MAX_CHARS

  return {
    definition: {
      description:
        'Read the most recent USER FEEDBACK submitted through the Khala CLI /feedback command. Returns one bounded entry per submission with its submitted-at time, source, ref, and the (truncated) feedback body. Use this to hear directly from users - spot capability gaps, bugs, and style preferences (e.g. "too wordy, prefer more conversational") - then triage each one: route an unsupported request to the unsupported-requests track (#6357) or plan a Codex fix. Read-only, side-effect-free, owner-scoped. Optional "limit" bounds the count.',
      name: 'get_khala_feedback',
      parameters: {
        additionalProperties: false,
        properties: {
          limit: {
            description: `Max number of feedback records to return, a positive integer up to ${maxLimit} (default ${defaultLimit}).`,
            type: 'number',
          },
        },
        required: [],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        if (reader === undefined) {
          return '(could not read feedback: no feedback reader is wired)'
        }
        const limit = parseFeedbackLimit(args, defaultLimit, maxLimit)

        const exit = yield* Effect.exit(Effect.tryPromise(() => reader(limit)))
        if (exit._tag === 'Failure') {
          return '(could not read feedback)'
        }

        const safe = exit.value.filter(feedbackRecordIsSafe).slice(0, limit)
        if (safe.length === 0) {
          return '(no recent Khala CLI feedback found)'
        }

        const header = `Recent Khala CLI feedback (${safe.length}):`
        return [
          header,
          ...safe.map(record => formatFeedbackRecordLine(record, maxTextChars)),
        ].join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// get_unsupported_requests — read a bounded, public-safe LIST of the live
// unsupported-request ledger (the user-facing capability gaps that block Khala
// adoption).
//
// This is Artanis's iteration-8 self-improvement capability. The
// `GET /api/operator/khala/unsupported-requests` route (#6357), backed by
// `makeD1KhalaUnsupportedRequestStore.listRecent`, maintains the running ledger
// of what testers try that Khala cannot do yet — fed by trace reviews, Khala
// CLI feedback, and forum reports. This tool lets Artanis READ that ledger
// himself during a turn so he can see exactly which capability gaps are
// suppressing usage, match each to an open issue, and target Codex
// dispatch / forum mobilization at the highest-leverage gaps — directly
// speeding the 10x-daily-Khala-token goal by closing the gaps that block usage.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It only calls the injected ledger reader
//     (backed by the same admin-gated store the operator route uses); it never
//     writes, mutates, dispatches, or spends. It grants no new authority and
//     fits the existing read-tool boundary.
//   - OWNER-SCOPED. It is only ever wired into the owner-authenticated operator
//     chat (the same admin-gated path as the other read tools).
//   - BOUNDED + FAIL-SOFT. A single read returns at most `maxLimit` records,
//     each summary truncated, so a chatty ledger stays cheap. A reader rejection
//     degrades to an honest "(could not read unsupported requests)"; no reader
//     wired is honest rather than inventive; an empty ledger reads
//     "(no unsupported requests ...)".
//   - PUBLIC-SAFE. The ledger already enforces public-safe summaries/refs on
//     write; the tool applies a defensive second pass so an upstream regression
//     cannot leak private material into Artanis's context.
// ---------------------------------------------------------------------------

// The default and max number of ledger records returned for one read, so a
// single read stays bounded even when the ledger is large.
export const ARTANIS_UNSUPPORTED_REQUESTS_DEFAULT_LIMIT = 25
export const ARTANIS_UNSUPPORTED_REQUESTS_MAX_LIMIT = 100

// Max characters of a single ledger summary surfaced into Artanis's context; a
// longer summary is truncated with an explicit marker so one long entry cannot
// blow the context budget.
export const ARTANIS_UNSUPPORTED_REQUESTS_SUMMARY_MAX_CHARS = 400

// The bounded set of ledger lifecycle states a model may filter on. Anything
// else is ignored (the read returns the unfiltered recent set).
export const ARTANIS_UNSUPPORTED_REQUEST_STATUSES = [
  'open',
  'needs_issue',
  'issue_opened',
  'closed',
  'wont_do',
] as const
export type ArtanisUnsupportedRequestStatus =
  (typeof ARTANIS_UNSUPPORTED_REQUEST_STATUSES)[number]

// A public-safe one-record projection of ONE unsupported-request ledger row.
// The production reader builds this from the `khala_unsupported_requests` store;
// tests inject a fake reader. Every field is a public-safe projection of real D1
// state; it never carries raw traces, raw feedback transcripts, private paths,
// or provider payloads.
export type ArtanisUnsupportedRequestRecord = Readonly<{
  // The opaque request ref, e.g. `khala_unsupported:ur_...`.
  requestRef: string
  // A short public-safe title naming the gap.
  title: string
  // A bounded public-safe summary (kept verbatim; truncated if long).
  summary: string
  // The triage kind, e.g. 'missing_capability' | 'bug' | 'needs_triage' |
  // 'wont_do'.
  triageKind: string
  // The lifecycle status, e.g. 'open' | 'needs_issue' | 'issue_opened'.
  status: string
  // The source the gap came in through, e.g. 'trace_review' | 'forum'.
  sourceKind: string
  // The linked GitHub issue ref when one exists, else null.
  githubIssueRef: string | null
  // The next action the ledger derived, e.g. 'open_github_issue' | 'triage'.
  nextAction: string
  // Public-safe "last updated" display string.
  updatedAt: string
}>

// The injected, owner-scoped reader seam. Given a max count and an optional
// status filter it resolves the most recent ledger records as public-safe
// projections, newest first. Returns `[]` when the ledger is empty (honest
// absence). A thrown rejection is treated by the tool as a soft read failure,
// never a fabricated list. The production reader
// (`artanis-operator-unsupported-requests.ts`) reads the admin-gated
// `khala_unsupported_requests` store.
export type ArtanisUnsupportedRequestsReader = (
  input: Readonly<{
    limit: number
    status?: ArtanisUnsupportedRequestStatus | undefined
  }>,
) => Promise<ReadonlyArray<ArtanisUnsupportedRequestRecord>>

export type ArtanisUnsupportedRequestsConfig = Readonly<{
  reader?: ArtanisUnsupportedRequestsReader | undefined
  defaultLimit?: number | undefined
  maxLimit?: number | undefined
  maxSummaryChars?: number | undefined
}>

// Coerce a model-produced `limit`/`count`/`max` arg into a bounded positive
// integer, clamped into [1, maxLimit]; an absent/invalid value falls back to the
// default. A model typo can never request an unboundedly large read.
const parseUnsupportedRequestsLimit = (
  args: unknown,
  defaultLimit: number,
  maxLimit: number,
): number => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}
  const raw = record.limit ?? record.count ?? record.max
  let value: number | undefined
  if (typeof raw === 'number' && Number.isInteger(raw)) {
    value = raw
  } else if (typeof raw === 'string' && /^\d+$/.test(raw.trim())) {
    value = Number.parseInt(raw.trim(), 10)
  }
  if (value === undefined || value < 1) {
    return defaultLimit
  }
  return Math.min(value, maxLimit)
}

// Pull an optional bounded `status` filter out of model-produced args; a value
// outside the bounded enum is ignored (the read returns the unfiltered set).
const parseUnsupportedRequestsStatus = (
  args: unknown,
): ArtanisUnsupportedRequestStatus | undefined => {
  if (typeof args !== 'object' || args === null) return undefined
  const record = args as Record<string, unknown>
  const raw = record.status
  if (typeof raw !== 'string') return undefined
  const trimmed = raw.trim().toLowerCase()
  return (
    ARTANIS_UNSUPPORTED_REQUEST_STATUSES as ReadonlyArray<string>
  ).includes(trimmed)
    ? (trimmed as ArtanisUnsupportedRequestStatus)
    : undefined
}

// Collapse internal whitespace and truncate a ledger summary to a bounded length
// with an explicit marker, so one long entry stays cheap and renders readably.
const truncateUnsupportedSummary = (text: string, maxChars: number): string => {
  const collapsed = text.replace(/\s+/g, ' ').trim()
  if (collapsed.length <= maxChars) return collapsed
  return `${collapsed.slice(0, maxChars)}...[truncated]`
}

// A ledger record is renderable only if its STRUCTURED fields pass the
// public-safety gate: the ref is ref-shaped and the triage/status/source/title/
// next-action fields are safe. The free-text summary is intentionally NOT gated
// (the owner is entitled to read it); it is only truncated. This is a defensive
// second pass so an upstream store regression cannot leak private material into
// a structured field surfaced to Artanis.
const unsupportedRequestRecordIsSafe = (
  record: ArtanisUnsupportedRequestRecord,
): boolean =>
  isSafeArtanisAssignmentRef(record.requestRef) &&
  typeof record.title === 'string' &&
  record.title.trim() !== '' &&
  dispatchFieldIsSafe(record.title) &&
  dispatchFieldIsSafe(record.triageKind) &&
  dispatchFieldIsSafe(record.status) &&
  dispatchFieldIsSafe(record.sourceKind) &&
  dispatchFieldIsSafe(record.nextAction) &&
  (record.githubIssueRef === null ||
    dispatchFieldIsSafe(record.githubIssueRef))

// Format ONE ledger record as a bounded, public-safe entry carrying the
// updated-at display, triage/status, source, next action, linked issue (when
// present), ref, title, and the (truncated) summary.
const formatUnsupportedRequestLine = (
  record: ArtanisUnsupportedRequestRecord,
  maxSummaryChars: number,
): string => {
  const issue =
    record.githubIssueRef !== null &&
    dispatchFieldIsSafe(record.githubIssueRef)
      ? ` issue=${record.githubIssueRef}`
      : ''
  const summary =
    record.summary.trim() === ''
      ? ''
      : ` — ${truncateUnsupportedSummary(record.summary, maxSummaryChars)}`
  return `- [${record.updatedAt}] (${record.triageKind}/${record.status}; from ${record.sourceKind}; next=${record.nextAction})${issue} ${record.requestRef}: ${record.title}${summary}`
}

// get_unsupported_requests(limit?, status?) — reads a bounded, public-safe LIST
// of the live unsupported-request ledger via the injected reader. Returns one
// entry per record (updated-at + triage/status + ref + title + truncated
// summary). Honest absence: an empty ledger reads "(no unsupported requests
// ...)"; a read failure reads "(could not read unsupported requests)"; with no
// reader wired it is honest rather than inventive. Side-effect-free,
// owner-scoped, no spend.
export const makeArtanisGetUnsupportedRequestsTool = (
  config: ArtanisUnsupportedRequestsConfig = {},
): ArtanisOperatorReadTool => {
  const reader = config.reader
  const defaultLimit =
    config.defaultLimit ?? ARTANIS_UNSUPPORTED_REQUESTS_DEFAULT_LIMIT
  const maxLimit = config.maxLimit ?? ARTANIS_UNSUPPORTED_REQUESTS_MAX_LIMIT
  const maxSummaryChars =
    config.maxSummaryChars ?? ARTANIS_UNSUPPORTED_REQUESTS_SUMMARY_MAX_CHARS

  return {
    definition: {
      description: `Read the live UNSUPPORTED-REQUEST ledger: the running list of user-facing capability gaps that block Khala adoption (what testers try that Khala cannot do yet), fed by trace reviews, Khala CLI feedback, and forum reports. Returns one bounded entry per gap with its updated-at time, triage kind, lifecycle status, source, next action, any linked GitHub issue, ref, title, and a truncated summary. Use this to see exactly which gaps suppress usage, match each to an open issue, and target Codex dispatch / forum mobilization at the highest-leverage gaps to drive the 10x-daily-token goal. Read-only, side-effect-free, owner-scoped. Optional "limit" bounds the count (default ${defaultLimit}, max ${maxLimit}); optional "status" filters to one lifecycle state (${ARTANIS_UNSUPPORTED_REQUEST_STATUSES.join(
        ', ',
      )}).`,
      name: 'get_unsupported_requests',
      parameters: {
        additionalProperties: false,
        properties: {
          limit: {
            description: `Max number of ledger records to return, a positive integer up to ${maxLimit} (default ${defaultLimit}).`,
            type: 'number',
          },
          status: {
            description: `Optional lifecycle-status filter, one of ${ARTANIS_UNSUPPORTED_REQUEST_STATUSES.join(
              ', ',
            )}. Omit to list all recent statuses (e.g. "needs_issue" to find gaps that still need an issue opened).`,
            type: 'string',
          },
        },
        required: [],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        if (reader === undefined) {
          return '(could not read unsupported requests: no ledger reader is wired)'
        }
        const limit = parseUnsupportedRequestsLimit(
          args,
          defaultLimit,
          maxLimit,
        )
        const status = parseUnsupportedRequestsStatus(args)

        const exit = yield* Effect.exit(
          Effect.tryPromise(() => reader({ limit, status })),
        )
        if (exit._tag === 'Failure') {
          return '(could not read unsupported requests)'
        }

        const safe = exit.value
          .filter(unsupportedRequestRecordIsSafe)
          .slice(0, limit)
        if (safe.length === 0) {
          return status === undefined
            ? '(no unsupported requests found in the ledger)'
            : `(no unsupported requests found with status "${status}")`
        }

        const header =
          status === undefined
            ? `Unsupported-request ledger (${safe.length}):`
            : `Unsupported-request ledger with status "${status}" (${safe.length}):`
        return [
          header,
          ...safe.map(record =>
            formatUnsupportedRequestLine(record, maxSummaryChars),
          ),
        ].join('\n')
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// update_unsupported_request — WRITE/TRIAGE the live unsupported-request ledger
// (the user-facing capability gaps that block Khala adoption, #6357).
//
// This is Artanis's iteration-9 self-improvement capability. iteration-8
// (`get_unsupported_requests`) lets him READ the ledger but he could not ACT on
// it — every triage decision had to round-trip through a human updating the
// ledger out of band. This WRITE tool closes that loop: in the SAME turn he
// reads a gap he can move it through its lifecycle (e.g. needs_issue ->
// issue_opened -> closed), reset its triage kind, and link the GitHub issue he
// dispatched to fix it. That directly accelerates flip-product-promises-green
// and adoption — the levers behind the 10x-daily-Khala-token goal — instead of
// leaving the loop open.
//
// Authority discipline — this is a WRITE tool, NOT a risky/gated one, and it is
// safe to execute freely BECAUSE every effect is conservative by construction:
//   - OWNER-SCOPED + INTERNAL ONLY. It only mutates rows in the same admin-gated
//     `khala_unsupported_requests` ledger the operator route owns. It NEVER
//     spends, pays out, deploys, deletes, opens a real GitHub issue, posts to a
//     forum, or reaches any outward/third-party surface. (Opening the GitHub
//     issue itself stays a separate gated/manual action; this tool only RECORDS
//     the link once the issue exists.)
//   - BOUNDED + VALIDATED. The ref must be a public-safe ref-shaped token; the
//     status and triage-kind must be members of the bounded enums; the linked
//     issue must normalize to a public-safe `owner/repo#N` ref. An unknown enum
//     value or non-public-safe field is BLOCKED (never silently coerced), and at
//     least one change field is required.
//   - HONEST ABSENCE. An unknown ref reads "(not found …)"; a writer rejection
//     reads "(could not update …)"; with no writer wired it is honest rather
//     than inventive. It NEVER fabricates an "updated" result.
// ---------------------------------------------------------------------------

// The bounded set of triage kinds a model may set on a ledger entry. Mirrors the
// `KhalaUnsupportedRequestTriageKind` literals in `khala-unsupported-request-
// routes.ts`. Anything else is BLOCKED (never coerced).
export const ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS = [
  'needs_triage',
  'bug',
  'missing_capability',
  'wont_do',
] as const
export type ArtanisUnsupportedRequestTriageKind =
  (typeof ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS)[number]

// The fixed public repo a bare issue NUMBER is linked against (mirrors the
// repo-read tools). A model passing `issue: 6310` links
// `OpenAgentsInc/openagents#6310`.
const ARTANIS_UNSUPPORTED_REQUEST_ISSUE_REPO = `${ARTANIS_REPO_READ_OWNER}/${ARTANIS_REPO_READ_REPO}`

// The validated, public-safe triage update the write tool hands to the writer
// seam. `ref` is the ledger entry to mutate; the optional fields are the changes
// to apply (only the present ones change). Every field has already passed the
// public-safety + enum gates.
export type ArtanisUnsupportedRequestUpdate = Readonly<{
  ref: string
  status?: ArtanisUnsupportedRequestStatus | undefined
  triageKind?: ArtanisUnsupportedRequestTriageKind | undefined
  // The public-safe linked GitHub issue ref, e.g. `OpenAgentsInc/openagents#6310`.
  githubIssueRef?: string | undefined
}>

// The injected, owner-scoped writer seam. Given a validated update it applies the
// change to the owner's `khala_unsupported_requests` ledger and resolves the
// UPDATED public-safe record projection, or `null` when no entry matches the ref
// (honest absence — it must never fabricate or create a row). A thrown rejection
// is treated by the tool as a soft write failure, never a fabricated result. The
// production writer (`artanis-operator-unsupported-requests.ts`) reads the
// existing row from the admin-gated store, merges the change, and upserts it.
export type ArtanisUnsupportedRequestWriter = (
  update: ArtanisUnsupportedRequestUpdate,
) => Promise<ArtanisUnsupportedRequestRecord | null>

export type ArtanisUpdateUnsupportedRequestConfig = Readonly<{
  writer?: ArtanisUnsupportedRequestWriter | undefined
}>

export const ARTANIS_UNSUPPORTED_REQUEST_ISSUE_OPEN_RISKY_ACTION_KIND =
  'github_issue_open' as const

export type ArtanisUnsupportedRequestIssueInput = Readonly<{
  ledgerRef: string
  title: string
  body: string
  labels: ReadonlyArray<string>
}>

export type ArtanisUnsupportedRequestIssueOpenResult =
  | Readonly<{
      kind: 'created'
      issueNumber: number
      issueRef: string
      issueUrl: string
    }>
  | Readonly<{ kind: 'rejected'; reason: string }>

export type ArtanisUnsupportedRequestIssueOpener = (
  input: ArtanisUnsupportedRequestIssueInput,
) => Effect.Effect<ArtanisUnsupportedRequestIssueOpenResult>

export type ArtanisOpenUnsupportedRequestIssueConfig = Readonly<{
  reader?: ArtanisUnsupportedRequestsReader | undefined
  writer?: ArtanisUnsupportedRequestWriter | undefined
  opener?: ArtanisUnsupportedRequestIssueOpener | undefined
  isOwnerApproved?: (() => Effect.Effect<boolean>) | undefined
  maxScan?: number | undefined
}>

export type ArtanisGithubIssueOpenerConfig = Readonly<{
  token?: string | undefined
  owner?: string | undefined
  repo?: string | undefined
  fetchImpl?: typeof fetch | undefined
}>

export const makeArtanisGithubIssueOpener = (
  config: ArtanisGithubIssueOpenerConfig,
): ArtanisUnsupportedRequestIssueOpener => {
  const token = config.token?.trim()
  const owner = config.owner ?? ARTANIS_REPO_READ_OWNER
  const repo = config.repo ?? ARTANIS_REPO_READ_REPO
  const fetchImpl = config.fetchImpl ?? globalThis.fetch

  return input =>
    Effect.gen(function* () {
      if (token === undefined || token === '') {
        return { kind: 'rejected', reason: 'github_issue_token_not_configured' }
      }
      const response = yield* Effect.tryPromise(() =>
        fetchImpl(`https://api.github.com/repos/${owner}/${repo}/issues`, {
          body: JSON.stringify({
            body: input.body,
            labels: input.labels,
            title: input.title,
          }),
          headers: {
            Accept: 'application/vnd.github+json',
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json',
            'User-Agent': 'artanis-operator',
            'X-GitHub-Api-Version': '2022-11-28',
          },
          method: 'POST',
        }),
      ).pipe(
        Effect.orElseSucceed(
          () => new Response('', { status: 599, statusText: 'fetch failed' }),
        ),
      )
      if (!response.ok) {
        return {
          kind: 'rejected',
          reason: `github_issue_open_http_${response.status}`,
        }
      }
      const body = yield* Effect.tryPromise(
        () => response.json() as Promise<unknown>,
      ).pipe(Effect.orElseSucceed(() => undefined))
      const record =
        typeof body === 'object' && body !== null
          ? (body as Record<string, unknown>)
          : {}
      const issueNumber =
        typeof record.number === 'number' && Number.isInteger(record.number)
          ? record.number
          : null
      const issueUrl =
        typeof record.html_url === 'string' ? record.html_url : null
      if (issueNumber === null || issueNumber <= 0 || issueUrl === null) {
        return { kind: 'rejected', reason: 'github_issue_open_bad_response' }
      }
      return {
        issueNumber,
        issueRef: `${owner}/${repo}#${issueNumber}`,
        issueUrl,
        kind: 'created',
      }
    })
}

// The parse outcome for the model-produced ref argument: ABSENT (no ref field ->
// honest "invalid arguments"), INVALID (present but unsafe/empty -> honest
// "(blocked …)"), or VALID.
type ArtanisUpdateRefArg =
  | Readonly<{ kind: 'ref'; value: string }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'invalid'; raw: string }>

const parseUpdateRef = (record: Record<string, unknown>): ArtanisUpdateRefArg => {
  const raw = record.ref ?? record.requestRef ?? record.request_ref
  if (raw === undefined || raw === null) return { kind: 'absent' }
  if (typeof raw !== 'string') return { kind: 'invalid', raw: String(raw) }
  const trimmed = raw.trim()
  if (trimmed === '') return { kind: 'absent' }
  // A ledger ref is the same public-safe ref shape the read tool validates.
  return isSafeArtanisAssignmentRef(trimmed)
    ? { kind: 'ref', value: trimmed }
    : { kind: 'invalid', raw: trimmed }
}

// The parse outcome for an optional enum field: ABSENT (not provided), INVALID
// (provided but not in the bounded enum -> BLOCKED), or VALID. Unlike the READ
// tool's optional status filter (which silently ignores an unknown value), the
// WRITE tool BLOCKS an unknown enum so it can never persist a bogus state.
type ArtanisUpdateEnumArg<T> =
  | Readonly<{ kind: 'value'; value: T }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'invalid'; raw: string }>

const parseUpdateEnum = <T extends string>(
  raw: unknown,
  allowed: ReadonlyArray<T>,
): ArtanisUpdateEnumArg<T> => {
  if (raw === undefined || raw === null) return { kind: 'absent' }
  if (typeof raw !== 'string') return { kind: 'invalid', raw: String(raw) }
  const trimmed = raw.trim().toLowerCase()
  if (trimmed === '') return { kind: 'absent' }
  return (allowed as ReadonlyArray<string>).includes(trimmed)
    ? { kind: 'value', value: trimmed as T }
    : { kind: 'invalid', raw: trimmed }
}

// The parse outcome for the optional linked-issue field. ABSENT, INVALID
// (BLOCKED — not a positive number / not a public-safe ref), or VALID (a
// normalized public-safe `owner/repo#N` or a passed-through public-safe ref).
type ArtanisUpdateIssueArg =
  | Readonly<{ kind: 'ref'; value: string }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'invalid'; raw: string }>

// Accepts `issue` as a positive integer (or `#`/bare numeric string) and
// normalizes it to `OpenAgentsInc/openagents#N`, OR an explicit public-safe issue
// ref string via `githubIssueRef`/`issue_ref`. A non-public-safe or malformed
// value is BLOCKED.
const parseUpdateIssue = (
  record: Record<string, unknown>,
): ArtanisUpdateIssueArg => {
  const numeric = record.issue ?? record.issueNumber ?? record.issue_number
  if (typeof numeric === 'number') {
    return Number.isInteger(numeric) && numeric > 0
      ? { kind: 'ref', value: `${ARTANIS_UNSUPPORTED_REQUEST_ISSUE_REPO}#${numeric}` }
      : { kind: 'invalid', raw: String(numeric) }
  }
  const explicit =
    record.githubIssueRef ?? record.github_issue_ref ?? record.issueRef ??
    record.issue_ref ?? (typeof numeric === 'string' ? numeric : undefined)
  if (explicit === undefined || explicit === null) return { kind: 'absent' }
  if (typeof explicit !== 'string') {
    return { kind: 'invalid', raw: String(explicit) }
  }
  const trimmed = explicit.trim()
  if (trimmed === '') return { kind: 'absent' }
  // A bare `6310` or `#6310` -> normalized repo ref.
  if (/^#?\d+$/.test(trimmed)) {
    const n = Number.parseInt(trimmed.replace(/^#/, ''), 10)
    return n > 0
      ? { kind: 'ref', value: `${ARTANIS_UNSUPPORTED_REQUEST_ISSUE_REPO}#${n}` }
      : { kind: 'invalid', raw: trimmed }
  }
  // An explicit ref must be public-safe AND ref-shaped.
  return dispatchFieldIsSafe(trimmed) && isSafeArtanisAssignmentRef(trimmed)
    ? { kind: 'ref', value: trimmed }
    : { kind: 'invalid', raw: trimmed }
}

// Format the UPDATED ledger record as a bounded, public-safe confirmation block
// naming the new triage/status/next-action/linked-issue so the model can
// summarize the change honestly. Defensively gates each structured field.
const formatUpdatedUnsupportedRequest = (
  record: ArtanisUnsupportedRequestRecord,
): string => {
  const issue =
    record.githubIssueRef !== null && dispatchFieldIsSafe(record.githubIssueRef)
      ? record.githubIssueRef
      : '(none)'
  const safe = (value: string): string =>
    dispatchFieldIsSafe(value) ? value : '(redacted)'
  return [
    `Updated unsupported request ${record.requestRef}:`,
    `- status: ${safe(record.status)}`,
    `- triage kind: ${safe(record.triageKind)}`,
    `- next action: ${safe(record.nextAction)}`,
    `- linked issue: ${issue}`,
    `- title: ${safe(record.title)}`,
  ].join('\n')
}

// update_unsupported_request(ref, status?, triageKind?, issue?) — TRIAGES one
// unsupported-request ledger entry via the injected writer. Validates the ref +
// enums, requires at least one change, normalizes a linked issue to a public-safe
// ref, applies the change, and returns the UPDATED public-safe record block.
// Honest absence: an unknown ref reads "(not found …)"; a writer rejection reads
// "(could not update …)"; with no writer wired it is honest rather than
// inventive. WRITE tool: owner-scoped, internal-ledger-only, NO spend/payout/
// deploy/delete/outward authority.
export const makeArtanisUpdateUnsupportedRequestTool = (
  config: ArtanisUpdateUnsupportedRequestConfig = {},
): ArtanisOperatorWriteTool => {
  const writer = config.writer

  return {
    definition: {
      description: `TRIAGE the live unsupported-request ledger (#6357): move ONE capability-gap entry through its lifecycle, set its triage kind, and link the GitHub issue you dispatched to fix it. Use this right after get_unsupported_requests to ACT on a gap in the same turn instead of leaving it for a human. Pass the entry "ref"; optionally a new "status" (${ARTANIS_UNSUPPORTED_REQUEST_STATUSES.join(
        ', ',
      )}), a new "triageKind" (${ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS.join(
        ', ',
      )}), and/or the public GitHub "issue" number you opened to link it (e.g. issue 6310, recorded as ${ARTANIS_UNSUPPORTED_REQUEST_ISSUE_REPO}#6310). At least one change is required. This RECORDS triage state in our own owner-scoped ledger only — it does NOT open the GitHub issue, spend, deploy, or take any outward/destructive action. An unknown ref reads "(not found …)".`,
      name: 'update_unsupported_request',
      parameters: {
        additionalProperties: false,
        properties: {
          issue: {
            description: `Public GitHub issue NUMBER to link (e.g. 6310), recorded as ${ARTANIS_UNSUPPORTED_REQUEST_ISSUE_REPO}#6310. Omit to leave the linked issue unchanged.`,
            type: 'number',
          },
          ref: {
            description:
              'The unsupported-request ledger entry ref to update, e.g. "khala_unsupported:ur_…". A public-safe ref string.',
            type: 'string',
          },
          status: {
            description: `Optional new lifecycle status, one of ${ARTANIS_UNSUPPORTED_REQUEST_STATUSES.join(
              ', ',
            )}. An unknown value is rejected.`,
            enum: [...ARTANIS_UNSUPPORTED_REQUEST_STATUSES],
            type: 'string',
          },
          triageKind: {
            description: `Optional new triage kind, one of ${ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS.join(
              ', ',
            )}. An unknown value is rejected.`,
            enum: [...ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS],
            type: 'string',
          },
        },
        required: ['ref'],
        type: 'object',
      },
    },
    execute: (args: unknown) =>
      Effect.gen(function* () {
        const record =
          typeof args === 'object' && args !== null
            ? (args as Record<string, unknown>)
            : {}

        const ref = parseUpdateRef(record)
        if (ref.kind === 'absent') {
          return '(invalid arguments: a string "ref" is required)'
        }
        if (ref.kind === 'invalid') {
          return `(blocked: "${ref.raw}" is not an allowed public-safe ledger ref)`
        }

        const status = parseUpdateEnum(
          record.status,
          ARTANIS_UNSUPPORTED_REQUEST_STATUSES,
        )
        if (status.kind === 'invalid') {
          return `(blocked: "${status.raw}" is not a valid status; use one of ${ARTANIS_UNSUPPORTED_REQUEST_STATUSES.join(
            ', ',
          )})`
        }

        const triageKind = parseUpdateEnum(
          record.triageKind ?? record.triage_kind ?? record.kind,
          ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS,
        )
        if (triageKind.kind === 'invalid') {
          return `(blocked: "${triageKind.raw}" is not a valid triage kind; use one of ${ARTANIS_UNSUPPORTED_REQUEST_TRIAGE_KINDS.join(
            ', ',
          )})`
        }

        const issue = parseUpdateIssue(record)
        if (issue.kind === 'invalid') {
          // The raw value may itself carry non-public-safe material, so it is
          // gated before being echoed back into the model's context.
          const shown = dispatchFieldIsSafe(issue.raw)
            ? `"${issue.raw}" `
            : ''
          return `(blocked: ${shown}is not a valid public-safe GitHub issue to link; pass a positive issue number like 6310)`
        }

        if (
          status.kind === 'absent' &&
          triageKind.kind === 'absent' &&
          issue.kind === 'absent'
        ) {
          return '(invalid arguments: provide at least one of "status", "triageKind", or "issue" to change)'
        }

        if (writer === undefined) {
          return `(could not update "${ref.value}": no ledger writer is wired)`
        }

        const update: ArtanisUnsupportedRequestUpdate = {
          githubIssueRef: issue.kind === 'ref' ? issue.value : undefined,
          ref: ref.value,
          status: status.kind === 'value' ? status.value : undefined,
          triageKind: triageKind.kind === 'value' ? triageKind.value : undefined,
        }

        const exit = yield* Effect.exit(
          Effect.tryPromise(() => writer(update)),
        )
        if (exit._tag === 'Failure') {
          return `(could not update "${ref.value}")`
        }
        const updated = exit.value
        if (updated === null) {
          return `(not found: no unsupported request matches ref "${ref.value}")`
        }
        return formatUpdatedUnsupportedRequest(updated)
      }),
    kind: 'write',
  }
}

const parseIssueOpenRef = (
  args: unknown,
): ArtanisUpdateRefArg => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}
  return parseUpdateRef(record)
}

const labelsForUnsupportedRequestIssue = (
  record: ArtanisUnsupportedRequestRecord,
): ReadonlyArray<string> => [
  'khala',
  record.triageKind === 'bug' ? 'bug' : 'missing-capability',
  'from-unsupported-request-ledger',
]

const buildUnsupportedRequestIssueBody = (
  record: ArtanisUnsupportedRequestRecord,
): string =>
  [
    '## Public-safe unsupported request',
    '',
    `Ledger ref: ${record.requestRef}`,
    `Source: ${record.sourceKind}`,
    `Triage: ${record.triageKind}`,
    `Status before issue: ${record.status}`,
    '',
    '## Summary',
    '',
    record.summary.trim() === ''
      ? '(No additional public-safe summary recorded.)'
      : record.summary.trim(),
    '',
    'This issue was opened from a public-safe unsupported-request ledger entry. Do not copy raw traces, private prompts, local paths, credentials, wallet material, or provider payloads into this issue.',
  ].join('\n')

const buildUnsupportedRequestIssuePlan = (
  recordRef: string,
): string =>
  [
    'Open one GitHub issue in OpenAgentsInc/openagents for an unsupported-request ledger item marked needs_issue, using public-safe title/summary refs only.',
    `Ledger ref: ${recordRef}`,
    'After GitHub returns the issue number, update the same ledger row to status issue_opened and link OpenAgentsInc/openagents#N.',
  ].join('\n')

// open_unsupported_request_issue(ref) — GATED outward action. It opens ONE
// public GitHub issue for a `needs_issue` unsupported-request ledger row, then
// records the resulting issue ref back onto that row as `issue_opened`.
export const makeArtanisOpenUnsupportedRequestIssueTool = (
  config: ArtanisOpenUnsupportedRequestIssueConfig = {},
): ArtanisOperatorGatedTool => {
  const maxScan = config.maxScan ?? ARTANIS_UNSUPPORTED_REQUESTS_MAX_LIMIT

  return {
    definition: {
      description:
        'Open a public GitHub issue for ONE unsupported-request ledger item whose status is needs_issue, using public-safe evidence refs only, then update that ledger row to issue_opened with the returned issue number. This is owner-gated because it writes to GitHub. It never includes raw traces, private prompts, local paths, credentials, wallet material, or provider payloads.',
      name: 'open_unsupported_request_issue',
      parameters: {
        additionalProperties: false,
        properties: {
          ref: {
            description:
              'The unsupported-request ledger entry ref to open an issue for, e.g. "khala_unsupported:ur_...".',
            type: 'string',
          },
        },
        required: ['ref'],
        type: 'object',
      },
    },
    kind: 'gated',
    riskyActionKind: ARTANIS_UNSUPPORTED_REQUEST_ISSUE_OPEN_RISKY_ACTION_KIND,
    run: (args: unknown): Effect.Effect<ArtanisOperatorGatedResult> =>
      Effect.gen(function* () {
        const ref = parseIssueOpenRef(args)
        if (ref.kind === 'absent') {
          return {
            outcome: 'deferred',
            plan: '(invalid arguments: a string "ref" is required)',
            reason: 'invalid_arguments',
          }
        }
        if (ref.kind === 'invalid') {
          return {
            outcome: 'deferred',
            plan: `(blocked: "${ref.raw}" is not an allowed public-safe ledger ref)`,
            reason: 'invalid_arguments',
          }
        }

        const plan = buildUnsupportedRequestIssuePlan(ref.value)
        if (config.reader === undefined) {
          return { outcome: 'deferred', plan, reason: 'reader_not_wired' }
        }
        if (config.writer === undefined) {
          return { outcome: 'deferred', plan, reason: 'ledger_writer_not_wired' }
        }
        if (config.opener === undefined) {
          return { outcome: 'deferred', plan, reason: 'github_opener_not_wired' }
        }

        const approved = yield* (config.isOwnerApproved?.() ??
          Effect.succeed(false)).pipe(Effect.orElseSucceed(() => false))
        if (!approved) {
          return {
            outcome: 'deferred',
            plan,
            reason: 'no_effective_owner_approval',
          }
        }

        const rows = yield* Effect.tryPromise(() =>
          config.reader!({ limit: maxScan, status: 'needs_issue' }),
        ).pipe(Effect.orElseSucceed(() => []))
        const record = rows.find(row => row.requestRef === ref.value)
        if (record === undefined) {
          return {
            outcome: 'deferred',
            plan,
            reason: 'needs_issue_row_not_found',
          }
        }
        if (
          record.githubIssueRef !== null ||
          record.nextAction !== 'open_github_issue'
        ) {
          return {
            outcome: 'deferred',
            plan,
            reason: 'issue_not_required',
          }
        }

        const title = record.title
        if (
          ![
            title,
            record.summary,
            record.requestRef,
            record.sourceKind,
            record.triageKind,
            ...labelsForUnsupportedRequestIssue(record),
          ].every(dispatchFieldIsSafe)
        ) {
          return {
            outcome: 'deferred',
            plan,
            reason: 'non_public_safe_issue_payload',
          }
        }
        const body = buildUnsupportedRequestIssueBody(record)

        const opened = yield* config
          .opener({
            body,
            labels: labelsForUnsupportedRequestIssue(record),
            ledgerRef: record.requestRef,
            title,
          })
          .pipe(
            Effect.orElseSucceed(
              () =>
                ({
                  kind: 'rejected',
                  reason: 'github_issue_open_failed',
                }) as const,
            ),
          )
        if (opened.kind === 'rejected') {
          return { outcome: 'deferred', plan, reason: opened.reason }
        }

        const updated = yield* Effect.tryPromise(() =>
          config.writer!({
            githubIssueRef: opened.issueRef,
            ref: record.requestRef,
            status: 'issue_opened',
          }),
        ).pipe(Effect.orElseSucceed(() => null))
        const ledgerLine =
          updated === null
            ? 'ledgerUpdate: failed_after_issue_created'
            : `ledgerUpdate: ${updated.status}; nextAction=${updated.nextAction}`
        return {
          assignmentRef: opened.issueRef,
          durableRequestId: null,
          outcome: 'executed',
          summary: [
            `issueRef: ${opened.issueRef}`,
            `issueUrl: ${opened.issueUrl}`,
            `ledgerRef: ${record.requestRef}`,
            ledgerLine,
          ].join('\n'),
        }
      }),
  }
}

// ---------------------------------------------------------------------------
// #6366 — dispatch_codex_task (GATED: pylon_job_dispatch).
// ---------------------------------------------------------------------------
//
// #6366 follow-up: this tool was originally plan-only (it returned the exact
// `pylon khala request --workflow codex_agent_task …` it WOULD run but never
// fired). It is now a GATED tool: it can actually CREATE the Khala -> Pylon ->
// Codex `codex_agent_task` assignment, but ONLY behind an effective owner
// approval and ONLY through a wired execution seam. Everything stays
// conservative by construction:
//   - own-capacity + no-spend ONLY (the seam targets the OWNER's linked Pylon
//     and uses the `unpaid_smoke` coding-delegation path; it moves no money,
//     grants no payout, and never touches pooled/third-party capacity);
//   - default DEFER: with no execution seam wired, or no effective owner
//     approval, or no eligible linked Pylon, it returns the public-safe plan
//     plus a typed reason and never fires;
//   - never fake: an `executed` outcome is returned ONLY when the seam really
//     created an assignment, and it carries the real `assignmentRef`.

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

// The validated, public-safe plan input the gated tool hands to the execution
// seam. Every field has already passed the public-safety gate.
export type ArtanisDispatchPlanInput = Readonly<{
  objective: string
  branch: string
  verify: string | undefined
  issue: number | undefined
  filePaths: ReadonlyArray<string>
  // The composed public-safe prompt the Pylon/Codex runner receives.
  prompt: string
}>

// What the execution seam returns: a real created assignment, or a typed
// rejection (mapped to a deferral so Artanis never fakes an execution).
export type ArtanisDispatchCreateResult =
  | Readonly<{
      kind: 'created'
      assignmentRef: string
      durableRequestId: string | null
      pylonRef: string
    }>
  | Readonly<{ kind: 'rejected'; reason: string }>

// The owner-scoped execution seam injected by the route. `isOwnerApproved`
// reports whether an effective owner approval for `pylon_job_dispatch` exists
// right now; `createCodexAssignment` actually creates the own-capacity,
// no-spend assignment via the server coding-delegation route, targeting the
// OWNER's linked Pylon. When absent, the tool stays plan-only (defers).
export type ArtanisDispatchExecution = Readonly<{
  isOwnerApproved: () => Effect.Effect<boolean>
  createCodexAssignment: (
    plan: ArtanisDispatchPlanInput,
  ) => Effect.Effect<ArtanisDispatchCreateResult>
}>

// Build (and public-safety-gate) the dispatch plan from model-produced args.
// Returns either the validated plan input + the public-safe plan text, or an
// honest message (invalid args / blocked field) used as the deferral reason.
const buildArtanisDispatchPlan = (
  args: unknown,
  defaultBranch: string,
):
  | Readonly<{ ok: true; input: ArtanisDispatchPlanInput; planText: string }>
  | Readonly<{ ok: false; message: string }> => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}
  const objective = asString(record.objective)
  if (objective === undefined) {
    return {
      message:
        '(invalid arguments: a public-safe "objective" string is required)',
      ok: false,
    }
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
    return {
      message:
        '(blocked: a dispatch field contained non-public-safe material; rephrase with public issue numbers, public file paths, and public verification commands only)',
      ok: false,
    }
  }
  const badPath = filePaths.find(path => !isSafeArtanisRepoPath(path))
  if (badPath !== undefined) {
    return {
      message: `(blocked: "${badPath}" is not an allowed public repo path)`,
      ok: false,
    }
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

  return {
    input: { branch, filePaths, issue, objective, prompt, verify },
    ok: true,
    planText: lines.join('\n'),
  }
}

// dispatch_codex_task — a GATED (pylon_job_dispatch) tool. With no execution
// seam wired (or no effective owner approval, or no eligible linked Pylon) it
// DEFERS and returns the exact public-safe Khala -> Pylon -> Codex dispatch it
// WOULD run. Behind an effective owner approval AND a wired execution seam it
// actually CREATES the own-capacity, no-spend assignment and reports the real
// `assignmentRef`. It never spends, grants payout, fakes an execution, or uses
// pooled/third-party capacity.
export const makeArtanisDispatchCodexTaskTool = (
  config: Readonly<{
    defaultBranch?: string | undefined
    execution?: ArtanisDispatchExecution | undefined
  }> = {},
): ArtanisOperatorGatedTool => {
  const defaultBranch = config.defaultBranch ?? 'main'
  const execution = config.execution

  return {
    definition: {
      description:
        'Dispatch a Codex coding task through the Khala -> Pylon -> Codex burndown loop against the OWNER\'s own linked Pylon (own-capacity, NO spend, no payout). This is a gated (pylon_job_dispatch) action: it executes ONLY behind an effective owner approval; otherwise it returns the exact public-safe dispatch that would run, pending approval. Inputs MUST be public-safe (public issue numbers, public file paths, public verification commands) — no secrets, tokens, or private content.',
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
    kind: 'gated',
    riskyActionKind: 'pylon_job_dispatch',
    run: (args: unknown): Effect.Effect<ArtanisOperatorGatedResult> =>
      Effect.gen(function* () {
        const built = buildArtanisDispatchPlan(args, defaultBranch)
        if (!built.ok) {
          // Invalid/blocked args never execute; defer with the honest message.
          return {
            outcome: 'deferred',
            plan: built.message,
            reason: 'invalid_arguments',
          }
        }

        // No execution seam wired -> stay plan-only (the original behavior).
        if (execution === undefined) {
          return {
            outcome: 'deferred',
            plan: built.planText,
            reason: 'execution_not_wired',
          }
        }

        // Owner-approval gate. Default conservative: any failure reads as
        // "not approved" and defers — never an accidental execution.
        const approved = yield* execution
          .isOwnerApproved()
          .pipe(Effect.orElseSucceed(() => false))
        if (!approved) {
          return {
            outcome: 'deferred',
            plan: built.planText,
            reason: 'no_effective_owner_approval',
          }
        }

        // Approved: actually create the own-capacity, no-spend assignment. A
        // seam defect degrades to an honest deferral, never a fabricated ref.
        const created = yield* execution
          .createCodexAssignment(built.input)
          .pipe(
            Effect.orElseSucceed(
              () =>
                ({
                  kind: 'rejected',
                  reason: 'dispatch_execution_failed',
                }) as const,
            ),
          )
        if (created.kind === 'rejected') {
          return {
            outcome: 'deferred',
            plan: built.planText,
            reason: created.reason,
          }
        }

        const summary = [
          `assignmentRef: ${created.assignmentRef}`,
          `pylonRef: ${created.pylonRef}`,
          `durableRequestId: ${created.durableRequestId ?? '(none)'}`,
          'paymentMode: unpaid_smoke (no spend, own_capacity)',
          'settlement: not_applicable; payoutClaimAllowed: false',
        ].join('\n')
        return {
          assignmentRef: created.assignmentRef,
          durableRequestId: created.durableRequestId,
          outcome: 'executed',
          summary,
        }
      }),
  }
}

// ---------------------------------------------------------------------------
// get_network_stats — LIVE public stats + token pace (epic #6359).
// ---------------------------------------------------------------------------
//
// A READ tool that fetches the three live public stats endpoints used by /stats
// (all-time scalar, per-day history in America/Chicago, model-mix) and returns a
// compact public-safe snapshot WITH the computed pace block. This is how Artanis
// checks, on demand, whether today is on track for the daily token target (at
// least 4x the prior day, goal 10x). Public-safe aggregates only; no per-user,
// provider, secret, or wallet material. Fail-soft: an unreachable endpoint
// degrades to an honest "(could not fetch …)" string, never invention.
export const makeArtanisGetNetworkStatsTool = (
  config: ArtanisGetNetworkStatsConfig = {},
): ArtanisOperatorReadTool => ({
  definition: {
    description:
      "Fetch the LIVE public Khala network stats (the /stats data): all-time tokens served, the last few days of per-day tokens served (America/Chicago), the model-family mix, and a computed PACE block \u2014 today's tokens so far, the fraction of the Central day elapsed, the projected total by midnight Central at the current pace, yesterday's tokens, the 4x daily floor and 10x stretch-goal targets, the gap to the 4x floor, and whether we are BEHIND pace. Use this to judge whether today is on track for the daily token target (at least 4x the prior day, goal 10x). Takes no arguments.",
    name: 'get_network_stats',
    parameters: {
      additionalProperties: false,
      properties: {},
      type: 'object',
    },
  },
  execute: (_args: unknown) =>
    Effect.promise(() =>
      // In the Worker the loadStats override reads D1 directly (the worker cannot
      // reliably HTTP-fetch its own public zone); otherwise fall back to HTTP.
      (config.loadStats ?? (() => fetchArtanisNetworkStats(config)))(),
    ).pipe(
      Effect.map(stats => {
        const paceLine =
          stats.pace === null
            ? 'Token pace: (not enough Central-day history to project a target yet).'
            : formatArtanisTokenPaceLine(stats.pace)
        const snapshot = {
          allTimeTokensServed: stats.allTimeTokensServed,
          history: stats.history,
          modelMix: stats.modelMix.slice(0, 6),
          pace: stats.pace,
          timezone: stats.timezone,
          todayTokens: stats.todayTokens,
        }
        return [
          paceLine,
          '',
          `All-time tokens served: ${stats.allTimeTokensServed.toLocaleString(
            'en-US',
          )}.`,
          'Public stats snapshot (JSON, public-safe aggregates only):',
          JSON.stringify(snapshot),
        ].join('\n')
      }),
    ),
  kind: 'read',
})

// ---------------------------------------------------------------------------
// get_fleet_status — unified operator fleet status for one-turn decisions.
// ---------------------------------------------------------------------------
//
// Read-only composition of the status buckets Artanis otherwise has to gather
// through several separate tools. It deliberately reuses the existing read
// loaders: pace from get_network_stats, Pylon/Codex spread from
// list_pylon_assignments, GLM readiness from get_glm_fleet_status, synthetic
// load/watchdog state from get_synthetic_load_status, and trace-review/goal
// health from get_trace_review. No dispatch, spend, scale-out, or quarantine
// authority is added here; this is one bounded status read.

export type ArtanisFleetStatusConfig = Readonly<{
  networkStats?: ArtanisGetNetworkStatsConfig | undefined
  pylonAssignments?: ArtanisPylonAssignmentsConfig | undefined
  glmFleetStatus?: ArtanisGlmFleetStatusConfig | undefined
  syntheticLoadStatus?: ArtanisSyntheticLoadStatusConfig | undefined
  traceReview?: ArtanisTraceReviewConfig | undefined
}>

const fleetStatusUnavailableLine = (label: string): string =>
  `${label}: unavailable (reader not wired or failed).`

const loadFleetStatusNetworkStats = (
  config: ArtanisGetNetworkStatsConfig | undefined,
): Effect.Effect<ArtanisNetworkStats | null> =>
  Effect.tryPromise(() =>
    (config?.loadStats ?? (() => fetchArtanisNetworkStats(config ?? {})))(),
  ).pipe(Effect.orElseSucceed(() => null))

const loadFleetStatusAssignments = (
  config: ArtanisPylonAssignmentsConfig | undefined,
): Effect.Effect<ReadonlyArray<ArtanisPylonAssignmentSummary> | null> => {
  if (config?.lister === undefined) {
    return Effect.succeed(null)
  }
  const limit =
    config.defaultLimit ?? ARTANIS_ASSIGNMENTS_LIST_DEFAULT_LIMIT
  return Effect.tryPromise(() => config.lister!(limit)).pipe(
    Effect.orElseSucceed(() => null),
  )
}

const loadFleetStatusGlm = (
  config: ArtanisGlmFleetStatusConfig | undefined,
): Effect.Effect<ArtanisGlmFleetStatus | null> => {
  if (config?.loadFleetStatus === undefined) {
    return Effect.succeed(null)
  }
  return Effect.tryPromise(() => config.loadFleetStatus!()).pipe(
    Effect.orElseSucceed(() => null),
  )
}

const loadFleetStatusSyntheticRuns = (
  config: ArtanisSyntheticLoadStatusConfig | undefined,
): Effect.Effect<ReadonlyArray<ArtanisSyntheticLoadRun> | null> => {
  if (config?.reader === undefined) {
    return Effect.succeed(null)
  }
  return Effect.tryPromise(() => config.reader!()).pipe(
    Effect.orElseSucceed(() => null),
  )
}

const loadFleetStatusTraceReview = (
  config: ArtanisTraceReviewConfig | undefined,
): Effect.Effect<ArtanisTraceReviewSummary | null> => {
  if (config?.loadReport === undefined) {
    return Effect.succeed(null)
  }
  return Effect.tryPromise(() => config.loadReport!()).pipe(
    Effect.map(normalizeArtanisTraceReview),
    Effect.orElseSucceed(() => null),
  )
}

const formatFleetAssignmentSpread = (
  assignments: ReadonlyArray<ArtanisPylonAssignmentSummary> | null,
): string => {
  if (assignments === null) {
    return fleetStatusUnavailableLine('Fleet')
  }
  if (assignments.length === 0) {
    return 'Fleet: no recent Pylon/Codex assignments.'
  }
  const active = assignments.filter(summary =>
    ['accepted', 'offered', 'running'].includes(summary.state),
  ).length
  const failed = assignments.filter(
    summary => summary.verifyResult === 'fail',
  ).length
  const passed = assignments.filter(
    summary => summary.verifyResult === 'pass',
  ).length
  const inProgress = assignments.length - failed - passed
  return [
    `Fleet: ${assignments.length} recent Pylon/Codex assignments; active=${active}, pass=${passed}, fail=${failed}, in-progress=${inProgress}.`,
    ...assignments.slice(0, 8).map(formatAssignmentSummaryLine),
  ].join('\n')
}

const formatFleetGlmLine = (status: ArtanisGlmFleetStatus | null): string =>
  status === null
    ? fleetStatusUnavailableLine('GLM')
    : `GLM: status=${status.status}, ready=${status.readyReplicas}/${status.totalReplicas}, warm=${status.warmReplicas ?? 0}.`

const formatFleetWatchdogLine = (
  runs: ReadonlyArray<ArtanisSyntheticLoadRun> | null,
): string => {
  if (runs === null) {
    return fleetStatusUnavailableLine('Watchdog')
  }
  if (runs.length === 0) {
    return 'Watchdog: no active synthetic-load runs reported.'
  }
  return [
    `Watchdog: ${runs.length} active synthetic-load run(s).`,
    ...runs.slice(0, 5).map(run => {
      const progress =
        run.targetTokens === null || run.targetTokens <= 0
          ? 'tokens unknown'
          : `${run.tokensBurned}/${run.targetTokens} tokens`
      return `- ${run.runRef} | ${run.runType} | state=${run.state} | ${progress}`
    }),
  ].join('\n')
}

const formatFleetBrainLine = (trace: ArtanisTraceReviewSummary | null): string =>
  trace === null
    ? fleetStatusUnavailableLine('Brain')
    : `Brain: trace-review ${trace.traceCount} turns / ${trace.totalTokens} tokens over ${trace.windowHours ?? 'unknown'}h; top outcome=${trace.outcomes[0]?.outcome ?? 'none'}.`

export const makeArtanisGetFleetStatusTool = (
  config: ArtanisFleetStatusConfig = {},
): ArtanisOperatorReadTool => ({
  definition: {
    description:
      'Read the unified operator fleet status in one bounded call: Pace (token burn rate and gap to floor), Fleet (Pylon/Codex concurrency, recent assignment spread, pass/fail/in-flight), Watchdog (active synthetic-load/lease alerts when wired), GLM (serving readiness), and Brain (Artanis/Khala trace-review health). Read-only, side-effect-free, public-safe aggregates only; grants no dispatch, spend, scale-out, quarantine, payout, or deployment authority.',
    name: 'get_fleet_status',
    parameters: {
      additionalProperties: false,
      properties: {},
      type: 'object',
    },
  },
  execute: (_args: unknown) =>
    Effect.gen(function* () {
      const [networkStats, assignments, glm, syntheticRuns, traceReview] =
        yield* Effect.all([
          loadFleetStatusNetworkStats(config.networkStats),
          loadFleetStatusAssignments(config.pylonAssignments),
          loadFleetStatusGlm(config.glmFleetStatus),
          loadFleetStatusSyntheticRuns(config.syntheticLoadStatus),
          loadFleetStatusTraceReview(config.traceReview),
        ])

      const pace =
        networkStats?.pace === undefined || networkStats.pace === null
          ? 'unavailable (reader not wired or failed).'
          : formatArtanisTokenPaceLine(networkStats.pace)

      return [
        'Unified fleet status:',
        '',
        `Pace: ${pace}`,
        '',
        formatFleetAssignmentSpread(assignments),
        '',
        formatFleetWatchdogLine(syntheticRuns),
        '',
        formatFleetGlmLine(glm),
        '',
        formatFleetBrainLine(traceReview),
      ].join('\n')
    }),
  kind: 'read',
})

// ---------------------------------------------------------------------------
// get_glm_fleet_status — LIVE GLM inference-fleet readiness (iteration 7).
// ---------------------------------------------------------------------------
//
// Artanis named this his ONE highest-value next tool in iteration 7 of his
// self-improvement loop: an on-demand READ of the live GLM serving fleet's
// readiness, so he can GATE synthetic-load and Codex-dispatch decisions on
// healthy capacity instead of piling work onto a saturated/cold fleet and
// degrading real users. This directly speeds the stress/Terminal-Bench and
// parallel-delegation goals: before he plans a synthetic-load burn or fans out
// parallel coding work, he can confirm the fleet is actually `ready` with spare
// ready replicas first.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It reads the SAME public-safe GLM fleet
//     readiness projection the `GET /v1/gateway/glm-fleet/readiness` route
//     exposes — aggregate replica COUNTS + an overall status only. It never
//     probes hosts, returns raw host origins, changes replica state, or touches
//     the GLM serving / admission path.
//   - PUBLIC-SAFE. Only the overall status and bounded ready/total/warm replica
//     COUNTS are surfaced; no host origins, credentials, prompts, completions,
//     prices, or balances ever enter the summary.
//   - FAIL-SOFT + HONEST. An unreachable source (or an unexpected response
//     shape) degrades to an honest "(could not fetch GLM fleet status ...)"
//     string and NEVER fabricates replica numbers.

// The default live source for the fleet readiness read (the public-safe route).
export const ARTANIS_GLM_FLEET_STATUS_URL =
  'https://openagents.com/api/v1/gateway/glm-fleet/readiness'

// A public-safe normalized projection of the GLM serving fleet's readiness. The
// production loader builds this from the same in-worker GLM fleet readiness
// projection the public route uses; tests inject a fake fetch returning the
// readiness payload. Aggregate counts + status only - never host-level detail.
export type ArtanisGlmFleetStatus = Readonly<{
  // The overall fleet status, e.g. 'ready' | 'degraded' | 'unavailable'.
  status: string
  // Replicas ready to serve right now.
  readyReplicas: number
  // Total configured replicas.
  totalReplicas: number
  // Replicas warm (loaded but not yet fully ready), or null when not reported.
  warmReplicas: number | null
}>

export type ArtanisGlmFleetStatusConfig = Readonly<{
  // The readiness URL to fetch (default the public-safe route). Used only on the
  // HTTP path (no `loadFleetStatus` override).
  url?: string | undefined
  // Injected for testability; defaults to the global fetch.
  fetchImpl?: typeof fetch | undefined
  // In-worker override (D1 + in-memory heartbeat projection). The Worker cannot
  // reliably HTTP-fetch its OWN public zone, so production wires this to read the
  // fleet readiness projection directly (mirroring get_network_stats' loadStats).
  // When provided it is used instead of an HTTP fetch.
  loadFleetStatus?: (() => Promise<ArtanisGlmFleetStatus>) | undefined
}>

// Coerce an unknown value into a non-negative integer count, or null when it is
// not a usable count. Never invents a number from a missing/garbage field.
const coerceFleetCount = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? Math.trunc(value)
    : null

// Normalize an unknown readiness payload into the public-safe fleet status, or
// null when the required status/ready/total fields cannot be resolved. It is
// tolerant of both the live route shape (`{ status, counts: { readyReplicaCount,
// totalReplicaCount, warmReplicaCount } }`) and a flatter
// `{ status, readyReplicas, totalReplicas }` shape, preferring the structured
// `counts` block when present. Honest absence over invention: a payload missing
// status or both replica counts returns null (the tool then reads as a soft
// "(could not fetch ...)").
export const normalizeArtanisGlmFleetStatus = (
  body: unknown,
): ArtanisGlmFleetStatus | null => {
  if (typeof body !== 'object' || body === null) return null
  const record = body as Record<string, unknown>
  const counts =
    typeof record.counts === 'object' && record.counts !== null
      ? (record.counts as Record<string, unknown>)
      : {}

  const status =
    typeof record.status === 'string' && record.status.trim() !== ''
      ? record.status.trim()
      : null
  const readyReplicas =
    coerceFleetCount(counts.readyReplicaCount) ??
    coerceFleetCount(record.readyReplicas) ??
    coerceFleetCount(record.ready_replicas)
  const totalReplicas =
    coerceFleetCount(counts.totalReplicaCount) ??
    coerceFleetCount(record.totalReplicas) ??
    coerceFleetCount(record.total_replicas)
  if (status === null || readyReplicas === null || totalReplicas === null) {
    return null
  }
  const warmReplicas =
    coerceFleetCount(counts.warmReplicaCount) ??
    coerceFleetCount(record.warmReplicas) ??
    coerceFleetCount(record.warm_replicas)

  // Defensive public-safety gate: the status is the only free-text field, and
  // the route only ever emits a bounded enum, but redact anything unexpected so
  // an upstream regression cannot leak private material into Artanis's context.
  return {
    readyReplicas,
    status: dispatchFieldIsSafe(status) ? status : '(redacted)',
    totalReplicas,
    warmReplicas,
  }
}

// Format the public-safe fleet status as one concise line naming the overall
// status and the ready/total replica counts (plus warm when reported).
const formatGlmFleetStatusLine = (status: ArtanisGlmFleetStatus): string => {
  const warm =
    status.warmReplicas === null ? '' : ` (${status.warmReplicas} warm)`
  return `GLM inference fleet: status=${status.status}, ${status.readyReplicas}/${status.totalReplicas} replicas ready${warm}.`
}

// get_glm_fleet_status() - reads the LIVE GLM inference-fleet readiness. Returns
// a concise public-safe summary naming the overall status and ready/total (and
// warm) replica counts. Honest absence: an unreachable source, a non-OK
// response, or an unexpected payload shape reads as "(could not fetch GLM fleet
// status ...)" and never fabricates replica numbers. Side-effect-free, read-only,
// no spend; takes no arguments.
export const makeArtanisGetGlmFleetStatusTool = (
  config: ArtanisGlmFleetStatusConfig = {},
): ArtanisOperatorReadTool => {
  const url = config.url ?? ARTANIS_GLM_FLEET_STATUS_URL
  const fetchImpl = config.fetchImpl ?? globalThis.fetch
  const loadFleetStatus = config.loadFleetStatus

  return {
    definition: {
      description:
        'Read the LIVE GLM inference-fleet readiness: the overall fleet status (ready/degraded/unavailable) and how many serving replicas are ready/warm out of the total configured. Use this to GATE your decisions on healthy capacity BEFORE you plan a synthetic-load burn or fan out parallel Codex/coding work - do not pile load onto a saturated or cold fleet and degrade real users. Read-only, side-effect-free, public-safe (aggregate counts + status only; no host origins or credentials). Takes no arguments; an unreachable source reads as "(could not fetch GLM fleet status ...)".',
      name: 'get_glm_fleet_status',
      parameters: {
        additionalProperties: false,
        properties: {},
        type: 'object',
      },
    },
    execute: (_args: unknown) =>
      Effect.gen(function* () {
        // In-worker override first (the Worker cannot reliably HTTP-fetch its own
        // public zone). A loader rejection degrades to an honest soft failure.
        if (loadFleetStatus !== undefined) {
          const exit = yield* Effect.exit(
            Effect.tryPromise(() => loadFleetStatus()),
          )
          if (exit._tag === 'Failure') {
            return '(could not fetch GLM fleet status)'
          }
          return formatGlmFleetStatusLine(exit.value)
        }

        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, { headers: { 'User-Agent': 'artanis-operator' } }),
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (response === undefined) {
          return `(could not fetch GLM fleet status from ${url})`
        }
        if (!response.ok) {
          return `(could not fetch GLM fleet status from ${url}: status ${response.status})`
        }

        const body = yield* Effect.tryPromise(
          () => response.json() as Promise<unknown>,
        ).pipe(Effect.orElseSucceed(() => undefined))

        const normalized = normalizeArtanisGlmFleetStatus(body)
        if (normalized === null) {
          return `(could not fetch GLM fleet status from ${url}: unexpected response shape)`
        }
        return formatGlmFleetStatusLine(normalized)
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// get_trace_review — read the LIVE Khala trace-review report (iteration-11).
// ---------------------------------------------------------------------------
//
// Artanis named this his ONE highest-value next tool in iteration 11 of his
// self-improvement loop: an on-demand READ of the existing live trace-review
// report (`GET /api/operator/khala/trace-review`, #6356), which already
// aggregates ATIF trace refs, exact token rows, the model mix, and the
// failure/outcome buckets over a recent window. Reading it IN-LOOP lets him spot
// recurring failure modes and unmet user intents, triage them into the
// unsupported-request ledger (#6357) via `update_unsupported_request`, and plan
// targeted Codex burndown (`dispatch_codex_task`) to fix the gaps that block
// adoption — directly serving the 10x-daily-Khala-token goal.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It reads the SAME public-safe report the
//     admin-gated operator route serves (built by `buildKhalaTraceReviewReport`
//     over `agent_traces` / `token_usage_events` / `pylon_codex_raw_events`).
//     It never returns raw trajectories, raw SDK payloads, prompts, or private
//     refs; it only surfaces aggregate counts + bounded buckets.
//   - IN-WORKER. The Worker cannot reliably HTTP-fetch its OWN admin-gated zone,
//     so production wires an in-worker `loadReport` seam that builds the report
//     directly (mirroring get_network_stats / get_glm_fleet_status). The HTTP
//     path against the public route is a test/override fallback only.
//   - BOUNDED + FAIL-SOFT + HONEST. Each section surfaces at most `maxBuckets`
//     rows. An unreachable source (or an unexpected payload shape) degrades to an
//     honest "(could not fetch trace review ...)" string and NEVER fabricates
//     numbers. An empty section reads "(none)" rather than inventing buckets.
//   - PUBLIC-SAFE. The report is public-safe by construction on the route; the
//     tool applies a defensive second pass so an upstream regression cannot leak
//     private material into a free-text bucket field surfaced to Artanis.

// The default live source for the trace-review read (the admin-gated operator
// route). Used only on the HTTP fallback path (no `loadReport` override).
export const ARTANIS_TRACE_REVIEW_URL =
  'https://openagents.com/api/operator/khala/trace-review'

// Max buckets surfaced per section (model mix / outcomes / failure modes) so a
// single read stays bounded and cheap even when the report is large.
export const ARTANIS_TRACE_REVIEW_MAX_BUCKETS = 8

// A public-safe one-row projection of ONE model-mix bucket.
export type ArtanisTraceReviewModelBucket = Readonly<{
  provider: string
  model: string
  count: number
  totalTokens: number
}>

// A public-safe one-row projection of ONE outcome bucket.
export type ArtanisTraceReviewOutcomeBucket = Readonly<{
  outcome: string
  count: number
  totalTokens: number
}>

// A public-safe one-row projection of ONE failure-mode bucket.
export type ArtanisTraceReviewFailureBucket = Readonly<{
  label: string
  count: number
  severity: string
  failureRef: string
}>

// The bounded, public-safe normalized projection of the trace-review report the
// tool surfaces into Artanis's context. Aggregate counts + bounded buckets only.
export type ArtanisTraceReviewSummary = Readonly<{
  windowHours: number | null
  since: string | null
  until: string | null
  tokenEventCount: number
  totalTokens: number
  traceCount: number
  backendIncidentCount: number
  backendIncidentCriticalCount: number
  rawEventRowCount: number
  modelMix: ReadonlyArray<ArtanisTraceReviewModelBucket>
  outcomes: ReadonlyArray<ArtanisTraceReviewOutcomeBucket>
  failureModes: ReadonlyArray<ArtanisTraceReviewFailureBucket>
}>

export type ArtanisTraceReviewConfig = Readonly<{
  // The trace-review URL to fetch (default the operator route). HTTP path only.
  url?: string | undefined
  // Injected for testability; defaults to the global fetch.
  fetchImpl?: typeof fetch | undefined
  // In-worker override that resolves the report directly (the Worker cannot
  // reliably HTTP-fetch its OWN admin-gated zone). When provided it is used
  // instead of an HTTP fetch. It returns the raw report object the route serves;
  // the tool normalizes it defensively.
  loadReport?: (() => Promise<unknown>) | undefined
  // Max buckets surfaced per section (default ARTANIS_TRACE_REVIEW_MAX_BUCKETS).
  maxBuckets?: number | undefined
}>

// Coerce an unknown value into a non-negative integer, defaulting to 0 (so a
// missing aggregate reads as 0, never NaN/invented). Trace-review aggregates are
// counts/totals and are never negative.
const traceReviewCount = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? Math.trunc(value)
    : 0

// Pull a non-empty trimmed string field, or a fallback. Defensively redact a
// value that carries non-public-safe material so an upstream regression cannot
// leak secrets into a bucket field surfaced to Artanis.
const traceReviewSafeField = (value: unknown, fallback: string): string => {
  const text = typeof value === 'string' ? value.trim() : ''
  if (text === '') return fallback
  return dispatchFieldIsSafe(text) ? text : '(redacted)'
}

// Pull a bounded model/provider IDENTIFIER field. These are controlled routing
// identifiers from `token_usage_events.provider/model` (the route already serves
// them public-safe) and legitimately contain `sk-`-shaped substrings (e.g.
// `hydralisk-vllm-glm-5p2-reap-504b`), so the blunt secret-redaction heuristic
// would false-positive them to "(redacted)" and hide the real top model. We only
// trim, collapse whitespace, and length-bound them; no secret-pattern redaction.
const traceReviewIdentifier = (value: unknown, fallback: string): string => {
  const text =
    typeof value === 'string' ? value.replace(/\s+/g, ' ').trim() : ''
  if (text === '') return fallback
  return text.length > 120 ? `${text.slice(0, 120)}...` : text
}

const traceReviewRecord = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : {}

// Normalize an unknown trace-review payload into the bounded, public-safe
// summary, or null when the body is not an object at all. It is tolerant of the
// live route shape (`{ window, aggregates: { tokens, traces, rawCodexEvents,
// backendIncidents }, modelMix, outcomes, failureModes }`): a missing section
// degrades to empty / 0 (honest absence over invention), never a throw.
export const normalizeArtanisTraceReview = (
  body: unknown,
  maxBuckets: number = ARTANIS_TRACE_REVIEW_MAX_BUCKETS,
): ArtanisTraceReviewSummary | null => {
  if (typeof body !== 'object' || body === null) return null
  const record = body as Record<string, unknown>

  const window = traceReviewRecord(record.window)
  const aggregates = traceReviewRecord(record.aggregates)
  const tokens = traceReviewRecord(aggregates.tokens)
  const traces = traceReviewRecord(aggregates.traces)
  const rawEvents = traceReviewRecord(aggregates.rawCodexEvents)
  const backendIncidents = traceReviewRecord(aggregates.backendIncidents)

  const windowHours =
    typeof window.hours === 'number' && Number.isFinite(window.hours)
      ? Math.trunc(window.hours)
      : null
  const since = typeof window.since === 'string' ? window.since : null
  const until = typeof window.until === 'string' ? window.until : null

  const modelMix = (Array.isArray(record.modelMix) ? record.modelMix : [])
    .slice(0, maxBuckets)
    .map(raw => {
      const row = traceReviewRecord(raw)
      return {
        count: traceReviewCount(row.count),
        model: traceReviewIdentifier(row.model, 'unknown'),
        provider: traceReviewIdentifier(row.provider, 'unknown'),
        totalTokens: traceReviewCount(row.totalTokens),
      }
    })

  const outcomes = (Array.isArray(record.outcomes) ? record.outcomes : [])
    .slice(0, maxBuckets)
    .map(raw => {
      const row = traceReviewRecord(raw)
      return {
        count: traceReviewCount(row.count),
        outcome: traceReviewSafeField(row.outcome, 'unknown'),
        totalTokens: traceReviewCount(row.totalTokens),
      }
    })

  const failureModes = (
    Array.isArray(record.failureModes) ? record.failureModes : []
  )
    .slice(0, maxBuckets)
    .map(raw => {
      const row = traceReviewRecord(raw)
      return {
        count: traceReviewCount(row.count),
        failureRef: traceReviewSafeField(row.failureRef, 'unknown'),
        label: traceReviewSafeField(row.label, 'unknown'),
        severity: traceReviewSafeField(row.severity, 'info'),
      }
    })

  return {
    backendIncidentCount: traceReviewCount(backendIncidents.rowCount),
    backendIncidentCriticalCount: traceReviewCount(
      backendIncidents.criticalCount,
    ),
    failureModes,
    modelMix,
    outcomes,
    rawEventRowCount: traceReviewCount(rawEvents.rowCount),
    since,
    tokenEventCount: traceReviewCount(tokens.eventCount),
    totalTokens: traceReviewCount(tokens.totalTokens),
    traceCount: traceReviewCount(traces.traceCount),
    until,
    windowHours,
  }
}

// Format the public-safe trace-review summary as a bounded multi-line report
// covering the window, the aggregate counts, the model mix, the outcome buckets,
// and the failure buckets. Empty sections read "(none)".
const formatTraceReviewSummary = (
  summary: ArtanisTraceReviewSummary,
): string => {
  const windowLine =
    summary.windowHours === null
      ? 'Khala trace review (window: unknown):'
      : `Khala trace review (last ${summary.windowHours}h${
          summary.since !== null && summary.until !== null
            ? `, ${summary.since} -> ${summary.until}`
            : ''
        }):`

  const aggregatesLine = `Aggregates: ${summary.tokenEventCount.toLocaleString(
    'en-US',
  )} token rows, ${summary.totalTokens.toLocaleString(
    'en-US',
  )} tokens; ${summary.traceCount.toLocaleString(
    'en-US',
  )} traces; ${summary.rawEventRowCount.toLocaleString(
    'en-US',
  )} raw Codex event rows; ${summary.backendIncidentCount.toLocaleString(
    'en-US',
  )} backend incident rows (${summary.backendIncidentCriticalCount.toLocaleString(
    'en-US',
  )} critical).`

  const modelMixBlock =
    summary.modelMix.length === 0
      ? 'Model mix: (none)'
      : [
          `Model mix (${summary.modelMix.length}):`,
          ...summary.modelMix.map(
            bucket =>
              `- ${bucket.provider}/${bucket.model}: ${bucket.count.toLocaleString(
                'en-US',
              )} calls, ${bucket.totalTokens.toLocaleString('en-US')} tokens`,
          ),
        ].join('\n')

  const outcomesBlock =
    summary.outcomes.length === 0
      ? 'Outcomes: (none)'
      : [
          `Outcomes (${summary.outcomes.length}):`,
          ...summary.outcomes.map(
            bucket =>
              `- ${bucket.outcome}: ${bucket.count.toLocaleString(
                'en-US',
              )} (${bucket.totalTokens.toLocaleString('en-US')} tokens)`,
          ),
        ].join('\n')

  const failuresBlock =
    summary.failureModes.length === 0
      ? 'Failure modes: (none)'
      : [
          `Failure modes (${summary.failureModes.length}):`,
          ...summary.failureModes.map(
            bucket =>
              `- [${bucket.severity}] ${bucket.label}: ${bucket.count.toLocaleString(
                'en-US',
              )} (${bucket.failureRef})`,
          ),
        ].join('\n')

  return [
    windowLine,
    aggregatesLine,
    modelMixBlock,
    outcomesBlock,
    failuresBlock,
  ].join('\n')
}

// get_trace_review() — reads the LIVE Khala trace-review report. Returns a
// bounded, public-safe summary naming the window, the aggregate counts, the
// model mix, and the outcome/failure buckets. Honest absence: an unreachable
// source, a non-OK response, or an unexpected payload shape reads as
// "(could not fetch trace review ...)" and never fabricates numbers; an empty
// section reads "(none)". Side-effect-free, read-only, owner-scoped, no spend;
// takes no arguments.
export const makeArtanisGetTraceReviewTool = (
  config: ArtanisTraceReviewConfig = {},
): ArtanisOperatorReadTool => {
  const url = config.url ?? ARTANIS_TRACE_REVIEW_URL
  const fetchImpl = config.fetchImpl ?? globalThis.fetch
  const loadReport = config.loadReport
  const maxBuckets = config.maxBuckets ?? ARTANIS_TRACE_REVIEW_MAX_BUCKETS

  return {
    definition: {
      description:
        'Read the LIVE Khala TRACE-REVIEW report: the recurring review over recent agent traces, exact token-usage rows, and Pylon/Codex raw-event metadata. Returns the review window, aggregate counts (token rows, total tokens, traces, raw Codex event rows), the MODEL MIX (provider/model calls + tokens), the OUTCOME buckets (finish reasons + tokens), and the FAILURE-MODE buckets (recurring problems with severity). Use this in-loop to spot recurring failure modes and unmet user intents, triage them into the unsupported-request ledger (update_unsupported_request), and plan targeted Codex burndown (dispatch_codex_task) at the gaps that block adoption - driving the 10x-daily-token goal. Read-only, side-effect-free, owner-scoped, public-safe (aggregate counts + bounded buckets only; no raw trajectories, prompts, or private refs). Takes no arguments; an empty section reads "(none)" and an unreachable source reads as "(could not fetch trace review ...)".',
      name: 'get_trace_review',
      parameters: {
        additionalProperties: false,
        properties: {},
        type: 'object',
      },
    },
    execute: (_args: unknown) =>
      Effect.gen(function* () {
        // In-worker override first (the Worker cannot reliably HTTP-fetch its own
        // admin-gated zone). A loader rejection degrades to an honest soft
        // failure, never a throw or fabricated numbers.
        if (loadReport !== undefined) {
          const exit = yield* Effect.exit(Effect.tryPromise(() => loadReport()))
          if (exit._tag === 'Failure') {
            return '(could not fetch trace review)'
          }
          const normalized = normalizeArtanisTraceReview(exit.value, maxBuckets)
          if (normalized === null) {
            return '(could not fetch trace review: unexpected report shape)'
          }
          return formatTraceReviewSummary(normalized)
        }

        const response = yield* Effect.tryPromise(() =>
          fetchImpl(url, { headers: { 'User-Agent': 'artanis-operator' } }),
        ).pipe(Effect.orElseSucceed(() => undefined))

        if (response === undefined) {
          return `(could not fetch trace review from ${url})`
        }
        if (!response.ok) {
          return `(could not fetch trace review from ${url}: status ${response.status})`
        }

        const body = yield* Effect.tryPromise(
          () => response.json() as Promise<unknown>,
        ).pipe(Effect.orElseSucceed(() => undefined))

        const normalized = normalizeArtanisTraceReview(body, maxBuckets)
        if (normalized === null) {
          return `(could not fetch trace review from ${url}: unexpected response shape)`
        }
        return formatTraceReviewSummary(normalized)
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// trigger_synthetic_load (RISKY: plan-only) — iteration-4 self-improvement.
// ---------------------------------------------------------------------------
//
// Artanis named this his ONE highest-value next tool in iteration 4 of his
// self-improvement loop: when the live token-pace block shows the fleet is
// BEHIND the daily Khala-token target, he can programmatically scale up
// background SYNTHETIC LOAD (Terminal-Bench agent stress runs or GLM serving
// stress load) to saturate idle capacity and burn tokens toward the 10x-daily
// mission, instead of waiting for a manual owner action.
//
// Authority discipline: this is a RISKY tool modeled exactly like the original
// plan-only `dispatch_codex_task` shape — it is STRUCTURALLY plan-only. It has
// no execute()/run() seam at all; the bounded loop only ever calls `plan(args)`,
// which returns a public-safe description of the synthetic-load run it WOULD
// trigger. The loop frames that as "REQUIRES OWNER APPROVAL — NOT EXECUTED" and
// sets `deferredToApprovalGate`. No new spend authority enters the loop: live
// triggering stays owner-gated via `artanis-approval-gates`. The run is
// own-capacity background work and grants no payout. Inputs are bounded and
// public-safe; non-public-safe free-text is redacted.

// The approval-gate risky-action kind this plan-only tool would require. A
// synthetic-load run is a benchmark/eval workload (Terminal-Bench / GLM stress),
// so it maps to the enumerated `eval_launch` risky kind — keeping the tool
// inside the existing approval-gate vocabulary without minting a new authority
// kind. (The capability is colloquially "synthetic_load_dispatch"; the gated
// authority it would consume is `eval_launch`.)
export const ARTANIS_SYNTHETIC_LOAD_RISKY_ACTION_KIND: ArtanisRiskyActionKind =
  'eval_launch'

// The bounded set of synthetic-load run types Artanis may plan. Anything else is
// rejected as an unknown run type (never silently coerced).
export const ARTANIS_SYNTHETIC_LOAD_RUN_TYPES = [
  'terminal-bench',
  'glm-stress',
] as const
export type ArtanisSyntheticLoadRunType =
  (typeof ARTANIS_SYNTHETIC_LOAD_RUN_TYPES)[number]

// Public-safe human labels for each run type, used in the plan description.
const SYNTHETIC_LOAD_RUN_LABELS: Record<ArtanisSyntheticLoadRunType, string> = {
  'glm-stress': 'GLM serving stress load',
  'terminal-bench': 'Terminal-Bench agent stress runs',
}

// The bounded token-budget window for a single synthetic-load plan. A request
// below the floor is too small to matter for the daily target; one above the
// ceiling is rejected so a model typo cannot plan an unboundedly large burn.
export const ARTANIS_SYNTHETIC_LOAD_MIN_TARGET_TOKENS = 1_000_000
export const ARTANIS_SYNTHETIC_LOAD_MAX_TARGET_TOKENS = 10_000_000_000

const isSyntheticLoadRunType = (
  value: string,
): value is ArtanisSyntheticLoadRunType =>
  (ARTANIS_SYNTHETIC_LOAD_RUN_TYPES as ReadonlyArray<string>).includes(value)

// Coerce a model-produced target-tokens argument into a finite positive integer,
// or `null` when it is not a usable number. Accepts a number or a bare numeric
// string; rejects floats, NaN, Infinity, and non-numeric strings.
const coerceTargetTokens = (value: unknown): number | null => {
  if (typeof value === 'number') {
    return Number.isInteger(value) ? value : null
  }
  if (typeof value === 'string') {
    const trimmed = value.trim()
    if (!/^-?\d+$/.test(trimmed)) return null
    const parsed = Number.parseInt(trimmed, 10)
    return Number.isInteger(parsed) ? parsed : null
  }
  return null
}

// Build the public-safe synthetic-load PLAN text from model-produced args, or an
// honest typed message ("(invalid arguments: …)" for an absent required field,
// "(blocked: …)" for a present-but-rejected value). The plan is plan-only: it
// describes the run Artanis WOULD trigger and never starts any load itself.
export const buildArtanisSyntheticLoadPlan = (
  args: unknown,
  bounds: Readonly<{ minTargetTokens: number; maxTargetTokens: number }>,
): string => {
  const record =
    typeof args === 'object' && args !== null
      ? (args as Record<string, unknown>)
      : {}

  // Run type (required; must be one of the bounded set).
  const typeRaw = record.type ?? record.runType ?? record.run_type
  if (typeRaw === undefined || typeRaw === null) {
    return `(invalid arguments: a "type" is required, one of ${ARTANIS_SYNTHETIC_LOAD_RUN_TYPES.join(
      ', ',
    )})`
  }
  if (typeof typeRaw !== 'string' || !isSyntheticLoadRunType(typeRaw.trim())) {
    return `(blocked: "${String(typeRaw)}" is not a known synthetic-load run type; use one of ${ARTANIS_SYNTHETIC_LOAD_RUN_TYPES.join(
      ', ',
    )})`
  }
  const runType = typeRaw.trim() as ArtanisSyntheticLoadRunType

  // Target tokens (required; bounded positive integer).
  const targetRaw =
    record.targetTokens ?? record.target_tokens ?? record.target
  if (targetRaw === undefined || targetRaw === null) {
    return '(invalid arguments: a numeric "targetTokens" is required)'
  }
  const targetTokens = coerceTargetTokens(targetRaw)
  if (targetTokens === null) {
    return `(blocked: "${String(
      targetRaw,
    )}" is not a valid token target; pass a positive integer)`
  }
  if (
    targetTokens < bounds.minTargetTokens ||
    targetTokens > bounds.maxTargetTokens
  ) {
    return `(blocked: targetTokens ${targetTokens.toLocaleString(
      'en-US',
    )} is out of range; allowed ${bounds.minTargetTokens.toLocaleString(
      'en-US',
    )}..${bounds.maxTargetTokens.toLocaleString('en-US')})`
  }

  // Optional free-text note. Public-safe by gate; redacted if it carries unsafe
  // material so a model-supplied note can never leak secrets into the plan.
  const noteRaw = asString(record.note ?? record.reason ?? record.label)
  const note =
    noteRaw === undefined
      ? '(none)'
      : dispatchFieldIsSafe(noteRaw)
        ? noteRaw
        : '(redacted)'

  const lines: Array<string> = [
    'Planned synthetic-load dispatch (own-capacity background work; NO spend, no payout). Requires owner approval before it runs:',
    '',
    `- type=${runType}`,
    `- lever=${SYNTHETIC_LOAD_RUN_LABELS[runType]}`,
    `- target=${targetTokens.toLocaleString('en-US')} tokens`,
    `- note: ${note}`,
    '',
    'Effect once approved: scale up background synthetic load to saturate idle Khala capacity and burn tokens toward the daily 10x served-token goal. This grants NO new spend authority; live triggering stays owner-gated via artanis-approval-gates (eval_launch).',
  ]
  return lines.join('\n')
}

// trigger_synthetic_load — a RISKY (plan-only) tool. It NEVER executes in the
// loop: the bounded loop only calls `plan(args)`, which returns the exact
// public-safe synthetic-load run it WOULD trigger. The loop frames that as
// pending owner approval and sets `deferredToApprovalGate`. No spend authority
// is granted here; live triggering stays gated behind `artanis-approval-gates`.
export const makeArtanisTriggerSyntheticLoadTool = (
  config: Readonly<{
    minTargetTokens?: number | undefined
    maxTargetTokens?: number | undefined
  }> = {},
): ArtanisOperatorRiskyTool => {
  const minTargetTokens =
    config.minTargetTokens ?? ARTANIS_SYNTHETIC_LOAD_MIN_TARGET_TOKENS
  const maxTargetTokens =
    config.maxTargetTokens ?? ARTANIS_SYNTHETIC_LOAD_MAX_TARGET_TOKENS

  return {
    definition: {
      description: `Plan a SYNTHETIC-LOAD run to saturate idle Khala capacity and burn tokens toward the daily 10x served-token goal when you are BEHIND pace. Choose a run "type" (${ARTANIS_SYNTHETIC_LOAD_RUN_TYPES.join(
        ' or ',
      )}) and a "targetTokens" budget (${minTargetTokens.toLocaleString(
        'en-US',
      )}..${maxTargetTokens.toLocaleString(
        'en-US',
      )}). This is a RISKY (eval_launch) action: it NEVER runs by itself — it returns the exact public-safe run it WOULD trigger, pending owner approval. Own-capacity background work only; NO spend, no payout. Inputs must be public-safe.`,
      name: 'trigger_synthetic_load',
      parameters: {
        additionalProperties: false,
        properties: {
          note: {
            description:
              'Optional short PUBLIC-SAFE note on why the load is needed (e.g. "behind 4x floor at midday"). No secrets.',
            type: 'string',
          },
          targetTokens: {
            description: `Token budget to burn with this run, a positive integer in [${minTargetTokens}, ${maxTargetTokens}], e.g. 500000000.`,
            type: 'number',
          },
          type: {
            description: `The synthetic-load run type: ${ARTANIS_SYNTHETIC_LOAD_RUN_TYPES.join(
              ' or ',
            )}.`,
            enum: [...ARTANIS_SYNTHETIC_LOAD_RUN_TYPES],
            type: 'string',
          },
        },
        required: ['type', 'targetTokens'],
        type: 'object',
      },
    },
    kind: 'risky',
    plan: (args: unknown) =>
      Effect.succeed(
        buildArtanisSyntheticLoadPlan(args, {
          maxTargetTokens,
          minTargetTokens,
        }),
      ),
    riskyActionKind: ARTANIS_SYNTHETIC_LOAD_RISKY_ACTION_KIND,
  }
}

// ---------------------------------------------------------------------------
// get_synthetic_load_status — read ACTIVE synthetic-load runs (iteration 12).
// ---------------------------------------------------------------------------
//
// Artanis named this his ONE highest-value next tool in iteration 12 of his
// self-improvement loop. He already has a PLAN-ONLY `trigger_synthetic_load`
// tool (iteration 4) that, behind owner approval, scales up background
// synthetic load to burn idle Khala capacity toward the daily 10x served-token
// goal. What he lacked was the READ counterpart: an on-demand look at which
// synthetic-load runs are ACTIVE right now and how far each has burned toward
// its token target. Without it the loop has a "duplicate-key placeholder" — he
// can plan a burn but cannot see the burn he (or the owner) already started, so
// he risks planning a redundant run on top of one already in flight.
//
// This is the read half of that pair, and it stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It only reports the status of runs an
//     injected owner-scoped reader returns. It never starts, scales, pauses, or
//     cancels a run, and it touches no GLM serving / admission / model-router
//     path.
//   - PUBLIC-SAFE. Only a bounded run ref, run type, state, and aggregate
//     token-burn progress are surfaced. No host origins, credentials, prompts,
//     completions, prices, payout targets, or wallet material ever enter the
//     summary; each free-text field is run through the same public-safety gate
//     the dispatch tool uses and redacted to "(redacted)" if it ever regresses.
//   - HONEST ABSENCE over invention. With no reader wired (the default today —
//     synthetic-load runs are plan-only/owner-gated and there is no live run
//     registry yet) or a reader that returns no runs, it reports an honest
//     "(no active synthetic-load runs)" rather than fabricating a run. A reader
//     rejection degrades to an honest "(could not read synthetic-load status)".

// The default cap on how many active runs are summarized in one read, so a
// surprising upstream cannot flood Artanis's bounded context.
export const ARTANIS_SYNTHETIC_LOAD_STATUS_MAX_RUNS = 20

// A public-safe view of a single synthetic-load run. Aggregate progress only —
// never host-level detail, prompts, or spend material.
export type ArtanisSyntheticLoadRun = Readonly<{
  // A bounded, public-safe run ref, e.g.
  // "synthetic_load.terminal_bench.2026_06_27_01".
  runRef: string
  // The synthetic-load run type, e.g. "terminal-bench" | "glm-stress".
  runType: string
  // The run state, e.g. "running" | "queued" | "completed" | "failed".
  state: string
  // Tokens burned so far by this run (own-capacity background work; no spend).
  tokensBurned: number
  // The run's token-burn target, or null when not reported.
  targetTokens: number | null
}>

// The injected, owner-scoped reader seam. Resolves the ACTIVE synthetic-load
// runs (own-capacity background work). A thrown rejection is treated by the tool
// as a soft read failure, never a fabricated status. With no reader wired the
// tool reports honest absence.
export type ArtanisSyntheticLoadStatusReader = () => Promise<
  ReadonlyArray<ArtanisSyntheticLoadRun>
>

export type ArtanisSyntheticLoadStatusConfig = Readonly<{
  reader?: ArtanisSyntheticLoadStatusReader | undefined
  maxRuns?: number | undefined
}>

// Coerce an unknown value into a non-negative integer token count, or null when
// it is not a usable count. Never invents a number from a missing/garbage field.
const coerceSyntheticLoadTokens = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? Math.trunc(value)
    : null

// Public-safe-gate a free-text run field: trim it, and redact to "(redacted)"
// if it ever carries non-public-safe material (it should not — these are
// bounded refs/enums — but an upstream regression must never leak private
// material into Artanis's context).
const safeSyntheticLoadField = (value: unknown): string => {
  const trimmed = typeof value === 'string' ? value.trim() : ''
  if (trimmed === '') return '(unknown)'
  return dispatchFieldIsSafe(trimmed) ? trimmed : '(redacted)'
}

// Normalize an unknown run into the public-safe shape. Tolerant of camelCase and
// snake_case token fields; honest absence for a missing target.
const normalizeSyntheticLoadRun = (
  run: unknown,
): ArtanisSyntheticLoadRun | null => {
  if (typeof run !== 'object' || run === null) return null
  const record = run as Record<string, unknown>
  const tokensBurned =
    coerceSyntheticLoadTokens(record.tokensBurned) ??
    coerceSyntheticLoadTokens(record.tokens_burned) ??
    0
  const targetTokens =
    coerceSyntheticLoadTokens(record.targetTokens) ??
    coerceSyntheticLoadTokens(record.target_tokens)
  return {
    runRef: safeSyntheticLoadField(record.runRef ?? record.run_ref),
    runType: safeSyntheticLoadField(record.runType ?? record.run_type),
    state: safeSyntheticLoadField(record.state),
    targetTokens,
    tokensBurned,
  }
}

// Format one run as a public-safe line naming the run ref, type, state, and
// token-burn progress (with a percent when a target is reported).
const formatSyntheticLoadRunLine = (run: ArtanisSyntheticLoadRun): string => {
  const burned = run.tokensBurned.toLocaleString('en-US')
  const progress =
    run.targetTokens === null
      ? `${burned} tokens burned`
      : `${burned}/${run.targetTokens.toLocaleString('en-US')} tokens burned (${
          run.targetTokens > 0
            ? Math.min(
                100,
                Math.round((run.tokensBurned / run.targetTokens) * 100),
              )
            : 0
        }%)`
  return `${run.runRef} [${run.runType}] state=${run.state}, ${progress}.`
}

// get_synthetic_load_status() — reads the ACTIVE synthetic-load runs and returns
// a concise public-safe summary naming each run's ref, type, state, and
// token-burn progress. Honest absence: no reader / no runs reads as "(no active
// synthetic-load runs)"; a reader rejection reads as "(could not read
// synthetic-load status)". Side-effect-free, read-only, no spend; takes no args.
export const makeArtanisGetSyntheticLoadStatusTool = (
  config: ArtanisSyntheticLoadStatusConfig = {},
): ArtanisOperatorReadTool => {
  const reader = config.reader
  const maxRuns = config.maxRuns ?? ARTANIS_SYNTHETIC_LOAD_STATUS_MAX_RUNS

  return {
    definition: {
      description:
        'Read the ACTIVE synthetic-load runs (own-capacity background work that burns idle Khala capacity toward the daily served-token goal). For each active run it returns a public-safe summary: the run ref, run type, state, and token-burn progress. Use this BEFORE you plan a new synthetic-load burn so you do not stack a redundant run on top of one already in flight. Read-only, side-effect-free, public-safe (run refs + states + aggregate token progress only; no host origins, prompts, prices, or spend material). Takes no arguments; no active runs reads as "(no active synthetic-load runs)".',
      name: 'get_synthetic_load_status',
      parameters: {
        additionalProperties: false,
        properties: {},
        type: 'object',
      },
    },
    execute: (_args: unknown) =>
      Effect.gen(function* () {
        if (reader === undefined) {
          return '(no active synthetic-load runs)'
        }
        const exit = yield* Effect.exit(Effect.tryPromise(() => reader()))
        if (exit._tag === 'Failure') {
          return '(could not read synthetic-load status)'
        }
        const runs = exit.value
          .map(normalizeSyntheticLoadRun)
          .filter((run): run is ArtanisSyntheticLoadRun => run !== null)
          .slice(0, maxRuns)
        if (runs.length === 0) {
          return '(no active synthetic-load runs)'
        }
        const header = `Synthetic-load runs (${runs.length} active):`
        return [header, ...runs.map(run => `- ${formatSyntheticLoadRunLine(run)}`)].join(
          '\n',
        )
      }),
    kind: 'read',
  }
}

// ---------------------------------------------------------------------------
// The default owner-scoped operator tool table.
// ---------------------------------------------------------------------------

// Build the standard Artanis operator tool set: the two public repo-read tools
// (#6365), the public issue-read tool (read_github_issue), the owner-scoped
// Pylon job-status read tool (get_pylon_job_status), plus the gated Codex
// dispatch tool (#6366). The route wires this into `artanisOperatorTurn`,
// optionally passing a `dispatchExecution` seam so the gated dispatch can
// execute behind an effective owner approval; with no seam the dispatch tool
// stays plan-only. `issueRead` defaults to the same fetch/owner/repo as
// `repoRead` so a single test/override seam covers all the GitHub read tools;
// pass it explicitly only when the issue API needs a different fetch stub than
// the raw-content fetch. The same `issueRead` fetch/owner/repo also backs the
// iteration-10 `list_github_issues` read tool (a bounded, filterable LIST of
// public issues for burndown triage; pull requests filtered out, count bounded).
// `pylonJobStatus.reader` is the owner-scoped status
// reader; with no reader wired the status tool returns an honest
// "(could not read status …)" rather than inventing a status.
// `pylonAssignments.lister` is the owner-scoped bulk assignments lister behind
// the iteration-5 `list_pylon_assignments` read tool; with no lister wired it
// returns an honest "(could not list assignments …)" rather than inventing one.
// `khalaFeedback.reader` is the owner-scoped Khala CLI feedback reader behind the
// iteration-6 `get_khala_feedback` read tool; with no reader wired it returns an
// honest "(could not read feedback …)" rather than inventing one.
// `glmFleetStatus.loadFleetStatus` is the in-worker GLM fleet readiness loader
// behind the iteration-7 `get_glm_fleet_status` read tool (the Worker cannot
// reliably HTTP-fetch its own public zone); with no override wired it falls back
// to an HTTP fetch of the public-safe readiness route, and an unreachable source
// reads as an honest "(could not fetch GLM fleet status …)" rather than
// inventing replica numbers.
// `syntheticLoadStatus.reader` is the owner-scoped ACTIVE synthetic-load run
// reader behind the iteration-12 `get_synthetic_load_status` read tool (the read
// half of the plan-only `trigger_synthetic_load` pair); with no reader wired it
// reports an honest "(no active synthetic-load runs)" rather than inventing one.
// `traceReview.loadReport` is the in-worker trace-review report loader behind the
// iteration-11 `get_trace_review` read tool (the live
// GET /api/operator/khala/trace-review report, #6356; the Worker cannot reliably
// HTTP-fetch its own admin-gated zone); with no override wired it falls back to an
// HTTP fetch of the operator route, and an unreachable source reads as an honest
// "(could not fetch trace review …)" rather than inventing buckets.
// `unsupportedRequests.reader` is the owner-scoped unsupported-request ledger
// reader behind the iteration-8 `get_unsupported_requests` read tool (the live
// `khala_unsupported_requests` ledger of user-facing capability gaps, #6357);
// with no reader wired it returns an honest "(could not read unsupported
// requests …)" rather than inventing one.
// `unsupportedRequestWriter` is the owner-scoped ledger WRITER behind the
// iteration-9 `update_unsupported_request` write tool (it triages an entry
// through its lifecycle, sets the triage kind, and links a GitHub issue in the
// same `khala_unsupported_requests` ledger #6357); with no writer wired the tool
// is honest ("(could not update …: no ledger writer is wired)") rather than
// inventive. It is a WRITE tool, not risky/gated: owner-scoped, internal-ledger-
// only, with no spend/payout/deploy/delete/outward authority.
export const makeArtanisOperatorTools = (
  config: Readonly<{
    repoRead?: ArtanisRepoReadConfig | undefined
    issueRead?: ArtanisIssueReadConfig | undefined
    pylonJobStatus?: ArtanisPylonJobStatusConfig | undefined
    pylonAssignments?: ArtanisPylonAssignmentsConfig | undefined
    khalaFeedback?: ArtanisKhalaFeedbackConfig | undefined
    traceReview?: ArtanisTraceReviewConfig | undefined
    fleetStatus?: ArtanisFleetStatusConfig | undefined
    unsupportedRequests?: ArtanisUnsupportedRequestsConfig | undefined
    unsupportedRequestWriter?: ArtanisUnsupportedRequestWriter | undefined
    unsupportedRequestIssueOpen?:
      | Omit<
          ArtanisOpenUnsupportedRequestIssueConfig,
          'reader' | 'writer'
        >
      | undefined
    defaultBranch?: string | undefined
    networkStats?: ArtanisGetNetworkStatsConfig | undefined
    glmFleetStatus?: ArtanisGlmFleetStatusConfig | undefined
    syntheticLoadStatus?: ArtanisSyntheticLoadStatusConfig | undefined
    dispatchExecution?: ArtanisDispatchExecution | undefined
    syntheticLoad?:
      | Readonly<{
          minTargetTokens?: number | undefined
          maxTargetTokens?: number | undefined
        }>
      | undefined
  }> = {},
): ReadonlyArray<ArtanisOperatorTool> => {
  const issueRead: ArtanisIssueReadConfig = config.issueRead ?? {
    fetchImpl: config.repoRead?.fetchImpl,
    owner: config.repoRead?.owner,
    repo: config.repoRead?.repo,
  }
  return [
    makeArtanisReadRepoFileTool(config.repoRead),
    makeArtanisListRepoDirTool(config.repoRead),
    makeArtanisReadGithubIssueTool(issueRead),
    makeArtanisListGithubIssuesTool({
      fetchImpl: issueRead.fetchImpl,
      owner: issueRead.owner,
      repo: issueRead.repo,
    }),
    makeArtanisGetNetworkStatsTool(config.networkStats),
    makeArtanisGetFleetStatusTool({
      glmFleetStatus: config.fleetStatus?.glmFleetStatus ?? config.glmFleetStatus,
      networkStats: config.fleetStatus?.networkStats ?? config.networkStats,
      pylonAssignments:
        config.fleetStatus?.pylonAssignments ?? config.pylonAssignments,
      syntheticLoadStatus:
        config.fleetStatus?.syntheticLoadStatus ??
        config.syntheticLoadStatus,
      traceReview: config.fleetStatus?.traceReview ?? config.traceReview,
    }),
    makeArtanisGetGlmFleetStatusTool(config.glmFleetStatus),
    makeArtanisGetSyntheticLoadStatusTool(config.syntheticLoadStatus),
    makeArtanisGetPylonJobStatusTool(config.pylonJobStatus),
    makeArtanisListPylonAssignmentsTool(config.pylonAssignments),
    makeArtanisGetKhalaFeedbackTool(config.khalaFeedback),
    makeArtanisGetTraceReviewTool(config.traceReview),
    makeArtanisGetUnsupportedRequestsTool(config.unsupportedRequests),
    makeArtanisUpdateUnsupportedRequestTool({
      writer: config.unsupportedRequestWriter,
    }),
    makeArtanisOpenUnsupportedRequestIssueTool({
      ...config.unsupportedRequestIssueOpen,
      reader: config.unsupportedRequests?.reader,
      writer: config.unsupportedRequestWriter,
    }),
    makeArtanisDispatchCodexTaskTool({
      defaultBranch: config.defaultBranch,
      execution: config.dispatchExecution,
    }),
    makeArtanisTriggerSyntheticLoadTool(config.syntheticLoad),
  ]
}
