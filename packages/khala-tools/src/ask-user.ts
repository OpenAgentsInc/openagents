import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolNeedsInput,
  khalaToolOk,
  khalaToolUnavailable,
  type KhalaInteractionAnswer,
  type KhalaInteractionAskResult,
  type KhalaInteractionChoice,
  type KhalaInteractionEvent,
  type KhalaPublicSafety,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export const askUserToolDefinition: KhalaToolDefinition = {
  authority: "interaction",
  availability: ["inspect", "coding", "owner_local_full"],
  description: "Ask the local operator for missing information or a preference without granting tool authority.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      allow_freeform: {
        description: "Whether the operator may answer with text outside the provided choices.",
        type: "boolean",
      },
      choices: {
        description: "Optional short answer choices.",
        items: {
          additionalProperties: false,
          properties: {
            description: { type: "string" },
            id: { type: "string" },
            label: { type: "string" },
          },
          required: ["id", "label"],
          type: "object",
        },
        type: "array",
      },
      default_answer: {
        description: "Optional answer to use when the host times out.",
        type: "string",
      },
      non_blocking: {
        description: "Return a pending needs-input result when the host supports non-blocking prompts.",
        type: "boolean",
      },
      prompt: {
        description: "Short question or preference prompt for the local operator.",
        type: "string",
      },
      public_safe: {
        description: "If true, the prompt and answer may appear in public-safe summaries after redaction.",
        type: "boolean",
      },
      timeout_ms: {
        description: "Optional host-side timeout for waiting on an answer.",
        minimum: 1,
        type: "integer",
      },
    },
    required: ["prompt"],
    type: "object",
  },
  internalId: "khala.interaction.ask_user",
  label: "Ask User",
  name: "ask_user",
  outputSchema: {
    additionalProperties: false,
    properties: {
      answerKind: { type: ["string", "null"] },
      requestId: { type: "string" },
      state: { enum: ["answered", "pending", "timed_out", "unavailable"], type: "string" },
    },
    required: ["requestId", "state", "answerKind"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Ask the local operator a short question without requesting authority.",
  promptGuidelines: [
    "Use for missing information, preferences, or clarification.",
    "Do not use this tool to request filesystem, shell, network, credential, or owner authority.",
    "Keep prompts short and mark public_safe only when the prompt and answer are safe to summarize publicly.",
  ],
  renderer: { kind: "user_question", rendererRef: "khala.renderer.user_question.v1" },
}

export function createAskUserTool(): RegisteredKhalaTool {
  return {
    definition: askUserToolDefinition,
    execute: executeAskUserTool,
  }
}

type AskUserInput = Readonly<{
  allowFreeform: boolean
  choices: ReadonlyArray<KhalaInteractionChoice>
  defaultAnswer?: string
  nonBlocking: boolean
  prompt: string
  publicSafe: boolean
  timeoutMs?: number
}>

function executeAskUserTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeAskUserInput(input)
      const result = await Effect.runPromise(
        context.services.interaction.askUser({
          allowFreeform: args.allowFreeform,
          choices: args.choices,
          ...(args.defaultAnswer === undefined ? {} : { defaultAnswer: args.defaultAnswer }),
          invocationId: context.invocation.id,
          khalaSessionId: context.invocation.sessionId,
          nonBlocking: args.nonBlocking,
          prompt: args.prompt,
          publicSafe: args.publicSafe,
          ...(args.timeoutMs === undefined ? {} : { timeoutMs: args.timeoutMs }),
        }),
      )
      return renderAskUserResult(args, result)
    } catch (error) {
      return khalaToolError("ask_user_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

function renderAskUserResult(args: AskUserInput, result: KhalaInteractionAskResult): KhalaToolResult {
  const ui = {
    allowFreeform: args.allowFreeform,
    answer: result.answer ?? null,
    choices: args.choices,
    events: result.events.map(eventToUi),
    kind: "user_question",
    nonBlocking: args.nonBlocking,
    prompt: args.prompt,
    publicSafe: args.publicSafe,
    reason: result.reason ?? null,
    requestId: result.requestId,
    state: result.status,
    ...(args.timeoutMs === undefined ? {} : { timeoutMs: args.timeoutMs }),
  }

  if (result.status === "answered") {
    if (result.answer === undefined) return khalaToolError("ask_user_missing_answer", "Host returned answered without an answer")
    return khalaToolOk({
      modelText: `User input received.\n${answerText(result.answer, args)}`,
      publicSafety: publicSafety(args),
      publicSummary: publicSummary("User input received", args, result.answer),
      ui,
    })
  }

  if (result.status === "pending") {
    return khalaToolNeedsInput({
      modelText: "User input requested and still pending.",
      publicSafety: publicSafety(args),
      publicSummary: publicSummary("User input requested", args),
      ui,
    })
  }

  if (result.status === "timed_out") {
    if (result.answer !== undefined) {
      return khalaToolOk({
        modelText: `User input timed out; using default.\n${answerText(result.answer, args)}`,
        publicSafety: publicSafety(args),
        publicSummary: publicSummary("User input timed out and default answer was used", args, result.answer),
        ui,
      })
    }
    return khalaToolNeedsInput({
      modelText: "User input timed out with no default answer.",
      publicSafety: publicSafety(args),
      publicSummary: publicSummary("User input requested but timed out", args),
      ui,
    })
  }

  return khalaToolUnavailable({
    modelText: `User input unavailable in this host.${result.reason === undefined ? "" : ` Reason: ${result.reason}.`}`,
    publicSafety: publicSafety(args),
    publicSummary: publicSummary("User input requested but unavailable", args),
    ui,
  })
}

function decodeAskUserInput(input: Readonly<Record<string, unknown>>): AskUserInput {
  rejectPermissionShapedRequest(input)
  const prompt = typeof input.prompt === "string" ? input.prompt.trim() : ""
  if (prompt.length === 0) throw new Error("ask_user requires prompt")
  if (prompt.length > 500) throw new Error("ask_user prompt must be 500 characters or fewer")

  const choices = decodeChoices(input.choices)
  const allowFreeform = typeof input.allow_freeform === "boolean" ? input.allow_freeform : choices.length === 0
  if (!allowFreeform && choices.length === 0) {
    throw new Error("ask_user requires choices when allow_freeform is false")
  }

  return {
    allowFreeform,
    choices,
    ...(typeof input.default_answer === "string" ? { defaultAnswer: input.default_answer } : {}),
    nonBlocking: input.non_blocking === true,
    prompt,
    publicSafe: input.public_safe === true,
    ...(input.timeout_ms === undefined ? {} : { timeoutMs: positiveInteger(input.timeout_ms, "timeout_ms") }),
  }
}

function decodeChoices(value: unknown): ReadonlyArray<KhalaInteractionChoice> {
  if (value === undefined) return []
  if (!Array.isArray(value)) throw new Error("ask_user choices must be an array")
  if (value.length > 8) throw new Error("ask_user choices may include at most 8 items")
  const seen = new Set<string>()
  return value.map((choice, index) => {
    if (!isRecord(choice)) throw new Error(`ask_user choice ${index + 1} must be an object`)
    const id = typeof choice.id === "string" ? choice.id.trim() : ""
    const label = typeof choice.label === "string" ? choice.label.trim() : ""
    if (id.length === 0) throw new Error(`ask_user choice ${index + 1} requires id`)
    if (label.length === 0) throw new Error(`ask_user choice ${index + 1} requires label`)
    if (id.length > 80 || label.length > 120) throw new Error(`ask_user choice ${index + 1} is too long`)
    if (seen.has(id)) throw new Error(`ask_user duplicate choice id: ${id}`)
    seen.add(id)
    return {
      ...(typeof choice.description === "string" ? { description: choice.description } : {}),
      id,
      label,
    }
  })
}

function rejectPermissionShapedRequest(input: Readonly<Record<string, unknown>>): void {
  const keyedAuthority = Object.keys(input).find(key => PERMISSION_SHAPED_KEYS.has(normalizeToken(key)))
  if (keyedAuthority !== undefined) {
    throw new Error(`ask_user cannot request authority through ${keyedAuthority}; use the permission flow`)
  }
  const prompt = typeof input.prompt === "string" ? input.prompt : ""
  const choices = Array.isArray(input.choices)
    ? input.choices.map(choice => isRecord(choice) ? `${String(choice.id ?? "")} ${String(choice.label ?? "")}` : "").join(" ")
    : ""
  const text = `${prompt} ${choices}`
  if (PERMISSION_SHAPED_TEXT.test(text)) {
    throw new Error("ask_user cannot be used for permission, approval, or authority requests")
  }
}

function answerText(answer: KhalaInteractionAnswer, args: AskUserInput): string {
  if (answer.kind === "choice") {
    const choice = args.choices.find(candidate => candidate.id === answer.choiceId)
    const label = choice?.label ?? answer.choiceId
    return `Answer choice: ${label}\nAnswer text: ${answer.text}`
  }
  if (answer.kind === "default") return `Default answer: ${answer.text}`
  return `Answer: ${answer.text}`
}

function publicSummary(base: string, args: AskUserInput, answer?: KhalaInteractionAnswer): string {
  if (!args.publicSafe) return `${base}.`
  const answerSummary = answer === undefined ? "" : ` Answer: ${answerPublicText(answer, args)}.`
  return `${base}: ${args.prompt}.${answerSummary}`
}

function answerPublicText(answer: KhalaInteractionAnswer, args: AskUserInput): string {
  if (answer.kind === "choice") {
    return args.choices.find(choice => choice.id === answer.choiceId)?.label ?? answer.text
  }
  return answer.text
}

function publicSafety(args: AskUserInput): KhalaPublicSafety {
  return args.publicSafe ? "public_safe" : "private"
}

function eventToUi(event: KhalaInteractionEvent): unknown {
  return {
    kind: event.kind,
    payload: event.payload,
    timestampMs: event.timestampMs,
  }
}

function positiveInteger(value: unknown, field: string): number {
  if (!Number.isInteger(value) || Number(value) < 1) throw new Error(`ask_user ${field} must be a positive integer`)
  return Number(value)
}

function normalizeToken(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/gu, "")
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

const PERMISSION_SHAPED_KEYS = new Set([
  "approval",
  "approvalrequest",
  "approve",
  "authority",
  "authoritymode",
  "filesystem",
  "grant",
  "network",
  "permission",
  "permissionmode",
  "resources",
  "shell",
  "toolcallid",
  "toolname",
  "workingdirectory",
])

const PERMISSION_SHAPED_TEXT =
  /\b(approval|approve|authorize|grant|permission)\b|\b(allow|deny|always)\b.{0,40}\b(filesystem|file system|shell|command|network|credential|owner|authority|access)\b|\b(run|execute)\b.{0,30}\b(shell|command)\b/iu
