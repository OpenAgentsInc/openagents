/**
 * Effuse Template Types
 *
 * Type definitions for the html tagged template system.
 */

import type { Effect } from "effect"

/**
 * Result of an html`` template literal.
 * Contains the template parts and can render to escaped HTML.
 */
export interface TemplateResult {
  readonly _tag: "TemplateResult"
  readonly strings: TemplateStringsArray
  readonly values: readonly unknown[]
  /** Render to escaped HTML string */
  toString(): string
}

/**
 * Values that can be interpolated in html`` templates.
 */
export type TemplateValue =
  | string
  | number
  | boolean
  | null
  | undefined
  | TemplateResult
  | Effect.Effect<TemplateResult, unknown, unknown>
  | TemplateValue[]

/**
 * Type guard for TemplateResult
 */
export const isTemplateResult = (value: unknown): value is TemplateResult =>
  typeof value === "object" &&
  value !== null &&
  "_tag" in value &&
  (value as { _tag: unknown })._tag === "TemplateResult"
