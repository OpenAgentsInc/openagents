import { spawnSync } from "node:child_process"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname, resolve } from "node:path"

import { canonicalArtifact } from "./artifact.ts"
import type { AssuranceEnvironmentProfileDocument } from "./environment.ts"
import type { AssuranceExecutionUnit, AssuranceManifest } from "./manifest.ts"
import { ASSURANCE_RECEIPT_FORMAT_VERSION, type AssuranceReceipt } from "./receipt.ts"
import { sha256Digest } from "./tooling.ts"

export const OPENAGENTS_BUN_TEST_ADAPTER_REF = "openagents.bun_test.v1" as const
export const OPENAGENTS_BUN_TEST_ADAPTER_VERSION = "1.0.0" as const

export type BunTestAdapterResult = Readonly<{
  receipt: AssuranceReceipt
  receiptBytes: string
  receiptDigest: string
  nativeReportPath: string
  stdoutPath: string
  stderrPath: string
  selectedTestNames: ReadonlyArray<string>
  exitCode: number | null
}>

export class BunTestAdapterError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.name = "BunTestAdapterError"
    this.code = code
  }
}

const xmlDecode = (value: string): string => value
  .replaceAll("&quot;", "\"")
  .replaceAll("&apos;", "'")
  .replaceAll("&lt;", "<")
  .replaceAll("&gt;", ">")
  .replaceAll("&amp;", "&")

export const inspectBunJUnit = (xml: string): Readonly<{
  total: number
  skipped: number
  failed: number
  unskippedNames: ReadonlyArray<string>
}> => {
  const cases = [...xml.matchAll(/<testcase\b([^>]*?)(?:\/>|>([\s\S]*?)<\/testcase>)/g)]
  const rows = cases.map((match) => {
    const attributes = match[1] ?? ""
    const body = match[2] ?? ""
    const name = attributes.match(/\bname="([^"]*)"/)?.[1] ?? ""
    return {
      name: xmlDecode(name),
      skipped: /<skipped\b/.test(body),
      failed: /<(?:failure|error)\b/.test(body),
    }
  })
  return {
    total: rows.length,
    skipped: rows.filter((row) => row.skipped).length,
    failed: rows.filter((row) => row.failed).length,
    unskippedNames: rows.filter((row) => !row.skipped).map((row) => row.name),
  }
}

const expectedTestName = (argv: ReadonlyArray<string>): string => {
  const index = argv.indexOf("--test-name-pattern")
  const value = index < 0 ? undefined : argv[index + 1]
  if (value === undefined || value.trim() === "") {
    throw new BunTestAdapterError("missing_test_name", "Bun execution units require one exact --test-name-pattern.")
  }
  return value
}

export const executeBunTestUnit = (input: Readonly<{
  workspaceRoot: string
  runRoot: string
  manifest: AssuranceManifest
  manifestDigest: string
  environment: AssuranceEnvironmentProfileDocument
  unit: AssuranceExecutionUnit
  producerRef: string
  reviewerRef: string
  sourceDigest: string
  bunExecutable?: string
}>): BunTestAdapterResult => {
  if (input.unit.adapter_ref !== OPENAGENTS_BUN_TEST_ADAPTER_REF) {
    throw new BunTestAdapterError("adapter_ref_mismatch", "Execution unit is not locked to openagents.bun_test.v1.")
  }
  if (input.unit.environment_ref !== input.environment.profile_id) {
    throw new BunTestAdapterError("environment_ref_mismatch", "Execution unit and Environment Profile differ.")
  }
  if (!input.environment.capabilities.includes("bun_test") || !input.environment.capabilities.includes("junit")) {
    throw new BunTestAdapterError("missing_environment_capability", "Environment lacks bun_test or junit capability.")
  }
  if (!input.environment.forbidden_actions.includes("network")) {
    throw new BunTestAdapterError("environment_network_not_forbidden", "Local fixture environment must forbid network access.")
  }
  if (input.unit.argv[0] !== "bun" || input.unit.argv[1] !== "test") {
    throw new BunTestAdapterError("invalid_bun_argv", "Adapter accepts only explicit bun test argv.")
  }
  const testName = expectedTestName(input.unit.argv)
  const nativeReportRef = input.unit.artifact_slots[0]
  if (nativeReportRef === undefined || nativeReportRef.startsWith("/") || nativeReportRef.includes("..")) {
    throw new BunTestAdapterError("invalid_artifact_slot", "Execution unit requires one safe run-relative JUnit slot.")
  }
  const workspaceRoot = resolve(input.workspaceRoot)
  const runRoot = resolve(input.runRoot)
  const nativeReportPath = resolve(runRoot, `${input.unit.role}.junit.xml`)
  const stdoutPath = resolve(runRoot, `${input.unit.role}.stdout.txt`)
  const stderrPath = resolve(runRoot, `${input.unit.role}.stderr.txt`)
  mkdirSync(dirname(nativeReportPath), { recursive: true })

  const argv = [...input.unit.argv.slice(1), "--reporter=junit", "--reporter-outfile", nativeReportPath]
  const result = spawnSync(input.bunExecutable ?? process.execPath, argv, {
    cwd: workspaceRoot,
    encoding: "utf8",
    env: {
      PATH: process.env.PATH ?? "",
      HOME: resolve(runRoot, "isolated-home"),
      TMPDIR: resolve(runRoot, "tmp"),
      NO_COLOR: "1",
      CI: "1",
    },
    shell: false,
    maxBuffer: 16 * 1024 * 1024,
  })
  writeFileSync(stdoutPath, result.stdout ?? "", "utf8")
  writeFileSync(stderrPath, result.stderr ?? "", "utf8")

  let nativeBytes = ""
  try {
    nativeBytes = readFileSync(nativeReportPath, "utf8")
  } catch {
    // Partial stdout/stderr remains retained; missing JUnit is never green.
  }
  const inspection = inspectBunJUnit(nativeBytes)
  const selectedExactlyOne = inspection.unskippedNames.length === 1 && inspection.unskippedNames[0] === testName
  const infrastructureReady = result.status === 0 && nativeBytes !== "" && selectedExactlyOne && inspection.failed === 0
  const observation = infrastructureReady
    ? input.unit.role === "candidate" ? "CONFIRMED" as const : "REFUTED" as const
    : "INCONCLUSIVE" as const
  const criterionRefs = input.manifest.obligation_graph.find((entry) =>
    entry.obligation_id === input.unit.obligation_id)?.criterion_refs ?? []
  const commandDigest = sha256Digest(JSON.stringify({ argv: input.unit.argv, adapter: OPENAGENTS_BUN_TEST_ADAPTER_REF }))
  const nativeDigest = sha256Digest(nativeBytes)
  const receiptSeed = {
    manifest_digest: input.manifestDigest,
    unit_ref: input.unit.unit_ref,
    native_report_digest: nativeDigest,
    command_digest: commandDigest,
  }
  const receipt: AssuranceReceipt = {
    assurance_receipt_format_version: ASSURANCE_RECEIPT_FORMAT_VERSION,
    receipt_ref: `assurance.receipt.${sha256Digest(JSON.stringify(receiptSeed)).slice("sha256:".length)}`,
    manifest_digest: input.manifestDigest,
    product_spec_digest: input.manifest.product_spec.document_digest,
    assurance_spec_digest: input.manifest.assurance_spec.document_digest,
    admission_digest: input.manifest.admission.digest,
    obligation_id: input.unit.obligation_id,
    criterion_refs: [...criterionRefs],
    environment_ref: input.unit.environment_ref,
    adapter_ref: input.unit.adapter_ref,
    execution_unit_ref: input.unit.unit_ref,
    producer_ref: input.producerRef,
    reviewer_ref: input.reviewerRef,
    native_report_ref: nativeReportRef,
    native_report_digest: nativeDigest,
    command_digest: commandDigest,
    source_digest: input.sourceDigest,
    axes: {
      admission: "admitted",
      readiness: infrastructureReady ? "executable" : "blocked",
      observation,
      infrastructure: infrastructureReady ? "ready" : result.error === undefined ? "failed" : "unavailable",
      stability: infrastructureReady ? "stable" : "unknown",
      freshness: "current",
      disposition: "pending_review",
      exception: "none",
    },
    public_safety: { classification: "private", contains_raw_output: false },
  }
  const receiptArtifact = canonicalArtifact(receipt)
  return {
    receipt,
    receiptBytes: receiptArtifact.bytes,
    receiptDigest: receiptArtifact.digest,
    nativeReportPath,
    stdoutPath,
    stderrPath,
    selectedTestNames: inspection.unskippedNames,
    exitCode: result.status,
  }
}
