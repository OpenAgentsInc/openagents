/**
 * Skill Library Schema
 *
 * Defines the structure of skills for MechaCoder's skill library.
 * Based on Voyager research showing 3.3x improvement with skill libraries.
 *
 * Skills are:
 * - Verified: Must pass verification before being stored
 * - Executable: Contain actual code/tool patterns
 * - Composable: Can be combined to solve complex problems
 * - Indexed: Searchable by description embedding
 */

import * as S from "effect/Schema";

// --- Skill Parameter Types ---

export const SkillParameterType = S.Literal(
  "string",
  "number",
  "boolean",
  "path",
  "code",
  "pattern",
  "array",
);
export type SkillParameterType = S.Schema.Type<typeof SkillParameterType>;

export const SkillParameter = S.Struct({
  name: S.String,
  type: SkillParameterType,
  description: S.String,
  required: S.Boolean,
  default: S.optional(S.Unknown),
  examples: S.optional(S.Array(S.String)),
});
export type SkillParameter = S.Schema.Type<typeof SkillParameter>;

// --- Skill Verification Types ---

export const VerificationType = S.Literal(
  "test",       // Run tests
  "typecheck",  // Run type checker
  "command",    // Run a command and check exit code
  "pattern",    // Check output matches pattern
  "none",       // No verification needed
);
export type VerificationType = S.Schema.Type<typeof VerificationType>;

export const SkillVerification = S.Struct({
  type: VerificationType,
  command: S.optional(S.String),
  pattern: S.optional(S.String),
  timeout: S.optional(S.Number),
});
export type SkillVerification = S.Schema.Type<typeof SkillVerification>;

// --- Skill Example Types ---

export const SkillExample = S.Struct({
  description: S.String,
  input: S.Record({ key: S.String, value: S.Unknown }),
  output: S.String,
  context: S.optional(S.String),
});
export type SkillExample = S.Schema.Type<typeof SkillExample>;

// --- Skill Category Types ---

export const SkillCategory = S.Literal(
  "file_operations",   // Read, write, edit files
  "testing",          // Run tests, assertions
  "debugging",        // Analyze errors, fix bugs
  "refactoring",      // Code transformations
  "git",              // Version control operations
  "shell",            // Terminal commands
  "search",           // Find code, patterns
  "documentation",    // Comments, docs
  "security",         // Security-related operations
  "performance",      // Optimization
  "meta",             // Skill management
  "api",              // API interactions
  "effect",           // Effect-TS specific skills
);
export type SkillCategory = S.Schema.Type<typeof SkillCategory>;

// --- Skill Status Types ---

export const SkillStatus = S.Literal(
  "active",      // Skill is available for use
  "archived",    // Skill is deprecated but preserved
  "draft",       // Skill is being developed
  "failed",      // Skill failed verification
);
export type SkillStatus = S.Schema.Type<typeof SkillStatus>;

// --- Main Skill Type ---

export const Skill = S.Struct({
  /** Unique skill identifier (e.g., "skill-fix-import-error-v1") */
  id: S.String,

  /** Human-readable skill name */
  name: S.String,

  /** Version string (semver-like) */
  version: S.String,

  /** Detailed description for embedding-based retrieval */
  description: S.String,

  /** Skill category for filtering */
  category: SkillCategory,

  /** Current status */
  status: SkillStatus,

  /** Embedding vector for similarity search (optional, populated by embedding service) */
  embedding: S.optional(S.Array(S.Number)),

  /** The executable code/pattern for this skill */
  code: S.String,

  /** Parameters that can be customized */
  parameters: S.Array(SkillParameter),

  /** Prerequisites (other skill IDs that should run first) */
  prerequisites: S.optional(S.Array(S.String)),

  /** Postconditions (what this skill achieves) */
  postconditions: S.optional(S.Array(S.String)),

  /** How to verify the skill worked */
  verification: SkillVerification,

  /** Usage examples */
  examples: S.optional(S.Array(SkillExample)),

  /** Tags for additional filtering */
  tags: S.optional(S.Array(S.String)),

  /** Applicable languages (e.g., ["typescript", "javascript"]) */
  languages: S.optional(S.Array(S.String)),

  /** Applicable frameworks (e.g., ["react", "effect"]) */
  frameworks: S.optional(S.Array(S.String)),

  /** Episode IDs that generated this skill */
  learnedFrom: S.optional(S.Array(S.String)),

  /** Success rate (0-1) from actual usage */
  successRate: S.optional(S.Number),

  /** Number of times this skill has been used */
  usageCount: S.optional(S.Number),

  /** Last time this skill was used (ISO timestamp) */
  lastUsed: S.optional(S.String),

  /** Creation timestamp (ISO) */
  createdAt: S.String,

  /** Last update timestamp (ISO) */
  updatedAt: S.String,

  /** Author/source of the skill */
  source: S.optional(S.Literal("bootstrap", "learned", "manual")),
});
export type Skill = S.Schema.Type<typeof Skill>;

// --- Skill Filter Types ---

export interface SkillFilter {
  categories?: SkillCategory[];
  status?: SkillStatus[];
  tags?: string[];
  languages?: string[];
  frameworks?: string[];
  minSuccessRate?: number;
  maxResults?: number;
}

// --- Skill Query Types ---

export interface SkillQuery {
  /** Natural language query for embedding search */
  query: string;
  /** Optional filters to apply */
  filter?: SkillFilter;
  /** Number of results to return (default: 5) */
  topK?: number;
  /** Minimum similarity score (0-1, default: 0.5) */
  minSimilarity?: number;
}

// --- Skill Match Result ---

export interface SkillMatch {
  skill: Skill;
  similarity: number;
  matchReason: string;
}

// --- Skill Call Types (for FM prompt) ---

export const SkillCall = S.Struct({
  skillId: S.String,
  parameters: S.Record({ key: S.String, value: S.Unknown }),
});
export type SkillCall = S.Schema.Type<typeof SkillCall>;

// --- Skill Execution Result ---

export const SkillExecutionResult = S.Struct({
  skillId: S.String,
  success: S.Boolean,
  output: S.optional(S.String),
  error: S.optional(S.String),
  duration: S.Number,
  verificationPassed: S.optional(S.Boolean),
});
export type SkillExecutionResult = S.Schema.Type<typeof SkillExecutionResult>;

// --- Helper Functions ---

/**
 * Generate a skill ID from name and version.
 */
export const generateSkillId = (name: string, version: string): string => {
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "");
  return `skill-${slug}-${version}`;
};

/**
 * Create a new skill with defaults.
 */
export const createSkill = (
  partial: Partial<Skill> & Pick<Skill, "name" | "description" | "code" | "category">,
): Skill => {
  const now = new Date().toISOString();
  const version = partial.version ?? "v1";
  const id = partial.id ?? generateSkillId(partial.name, version);

  return {
    id,
    name: partial.name,
    version,
    description: partial.description,
    category: partial.category,
    status: partial.status ?? "active",
    code: partial.code,
    parameters: partial.parameters ?? [],
    verification: partial.verification ?? { type: "none" },
    createdAt: partial.createdAt ?? now,
    updatedAt: partial.updatedAt ?? now,
    source: partial.source ?? "manual",
    tags: partial.tags ?? [],
    examples: partial.examples ?? [],
    // Include optional fields if provided
    ...(partial.successRate !== undefined && { successRate: partial.successRate }),
    ...(partial.usageCount !== undefined && { usageCount: partial.usageCount }),
    ...(partial.lastUsed !== undefined && { lastUsed: partial.lastUsed }),
    ...(partial.prerequisites !== undefined && { prerequisites: partial.prerequisites }),
    ...(partial.postconditions !== undefined && { postconditions: partial.postconditions }),
    ...(partial.languages !== undefined && { languages: partial.languages }),
    ...(partial.frameworks !== undefined && { frameworks: partial.frameworks }),
    ...(partial.learnedFrom !== undefined && { learnedFrom: partial.learnedFrom }),
    ...(partial.embedding !== undefined && { embedding: partial.embedding }),
  };
};

/**
 * Format a skill for prompt injection.
 */
export const formatSkillForPrompt = (skill: Skill): string => {
  const params = skill.parameters
    .map((p) => `  - ${p.name}: ${p.type}${p.required ? " (required)" : ""} - ${p.description}`)
    .join("\n");

  const examples = skill.examples
    ?.map((e) => `  Example: ${e.description}\n    Input: ${JSON.stringify(e.input)}\n    Output: ${e.output}`)
    .join("\n");

  return `### ${skill.name} (${skill.id})
Category: ${skill.category}
Description: ${skill.description}
${skill.successRate !== undefined ? `Success Rate: ${(skill.successRate * 100).toFixed(0)}%\n` : ""}
${params ? `Parameters:\n${params}` : ""}
${examples ? `\n${examples}` : ""}
Code:
\`\`\`
${skill.code}
\`\`\``;
};

/**
 * Format multiple skills for prompt injection.
 */
export const formatSkillsForPrompt = (skills: Skill[]): string => {
  if (skills.length === 0) return "No relevant skills found.";
  return skills.map(formatSkillForPrompt).join("\n\n---\n\n");
};
