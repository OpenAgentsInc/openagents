/**
 * Runner-safe AssuranceSpec execution surface.
 *
 * Execution adapters need exact Manifest and Receipt contracts without pulling
 * repository handlers, CLI composition, or MCP services into their program.
 */
export * from "./artifact.ts"
export * from "./manifest.ts"
export * from "./receipt.ts"
export { sha256Digest } from "./tooling.ts"
