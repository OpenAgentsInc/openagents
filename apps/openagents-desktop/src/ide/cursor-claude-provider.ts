import { sha256 } from "@noble/hashes/sha256"
import { Effect, Schema, Stream } from "effect"

import { IdeAgentProposalSchema } from "./agent-code-contract.ts"
import {
  IdeCursorCandidateRefSchema,
  IdeCursorCandidateSchema,
  IdeCursorCapabilitiesSchema,
  IdeCursorDisclosureSchema,
  IdeCursorStreamEventSchema,
  type IdeCursorCandidate,
  type IdeCursorDisclosure,
  type IdeCursorProviderInput,
  type IdeCursorStreamEvent,
} from "./cursor-contract.ts"
import {
  IdeCursorProviderFailure,
  type IdeCursorProviderShape,
} from "./cursor-provider.ts"
import { IdeProposalRefSchema, IdeTextRangeSchema, IdeTimestampSchema } from "./project-contract.ts"

const boundedText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(100_000))
const detail = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000))
const nonNegative = Schema.Number.check(Schema.isFinite(), Schema.isGreaterThanOrEqualTo(0))

export const IdeCursorClaudeOutputSchema = Schema.TaggedUnion({
  Completion: {
    replace: IdeTextRangeSchema,
    text: Schema.String.check(Schema.isMaxLength(100_000)),
    confidence: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(1)),
  },
  NextEdit: {
    targetPathRef: IdeCursorCandidateSchema.cases.NextEdit.fields.targetPathRef,
    replace: IdeTextRangeSchema,
    text: Schema.String.check(Schema.isMaxLength(100_000)),
    explanation: detail,
    confidence: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(1)),
  },
  Answer: {
    markdown: boundedText,
    confidence: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(1)),
  },
  Proposal: {
    proposalRef: IdeProposalRefSchema,
    proposal: IdeAgentProposalSchema,
    confidence: Schema.Number.check(Schema.isGreaterThanOrEqualTo(0), Schema.isLessThanOrEqualTo(1)),
  },
}).annotate({ identifier: "IdeCursorClaudeOutput" })
export type IdeCursorClaudeOutput = typeof IdeCursorClaudeOutputSchema.Type

const ClaudeUsageSchema = Schema.Struct({
  input_tokens: nonNegative,
  output_tokens: nonNegative,
  cache_creation_input_tokens: nonNegative,
  cache_read_input_tokens: nonNegative,
})

const ClaudeModelUsageSchema = Schema.Struct({
  inputTokens: nonNegative,
  outputTokens: nonNegative,
  cacheReadInputTokens: nonNegative,
  cacheCreationInputTokens: nonNegative,
  webSearchRequests: nonNegative,
  costUSD: nonNegative,
  contextWindow: nonNegative,
  maxOutputTokens: nonNegative,
})

const ClaudeResultSuccessSchema = Schema.Struct({
  type: Schema.Literal("result"),
  subtype: Schema.Literal("success"),
  is_error: Schema.Literal(false),
  num_turns: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum: 1 })),
  total_cost_usd: nonNegative,
  usage: ClaudeUsageSchema,
  modelUsage: Schema.Record(Schema.String, ClaudeModelUsageSchema),
  permission_denials: Schema.Array(Schema.Unknown).check(Schema.isMaxLength(0)),
  structured_output: Schema.Unknown,
})

export type IdeCursorClaudeQueryResult = AsyncIterable<unknown> & Readonly<{ close?: () => void }>
export interface IdeCursorClaudeQueryOptions {
  readonly cwd: string
  readonly model: string
  readonly abortController: AbortController
  readonly tools: string[]
  readonly allowedTools: string[]
  readonly skills: string[]
  readonly agents: Record<string, never>
  readonly mcpServers: Record<string, never>
  readonly strictMcpConfig: true
  readonly plugins: never[]
  readonly settingSources: never[]
  readonly maxTurns: 1
  readonly persistSession: false
  readonly enableFileCheckpointing: false
  readonly additionalDirectories: string[]
  readonly permissionMode: "dontAsk"
  readonly outputFormat: Readonly<{ type: "json_schema"; schema: Record<string, unknown> }>
}
export type IdeCursorClaudeQuery = (input: Readonly<{
  prompt: string
  options: IdeCursorClaudeQueryOptions
}>) => IdeCursorClaudeQueryResult

export interface IdeCursorClaudeProviderOptions {
  readonly query?: IdeCursorClaudeQuery
  readonly isolatedCwd?: string
  readonly providerRef?: string
  readonly modelRefs?: ReadonlyArray<string>
  readonly harnessRef?: string
  readonly accountRef?: string
  readonly now?: () => string
}

const DEFAULT_PROVIDER_REF = "provider.anthropic.claude-agent-sdk"
const DEFAULT_MODEL_REFS = ["claude-sonnet-4-6"] as const
const DEFAULT_HARNESS_REF = "harness.claude-agent-sdk"

const hexDigest = (value: string): `sha256:${string}` =>
  `sha256:${Array.from(sha256(new TextEncoder().encode(value)), byte =>
    byte.toString(16).padStart(2, "0")).join("")}`

const providerFailure = (
  operation: string,
  reason: IdeCursorProviderFailure["reason"],
  value: string,
): IdeCursorProviderFailure => new IdeCursorProviderFailure({
  operation,
  reason,
  detail: value.slice(0, 2_000),
})

const exactKeys = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false
  const actual = Object.keys(value).sort()
  const expected = [...allowed].sort()
  return actual.length === expected.length && actual.every((key, index) => key === expected[index])
}

const strictOutputKeys = (value: unknown): boolean => {
  if (value === null || typeof value !== "object" || Array.isArray(value) || !("_tag" in value)) return false
  switch ((value as { readonly _tag?: unknown })._tag) {
    case "Completion": return exactKeys(value, ["_tag", "replace", "text", "confidence"])
    case "NextEdit": return exactKeys(value, ["_tag", "targetPathRef", "replace", "text", "explanation", "confidence"])
    case "Answer": return exactKeys(value, ["_tag", "markdown", "confidence"])
    case "Proposal": return exactKeys(value, ["_tag", "proposalRef", "proposal", "confidence"])
    default: return false
  }
}

const toolUseIn = (value: unknown): boolean => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false
  const record = value as Readonly<Record<string, unknown>>
  if (record.type === "tool_use") return true
  if (Array.isArray(record.content) && record.content.some(toolUseIn)) return true
  return record.message !== undefined && toolUseIn(record.message)
}

const secretMaterial = (value: string): boolean =>
  /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----|(?:^|[^A-Za-z0-9])(?:github_pat|gh[pousr]_|sk-|AKIA)[A-Za-z0-9_-]{8,}|authorization\s*:\s*bearer\s+\S+|(?:password|secret|token)\s*[:=]\s*[^\s]+/iu.test(value)

const promptFor = (input: IdeCursorProviderInput): string => JSON.stringify({
  instruction: [
    "Return exactly one structured output value for the admitted intent.",
    "Use only the supplied document and context. Do not request or use tools, files, commands, skills, plugins, or MCP.",
    "Do not change immutable identity or anchor fields. A Proposal must use the supplied IDE-08 proposal contract.",
  ],
  intent: input.request.intent,
  anchor: input.request.anchor,
  proposalContext: input.proposalContext,
  documentText: input.documentText,
  context: input.context,
})

const eligible = (intent: IdeCursorProviderInput["request"]["intent"], output: IdeCursorClaudeOutput): boolean => {
  switch (intent._tag) {
    case "Complete": return output._tag === "Completion"
    case "NextEdit": return output._tag === "NextEdit"
    case "Ask": return output._tag === "Answer"
    case "Edit":
    case "Generate": return output._tag === "Proposal"
  }
}

const offsetsFor = (
  content: string,
  range: typeof IdeTextRangeSchema.Type,
): Readonly<{ start: number; end: number }> | null => {
  const starts = [0]
  for (let index = 0; index < content.length; index += 1) {
    if (content.charCodeAt(index) === 10) starts.push(index + 1)
  }
  const offset = (position: typeof range.start): number | null => {
    const start = starts[position.line - 1]
    if (start === undefined) return null
    const next = starts[position.line]
    let end = next === undefined ? content.length : next - 1
    if (end > start && content.charCodeAt(end - 1) === 13) end -= 1
    const candidate = start + position.column - 1
    return candidate <= end ? candidate : null
  }
  const start = offset(range.start)
  const end = offset(range.end)
  return start === null || end === null || end < start ? null : { start, end }
}

const candidateResultDigest = (
  input: IdeCursorProviderInput,
  output: IdeCursorClaudeOutput,
): Effect.Effect<`sha256:${string}`, IdeCursorProviderFailure> => {
  switch (output._tag) {
    case "Completion":
    case "NextEdit": {
      if (output._tag === "NextEdit" && output.targetPathRef !== input.request.anchor.pathRef) {
        return Effect.succeed(hexDigest(JSON.stringify(output)))
      }
      const offsets = offsetsFor(input.documentText, output.replace)
      if (offsets === null) return Effect.fail(providerFailure(
        "IdeCursorClaudeProvider.resultDigest",
        "invalid_event",
        "Claude returned a replacement range outside the admitted document.",
      ))
      return Effect.succeed(hexDigest(
        `${input.documentText.slice(0, offsets.start)}${output.text}${input.documentText.slice(offsets.end)}`,
      ))
    }
    case "Answer": return Effect.succeed(hexDigest(output.markdown))
    case "Proposal": return Effect.succeed(hexDigest(JSON.stringify(output.proposal)))
  }
}

const disclosureFor = (
  promptBytes: number,
  result: typeof ClaudeResultSuccessSchema.Type,
): IdeCursorDisclosure => IdeCursorDisclosureSchema.make({
  dataDestinations: [{
    destination: "Anthropic Claude API through Claude Agent SDK",
    purpose: "Generate the admitted IDE cursor candidate",
    bytes: { _tag: "Measured", value: promptBytes, unit: "bytes" },
    retention: "provider_policy",
  }],
  usage: {
    input: {
      _tag: "Measured",
      value: result.usage.input_tokens + result.usage.cache_creation_input_tokens + result.usage.cache_read_input_tokens,
      unit: "tokens",
    },
    output: { _tag: "Measured", value: result.usage.output_tokens, unit: "tokens" },
    cost: { _tag: "Measured", value: Math.round(result.total_cost_usd * 1_000_000), unit: "usd_micros" },
  },
  noRemoteIndexDependency: true,
  secretsSent: false,
})

const provenanceFor = (input: IdeCursorProviderInput): IdeCursorCandidate["provenance"] => [{
  sourceRef: `document:${input.request.anchor.documentRef}`,
  source: "document",
  freshness: "current",
}, ...input.context.map(item => ({
  sourceRef: item.contextRef,
  source: item.source === "workspace" ? "context" as const : item.source,
  freshness: item.freshness,
}))]

const candidateFor = Effect.fn("IdeCursorClaudeProvider.candidateFor")(function* (
  input: IdeCursorProviderInput,
  output: IdeCursorClaudeOutput,
  disclosure: IdeCursorDisclosure,
  now: () => string,
) {
  const candidateRef = IdeCursorCandidateRefSchema.make(
    `ide.cursor-candidate.claude.${hexDigest(`${input.request.requestRef}:${input.request.attemptRef}`).slice(7, 39)}`,
  )
  const common = {
    schemaVersion: "openagents.ide-cursor.v1" as const,
    candidateRef,
    requestRef: input.request.requestRef,
    attemptRef: input.request.attemptRef,
    sequence: input.request.sequence,
    anchor: input.request.anchor,
    identity: input.request.identity,
    disclosure,
    provenance: provenanceFor(input),
    quality: { confidence: output.confidence, syntaxChecked: false, diagnosticsChecked: false },
    staleness: { _tag: "Fresh" as const },
    createdAt: IdeTimestampSchema.make(now()),
    resultDigest: yield* candidateResultDigest(input, output),
  }
  switch (output._tag) {
    case "Completion": return IdeCursorCandidateSchema.cases.Completion.make({ ...common, replace: output.replace, text: output.text })
    case "NextEdit": return IdeCursorCandidateSchema.cases.NextEdit.make({
      ...common,
      targetPathRef: output.targetPathRef,
      replace: output.replace,
      text: output.text,
      explanation: output.explanation,
    })
    case "Answer": return IdeCursorCandidateSchema.cases.Answer.make({ ...common, markdown: output.markdown })
    case "Proposal": {
      if (output.proposalRef !== output.proposal.proposalRef ||
        JSON.stringify(output.proposal.attachment) !== JSON.stringify(input.proposalContext.attachment) ||
        output.proposal.manifestRef !== input.proposalContext.manifestRef ||
        output.proposal.turnRef !== input.proposalContext.turnRef ||
        output.proposal.conversationThreadRef !== input.proposalContext.conversationThreadRef ||
        output.proposal.attachment.projectRef !== input.request.anchor.projectRef ||
        output.proposal.attachment.rootRef !== input.request.anchor.rootRef ||
        output.proposal.attachment.worktreeRef !== input.request.anchor.worktreeRef ||
        output.proposal.attachment.sessionRef !== input.request.anchor.sessionRef ||
        output.proposal.attachment.attachmentGeneration !== input.request.anchor.attachmentGeneration) {
        return yield* Effect.fail(providerFailure(
          "IdeCursorClaudeProvider.candidateFor",
          "invalid_event",
          "Claude returned a proposal outside the admitted IDE-08 attachment identity.",
        ))
      }
      return IdeCursorCandidateSchema.cases.Proposal.make({
        ...common,
        proposalRef: output.proposalRef,
        proposal: output.proposal,
      })
    }
  }
})

const configuredIdentity = (
  input: IdeCursorProviderInput,
  options: Required<Pick<IdeCursorClaudeProviderOptions, "providerRef" | "harnessRef" | "accountRef">> &
    Readonly<{ modelRefs: ReadonlyArray<string> }>,
): boolean => {
  const identity = input.request.identity
  return identity.admitted.provider.value === options.providerRef &&
    identity.effective.provider.value === options.providerRef &&
    options.modelRefs.includes(identity.admitted.model.value) &&
    identity.effective.model.value === identity.admitted.model.value &&
    identity.admitted.harness.value === options.harnessRef &&
    identity.effective.harness.value === options.harnessRef &&
    identity.admitted.account.value === options.accountRef &&
    identity.effective.account.value === options.accountRef &&
    identity.effective.networkPosture === "networked" &&
    identity.effective.indexPosture !== "remote"
}

const outputJsonSchema = Object.fromEntries(
  Object.entries(Schema.toJsonSchemaDocument(IdeCursorClaudeOutputSchema)),
)

export const makeIdeCursorClaudeProvider = (
  options: IdeCursorClaudeProviderOptions = {},
): IdeCursorProviderShape => {
  const providerRef = options.providerRef ?? DEFAULT_PROVIDER_REF
  const modelRefs = options.modelRefs === undefined || options.modelRefs.length === 0
    ? [...DEFAULT_MODEL_REFS]
    : [...options.modelRefs]
  const harnessRef = options.harnessRef ?? DEFAULT_HARNESS_REF
  const capabilities = IdeCursorCapabilitiesSchema.make({
    providerRef,
    modelRefs,
    intents: ["complete", "next_edit", "ask", "change"],
    noFilesystemAccess: true,
    noShellAccess: true,
    identityBeforeCandidate: true,
    supportsCancellation: true,
    supportsOffline: false,
  })

  const generate: IdeCursorProviderShape["generate"] = input => {
    const identityEvent = IdeCursorStreamEventSchema.cases.Identity.make({
      requestRef: input.request.requestRef,
      attemptRef: input.request.attemptRef,
      identity: input.request.identity,
    })
    if (options.query === undefined || options.isolatedCwd === undefined || options.accountRef === undefined) {
      return Stream.fail(providerFailure(
        "IdeCursorClaudeProvider.generate",
        "unavailable",
        "Claude cursor generation requires an injected SDK query, isolated cwd, and admitted account identity.",
      ))
    }
    if (!configuredIdentity(input, { providerRef, modelRefs, harnessRef, accountRef: options.accountRef })) {
      return Stream.fail(providerFailure(
        "IdeCursorClaudeProvider.generate",
        "rejected",
        "The request effective harness, provider, model, account, network, or index posture is not admitted by this provider.",
      ))
    }
    const prompt = promptFor(input)
    const promptBytes = new TextEncoder().encode(prompt).byteLength
    if (secretMaterial(prompt)) {
      return Stream.fail(providerFailure(
        "IdeCursorClaudeProvider.generate",
        "rejected",
        "Secret-shaped material is not admitted to the remote Claude cursor provider.",
      ))
    }
    if (Math.ceil(promptBytes / 4) > input.request.budget.maxInputTokens) {
      return Stream.fail(providerFailure(
        "IdeCursorClaudeProvider.generate",
        "rejected",
        "The bounded Claude cursor prompt exceeds the admitted input-token budget.",
      ))
    }
    const query = options.query
    const isolatedCwd = options.isolatedCwd
    const run = Effect.callback<unknown, IdeCursorProviderFailure>((resume, signal) => {
        const abortController = new AbortController()
      let session: IdeCursorClaudeQueryResult | null = null
      let cleaned = false
      const cleanup = () => {
        if (cleaned) return
        cleaned = true
        abortController.abort()
        session?.close?.()
      }
      signal.addEventListener("abort", cleanup, { once: true })
      const settle = <A>(effect: Effect.Effect<A, IdeCursorProviderFailure>) => {
        signal.removeEventListener("abort", cleanup)
        cleanup()
        resume(effect)
      }
      try {
        session = query({
            prompt,
            options: {
              cwd: isolatedCwd,
              model: input.request.identity.effective.model.value,
              abortController,
              tools: [],
              allowedTools: [],
              skills: [],
              agents: {},
              mcpServers: {},
              strictMcpConfig: true,
              plugins: [],
              settingSources: [],
              maxTurns: 1,
              persistSession: false,
              enableFileCheckpointing: false,
              additionalDirectories: [],
              permissionMode: "dontAsk",
              outputFormat: { type: "json_schema", schema: outputJsonSchema },
            },
          })
      } catch (error) {
        settle(Effect.fail(providerFailure("IdeCursorClaudeProvider.query", "unavailable", String(error))))
        return Effect.void
      }
      void (async () => {
          let terminal: unknown = null
        for await (const message of session) {
            if (toolUseIn(message)) throw providerFailure(
              "IdeCursorClaudeProvider.query",
              "invalid_event",
              "Claude emitted a tool call despite the empty tool capability.",
            )
            if (message !== null && typeof message === "object" &&
              "type" in message && message.type === "result") {
              if (terminal !== null) throw providerFailure(
                "IdeCursorClaudeProvider.query",
                "invalid_event",
                "Claude emitted more than one terminal result.",
              )
              terminal = message
            }
          }
          if (terminal === null) throw providerFailure(
            "IdeCursorClaudeProvider.query",
            "invalid_event",
            "Claude ended without one terminal structured result.",
          )
          return terminal
      })().then(
        terminal => settle(Effect.succeed(terminal)),
        error => settle(Effect.fail(error instanceof IdeCursorProviderFailure
          ? error
          : providerFailure("IdeCursorClaudeProvider.query", "unavailable", String(error)))),
      )
      return Effect.sync(cleanup)
    }).pipe(
      Effect.timeoutOrElse({
        duration: `${input.request.budget.maxLatencyMs} millis`,
        orElse: () => Effect.fail(providerFailure(
          "IdeCursorClaudeProvider.query",
          "unavailable",
          `Claude cursor generation exceeded ${input.request.budget.maxLatencyMs} ms.`,
        )),
      }),
      Effect.flatMap(value => Schema.decodeUnknownEffect(ClaudeResultSuccessSchema)(value).pipe(
        Effect.mapError(error => providerFailure(
          "IdeCursorClaudeProvider.result",
          "invalid_event",
          `Claude returned an invalid or failed terminal result: ${String(error)}`,
        )),
      )),
      Effect.flatMap(result => {
        const usedModels = Object.keys(result.modelUsage)
        if (usedModels.length !== 1 || usedModels[0] !== input.request.identity.effective.model.value) {
          return Effect.fail(providerFailure(
            "IdeCursorClaudeProvider.result",
            "invalid_event",
            "Claude result model usage does not prove the admitted effective model.",
          ))
        }
        if (!strictOutputKeys(result.structured_output)) return Effect.fail(providerFailure(
          "IdeCursorClaudeProvider.result",
          "invalid_event",
          "Claude returned a candidate payload with unknown, missing, or extra top-level fields.",
        ))
        return Schema.decodeUnknownEffect(IdeCursorClaudeOutputSchema)(result.structured_output).pipe(
          Effect.mapError(error => providerFailure(
            "IdeCursorClaudeProvider.result",
            "invalid_event",
            `Claude returned an invalid candidate payload: ${String(error)}`,
          )),
          Effect.flatMap(output => eligible(input.request.intent, output)
            ? Effect.succeed(output)
            : Effect.fail(providerFailure(
                "IdeCursorClaudeProvider.result",
                "invalid_event",
                "Claude candidate kind does not match the admitted request intent.",
              ))),
          Effect.flatMap(output => {
            const disclosure = disclosureFor(promptBytes, result)
            return candidateFor(input, output, disclosure, options.now ?? (() => new Date().toISOString())).pipe(
              Effect.map(candidate => [
                IdeCursorStreamEventSchema.cases.Candidate.make({ candidate }),
                IdeCursorStreamEventSchema.cases.Finished.make({
                  requestRef: input.request.requestRef,
                  attemptRef: input.request.attemptRef,
                  disclosure,
                }),
              ] satisfies ReadonlyArray<IdeCursorStreamEvent>),
            )
          }),
        )
      }),
    )
    return Stream.concat(
      Stream.make(identityEvent),
      Stream.fromIterableEffect(run),
    )
  }

  return { capabilities, generate }
}

export const decodeIdeCursorClaudeOutput = (value: unknown) => {
  if (!strictOutputKeys(value)) return Effect.fail(providerFailure(
    "IdeCursorClaudeProvider.decodeOutput",
    "invalid_event",
    "Claude candidate payload has unknown, missing, or extra top-level fields.",
  ))
  return Schema.decodeUnknownEffect(IdeCursorClaudeOutputSchema)(value).pipe(
    Effect.mapError(error => providerFailure(
      "IdeCursorClaudeProvider.decodeOutput",
      "invalid_event",
      String(error),
    )),
  )
}
