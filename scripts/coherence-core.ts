/**
 * Deterministic conversation-coherence screening core.
 *
 * Implements the machine-checkable layer of
 * docs/analysis/conversation-thread-coherence-rubric.md: user frustration
 * and correction signals, interruption signals, and activity shape. The
 * semantic dimensions (D1..D8) and hard-fail gates need evidence review or
 * a model grader and are NOT computed here. The screening score is a
 * proxy that supports hill-climbing and regression detection only. It is
 * analysis, never assurance, release, or public-claim authority.
 */

export type ConversationSource = "codex" | "claude-code";

export type CoherenceSignalKind = "profanity" | "correction" | "interrupt";

export interface CoherenceSignal {
  readonly kind: CoherenceSignalKind;
  readonly userTurnIndex: number;
  /** Bounded lowercase excerpt around the match. Local diagnostics only. */
  readonly excerpt: string;
}

export interface ParsedConversation {
  readonly source: ConversationSource;
  readonly path: string;
  readonly userTurnCount: number;
  readonly assistantTurnCount: number;
  readonly toolCallCount: number;
  readonly fileChangeCount: number;
  readonly interruptCount: number;
  readonly firstTimestamp: string | null;
  readonly signals: readonly CoherenceSignal[];
  /** Distinct tool/item kinds observed (complexity feature). */
  readonly toolKinds: readonly string[];
  /** Distinct model identifiers observed (complexity feature). */
  readonly models: readonly string[];
  /** Sub-agent spawn events (complexity feature). */
  readonly subAgentStarts: number;
  /** Sub-agent communication events after spawn (complexity feature). */
  readonly subAgentInteractions: number;
  /** Distinct sub-agent identities (complexity feature). */
  readonly distinctSubAgents: number;
}

export type ComplexityTier = "C0" | "C1" | "C2" | "C3" | "C4";

export interface ComplexityAssessment {
  /** 0-100 deterministic orchestration-breadth score. */
  readonly score: number;
  readonly tier: ComplexityTier;
  readonly components: Readonly<Record<string, number>>;
}

export interface ScoredConversation extends ParsedConversation {
  readonly score: number;
  readonly grade: "A" | "B" | "C" | "D" | "F";
  readonly disposition: "screening_pass" | "needs_review" | "skipped";
  readonly deductions: Readonly<Record<CoherenceSignalKind, number>>;
  readonly complexity: ComplexityAssessment;
}

const complexityTier = (score: number): ComplexityTier =>
  score >= 75 ? "C4" : score >= 50 ? "C3" : score >= 25 ? "C2" : score >= 10 ? "C1" : "C0";

/**
 * Deterministic complexity score per
 * docs/analysis/complexity-rubric.md (metric coherence-screen-v2).
 * A coherence score carries evidence weight proportional to this tier:
 * 100/A at C0 proves almost nothing; 90/A at C3 is real evidence.
 */
export const computeComplexity = (parsed: ParsedConversation): ComplexityAssessment => {
  const log2 = (value: number): number => Math.log2(1 + Math.max(0, value));
  const components: Record<string, number> = {
    continuations: Math.min(Math.max(parsed.assistantTurnCount - 1, 0) * 2, 10),
    dialogue: Math.min(Math.max(parsed.userTurnCount - 1, 0), 5),
    tools: Math.min(Math.round(8 * log2(parsed.toolCallCount)), 24),
    toolDiversity: Math.min(parsed.toolKinds.length * 3, 12),
    mutations: Math.min(parsed.fileChangeCount * 2, 10),
    subAgents: Math.min(
      Math.round(7 * log2(parsed.subAgentStarts + parsed.subAgentInteractions)),
      21,
    ),
    subAgentBreadth: Math.min(parsed.distinctSubAgents * 4, 12),
    multiModel: Math.min(Math.max(parsed.models.length - 1, 0) * 5, 10),
  };
  const score = Math.min(
    Object.values(components).reduce((sum, value) => sum + value, 0),
    100,
  );
  return { score, tier: complexityTier(score), components };
};

/**
 * User frustration lexicon. The owner directive for this screen: deduct
 * points when the user swears or corrects the agent ("No, you did this
 * wrong"). Matches run against user text only, lowercased. Quoted or
 * pasted text can false-positive; the screen accepts that noise and the
 * rubric's evidence review resolves it.
 */
const PROFANITY_PATTERNS: readonly RegExp[] = [
  /\bf+u+c*k+\w*\b/,
  /\bs+h+i+t+\w*\b/,
  /\bbullshit\b/,
  /\bwtf\b/,
  /\bgod?dammit\b/,
  /\bgoddamn\w*\b/,
  /\bdamn ?it\b/,
  /\bpissed\b/,
  /\bcrap+y?\b/,
];

const CORRECTION_PATTERNS: readonly RegExp[] = [
  /\bno[,.! ]+you\b/,
  /\byou did (this|that|it) wrong\b/,
  /\bthat'?s? not what i (asked|wanted|meant|said)\b/,
  /\bnot what i asked\b/,
  /\bi didn'?t ask\b/,
  /\byou ignored\b/,
  /\byou keep (doing|making|getting)\b/,
  /\bwhy (did|would) you\b/,
  /\bstill (wrong|broken|failing|not working|doesn'?t work)\b/,
  /\bwrong again\b/,
  /\bdo it again\b/,
  /\byou broke\b/,
  /\bstop (doing|that)\b/,
  /\btry again\b/,
  /\bundo (that|this)\b/,
  /\brevert (that|this)\b/,
];

const INTERRUPT_MARKERS: readonly string[] = [
  "[request interrupted by user",
  "[request cancelled by user",
];

export const SCREENING_DEDUCTIONS: Readonly<
  Record<CoherenceSignalKind, { readonly perSignal: number; readonly cap: number }>
> = {
  profanity: { perSignal: 15, cap: 45 },
  correction: { perSignal: 10, cap: 40 },
  interrupt: { perSignal: 5, cap: 20 },
};

const EXCERPT_RADIUS = 40;

const boundedExcerpt = (text: string, index: number, matchLength: number): string => {
  const start = Math.max(0, index - EXCERPT_RADIUS);
  const end = Math.min(text.length, index + matchLength + EXCERPT_RADIUS);
  return text.slice(start, end).replace(/\s+/g, " ").trim();
};

export const detectUserSignals = (
  userText: string,
  userTurnIndex: number,
): CoherenceSignal[] => {
  const lowered = userText.toLowerCase();
  const signals: CoherenceSignal[] = [];
  const scan = (patterns: readonly RegExp[], kind: CoherenceSignalKind): void => {
    for (const pattern of patterns) {
      const match = lowered.match(pattern);
      if (match && match.index !== undefined) {
        signals.push({
          kind,
          userTurnIndex,
          excerpt: boundedExcerpt(lowered, match.index, match[0].length),
        });
        break;
      }
    }
  };
  scan(PROFANITY_PATTERNS, "profanity");
  scan(CORRECTION_PATTERNS, "correction");
  return signals;
};

const isInterruptText = (text: string): boolean => {
  const lowered = text.toLowerCase();
  return INTERRUPT_MARKERS.some((marker) => lowered.includes(marker));
};

/** Skip harness-injected user lines that carry no human text. */
const isInjectedUserText = (text: string): boolean => {
  const trimmed = text.trimStart();
  return (
    trimmed.startsWith("<") ||
    trimmed.startsWith("[SYSTEM NOTIFICATION") ||
    trimmed.startsWith("Caveat:")
  );
};

interface JsonLine {
  readonly [key: string]: unknown;
}

const parseJsonLines = (content: string): JsonLine[] => {
  const lines: JsonLine[] = [];
  for (const raw of content.split("\n")) {
    if (raw.trim() === "") continue;
    try {
      const value = JSON.parse(raw) as unknown;
      if (typeof value === "object" && value !== null) lines.push(value as JsonLine);
    } catch {
      // Ignore truncated or corrupt lines. Screening stays fail-soft.
    }
  }
  return lines;
};

/** Parse one Codex CLI rollout JSONL file (~/.codex/sessions). */
export const parseCodexConversation = (
  path: string,
  content: string,
): ParsedConversation => {
  let userTurnCount = 0;
  let assistantTurnCount = 0;
  let toolCallCount = 0;
  let fileChangeCount = 0;
  let interruptCount = 0;
  let firstTimestamp: string | null = null;
  const signals: CoherenceSignal[] = [];
  const toolKinds = new Set<string>();
  const models = new Set<string>();
  const subAgentIds = new Set<string>();
  let subAgentStarts = 0;
  let subAgentInteractions = 0;
  for (const line of parseJsonLines(content)) {
    const timestamp = typeof line.timestamp === "string" ? line.timestamp : null;
    if (firstTimestamp === null && timestamp !== null) firstTimestamp = timestamp;
    const payload =
      typeof line.payload === "object" && line.payload !== null
        ? (line.payload as JsonLine)
        : null;
    if (line.type === "turn_context" && payload !== null) {
      if (typeof payload.model === "string" && payload.model !== "") models.add(payload.model);
    } else if (line.type === "event_msg" && payload !== null) {
      const payloadType = payload.type;
      if (payloadType === "user_message" && typeof payload.message === "string") {
        if (isInjectedUserText(payload.message)) continue;
        userTurnCount += 1;
        if (userTurnCount > 1) signals.push(...detectUserSignals(payload.message, userTurnCount - 1));
      } else if (payloadType === "agent_message") {
        assistantTurnCount += 1;
      } else if (payloadType === "turn_aborted") {
        interruptCount += 1;
      } else if (payloadType === "patch_apply_end" || payloadType === "mcp_tool_call_end") {
        fileChangeCount += payloadType === "patch_apply_end" ? 1 : 0;
        toolCallCount += 1;
        toolKinds.add(payloadType === "patch_apply_end" ? "file_change" : "mcp_tool_call");
      } else if (payloadType === "web_search_end") {
        toolCallCount += 1;
        toolKinds.add("web_search");
      } else if (payloadType === "sub_agent_activity") {
        if (typeof payload.agent_thread_id === "string") subAgentIds.add(payload.agent_thread_id);
        if (payload.kind === "started") subAgentStarts += 1;
        else subAgentInteractions += 1;
      }
    } else if (line.type === "response_item" && payload !== null) {
      if (payload.type === "function_call" || payload.type === "custom_tool_call") {
        toolCallCount += 1;
        toolKinds.add(typeof payload.name === "string" ? payload.name : "function_call");
      }
    }
  }
  return {
    source: "codex",
    path,
    userTurnCount,
    assistantTurnCount,
    toolCallCount,
    fileChangeCount,
    interruptCount,
    firstTimestamp,
    signals,
    toolKinds: [...toolKinds].sort(),
    models: [...models].sort(),
    subAgentStarts,
    subAgentInteractions,
    distinctSubAgents: subAgentIds.size,
  };
};

const claudeTextFromContent = (content: unknown): string[] => {
  if (typeof content === "string") return [content];
  if (!Array.isArray(content)) return [];
  const texts: string[] = [];
  for (const block of content) {
    if (
      typeof block === "object" &&
      block !== null &&
      (block as JsonLine).type === "text" &&
      typeof (block as JsonLine).text === "string"
    ) {
      texts.push((block as JsonLine).text as string);
    }
  }
  return texts;
};

/** Parse one Claude Code session JSONL file (~/.claude/projects). */
export const parseClaudeConversation = (
  path: string,
  content: string,
): ParsedConversation => {
  let userTurnCount = 0;
  let assistantTurnCount = 0;
  let toolCallCount = 0;
  let fileChangeCount = 0;
  let interruptCount = 0;
  let firstTimestamp: string | null = null;
  const signals: CoherenceSignal[] = [];
  const toolKinds = new Set<string>();
  const models = new Set<string>();
  let subAgentStarts = 0;
  let subAgentInteractions = 0;
  for (const line of parseJsonLines(content)) {
    if (line.isSidechain === true) continue;
    if (line.isMeta === true) continue;
    const timestamp = typeof line.timestamp === "string" ? line.timestamp : null;
    if (firstTimestamp === null && timestamp !== null) firstTimestamp = timestamp;
    const message =
      typeof line.message === "object" && line.message !== null
        ? (line.message as JsonLine)
        : null;
    if (message === null) continue;
    if (line.type === "user" && message.role === "user") {
      const texts = claudeTextFromContent(message.content);
      const humanTexts = texts.filter((text) => !isInjectedUserText(text));
      if (humanTexts.some(isInterruptText)) {
        interruptCount += 1;
        continue;
      }
      if (humanTexts.length === 0) continue;
      userTurnCount += 1;
      if (userTurnCount > 1) {
        for (const text of humanTexts) {
          signals.push(...detectUserSignals(text, userTurnCount - 1));
        }
      }
    } else if (line.type === "assistant" && message.role === "assistant") {
      if (typeof message.model === "string" && message.model !== "") models.add(message.model);
      const content = message.content;
      if (Array.isArray(content)) {
        let hasText = false;
        for (const block of content) {
          if (typeof block !== "object" || block === null) continue;
          const blockType = (block as JsonLine).type;
          if (blockType === "tool_use") {
            toolCallCount += 1;
            const name = (block as JsonLine).name;
            if (typeof name === "string") {
              toolKinds.add(name);
              if (name === "Agent" || name === "Task" || name === "Workflow") subAgentStarts += 1;
              if (name === "SendMessage") subAgentInteractions += 1;
            }
            if (name === "Write" || name === "Edit" || name === "NotebookEdit") {
              fileChangeCount += 1;
            }
          } else if (blockType === "text") {
            hasText = true;
          }
        }
        if (hasText) assistantTurnCount += 1;
      }
    }
  }
  return {
    source: "claude-code",
    path,
    userTurnCount,
    assistantTurnCount,
    toolCallCount,
    fileChangeCount,
    interruptCount,
    firstTimestamp,
    signals,
    toolKinds: [...toolKinds].sort(),
    models: [...models].sort(),
    subAgentStarts,
    subAgentInteractions,
    distinctSubAgents: subAgentStarts,
  };
};

/**
 * Score a parsed conversation. 100 minus capped deductions per signal
 * kind. Grade bands mirror the rubric. Conversations without at least one
 * user turn and one assistant turn are skipped, not graded.
 */
export const scoreConversation = (parsed: ParsedConversation): ScoredConversation => {
  if (parsed.userTurnCount === 0 || parsed.assistantTurnCount === 0) {
    return {
      ...parsed,
      score: 0,
      grade: "F",
      disposition: "skipped",
      deductions: { profanity: 0, correction: 0, interrupt: 0 },
      complexity: computeComplexity(parsed),
    };
  }
  const counts: Record<CoherenceSignalKind, number> = {
    profanity: 0,
    correction: 0,
    interrupt: parsed.interruptCount,
  };
  for (const signal of parsed.signals) {
    if (signal.kind !== "interrupt") counts[signal.kind] += 1;
  }
  const deductions: Record<CoherenceSignalKind, number> = {
    profanity: Math.min(
      counts.profanity * SCREENING_DEDUCTIONS.profanity.perSignal,
      SCREENING_DEDUCTIONS.profanity.cap,
    ),
    correction: Math.min(
      counts.correction * SCREENING_DEDUCTIONS.correction.perSignal,
      SCREENING_DEDUCTIONS.correction.cap,
    ),
    interrupt: Math.min(
      counts.interrupt * SCREENING_DEDUCTIONS.interrupt.perSignal,
      SCREENING_DEDUCTIONS.interrupt.cap,
    ),
  };
  const score = Math.max(
    0,
    100 - deductions.profanity - deductions.correction - deductions.interrupt,
  );
  const grade =
    score >= 90 ? "A" : score >= 80 ? "B" : score >= 70 ? "C" : score >= 50 ? "D" : "F";
  return {
    ...parsed,
    score,
    grade,
    disposition: score >= 80 ? "screening_pass" : "needs_review",
    deductions,
    complexity: computeComplexity(parsed),
  };
};

export interface SourceAggregate {
  readonly source: ConversationSource;
  readonly graded: number;
  readonly skipped: number;
  readonly meanScore: number;
  readonly medianScore: number;
  readonly gradeCounts: Readonly<Record<"A" | "B" | "C" | "D" | "F", number>>;
  readonly signalCounts: Readonly<Record<CoherenceSignalKind, number>>;
  readonly needsReview: number;
  readonly meanComplexity: number;
  readonly tierCounts: Readonly<Record<ComplexityTier, number>>;
  /** Coherence mean weighted by complexity: high-complexity threads dominate. */
  readonly complexityWeightedCoherence: number;
}

export const aggregateBySource = (
  conversations: readonly ScoredConversation[],
): SourceAggregate[] => {
  const sources: ConversationSource[] = ["codex", "claude-code"];
  const aggregates: SourceAggregate[] = [];
  for (const source of sources) {
    const all = conversations.filter((item) => item.source === source);
    const graded = all.filter((item) => item.disposition !== "skipped");
    const scores = graded.map((item) => item.score).sort((left, right) => left - right);
    const gradeCounts = { A: 0, B: 0, C: 0, D: 0, F: 0 };
    const signalCounts: Record<CoherenceSignalKind, number> = {
      profanity: 0,
      correction: 0,
      interrupt: 0,
    };
    for (const item of graded) {
      gradeCounts[item.grade] += 1;
      signalCounts.interrupt += item.interruptCount;
      for (const signal of item.signals) {
        if (signal.kind !== "interrupt") signalCounts[signal.kind] += 1;
      }
    }
    const meanScore =
      scores.length === 0
        ? 0
        : scores.reduce((sum, value) => sum + value, 0) / scores.length;
    const medianScore =
      scores.length === 0 ? 0 : scores[Math.floor((scores.length - 1) / 2)];
    const tierCounts: Record<ComplexityTier, number> = { C0: 0, C1: 0, C2: 0, C3: 0, C4: 0 };
    let complexitySum = 0;
    let weightSum = 0;
    let weightedCoherenceSum = 0;
    for (const item of graded) {
      tierCounts[item.complexity.tier] += 1;
      complexitySum += item.complexity.score;
      const weight = Math.max(item.complexity.score, 1);
      weightSum += weight;
      weightedCoherenceSum += item.score * weight;
    }
    aggregates.push({
      source,
      graded: graded.length,
      skipped: all.length - graded.length,
      meanScore: Math.round(meanScore * 10) / 10,
      medianScore,
      gradeCounts,
      signalCounts,
      needsReview: graded.filter((item) => item.disposition === "needs_review").length,
      meanComplexity: graded.length === 0 ? 0 : Math.round((complexitySum / graded.length) * 10) / 10,
      tierCounts,
      complexityWeightedCoherence: weightSum === 0 ? 0 : Math.round((weightedCoherenceSum / weightSum) * 10) / 10,
    });
  }
  return aggregates;
};
