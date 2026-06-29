// Autopilot Concierge structured Output Spec — inference-surface re-export
// (issue #6148).
//
// The onboarding PROGRAM owns the Output Spec schema and the extraction/merge
// logic (autopilot-onboarding-program.ts), so there is ONE source of truth and
// no circular dependency (inference -> program only). This module re-exports the
// program's spec extractor under inference-surface names and owns the
// system-prompt instruction that tells the concierge model how to surface the
// structured artifact over `/api/v1/chat/completions`.
//
// THE WIRE CONTRACT
// -----------------
// The model emits the current spec as a fenced block tagged `oa-output-spec`
// containing a single JSON object whose keys are the 10 spec fields (every field
// optional; a partial spec is valid mid-interview):
//
//   ```oa-output-spec
//   {"business":"Acme LLC, …","goal":"…","quickWin":"…"}
//   ```
//
// The gateway PARSES + VALIDATES that block (against the program's typed schema)
// and surfaces the result on the response's `openagents` disclosure block as
// `output_spec`, so a non-browser consumer reads the accumulated intake state as
// a STRUCTURED field. A markdown `Output Spec` section is a bounded best-effort
// fallback. Pure; never throws.

import {
  OnboardingOutputSpec,
  ONBOARDING_OUTPUT_SPEC_FIELDS,
  OA_OUTPUT_SPEC_FENCE_TAG,
  extractOnboardingOutputSpec,
  mergeOnboardingOutputSpec,
} from '../autopilot-onboarding-program'

// Re-export the canonical schema + extractor under inference-surface names so
// the gateway depends on the program's single source of truth.
export { OnboardingOutputSpec } from '../autopilot-onboarding-program'
export type AutopilotConciergeOutputSpec = typeof OnboardingOutputSpec.Type

export const OA_OUTPUT_SPEC_FENCE_TAG_INFERENCE = OA_OUTPUT_SPEC_FENCE_TAG

export const AUTOPILOT_CONCIERGE_OUTPUT_SPEC_FIELDS =
  ONBOARDING_OUTPUT_SPEC_FIELDS

// Extract the structured Output Spec from a concierge completion. Delegates to
// the program's owner-of-truth extractor.
export const extractConciergeOutputSpec = (
  completion: string,
): AutopilotConciergeOutputSpec | undefined =>
  extractOnboardingOutputSpec(completion)

// Merge a freshly-extracted spec over a prior accumulated spec (the session spec
// only grows). Delegates to the program.
export const mergeConciergeOutputSpec = (
  prior: AutopilotConciergeOutputSpec,
  next: AutopilotConciergeOutputSpec | undefined,
): AutopilotConciergeOutputSpec => mergeOnboardingOutputSpec(prior, next)

// The system-prompt instruction telling the model to emit the structured spec
// block. Appended to the Concierge system prompt so a programmatic consumer
// reliably receives `output_spec`. Kept beside the wire contract so the prompt
// and parser can never drift.
export const AUTOPILOT_CONCIERGE_OUTPUT_SPEC_PROMPT = [
  'STRUCTURED OUTPUT SPEC (for programmatic consumers).',
  `When you have any Output Spec content, emit the current spec as a single fenced \`\`\`${OA_OUTPUT_SPEC_FENCE_TAG}\`\`\` JSON block, in addition to your normal prose.`,
  `The JSON object's keys are exactly: ${ONBOARDING_OUTPUT_SPEC_FIELDS.join(', ')}. Every key is optional; include only fields you can support from the conversation. Values are short plain strings.`,
  'Re-emit the full current spec each materially-complete turn (it accumulates). Put the block at the end of your reply. Never invent a key outside that list and never put model/provider identity inside it.',
].join(' ')
