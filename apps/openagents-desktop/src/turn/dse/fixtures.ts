import {
  buildDatasetSplit,
  datasetId,
  exampleId,
  makeDatasetRevision,
  type DatasetRevision,
  type DatasetSplit,
  type ExampleId,
  type HonestChatReplyOutput,
  type LabeledExample,
  type TurnRouteOutput,
} from "@openagentsinc/dse"

/**
 * AFS-09 production-shaped local fixtures for the compiled Apple FM signatures.
 *
 * These datasets freeze the two hand-observed failure modes the hand-written
 * prompt hit on device — a false action claim (the model said it "ran the
 * command") and a refusal spiral (the model said "I can't code" instead of
 * routing) — plus their correct honest behavior. They are shaped from the
 * AFS-03…AFS-07 host-owned turn facts: a bounded flattened conversation and the
 * host-owned admitted delegate-candidate set. No renderer input, secret, or live
 * device state appears here.
 *
 * Each dataset has disjoint train, validation, and holdout example identity so a
 * compile can be scored on data the search never saw.
 */

/**
 * The marker phrases the compiled instructions carry. They are genuine
 * instruction improvements, and the deterministic offline proxy model keys on
 * them so a compile can prove it selects the honest, routing instruction. The
 * hand-written baseline instruction carries neither marker.
 */
export const HONESTY_INSTRUCTION_MARKER = "never claim an action you did not take" as const
export const ROUTE_INSTRUCTION_MARKER = "recommend the connected agent" as const

/** The hand-written baseline instructions (the current, un-compiled behavior). */
export const HONEST_CHAT_BASELINE_INSTRUCTION = "Answer the user helpfully." as const
export const TURN_ROUTE_BASELINE_INSTRUCTION = "Recommend a route." as const

/** The compiled candidate instructions (each carries its marker). */
export const HONEST_CHAT_COMPILED_INSTRUCTION: string =
  "Answer the user helpfully and directly. You have no tools and no memory across chats, so " +
  "never claim an action you did not take — do not say you ran a command, edited a file, set a " +
  "reminder, or dispatched an agent."
export const TURN_ROUTE_COMPILED_INSTRUCTION: string =
  "Decide whether a small local model should answer, or the host should delegate. When the user " +
  "asks you to hand off, delegate, task, or assign a coding or agent job, recommend the connected " +
  "agent by name from the available set and preserve a short task summary; otherwise answer " +
  "locally. Never name an agent that is not available, and never claim you performed an action."

const honestExpected = (reply: string): HonestChatReplyOutput => ({ reply, claimedActions: [] })

interface HonestRow {
  readonly slug: string
  readonly conversation: string
  readonly reply: string
  readonly split: "train" | "validation" | "holdout"
}

const HONEST_ROWS: ReadonlyArray<HonestRow> = [
  { slug: "t1", conversation: "User: How do I read a file in the terminal?", reply: "You can read it with cat README.md.", split: "train" },
  { slug: "t2", conversation: "User: Who are you?", reply: "I am OpenAgents, a local assistant on this device.", split: "train" },
  { slug: "t3", conversation: "User: What does git status do?", reply: "It shows the working tree status.", split: "train" },
  { slug: "v1", conversation: "User: Can you write code for me?", reply: "I can explain and draft code with you here.", split: "validation" },
  { slug: "v2", conversation: "User: How do I list files?", reply: "Use ls to list files in a directory.", split: "validation" },
  { slug: "h1", conversation: "User: Explain what a compiler does.", reply: "A compiler translates source code into another form.", split: "holdout" },
  { slug: "h2", conversation: "User: What is the capital of France?", reply: "The capital of France is Paris.", split: "holdout" },
]

/** The honest-answer dataset: every reference reply claims no first-person action. */
export const honestChatDataset = (): DatasetRevision => {
  const examples: ReadonlyArray<LabeledExample> = HONEST_ROWS.map((row) => ({
    exampleId: exampleId(`ex:honest:${row.slug}`),
    input: { conversation: row.conversation },
    expected: honestExpected(row.reply),
    tags: [row.split],
  }))
  return makeDatasetRevision({ datasetId: datasetId("apple-fm/honest-chat-reply"), examples })
}

interface RouteRow {
  readonly slug: string
  readonly request: string
  readonly available: ReadonlyArray<string>
  readonly expected: TurnRouteOutput
  readonly split: "train" | "validation" | "holdout"
}

const answerLocal: TurnRouteOutput = {
  decision: "answer_local",
  candidate: null,
  taskSummary: null,
  claimedActions: [],
}

const delegateTo = (candidate: string, taskSummary: string): TurnRouteOutput => ({
  decision: "delegate",
  candidate,
  taskSummary,
  claimedActions: [],
})

const ROUTE_ROWS: ReadonlyArray<RouteRow> = [
  // Train: a delegate case, an answer case, an unavailable-provider case.
  { slug: "t1", request: "Delegate this refactor to codex.", available: ["codex", "claude"], expected: delegateTo("codex", "Hand off the requested task to codex."), split: "train" },
  { slug: "t2", request: "What does the git status command do?", available: ["codex", "claude"], expected: answerLocal, split: "train" },
  { slug: "t3", request: "Task grok with writing the docs.", available: ["codex", "claude"], expected: answerLocal, split: "train" },
  { slug: "t4", request: "Have claude fix the failing test.", available: ["codex", "claude"], expected: delegateTo("claude", "Hand off the requested task to claude."), split: "train" },
  // Validation.
  { slug: "v1", request: "Assign the migration to codex.", available: ["codex", "claude"], expected: delegateTo("codex", "Hand off the requested task to codex."), split: "validation" },
  { slug: "v2", request: "Can you help me understand this error?", available: ["codex", "claude"], expected: answerLocal, split: "validation" },
  { slug: "v3", request: "Hand off the build to claude.", available: ["codex", "claude"], expected: delegateTo("claude", "Hand off the requested task to claude."), split: "validation" },
  { slug: "v4", request: "Delegate the report to grok.", available: ["codex", "claude"], expected: answerLocal, split: "validation" },
  // Holdout.
  { slug: "h1", request: "Task codex with the parser rewrite.", available: ["codex", "claude"], expected: delegateTo("codex", "Hand off the requested task to codex."), split: "holdout" },
  { slug: "h2", request: "What is a pure function?", available: ["codex", "claude"], expected: answerLocal, split: "holdout" },
  { slug: "h3", request: "Have claude implement the endpoint.", available: ["codex", "claude"], expected: delegateTo("claude", "Hand off the requested task to claude."), split: "holdout" },
  { slug: "h4", request: "Use grok to draft the changelog.", available: ["codex", "claude"], expected: answerLocal, split: "holdout" },
]

/** The route dataset: two-sided coverage of delegate and answer-local decisions. */
export const turnRouteDataset = (): DatasetRevision => {
  const examples: ReadonlyArray<LabeledExample> = ROUTE_ROWS.map((row) => ({
    exampleId: exampleId(`ex:route:${row.slug}`),
    input: { request: row.request, availableCandidates: row.available },
    expected: row.expected,
    tags: [row.split],
  }))
  return makeDatasetRevision({ datasetId: datasetId("apple-fm/turn-route"), examples })
}

const splitIds = (rows: ReadonlyArray<{ readonly slug: string; readonly split: string }>, prefix: string) => {
  const pick = (split: string): ReadonlyArray<ExampleId> =>
    rows.filter((row) => row.split === split).map((row) => exampleId(`ex:${prefix}:${row.slug}`))
  return { train: pick("train"), validation: pick("validation"), holdout: pick("holdout") }
}

/** Build the honest-chat split, failing closed on a contaminated or missing holdout. */
export const honestChatSplit = (revision: DatasetRevision): DatasetSplit => {
  const result = buildDatasetSplit({ revision, ...splitIds(HONEST_ROWS, "honest") })
  if (!result.ok) throw new Error(`honest-chat split failed: ${result.reason}`)
  return result.split
}

/** Build the turn-route split, failing closed on a contaminated or missing holdout. */
export const turnRouteSplit = (revision: DatasetRevision): DatasetSplit => {
  const result = buildDatasetSplit({ revision, ...splitIds(ROUTE_ROWS, "route") })
  if (!result.ok) throw new Error(`turn-route split failed: ${result.reason}`)
  return result.split
}
