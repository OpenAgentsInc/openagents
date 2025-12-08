/**
 * Micro-Task Decomposition for FM
 *
 * Ensures all FM prompts fit within the ~1100 char safe context window
 * by decomposing larger tasks into tiny, focused steps.
 *
 * Based on docs/logs/20251206/1421-coding-thoughts.md section 4:
 * "Always work in micro-tasks that fit inside the window"
 */

import type { Skill } from "../skills/schema.js";

// --- Constants ---

/**
 * FM context budget (chars). Keep VERY conservative to leave room for response.
 * 
 * Based on empirical testing:
 * - Even 555 chars of content fails with "Exceeded model context window size"
 * - JSON serialization adds ~50-100 chars overhead (roles, structure)
 * - Need to leave room for response generation
 * 
 * Using 400 chars as the absolute maximum content size to be safe.
 */
export const FM_CONTEXT_BUDGET = 400; // Very conservative - actual limit appears to be ~500-600 total

/**
 * Maximum chars for skills section.
 */
export const MAX_SKILLS_CHARS = 300;

/**
 * Maximum chars for memories section.
 */
export const MAX_MEMORIES_CHARS = 150;

/**
 * Maximum chars for reflections section.
 */
export const MAX_REFLECTIONS_CHARS = 100;

/**
 * Maximum chars per skill (signature only, no full code).
 */
export const MAX_SKILL_CHARS = 80;

// --- Skill Condensation ---

/**
 * Condense a skill to a minimal signature.
 * Instead of full code, just show pattern name + key parameters.
 */
export function condenseSkill(skill: Skill): string {
  const params = skill.parameters
    .filter((p) => p.required)
    .map((p) => p.name)
    .join(", ");

  // Short form: "skill_name(param1, param2) - description"
  const shortDesc = skill.description.slice(0, 40).replace(/\n/g, " ");
  return `${skill.name}(${params}) - ${shortDesc}`;
}

/**
 * Select and condense skills to fit within budget.
 * Returns formatted skills section that fits in MAX_SKILLS_CHARS.
 */
export function condenseSkillsForPrompt(skills: Skill[], maxChars = MAX_SKILLS_CHARS): string {
  if (!skills || skills.length === 0) return "";

  const condensed: string[] = [];
  let totalChars = 0;

  // Sort by success rate (highest first)
  const sorted = [...skills].sort((a, b) => (b.successRate ?? 0) - (a.successRate ?? 0));

  for (const skill of sorted) {
    const line = condenseSkill(skill);
    if (totalChars + line.length + 2 > maxChars) break;

    condensed.push(line);
    totalChars += line.length + 2; // +2 for newline
  }

  if (condensed.length === 0) return "";

  return `Skills: ${condensed.join("; ")}`;
}

// --- Memory Condensation ---

/**
 * Condense memories to fit within budget.
 * Truncates to key insights only.
 */
export function condenseMemoriesForPrompt(memories: string | undefined, maxChars = MAX_MEMORIES_CHARS): string {
  if (!memories || !memories.trim()) return "";

  // Extract first few lines only
  const lines = memories.split("\n").filter((l) => l.trim());
  let result = "";

  for (const line of lines) {
    const trimmed = line.trim().slice(0, 60);
    if (result.length + trimmed.length + 2 > maxChars) break;
    result += (result ? "; " : "") + trimmed;
  }

  if (!result) return "";
  return `Note: ${result}`;
}

// --- Reflection Condensation ---

/**
 * Condense reflections to fit within budget.
 * Keep only the most critical lesson.
 */
export function condenseReflectionsForPrompt(
  reflections: string | undefined,
  maxChars = MAX_REFLECTIONS_CHARS,
): string {
  if (!reflections || !reflections.trim()) return "";

  // Just take first sentence or line
  const firstLine = reflections.split(/[.\n]/)[0]?.trim() ?? "";
  if (!firstLine) return "";

  const truncated = firstLine.slice(0, maxChars - 10);
  return `Tip: ${truncated}`;
}

// --- Micro-Task Prompt Building ---

/**
 * Build a minimal FM prompt that fits within context budget.
 *
 * Structure:
 * - Core instructions (fixed, ~150 chars)
 * - Condensed skills (max 300 chars)
 * - Condensed memories (max 150 chars)
 * - Condensed reflections (max 100 chars)
 * - User task (remaining budget)
 */
export function buildMicroTaskPrompt(options: {
  skills?: Skill[];
  memories?: string;
  reflections?: string;
}): string {
  const { skills, memories, reflections } = options;

  // Core instructions - keep VERY short (minimal for FM's tiny context)
  const core = `Tools: write_file, read_file, run_command
Format: <tool_call>{"name":"..","arguments":{..}}</tool_call>
Done: TASK_COMPLETE`;

  // Condense each section
  const skillsSection = condenseSkillsForPrompt(skills ?? []);
  const memoriesSection = condenseMemoriesForPrompt(memories);
  const reflectionsSection = condenseReflectionsForPrompt(reflections);

  // Build prompt, only including non-empty sections
  const parts = [core];
  if (skillsSection) parts.push(skillsSection);
  if (memoriesSection) parts.push(memoriesSection);
  if (reflectionsSection) parts.push(reflectionsSection);

  return parts.join("\n");
}

/**
 * Calculate remaining budget for user message after system prompt.
 */
export function getUserMessageBudget(systemPrompt: string): number {
  const used = systemPrompt.length;
  // Reserve 120 chars for:
  // - JSON structure overhead (~50 chars: {"role":"system","content":"..."} + outer structure)
  // - Message formatting (~20 chars)
  // - Response generation buffer (~50 chars)
  // Be VERY conservative to avoid hitting the limit
  const remaining = FM_CONTEXT_BUDGET - used - 120;
  return Math.max(30, remaining); // At least 30 chars for task (extremely minimal)
}

/**
 * Truncate user task description to fit budget.
 */
export function truncateTaskDescription(description: string, maxChars: number): string {
  if (description.length <= maxChars) return description;

  // Find a good break point
  const truncated = description.slice(0, maxChars - 3);
  const lastSpace = truncated.lastIndexOf(" ");

  if (lastSpace > maxChars * 0.7) {
    return truncated.slice(0, lastSpace) + "...";
  }

  return truncated + "...";
}

// --- Micro-Step Decomposition ---

/**
 * Decompose a complex task into FM-sized micro-steps.
 * Each step is a single, focused action.
 */
export function decomposeTask(taskDescription: string): string[] {
  // Common decomposition patterns
  const steps: string[] = [];

  // Check for multi-file tasks
  const fileMatches = taskDescription.match(/\b[\w./]+\.(ts|js|json|txt|md)\b/g);
  if (fileMatches && fileMatches.length > 1) {
    // One step per file
    for (const file of fileMatches) {
      steps.push(`Handle file: ${file}`);
    }
    return steps;
  }

  // Check for multi-action tasks (and/then/also keywords)
  if (taskDescription.match(/\b(and|then|also|after)\b/i)) {
    // Split on conjunctions
    const parts = taskDescription.split(/\b(?:and|then|also|after)\b/i);
    for (const part of parts) {
      const trimmed = part.trim();
      if (trimmed.length > 10) {
        steps.push(trimmed);
      }
    }
    if (steps.length > 1) return steps;
  }

  // Single step task
  return [taskDescription];
}

/**
 * Build micro-step prompt for a single step in a larger task.
 */
export function buildMicroStepPrompt(
  step: string,
  stepNumber: number,
  totalSteps: number,
  context: string = "",
): string {
  let prompt = `Step ${stepNumber}/${totalSteps}: ${step}`;

  if (context) {
    prompt += `\nContext: ${context.slice(0, 100)}`;
  }

  return prompt;
}
