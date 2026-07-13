/**
 * Build-time bundled dogfood artifact.
 *
 * `scripts/build.ts` parses the exact checked-in AssuranceSpec and replaces
 * this identifier with its bounded presentation snapshot. Future editor-opened
 * source uses the same app-owned projection boundary.
 */
import { decodeBundledAssuranceSpecProjection } from "../assurance-spec-document.ts"

declare const __OPENAGENTS_MVP_ASSURANCE_SPEC_SNAPSHOT__: string

export const bundledMvpAssuranceSpecProjection = decodeBundledAssuranceSpecProjection(
  typeof __OPENAGENTS_MVP_ASSURANCE_SPEC_SNAPSHOT__ === "string"
    ? __OPENAGENTS_MVP_ASSURANCE_SPEC_SNAPSHOT__
    : ""
)
