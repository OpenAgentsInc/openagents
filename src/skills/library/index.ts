/**
 * Skill Library Index
 *
 * Exports all skill collections for the MechaCoder learning system.
 */

import { primitiveSkills } from "./primitives.js";
import { compositionalSkills } from "./compositional.js";
import type { Skill } from "../schema.js";

/**
 * All bootstrap skills (primitives + compositional).
 * These are the foundational skills available from the start.
 */
export const bootstrapSkills: Skill[] = [...primitiveSkills, ...compositionalSkills];

/**
 * Get all skills by category.
 */
export const getSkillsByCategory = (
  category: string,
): Skill[] => bootstrapSkills.filter((s) => s.category === category);

/**
 * Get skill statistics.
 */
export const getSkillStats = (): {
  total: number;
  byCategory: Record<string, number>;
  bySource: Record<string, number>;
} => {
  const byCategory: Record<string, number> = {};
  const bySource: Record<string, number> = {};

  for (const skill of bootstrapSkills) {
    byCategory[skill.category] = (byCategory[skill.category] ?? 0) + 1;
    const source = skill.source ?? "unknown";
    bySource[source] = (bySource[source] ?? 0) + 1;
  }

  return {
    total: bootstrapSkills.length,
    byCategory,
    bySource,
  };
};

// Re-export individual collections
export { primitiveSkills } from "./primitives.js";
export { compositionalSkills } from "./compositional.js";
