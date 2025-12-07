/**
 * Memory System Schema
 *
 * Types and helpers for MechaCoder's memory system.
 * Based on Generative Agents research showing 8 std dev improvement with memory + reflection.
 *
 * Memory Types:
 * - Episodic: What happened (task runs, errors, successes)
 * - Semantic: What is known (patterns, conventions, codebase structure)
 * - Procedural: How to do things (linked to skills)
 *
 * Scoring Formula (from Generative Agents):
 * score = α * recency + β * importance + γ * relevance
 * where α = 1.0, β = 1.0, γ = 1.0 (can be tuned)
 */

import * as S from "effect/Schema";

// --- Memory Types ---

export const MemoryType = S.Literal("episodic", "semantic", "procedural");
export type MemoryType = S.Schema.Type<typeof MemoryType>;

export const MemoryScope = S.Literal("global", "project", "session");
export type MemoryScope = S.Schema.Type<typeof MemoryScope>;

export const MemoryStatus = S.Literal("active", "archived", "decayed");
export type MemoryStatus = S.Schema.Type<typeof MemoryStatus>;

// --- Priority Levels ---

/**
 * Priority levels for memory importance scoring.
 * Based on Generative Agents importance ratings.
 */
export const ImportanceLevel = S.Literal(
  "trivial",      // 1-2: routine, forgettable
  "low",          // 3-4: slightly notable
  "medium",       // 5-6: moderately important
  "high",         // 7-8: very important
  "critical",     // 9-10: life-changing/critical errors
);
export type ImportanceLevel = S.Schema.Type<typeof ImportanceLevel>;

export const importanceToScore = (level: ImportanceLevel): number => {
  switch (level) {
    case "trivial": return 0.1;
    case "low": return 0.3;
    case "medium": return 0.5;
    case "high": return 0.7;
    case "critical": return 1.0;
  }
};

// --- Memory Content Types ---

export interface EpisodicContent {
  type: "episodic";
  /** What task was being performed */
  taskDescription: string;
  /** What happened */
  outcome: "success" | "failure" | "partial" | "timeout";
  /** Error message if failed */
  errorMessage?: string;
  /** Skills used (if any) */
  skillsUsed?: string[];
  /** Files touched */
  filesModified?: string[];
  /** Duration in ms */
  durationMs?: number;
}

export interface SemanticContent {
  type: "semantic";
  /** Category of knowledge */
  category: "pattern" | "convention" | "structure" | "api" | "dependency";
  /** The knowledge itself */
  knowledge: string;
  /** Where this knowledge applies */
  context?: string;
  /** Examples demonstrating this knowledge */
  examples?: string[];
}

export interface ProceduralContent {
  type: "procedural";
  /** Linked skill ID */
  skillId: string;
  /** When to use this procedure */
  triggerPatterns: string[];
  /** Success rate from usage */
  successRate?: number;
  /** Example invocations */
  examples?: string[];
}

export type MemoryContent = EpisodicContent | SemanticContent | ProceduralContent;

// --- Memory Record ---

/**
 * A single memory record.
 */
export interface Memory {
  /** Unique identifier */
  id: string;
  /** Memory type determines content structure */
  memoryType: MemoryType;
  /** Scope determines where memory applies */
  scope: MemoryScope;
  /** Status for lifecycle management */
  status: MemoryStatus;
  /** Natural language description for embedding */
  description: string;
  /** Structured content based on type */
  content: MemoryContent;
  /** Importance level (1-10 mapped to level) */
  importance: ImportanceLevel;
  /** Tags for filtering */
  tags: string[];
  /** Embedding vector for semantic search */
  embedding?: number[];
  /** Access count (for recency scoring) */
  accessCount: number;
  /** Last accessed timestamp */
  lastAccessedAt: string;
  /** Creation timestamp */
  createdAt: string;
  /** Update timestamp */
  updatedAt: string;
  /** Project ID if project-scoped */
  projectId?: string;
  /** Session ID if session-scoped */
  sessionId?: string;
  /** Related memory IDs */
  relatedMemories?: string[];
  /** Source of this memory (task, reflection, user) */
  source: "task" | "reflection" | "user" | "system";
}

// --- Memory Scoring ---

/**
 * Generative Agents scoring weights.
 * These can be tuned based on empirical results.
 */
export interface ScoringWeights {
  /** Weight for recency (how recently accessed) */
  recency: number;
  /** Weight for importance (how critical the memory is) */
  importance: number;
  /** Weight for relevance (semantic similarity to query) */
  relevance: number;
}

export const DEFAULT_SCORING_WEIGHTS: ScoringWeights = {
  recency: 1.0,
  importance: 1.0,
  relevance: 1.0,
};

/**
 * Calculate recency score using exponential decay.
 * score = exp(-λ * hours_since_access)
 * where λ = 0.99 (decays to ~0.1 after 24 hours)
 */
export const calculateRecency = (
  lastAccessedAt: string,
  decayRate: number = 0.99,
): number => {
  const lastAccess = new Date(lastAccessedAt).getTime();
  const now = Date.now();
  const hoursSince = (now - lastAccess) / (1000 * 60 * 60);
  return Math.exp(-decayRate * hoursSince);
};

/**
 * Calculate memory score using Generative Agents formula.
 * score = α * recency + β * importance + γ * relevance
 */
export const calculateMemoryScore = (
  memory: Memory,
  relevance: number,
  weights: ScoringWeights = DEFAULT_SCORING_WEIGHTS,
): number => {
  const recency = calculateRecency(memory.lastAccessedAt);
  const importance = importanceToScore(memory.importance);

  return (
    weights.recency * recency +
    weights.importance * importance +
    weights.relevance * relevance
  );
};

// --- Memory Match ---

export interface MemoryMatch {
  memory: Memory;
  score: number;
  relevance: number;
  matchReason: string;
}

// --- Memory Query ---

export interface MemoryQuery {
  /** Natural language query for semantic search */
  query: string;
  /** Filter by memory type */
  types?: MemoryType[];
  /** Filter by scope */
  scopes?: MemoryScope[];
  /** Filter by status */
  status?: MemoryStatus[];
  /** Filter by tags */
  tags?: string[];
  /** Filter by project */
  projectId?: string;
  /** Filter by session */
  sessionId?: string;
  /** Maximum results to return */
  limit?: number;
  /** Minimum relevance threshold */
  minRelevance?: number;
  /** Custom scoring weights */
  weights?: ScoringWeights;
}

// --- Memory Filter ---

export interface MemoryFilter {
  types?: MemoryType[];
  scopes?: MemoryScope[];
  status?: MemoryStatus[];
  tags?: string[];
  projectId?: string;
  sessionId?: string;
  source?: Memory["source"][];
  since?: string;
  until?: string;
}

// --- Helper Functions ---

/**
 * Generate a unique memory ID.
 */
export const generateMemoryId = (type: MemoryType): string => {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 8);
  return `mem-${type.slice(0, 3)}-${timestamp}-${random}`;
};

/**
 * Create a new memory with defaults.
 */
export const createMemory = (
  partial: Partial<Memory> & Pick<Memory, "memoryType" | "description" | "content">,
): Memory => {
  const now = new Date().toISOString();
  const baseMemory = {
    id: partial.id ?? generateMemoryId(partial.memoryType),
    memoryType: partial.memoryType,
    scope: (partial.scope ?? "project") as Memory["scope"],
    status: (partial.status ?? "active") as Memory["status"],
    description: partial.description,
    content: partial.content,
    importance: "medium" as const,
    tags: [] as string[],
    accessCount: 0,
    lastAccessedAt: now,
    createdAt: now,
    updatedAt: now,
    source: "system" as Memory["source"],
  };

  return {
    ...baseMemory,
    ...(partial.importance && { importance: partial.importance }),
    ...(partial.tags && { tags: partial.tags }),
    ...(partial.embedding && { embedding: partial.embedding }),
    ...(partial.accessCount && { accessCount: partial.accessCount }),
    ...(partial.lastAccessedAt && { lastAccessedAt: partial.lastAccessedAt }),
    ...(partial.createdAt && { createdAt: partial.createdAt }),
    ...(partial.updatedAt && { updatedAt: partial.updatedAt }),
    ...(partial.projectId && { projectId: partial.projectId }),
    ...(partial.sessionId && { sessionId: partial.sessionId }),
    ...(partial.relatedMemories && { relatedMemories: partial.relatedMemories }),
    ...(partial.source && { source: partial.source }),
  };
};

/**
 * Create an episodic memory from a task result.
 */
export const createEpisodicMemory = (
  taskDescription: string,
  outcome: "success" | "failure" | "partial" | "timeout",
  options?: {
    errorMessage?: string;
    skillsUsed?: string[];
    filesModified?: string[];
    durationMs?: number;
    importance?: ImportanceLevel;
    projectId?: string;
    sessionId?: string;
    tags?: string[];
  },
): Memory => {
  const baseContent: EpisodicContent = {
    type: "episodic",
    taskDescription,
    outcome,
  };

  const content: EpisodicContent = {
    ...baseContent,
    ...(options?.errorMessage && { errorMessage: options.errorMessage }),
    ...(options?.skillsUsed && { skillsUsed: options.skillsUsed }),
    ...(options?.filesModified && { filesModified: options.filesModified }),
    ...(options?.durationMs && { durationMs: options.durationMs }),
  };

  // Failures are more important to remember
  const importance = options?.importance ??
    (outcome === "failure" || outcome === "timeout"
      ? "high"
      : outcome === "partial"
        ? "medium"
        : "low");

  return createMemory({
    memoryType: "episodic",
    description: `Task: ${taskDescription} - Outcome: ${outcome}${options?.errorMessage ? ` - Error: ${options.errorMessage}` : ""}`,
    content,
    importance,
    tags: options?.tags ?? [outcome],
    ...(options?.projectId ? { projectId: options.projectId } : {}),
    ...(options?.sessionId ? { sessionId: options.sessionId } : {}),
    source: "task",
  });
};

/**
 * Create a semantic memory (knowledge).
 */
export const createSemanticMemory = (
  category: SemanticContent["category"],
  knowledge: string,
  options?: {
    context?: string;
    examples?: string[];
    importance?: ImportanceLevel;
    projectId?: string;
    tags?: string[];
  },
): Memory => {
  const baseContent: SemanticContent = {
    type: "semantic",
    category,
    knowledge,
  };

  const content: SemanticContent = {
    ...baseContent,
    ...(options?.context && { context: options.context }),
    ...(options?.examples && { examples: options.examples }),
  };

  return createMemory({
    memoryType: "semantic",
    description: `${category}: ${knowledge}`,
    content,
    importance: options?.importance ?? "medium",
    tags: options?.tags ?? [category],
    ...(options?.projectId && { projectId: options.projectId }),
    source: "reflection",
  });
};

/**
 * Create a procedural memory linked to a skill.
 */
export const createProceduralMemory = (
  skillId: string,
  triggerPatterns: string[],
  options?: {
    successRate?: number;
    examples?: string[];
    importance?: ImportanceLevel;
    projectId?: string;
    tags?: string[];
  },
): Memory => {
  const baseContent: ProceduralContent = {
    type: "procedural",
    skillId,
    triggerPatterns,
  };

  const content: ProceduralContent = {
    ...baseContent,
    ...(options?.successRate && { successRate: options.successRate }),
    ...(options?.examples && { examples: options.examples }),
  };

  return createMemory({
    memoryType: "procedural",
    description: `Skill: ${skillId} - Triggers: ${triggerPatterns.slice(0, 3).join(", ")}`,
    content,
    importance: options?.importance ?? "medium",
    tags: options?.tags ?? ["skill", skillId],
    ...(options?.projectId ? { projectId: options.projectId } : {}),
    source: "system",
  });
};

/**
 * Format memories for prompt injection.
 */
export const formatMemoriesForPrompt = (memories: Memory[]): string => {
  if (memories.length === 0) {
    return "No relevant memories found.";
  }

  return memories
    .map((m) => {
      const lines = [`[${m.memoryType}] ${m.description}`];
      if (m.content.type === "episodic" && m.content.errorMessage) {
        lines.push(`  Error: ${m.content.errorMessage}`);
      }
      if (m.content.type === "semantic") {
        lines.push(`  Context: ${m.content.context ?? "general"}`);
      }
      if (m.content.type === "procedural") {
        lines.push(`  Skill: ${m.content.skillId}`);
      }
      return lines.join("\n");
    })
    .join("\n\n");
};

/**
 * Build text for embedding a memory.
 */
export const buildMemoryText = (memory: Memory): string => {
  const parts = [memory.description];

  if (memory.content.type === "episodic") {
    parts.push(memory.content.taskDescription);
    if (memory.content.errorMessage) {
      parts.push(memory.content.errorMessage);
    }
  } else if (memory.content.type === "semantic") {
    parts.push(memory.content.knowledge);
    if (memory.content.context) {
      parts.push(memory.content.context);
    }
  } else if (memory.content.type === "procedural") {
    parts.push(...memory.content.triggerPatterns);
  }

  parts.push(...memory.tags);

  return parts.join(" ");
};
