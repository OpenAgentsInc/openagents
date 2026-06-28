// Artanis operator GROUNDING GATE — the enforcement layer that makes the
// Blueprint Signature-6 `operator-grounded-assertion` (and the Signature-4
// `command-execution-source-verified` sub-check) STRUCTURAL rather than merely
// prompt-level. (Epic: full-Blueprint-set wiring, slice 1.)
//
// WHY THIS EXISTS
// ---------------
// The operator system prompt has a GROUNDED-ASSERTION RULE telling Artanis to
// call the grounding tools (repo_path_exists / repo_grep / route_exists) before
// naming a runnable artifact. But a PROMPT is advisory: a headless model can —
// and historically did — present a fabricated `scripts/distill_traces.ts` or a
// hallucinated `POST /api/admin/khala/mint` endpoint as if it were real, never
// having looked it up. This module turns that rule into a GATE: it audits the
// operator's OWN final reply for runnable-artifact references, correlates each
// against the grounding lookups actually performed THIS turn, runs the shared
// Blueprint gate state machines (imported, not re-described, from
// `@openagentsinc/blueprint-contracts`), and forces any UNGROUNDED reference to
// be labeled SPECULATIVE in the returned reply. A path the model never verified
// can no longer reach the owner as "runnable".
//
// SCOPE / BOUNDARY (honest):
//   - This is a bounded AUDIT predicate over Artanis's own output (like the
//     persona-separation guard) — NOT user-intent routing.
//   - The ENFORCED gate is S6 (operator-grounded-assertion): UNGROUNDED
//     references are tagged SPECULATIVE. S4 (command-execution-source-verified)
//     is applied and ATTACHED as structured evidence for command artifacts using
//     the evidence reachable to a headless operator (source-read + flag content
//     match); the runtime `--help`/dry-run predicate is NOT reachable headless,
//     so S4 cannot reach SAFE_TO_PROPOSE here — that final probe is a documented
//     server-side follow-up. S6 is the blocker; S4 is the report.
//   - Grounding evidence comes from the three canonical S6 tools
//     (repo_path_exists / repo_grep / route_exists). A successful repo READ
//     (read_repo_file / list_repo_dir) is also accepted as path-existence
//     grounding because it is strictly stronger evidence the path exists; this
//     avoids false-flagging a file Artanis actually read.

import {
  COMMAND_SOURCE_VERIFIED_EVIDENCE,
  evaluateCommandSourceVerified,
  evaluateOperatorGroundedAssertion,
  parseCommandFlags,
  type CommandSourceVerifiedResult,
  type OperatorGroundedArtifactKind,
  type OperatorGroundedAssertionState,
  type OperatorGroundedLookupResult,
} from '@openagentsinc/blueprint-contracts'

import { parseJsonUnknown } from './json-boundary'

// ---------------------------------------------------------------------------
// Runnable-artifact extraction (the audit predicate over the operator's reply).
// ---------------------------------------------------------------------------

// File/script extensions we treat as a referenced repo path. A path must also
// contain a "/" so a bare word like "index.ts" in prose is not mis-extracted.
const ARTANIS_PATH_EXTENSIONS = [
  'ts',
  'tsx',
  'js',
  'jsx',
  'mjs',
  'cjs',
  'sh',
  'py',
  'sql',
  'toml',
  'yml',
  'yaml',
  'rs',
  'md',
] as const

// Extensions whose files are runnable scripts (drive the S4 command sub-check).
const ARTANIS_RUNNABLE_SCRIPT_EXTENSIONS: ReadonlyArray<string> = [
  'ts',
  'js',
  'mjs',
  'cjs',
  'sh',
  'py',
  'rs',
]

const PATH_REGEX = new RegExp(
  // The negative lookbehind rejects a token that is itself the tail of a larger
  // path or a "../" traversal (so "../../etc/passwd.sh" is not mined as a ref).
  `(?<![A-Za-z0-9_./-])([A-Za-z0-9_][A-Za-z0-9_./-]*\\/[A-Za-z0-9_./-]*\\.(?:${ARTANIS_PATH_EXTENSIONS.join('|')}))`,
  'g',
)

// METHOD + path, e.g. "POST /api/admin/khala/mint".
const METHOD_ENDPOINT_REGEX =
  /\b(GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)\s+(\/[A-Za-z0-9_./{}:-]+)/g

// Bare API path, e.g. "/api/admin/khala/mint" (method unknown).
const BARE_API_PATH_REGEX = /(\/api\/[A-Za-z0-9_./{}:-]+)/g

// Trim trailing punctuation a model leaves on an inline ref ("...mint." -> path).
const trimArtifactRef = (raw: string): string =>
  raw.replace(/[.,;:)\]\}>'"`]+$/, '').trim()

export type ArtanisRunnableArtifact = Readonly<{
  kind: 'file_path' | 'api_endpoint' | 'command'
  // For file_path/api_endpoint: the path. For command: the path of the script
  // the command runs (commands are anchored to a runnable repo script).
  ref: string
  // For api_endpoint: the HTTP method when present, else null.
  method: string | null
  // For command: the full command-line text and its parsed flags.
  commandString: string | null
  flags: ReadonlyArray<string>
}>

// Pull the line containing `index` so a command artifact can carry its flags.
const lineContaining = (text: string, index: number): string => {
  const start = text.lastIndexOf('\n', index) + 1
  const endNl = text.indexOf('\n', index)
  const end = endNl === -1 ? text.length : endNl
  return text.slice(start, end)
}

const extensionOf = (path: string): string => {
  const dot = path.lastIndexOf('.')
  return dot === -1 ? '' : path.slice(dot + 1).toLowerCase()
}

// Extract the runnable artifacts referenced by the reply. High-precision: only
// "/"-bearing paths with a known extension and absolute API paths are caught. A
// runnable-script path that appears on a line carrying flags is ALSO emitted as
// a `command` artifact so the S4 sub-check can run.
export const extractArtanisRunnableArtifacts = (
  reply: string,
): ReadonlyArray<ArtanisRunnableArtifact> => {
  if (typeof reply !== 'string' || reply.trim() === '') return []
  const out: Array<ArtanisRunnableArtifact> = []
  const seen = new Set<string>()

  const push = (artifact: ArtanisRunnableArtifact): void => {
    const key = `${artifact.kind}:${artifact.method ?? ''}:${artifact.ref}:${artifact.commandString ?? ''}`
    if (seen.has(key)) return
    seen.add(key)
    out.push(artifact)
  }

  // File / script paths.
  for (const match of reply.matchAll(PATH_REGEX)) {
    const ref = trimArtifactRef(match[1] ?? '')
    if (ref === '' || ref.includes('..')) continue
    push({ kind: 'file_path', ref, method: null, commandString: null, flags: [] })

    // If this is a runnable script and its line carries flags, emit a command.
    if (ARTANIS_RUNNABLE_SCRIPT_EXTENSIONS.includes(extensionOf(ref))) {
      const line = lineContaining(reply, match.index ?? 0).trim()
      const flags = parseCommandFlags(line)
      if (flags.length > 0) {
        push({
          kind: 'command',
          ref,
          method: null,
          commandString: line,
          flags,
        })
      }
    }
  }

  // METHOD + endpoint.
  const methodPaths = new Set<string>()
  for (const match of reply.matchAll(METHOD_ENDPOINT_REGEX)) {
    const method = (match[1] ?? '').toUpperCase()
    const ref = trimArtifactRef(match[2] ?? '')
    if (ref === '' || ref.includes('..')) continue
    methodPaths.add(ref)
    push({ kind: 'api_endpoint', ref, method, commandString: null, flags: [] })
  }

  // Bare API paths (only if not already captured with a method).
  for (const match of reply.matchAll(BARE_API_PATH_REGEX)) {
    const ref = trimArtifactRef(match[1] ?? '')
    if (ref === '' || ref.includes('..') || methodPaths.has(ref)) continue
    push({ kind: 'api_endpoint', ref, method: null, commandString: null, flags: [] })
  }

  return out
}

// ---------------------------------------------------------------------------
// Grounding-lookup capture (correlate the operator's tool calls to artifacts).
// ---------------------------------------------------------------------------

// The grounding/read tools whose outputs ground a runnable artifact's existence.
export const ARTANIS_GROUNDING_TOOL_NAMES: ReadonlyArray<string> = [
  'repo_path_exists',
  'repo_grep',
  'route_exists',
  'read_repo_file',
  'list_repo_dir',
]

// A single grounding lookup Artanis actually performed this turn, distilled from
// a grounding/read tool call's arguments + result text.
export type ArtanisGroundingLookup = Readonly<{
  tool: string
  // The repo path or API path that was looked up.
  ref: string
  result: 'positive' | 'negative'
  // For repo_grep positives: flag tokens found in the matched lines (feeds the
  // S4 `declaredFlags` argument surface) and the matched text (S4 source hash).
  matchedFlags: ReadonlyArray<string>
  matchedText: string | null
}>

// Read-tool error prefixes: a read whose result starts with one of these did
// NOT confirm the path. Everything else from a read tool is real file/dir body.
const READ_NEGATIVE_PREFIXES: ReadonlyArray<string> = [
  '(tool error',
  '(file not found',
  '(blocked',
  '(could not',
  '(read failed',
  '(invalid',
  '("',
]

const startsWithAny = (text: string, prefixes: ReadonlyArray<string>): boolean =>
  prefixes.some(prefix => text.startsWith(prefix))

const readPathArgument = (args: unknown): string | undefined => {
  if (typeof args !== 'object' || args === null) return undefined
  const value = (args as Record<string, unknown>).path
  return typeof value === 'string' && value.trim() !== ''
    ? trimArtifactRef(value.trim())
    : undefined
}

const parseArgs = (raw: string): unknown => {
  const trimmed = (raw ?? '').trim()
  if (trimmed === '') return {}
  try {
    return parseJsonUnknown(trimmed)
  } catch {
    return {}
  }
}

// Pull flag tokens out of repo_grep matched-line text (the script's declared
// argument surface, as observed in the real file).
const flagsInText = (text: string): ReadonlyArray<string> => {
  const flags: Array<string> = []
  for (const match of text.matchAll(/(?:^|[\s"'`(])(--?[A-Za-z][A-Za-z0-9-]*)/g)) {
    const flag = match[1] ?? ''
    if (flag.length > 1 && !flags.includes(flag)) flags.push(flag)
  }
  return flags
}

// Distill a grounding lookup from one grounding/read tool result, or null when
// the tool is not a grounding/read tool or carried no usable path argument.
export const extractArtanisGroundingLookup = (input: {
  toolName: string
  rawArguments: string
  content: string
}): ArtanisGroundingLookup | null => {
  const { toolName, content } = input
  if (!ARTANIS_GROUNDING_TOOL_NAMES.includes(toolName)) return null
  const args = parseArgs(input.rawArguments)
  const ref = readPathArgument(args)
  if (ref === undefined) return null

  if (toolName === 'read_repo_file' || toolName === 'list_repo_dir') {
    const negative = startsWithAny(content.trim(), READ_NEGATIVE_PREFIXES)
    return {
      tool: toolName,
      ref,
      result: negative ? 'negative' : 'positive',
      matchedFlags: [],
      matchedText: null,
    }
  }

  // The three canonical S6 grounding tools emit a "GROUNDED:" prefix on a
  // positive lookup and "UNGROUNDED" (within "GROUNDING: …") on a negative one.
  const positive =
    content.startsWith('GROUNDED:') && !content.includes('UNGROUNDED')
  if (toolName === 'repo_grep') {
    return {
      tool: toolName,
      ref,
      result: positive ? 'positive' : 'negative',
      matchedFlags: positive ? flagsInText(content) : [],
      matchedText: positive ? content : null,
    }
  }
  return {
    tool: toolName,
    ref,
    result: positive ? 'positive' : 'negative',
    matchedFlags: [],
    matchedText: null,
  }
}

// True when a concrete request path matches an OpenAPI-style template segment
// (a `{param}` segment matches any one non-empty concrete segment). Mirrors the
// route_exists matcher so a concrete endpoint grounds against a templated lookup.
const pathMatchesTemplate = (requestPath: string, template: string): boolean => {
  if (requestPath === template) return true
  const a = requestPath.split('/')
  const b = template.split('/')
  if (a.length !== b.length) return false
  for (let i = 0; i < b.length; i += 1) {
    const t = b[i] ?? ''
    const r = a[i] ?? ''
    if (t.startsWith('{') && t.endsWith('}')) {
      if (r === '') return false
      continue
    }
    if (t !== r) return false
  }
  return true
}

// Find the lookup that grounds an artifact, or null if it was never looked up.
const findLookupForArtifact = (
  artifact: ArtanisRunnableArtifact,
  lookups: ReadonlyArray<ArtanisGroundingLookup>,
): ArtanisGroundingLookup | null => {
  const candidates =
    artifact.kind === 'api_endpoint'
      ? lookups.filter(
          lookup =>
            lookup.tool === 'route_exists' &&
            pathMatchesTemplate(artifact.ref, lookup.ref),
        )
      : lookups.filter(
          lookup =>
            lookup.tool !== 'route_exists' && lookup.ref === artifact.ref,
        )
  if (candidates.length === 0) return null
  // A positive lookup grounds; otherwise report the (negative) lookup that ran.
  return candidates.find(lookup => lookup.result === 'positive') ?? candidates[0] ?? null
}

// ---------------------------------------------------------------------------
// The gate verdict (structured, attached to the turn).
// ---------------------------------------------------------------------------

export type ArtanisGroundingArtifactVerdict = Readonly<{
  artifactKind: OperatorGroundedArtifactKind
  artifactRef: string
  state: OperatorGroundedAssertionState
  grounded: boolean
  lookupTool: string | null
  lookupResult: OperatorGroundedLookupResult
  satisfiedEvidence: ReadonlyArray<string>
  missingEvidence: ReadonlyArray<string>
  // S4 sub-verdict for `command` artifacts; null for file_path/api_endpoint.
  commandSourceVerified: CommandSourceVerifiedResult | null
}>

export type ArtanisOperatorGroundingGateResult = Readonly<{
  evaluated: ReadonlyArray<ArtanisGroundingArtifactVerdict>
  // True when every referenced artifact reached GROUNDED (or none were named).
  allGrounded: boolean
  // The ungrounded references that were (or would be) tagged SPECULATIVE.
  speculativeArtifacts: ReadonlyArray<
    Readonly<{ artifactKind: OperatorGroundedArtifactKind; artifactRef: string }>
  >
  // True when the reply was augmented with the grounding addendum.
  enforced: boolean
}>

const EMPTY_GATE: ArtanisOperatorGroundingGateResult = {
  evaluated: [],
  allGrounded: true,
  speculativeArtifacts: [],
  enforced: false,
}

const artifactKindFor = (
  artifact: ArtanisRunnableArtifact,
): OperatorGroundedArtifactKind =>
  artifact.kind === 'command'
    ? 'command'
    : artifact.kind === 'api_endpoint'
      ? 'api_endpoint'
      : 'file_path'

// Speculative-marker tokens (lowercased) used to detect that the reply ALREADY
// flagged an artifact, so the gate does not double-tag it.
export const ARTANIS_SPECULATIVE_MARKERS: ReadonlyArray<string> = [
  'speculative',
  'have not verified',
  "haven't verified",
  'not verified',
  'unverified',
  'unconfirmed',
  'did not verify',
  'cannot confirm',
  "can't confirm",
  'does not exist',
  "doesn't exist",
  'may not exist',
  'if it exists',
]

// True when the reply already labels `ref` speculative within its own line.
const replyAlreadyFlags = (reply: string, ref: string): boolean => {
  const lower = reply.toLowerCase()
  const refLower = ref.toLowerCase()
  let from = 0
  for (;;) {
    const at = lower.indexOf(refLower, from)
    if (at === -1) return false
    const line = lineContaining(lower, at)
    if (ARTANIS_SPECULATIVE_MARKERS.some(marker => line.includes(marker))) {
      return true
    }
    from = at + refLower.length
  }
}

// Evaluate the grounding gate over a reply + the lookups performed this turn.
// Pure: produces the structured verdict list WITHOUT mutating the reply.
export const evaluateArtanisGroundingGate = (input: {
  reply: string
  lookups: ReadonlyArray<ArtanisGroundingLookup>
}): ArtanisOperatorGroundingGateResult => {
  const artifacts = extractArtanisRunnableArtifacts(input.reply)
  if (artifacts.length === 0) return EMPTY_GATE

  const evaluated: Array<ArtanisGroundingArtifactVerdict> = []
  const speculative: Array<{
    artifactKind: OperatorGroundedArtifactKind
    artifactRef: string
  }> = []

  for (const artifact of artifacts) {
    const lookup = findLookupForArtifact(artifact, input.lookups)
    const lookupResult: OperatorGroundedLookupResult =
      lookup === null ? 'not_looked_up' : lookup.result === 'positive' ? 'positive' : 'negative'
    const artifactKind = artifactKindFor(artifact)
    const refForGate = artifact.kind === 'command' ? artifact.ref : artifact.ref

    const s6 = evaluateOperatorGroundedAssertion({
      artifactKind,
      artifactRef: refForGate,
      lookupTool: lookup?.tool ?? null,
      lookupResult,
    })

    // S4 sub-check for command artifacts: apply the shared evaluator with the
    // evidence reachable headless (source-read + flag content match). The
    // dry-run runtime probe is NOT reachable here, so S4 cannot reach
    // SAFE_TO_PROPOSE — it is attached as structured evidence, not a blocker.
    let commandSourceVerified: CommandSourceVerifiedResult | null = null
    if (artifact.kind === 'command') {
      const grep = input.lookups.find(
        candidate =>
          candidate.tool === 'repo_grep' &&
          candidate.ref === artifact.ref &&
          candidate.result === 'positive',
      )
      const grounded = lookup !== null && lookup.result === 'positive'
      commandSourceVerified = evaluateCommandSourceVerified({
        commandString: artifact.commandString ?? '',
        scriptPath: artifact.ref,
        expectedFlags: artifact.flags,
        sourceReadHash: grounded ? `grounded:${lookup?.tool}:${artifact.ref}` : null,
        declaredFlags: grep?.matchedFlags ?? [],
        dryRunExitCode: null,
      })
    }

    evaluated.push({
      artifactKind,
      artifactRef: refForGate,
      state: s6.state,
      grounded: s6.canAssert,
      lookupTool: s6.lookupTool,
      lookupResult: s6.lookupResult,
      satisfiedEvidence: s6.satisfiedEvidence,
      missingEvidence: s6.missingEvidence,
      commandSourceVerified,
    })

    if (!s6.canAssert && !replyAlreadyFlags(input.reply, refForGate)) {
      speculative.push({ artifactKind, artifactRef: refForGate })
    }
  }

  return {
    evaluated,
    allGrounded: evaluated.every(verdict => verdict.grounded),
    speculativeArtifacts: speculative,
    enforced: false,
  }
}

// The header line of the enforcement addendum (exported for tests).
export const ARTANIS_GROUNDING_ADDENDUM_HEADER =
  'GROUNDING GATE (Blueprint Signature 6 — operator-grounded-assertion):'

const reasonFor = (artifactKind: OperatorGroundedArtifactKind): string =>
  artifactKind === 'api_endpoint'
    ? 'not confirmed in the OpenAPI route registry this turn'
    : 'no repo grounding lookup confirmed it this turn'

// Enforce the gate: evaluate it, and if any referenced artifact is UNGROUNDED
// and not already labeled, APPEND a SPECULATIVE addendum to the reply so the
// model can never present an unverified path/endpoint as runnable. Returns the
// (possibly augmented) reply and the final gate result (with `enforced` set).
export const enforceArtanisGroundingGate = (input: {
  reply: string
  lookups: ReadonlyArray<ArtanisGroundingLookup>
}): Readonly<{ reply: string; gate: ArtanisOperatorGroundingGateResult }> => {
  const gate = evaluateArtanisGroundingGate(input)
  if (gate.speculativeArtifacts.length === 0) {
    return { reply: input.reply, gate }
  }

  const lines = [
    '',
    '---',
    `${ARTANIS_GROUNDING_ADDENDUM_HEADER} I referenced ${gate.speculativeArtifacts.length} runnable artifact(s) I did not verify exist this turn. Treat these as SPECULATIVE (unverified), not as confirmed runnable:`,
    ...gate.speculativeArtifacts.map(
      artifact =>
        `- [${artifact.artifactKind}] ${artifact.artifactRef} — UNGROUNDED (${reasonFor(artifact.artifactKind)})`,
    ),
  ]
  return {
    reply: `${input.reply}\n${lines.join('\n')}`,
    gate: { ...gate, enforced: true },
  }
}

// Re-export the gate evidence vocabulary so the doc/tests share one source.
export { COMMAND_SOURCE_VERIFIED_EVIDENCE }
