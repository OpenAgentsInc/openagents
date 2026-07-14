import { existsSync, readFileSync, realpathSync } from "node:fs"
import { isAbsolute, relative, resolve } from "node:path"

import { Schema as S } from "effect"

import {
  beginAssuranceSession,
  checkAssuranceSession,
  getCoverageLedgers,
  runTool,
  validateAssuranceSpecFile,
} from "./handlers.ts"
import { RelativePath } from "./schema.ts"

export const OWNED_RUNNER_FORMAT_VERSION = "0.1" as const
export const OWNED_RUNNER_RECEIPT_FORMAT_VERSION = "0.1" as const

export const OwnedRunnerConfigSchema = S.Struct({
  owned_runner_format_version: S.Literal(OWNED_RUNNER_FORMAT_VERSION),
  spec_paths: S.Array(RelativePath).check(S.isMinLength(1)),
  session_pins: S.Array(S.Struct({ spec_path: RelativePath, pin_path: RelativePath })),
  validation_blocks: S.Literal(true),
  ledgers_are_informational: S.Literal(true),
  execution_authority: S.Literal("openagents_owned_runner"),
  github_hosted_ci: S.Literal(false),
})
export type OwnedRunnerConfig = typeof OwnedRunnerConfigSchema.Type

export const OwnedRunnerReceiptSchema = S.Struct({
  owned_runner_receipt_format_version: S.Literal(OWNED_RUNNER_RECEIPT_FORMAT_VERSION),
  execution_authority: S.Literal("openagents_owned_runner"),
  github_hosted_ci: S.Literal(false),
  blocking_verdict: S.Literals(["pass", "fail"]),
  validation_policy: S.Literal("structural_validation_blocks"),
  ledger_policy: S.Literal("informational_never_threshold"),
  specs: S.Array(S.Struct({
    path: RelativePath,
    structurally_valid: S.Boolean,
    subject_binding: S.NullOr(S.String),
    session_id: S.NullOr(S.String),
    traceability: S.NullOr(S.Struct({ total_criteria: S.Number, traceable_criteria: S.Number })),
    execution: S.NullOr(S.Struct({
      total_obligations: S.Number,
      executed_obligations: S.Number,
      receipt_source: S.String,
    })),
    errors: S.Array(S.Struct({ code: S.String, message: S.String })),
  })),
  session_checks: S.Array(S.Struct({
    spec_path: RelativePath,
    pin_path: RelativePath,
    status: S.String,
    blocking: S.Boolean,
  })),
})
export type OwnedRunnerReceipt = typeof OwnedRunnerReceiptSchema.Type

const decodeConfig = S.decodeUnknownSync(OwnedRunnerConfigSchema)
const decodeReceipt = S.decodeUnknownSync(OwnedRunnerReceiptSchema)

const confinedPath = (root: string, path: string): string => {
  if (isAbsolute(path) || path.split(/[\\/]/).includes("..")) throw new Error(`owned_runner_path_escape:${path}`)
  const absolute = resolve(root, path)
  if (!existsSync(absolute)) throw new Error(`owned_runner_file_not_found:${path}`)
  const realRoot = realpathSync(root)
  const real = realpathSync(absolute)
  const rel = relative(realRoot, real)
  if (rel === ".." || rel.startsWith("../") || rel.startsWith("..\\") || isAbsolute(rel)) {
    throw new Error(`owned_runner_path_escape:${path}`)
  }
  return real
}

export const parseOwnedRunnerConfig = (input: unknown): OwnedRunnerConfig => decodeConfig(input)

export const readOwnedRunnerConfig = (root: string, path: string): OwnedRunnerConfig =>
  parseOwnedRunnerConfig(JSON.parse(readFileSync(confinedPath(root, path), "utf8")))

export const runOwnedRunnerVerification = (root: string, config: OwnedRunnerConfig): OwnedRunnerReceipt => {
  const specs = config.spec_paths.map((path) => {
    const validation = runTool(validateAssuranceSpecFile({ root, path }))
    if (!validation.ok || !validation.value.valid) {
      const errors = validation.ok
        ? validation.value.errors.map((error) => ({ code: error.code, message: error.message }))
        : [{ code: validation.code, message: validation.message }]
      return {
        path, structurally_valid: false, subject_binding: null, session_id: null,
        traceability: null, execution: null, errors,
      }
    }
    const session = runTool(beginAssuranceSession({ root, path }))
    const ledgers = runTool(getCoverageLedgers({ root, path }))
    const errors = [session, ledgers].flatMap((result) => result.ok
      ? []
      : [{ code: result.code, message: result.message }])
    return {
      path,
      structurally_valid: true,
      subject_binding: session.ok ? session.value.subject_binding : null,
      session_id: session.ok ? session.value.session_id : null,
      traceability: ledgers.ok ? {
        total_criteria: ledgers.value.criterion_traceability.total_criteria,
        traceable_criteria: ledgers.value.criterion_traceability.traceable_criteria,
      } : null,
      execution: ledgers.ok ? {
        total_obligations: ledgers.value.execution.total_obligations,
        executed_obligations: ledgers.value.execution.executed_obligations,
        receipt_source: ledgers.value.execution.receipt_source,
      } : null,
      errors,
    }
  })

  const sessionChecks = config.session_pins.map(({ spec_path, pin_path }) => {
    try {
      const pin = JSON.parse(readFileSync(confinedPath(root, pin_path), "utf8"))
      const result = runTool(checkAssuranceSession({ root, path: spec_path, pin }))
      return {
        spec_path, pin_path,
        status: result.ok ? result.value.status : result.code,
        blocking: !result.ok || result.value.status !== "unchanged",
      }
    } catch (error) {
      return {
        spec_path, pin_path,
        status: error instanceof Error ? error.message : String(error),
        blocking: true,
      }
    }
  })

  const blocking = specs.some((result) => !result.structurally_valid || result.errors.length > 0) ||
    sessionChecks.some((result) => result.blocking)
  return decodeReceipt({
    owned_runner_receipt_format_version: OWNED_RUNNER_RECEIPT_FORMAT_VERSION,
    execution_authority: config.execution_authority,
    github_hosted_ci: config.github_hosted_ci,
    blocking_verdict: blocking ? "fail" : "pass",
    validation_policy: "structural_validation_blocks",
    ledger_policy: "informational_never_threshold",
    specs,
    session_checks: sessionChecks,
  })
}
