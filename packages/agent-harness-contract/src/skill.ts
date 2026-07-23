import { Schema as S } from "effect";

/**
 * A skill as runtime-neutral data. Adapters decide how to surface it: Claude
 * Code materializes real skill directories, an ACP peer injects the content
 * differently. The contract only carries the payload.
 */
export const HarnessSkillFile = S.Struct({
  /** Repo-relative path the file is written to when the adapter materializes the skill. */
  path: S.NonEmptyString,
  /** UTF-8 text content for the file. */
  content: S.String,
});
export interface HarnessSkillFile extends S.Schema.Type<typeof HarnessSkillFile> {}

export const HarnessSkill = S.Struct({
  /** Stable identifier for the skill (kebab-case slug). */
  name: S.NonEmptyString,
  /** One-line summary used to decide relevance. */
  description: S.String,
  /** Full skill content the model loads when the skill is active. */
  content: S.String,
  /** Optional attached files the skill references. */
  files: S.optionalKey(S.Array(HarnessSkillFile)),
});
export interface HarnessSkill extends S.Schema.Type<typeof HarnessSkill> {}

export const decodeHarnessSkill = S.decodeUnknownSync(HarnessSkill);
