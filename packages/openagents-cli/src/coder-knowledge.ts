/**
 * The knowledge-base rail (OpenAgentsInc/openagents#49): the harness asks the
 * `knowledge_base` plugin about each incoming message and, when the corpus
 * clearly answers, attaches its stances and doc summaries to the outgoing
 * turn. The model never calls the knowledge base as a tool — by the time it
 * writes, the documented position is already in front of it, dated and
 * sourced, so it can prefer what the project actually decided over a guess.
 */

/** One hit as the `knowledge_base` guest returns it. */
export interface KnowledgeHit {
  readonly kind: "stance" | "doc";
  readonly title: string;
  readonly body: string;
  readonly state?: string | undefined;
  readonly sources: ReadonlyArray<string>;
  readonly date?: string | undefined;
  readonly score: number;
}

/**
 * The overlap a hit needs before it is worth attaching. The guest scores a
 * question-level word at 3, so the floor asks for roughly two intent-carrying
 * words — a lone body-text collision never interrupts the conversation.
 */
export const KNOWLEDGE_ATTACH_FLOOR = 5;

/** Most hits one note carries; more is noise, not context. */
const NOTE_LIMIT = 2;

/** A body's share of the note; stances are prose and can run long. */
const BODY_BOUND = 700;

const clipped = (text: string): string =>
  text.length <= BODY_BOUND ? text : `${text.slice(0, BODY_BOUND - 1).trimEnd()}…`;

const line = (hit: KnowledgeHit): string => {
  const provenance =
    hit.kind === "stance"
      ? `stance${hit.state === undefined ? "" : `, ${hit.state}`}${
          hit.date === undefined ? "" : `; reviewed ${hit.date}`
        }`
      : `doc${hit.sources.length === 0 ? "" : `, ${hit.sources[0] ?? ""}`}`;
  return `- ${hit.title} (${provenance}): ${clipped(hit.body)}`;
};

/**
 * The note for one message, or nothing when the corpus has nothing strong
 * enough to say. Hits arrive best-first from the guest; the floor and the
 * limit are applied here so the plugin stays a plain ranking function.
 */
export const knowledgeNote = (hits: ReadonlyArray<KnowledgeHit>): string | undefined => {
  const strong = hits.filter((hit) => hit.score >= KNOWLEDGE_ATTACH_FLOOR).slice(0, NOTE_LIMIT);
  if (strong.length === 0) return undefined;
  return (
    "[From the OpenAgents knowledge base — reviewed positions and public docs. " +
    "Where these speak, prefer them over general knowledge, and say when a thing " +
    "is parked or planned rather than live:\n" +
    strong.map(line).join("\n") +
    "]"
  );
};

/** A guarded read of the guest's envelope into typed hits; junk is no hits. */
export const knowledgeHits = (envelope: unknown): KnowledgeHit[] => {
  if (envelope === null || typeof envelope !== "object") return [];
  const ok = (envelope as Record<string, unknown>)["ok"];
  if (ok === null || typeof ok !== "object") return [];
  const hits = (ok as Record<string, unknown>)["hits"];
  if (!Array.isArray(hits)) return [];
  const typed: KnowledgeHit[] = [];
  for (const candidate of hits) {
    if (candidate === null || typeof candidate !== "object") continue;
    const record = candidate as Record<string, unknown>;
    const kind = record["kind"];
    const title = record["title"];
    const body = record["body"];
    const score = record["score"];
    if (kind !== "stance" && kind !== "doc") continue;
    if (typeof title !== "string" || typeof body !== "string" || typeof score !== "number") {
      continue;
    }
    typed.push({
      kind,
      title,
      body,
      state: typeof record["state"] === "string" ? record["state"] : undefined,
      sources: Array.isArray(record["sources"])
        ? record["sources"].filter((source): source is string => typeof source === "string")
        : [],
      date: typeof record["date"] === "string" ? record["date"] : undefined,
      score,
    });
  }
  return typed;
};
