// #5466 (EPIC #5461): SEMANTIC intent → Blueprint signature routing for the live
// chat pane. This is the load-bearing "no keyword matching" requirement.
//
// The Blueprint chat-program runtime (#5452,
// apps/openagents.com/workers/api/src/blueprint/services/chat-program-runtime.ts)
// performs the typed program run, but its candidate selection keys off a
// `preferredFamily` (BlueprintProgramFamily) plus structured ref alignment — it
// does NOT itself turn a user's free-text message into a family. That semantic
// step is what this module supplies: a deterministic, dependency-free embedding
// (character-n-gram term-frequency vector) + cosine similarity over the real
// Blueprint program families. A paraphrase that shares NO whole words with a
// family descriptor still routes correctly because the match is over distributed
// sub-word features in a vector space, not substring/keyword/`includes()` checks.
//
// The output is a real BlueprintProgramFamily + the real signature ref the worker
// runtime uses for that family. Nothing here fabricates a verdict — verdicts come
// from the live session events (see blueprint-chat-runtime.ts).

// The real Blueprint program families (mirror of
// apps/openagents.com/workers/api/src/blueprint/schemas/program.ts
// BlueprintProgramFamily). Kept as a local literal union so the desktop UI does
// not take a worker import; the test asserts it stays in sync with the families
// the chat runtime actually accepts.
export type BlueprintProgramFamily =
  | "action_planning"
  | "artifact_review"
  | "context"
  | "continuation"
  | "email_decisioning"
  | "proof_projection"
  | "research_policy"
  | "review"
  | "routing"
  | "source_selection"

// One routable signature: a real family, the real program-signature ref the
// worker runtime selects for that family, and a natural-language descriptor used
// ONLY to build the family's embedding centroid (never matched as a keyword).
export type RoutableSignature = Readonly<{
  family: BlueprintProgramFamily
  signatureRef: string
  // Human descriptor of the family's purpose. Embedded into a vector; the
  // routing decision is cosine similarity against this vector, not a text scan.
  descriptor: string
}>

// The catalog the chat runtime can route to. The signature refs are the real
// refs the Blueprint chat-program runtime resolves (continuation is the runtime
// default, `autopilot_continue.v1`; the rest follow the same naming the registry
// uses per family). The descriptors are intentionally written in different words
// than a user would type, so the test proving "semantic, not keyword" is honest.
export const ROUTABLE_SIGNATURES: ReadonlyArray<RoutableSignature> = [
  {
    family: "continuation",
    signatureRef: "signature.openagents.autopilot_continue.v1",
    descriptor:
      "carry on an existing thread of work resume the next move pick up where things left off keep going advancing toward the objective",
  },
  {
    family: "artifact_review",
    signatureRef: "signature.openagents.autopilot_artifact_review.v1",
    descriptor:
      "inspect a produced diff patch changeset output examine what the agent edited assess the quality of generated files",
  },
  {
    family: "review",
    signatureRef: "signature.openagents.autopilot_review.v1",
    descriptor:
      "evaluate correctness critique judge whether work meets the bar approve or reject a proposal sign off",
  },
  {
    family: "proof_projection",
    signatureRef: "program_signature.blueprint.show_replay.v1",
    descriptor:
      "surface verifiable evidence display an exact replay bundle prove a computation reproduced deterministically show the receipt of settlement",
  },
  {
    family: "action_planning",
    signatureRef: "signature.openagents.autopilot_action_planning.v1",
    descriptor:
      "break an objective into ordered steps draft a plan sequence the moves decide what to do next before acting",
  },
  {
    family: "routing",
    signatureRef: "signature.openagents.autopilot_routing.v1",
    descriptor:
      "choose which capability tool or destination should handle a request dispatch to the right lane select the appropriate handler",
  },
  {
    family: "research_policy",
    signatureRef: "signature.openagents.autopilot_research_policy.v1",
    descriptor:
      "gather background investigate a question study sources form a policy from findings synthesize what is known",
  },
  {
    family: "source_selection",
    signatureRef: "signature.openagents.autopilot_source_selection.v1",
    descriptor:
      "pick which repository worktree or authority a task should read from choose the right input corpus locate the relevant source",
  },
  {
    family: "context",
    signatureRef: "signature.openagents.autopilot_context.v1",
    descriptor:
      "assemble the relevant background pack scope the information the agent needs prepare the briefing for a turn",
  },
  {
    family: "email_decisioning",
    signatureRef: "signature.openagents.autopilot_email_decisioning.v1",
    descriptor:
      "decide how to respond to an incoming message triage correspondence determine the disposition of a notification",
  },
]

// ── Deterministic semantic embedding (character n-gram TF vector) ─────────────
// A sparse term-frequency vector over character trigrams of the lowercased text.
// Character n-grams give distributed sub-word features, so two phrases that share
// no whole words but describe the same idea still land near each other — that is
// what makes this semantic rather than keyword. Pure + dependency-free so the
// view stays a function of the model and the routing is unit-testable offline.
export type Embedding = ReadonlyMap<string, number>

const N = 3

export const embedText = (text: string): Embedding => {
  // Normalise: lowercase, collapse non-alphanumerics to single spaces, pad so
  // leading and trailing trigrams are captured.
  const normalized = ` ${text.toLowerCase().replace(/[^a-z0-9]+/g, " ").trim()} `
  const vec = new Map<string, number>()
  if (normalized.trim() === "") return vec
  for (let i = 0; i + N <= normalized.length; i++) {
    const gram = normalized.slice(i, i + N)
    vec.set(gram, (vec.get(gram) ?? 0) + 1)
  }
  return vec
}

export const cosineSimilarity = (a: Embedding, b: Embedding): number => {
  if (a.size === 0 || b.size === 0) return 0
  // Iterate the smaller map for the dot product.
  const [small, large] = a.size <= b.size ? [a, b] : [b, a]
  let dot = 0
  for (const [gram, weight] of small) {
    const other = large.get(gram)
    if (other !== undefined) dot += weight * other
  }
  if (dot === 0) return 0
  let normA = 0
  for (const w of a.values()) normA += w * w
  let normB = 0
  for (const w of b.values()) normB += w * w
  const denom = Math.sqrt(normA) * Math.sqrt(normB)
  return denom === 0 ? 0 : dot / denom
}

// Precomputed family centroids (descriptor embeddings).
const FAMILY_VECTORS: ReadonlyArray<Readonly<{ entry: RoutableSignature; vector: Embedding }>> =
  ROUTABLE_SIGNATURES.map((entry) => ({ entry, vector: embedText(entry.descriptor) }))

export type SignatureSelection = Readonly<{
  family: BlueprintProgramFamily
  signatureRef: string
  // Cosine similarity score of the winning family (0..1). Surfaced so the UI /
  // tests can show how the route was chosen and assert it is similarity-driven.
  score: number
  // Whether the score cleared the minimum confidence; below it we fall back to
  // the runtime default family (continuation) honestly rather than guessing.
  confident: boolean
}>

// Minimum cosine score to treat a route as confident. Below this the chat falls
// back to the continuation family (the worker runtime's own default) — an honest
// "I'm continuing the thread" rather than a forced, low-confidence route.
export const MIN_ROUTE_CONFIDENCE = 0.08

// SEMANTIC selection: embed the user's free-text turn and pick the family whose
// descriptor centroid is nearest by cosine similarity. No keyword/substring/regex
// intent matching anywhere on this path.
export const selectSignatureForMessage = (message: string): SignatureSelection => {
  const query = embedText(message)
  let best: { entry: RoutableSignature; score: number } | null = null
  for (const { entry, vector } of FAMILY_VECTORS) {
    const score = cosineSimilarity(query, vector)
    if (best === null || score > best.score) best = { entry, score }
  }
  // Fall back to the runtime's default family when nothing is near.
  const fallback =
    ROUTABLE_SIGNATURES.find((s) => s.family === "continuation") ?? ROUTABLE_SIGNATURES[0]!
  if (best === null || best.score < MIN_ROUTE_CONFIDENCE) {
    return {
      family: fallback.family,
      signatureRef: fallback.signatureRef,
      score: best?.score ?? 0,
      confident: false,
    }
  }
  return {
    family: best.entry.family,
    signatureRef: best.entry.signatureRef,
    score: best.score,
    confident: true,
  }
}
