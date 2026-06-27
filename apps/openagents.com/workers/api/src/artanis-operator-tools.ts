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
  ArtanisRiskyActionKind,
} from './artanis-operator'
import {
  type ArtanisGetNetworkStatsConfig,
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
// the raw-content fetch. `pylonJobStatus.reader` is the owner-scoped status
// reader; with no reader wired the status tool returns an honest
// "(could not read status …)" rather than inventing a status.
export const makeArtanisOperatorTools = (
  config: Readonly<{
    repoRead?: ArtanisRepoReadConfig | undefined
    issueRead?: ArtanisIssueReadConfig | undefined
    pylonJobStatus?: ArtanisPylonJobStatusConfig | undefined
    defaultBranch?: string | undefined
    networkStats?: ArtanisGetNetworkStatsConfig | undefined
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
    makeArtanisGetNetworkStatsTool(config.networkStats),
    makeArtanisGetPylonJobStatusTool(config.pylonJobStatus),
    makeArtanisDispatchCodexTaskTool({
      defaultBranch: config.defaultBranch,
      execution: config.dispatchExecution,
    }),
    makeArtanisTriggerSyntheticLoadTool(config.syntheticLoad),
  ]
}
