import { spawnSync } from "node:child_process"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { delimiter, dirname, resolve } from "node:path"

import { canonicalArtifact } from "./artifact.ts"
import type { AssuranceEnvironmentProfileDocument } from "./environment.ts"
import type { AssuranceExecutionUnit, AssuranceManifest } from "./manifest.ts"
import { ASSURANCE_RECEIPT_FORMAT_VERSION, type AssuranceReceipt } from "./receipt.ts"
import { sha256Digest } from "./tooling.ts"

export const OPENAGENTS_VITE_PLUS_TEST_ADAPTER_REF = "openagents.vite_plus_test.v1" as const
export const OPENAGENTS_VITE_PLUS_TEST_ADAPTER_VERSION = "1.1.0" as const

export type NodeRuntimeObservation = Readonly<{
  os: string
  architecture: string
  runtime: string
}>

export type VitePlusTestAdapterResult = Readonly<{
  receipt: AssuranceReceipt
  receiptBytes: string
  receiptDigest: string
  nativeReportPath: string
  stdoutPath: string
  stderrPath: string
  selectedTestNames: ReadonlyArray<string>
  exitCode: number | null
}>

export class VitePlusTestAdapterError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.name = "VitePlusTestAdapterError"
    this.code = code
  }
}

const xmlDecode = (value: string): string => value
  .replaceAll("&quot;", "\"")
  .replaceAll("&apos;", "'")
  .replaceAll("&lt;", "<")
  .replaceAll("&gt;", ">")
  .replaceAll("&amp;", "&")

export const inspectVitePlusJUnit = (xml: string): Readonly<{
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
  const index = argv.indexOf("--testNamePattern")
  const value = index < 0 ? undefined : argv[index + 1]
  if (value === undefined || value.trim() === "") {
    throw new VitePlusTestAdapterError("missing_test_name", "Vite Plus execution units require one exact --testNamePattern.")
  }
  return value
}

const normalizedNodeOs = (platform: NodeJS.Platform): string => platform === "darwin" ? "macos" : platform

export const observeNodeRuntime = (): NodeRuntimeObservation => ({
  os: normalizedNodeOs(process.platform),
  architecture: process.arch,
  runtime: `Node ${process.versions.node}`,
})

export const assertVitePlusRuntimeFidelity = (
  environment: AssuranceEnvironmentProfileDocument,
  observed: NodeRuntimeObservation,
): void => {
  const expected = environment.platform
  const mismatches = (["os", "architecture", "runtime"] as const).filter((field) => expected[field] !== observed[field])
  if (mismatches.length > 0) {
    const details = mismatches.map((field) => `${field}: expected ${expected[field]}, observed ${observed[field]}`).join("; ")
    throw new VitePlusTestAdapterError(
      "runtime_fidelity_mismatch",
      `Observed runtime does not match the admitted Environment Profile (${details}).`,
    )
  }
}

export const executeVitePlusTestUnit = (input: Readonly<{
  workspaceRoot: string
  runRoot: string
  manifest: AssuranceManifest
  manifestDigest: string
  environment: AssuranceEnvironmentProfileDocument
  unit: AssuranceExecutionUnit
  producerRef: string
  reviewerRef: string
  sourceDigest: string
  vitePlusExecutable?: string
}>): VitePlusTestAdapterResult => {
  if (input.unit.adapter_ref !== OPENAGENTS_VITE_PLUS_TEST_ADAPTER_REF) {
    throw new VitePlusTestAdapterError("adapter_ref_mismatch", "Execution unit is not locked to openagents.vite_plus_test.v1.")
  }
  if (input.unit.environment_ref !== input.environment.profile_id) {
    throw new VitePlusTestAdapterError("environment_ref_mismatch", "Execution unit and Environment Profile differ.")
  }
  if (!input.environment.capabilities.includes("vite_plus_test") || !input.environment.capabilities.includes("junit")) {
    throw new VitePlusTestAdapterError("missing_environment_capability", "Environment lacks vite_plus_test or junit capability.")
  }
  if (!input.environment.forbidden_actions.includes("network")) {
    throw new VitePlusTestAdapterError("environment_network_not_forbidden", "Local fixture environment must forbid network access.")
  }
  assertVitePlusRuntimeFidelity(input.environment, observeNodeRuntime())
  if (!input.environment.required_commands.includes("vp")) {
    throw new VitePlusTestAdapterError("required_command_mismatch", "Environment Profile must require the Vite Plus executable.")
  }
  if (input.unit.argv[0] !== "vp" || input.unit.argv[1] !== "test") {
    throw new VitePlusTestAdapterError("invalid_vite_plus_argv", "Adapter accepts only explicit vp test argv.")
  }
  const testName = expectedTestName(input.unit.argv)
  const nativeReportRef = input.unit.artifact_slots[0]
  if (nativeReportRef === undefined || nativeReportRef.startsWith("/") || nativeReportRef.includes("..")) {
    throw new VitePlusTestAdapterError("invalid_artifact_slot", "Execution unit requires one safe run-relative JUnit slot.")
  }
  const workspaceRoot = resolve(input.workspaceRoot)
  const runRoot = resolve(input.runRoot)
  const dependencyLockPath = resolve(workspaceRoot, input.environment.dependency_lock.path)
  let dependencyLockBytes: string
  try {
    dependencyLockBytes = readFileSync(dependencyLockPath, "utf8")
  } catch {
    throw new VitePlusTestAdapterError("dependency_lock_unavailable", "Admitted dependency lock could not be read.")
  }
  if (sha256Digest(dependencyLockBytes) !== input.environment.dependency_lock.digest) {
    throw new VitePlusTestAdapterError("dependency_lock_mismatch", "Observed dependency lock differs from the admitted Environment Profile.")
  }
  const vitePlusEntrypoint = resolve(input.vitePlusExecutable ?? resolve(workspaceRoot, "node_modules/vite-plus/bin/vp"))
  let vitePlusEntrypointBytes: string
  try {
    vitePlusEntrypointBytes = readFileSync(vitePlusEntrypoint, "utf8")
  } catch {
    throw new VitePlusTestAdapterError("vite_plus_entrypoint_unavailable", "Vite Plus entrypoint could not be read.")
  }
  const nativeReportPath = resolve(runRoot, `${input.unit.role}.junit.xml`)
  const stdoutPath = resolve(runRoot, `${input.unit.role}.stdout.txt`)
  const stderrPath = resolve(runRoot, `${input.unit.role}.stderr.txt`)
  mkdirSync(dirname(nativeReportPath), { recursive: true })

  const argv = [...input.unit.argv.slice(1), "--run", "--reporter=junit", "--outputFile", nativeReportPath]
  const result = spawnSync(process.execPath, [vitePlusEntrypoint, ...argv], {
    cwd: workspaceRoot,
    encoding: "utf8",
    env: {
      PATH: [dirname(process.execPath), process.env.PATH ?? ""].filter((entry) => entry !== "").join(delimiter),
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
  const inspection = inspectVitePlusJUnit(nativeBytes)
  const selectedName = inspection.unskippedNames[0]
  const selectedExactlyOne = inspection.unskippedNames.length === 1 && (
    selectedName === testName || selectedName?.endsWith(` > ${testName}`) === true
  )
  const oraclePassed = result.status === 0 && inspection.failed === 0
  const oracleRefuted = result.status !== null && result.status !== 0 && inspection.failed > 0
  const infrastructureReady = nativeBytes !== "" && selectedExactlyOne && (oraclePassed || oracleRefuted)
  const observation = !infrastructureReady
    ? "INCONCLUSIVE" as const
    : oraclePassed
      ? input.unit.expected_observation
      : input.unit.expected_observation === "CONFIRMED" ? "REFUTED" as const : "CONFIRMED" as const
  const criterionRefs = input.manifest.obligation_graph.find((entry) =>
    entry.obligation_id === input.unit.obligation_id)?.criterion_refs ?? []
  const commandDigest = sha256Digest(JSON.stringify({
    argv: input.unit.argv,
    adapter: OPENAGENTS_VITE_PLUS_TEST_ADAPTER_REF,
    adapter_version: OPENAGENTS_VITE_PLUS_TEST_ADAPTER_VERSION,
    node_version: process.versions.node,
    vite_plus_entrypoint_digest: sha256Digest(vitePlusEntrypointBytes),
    dependency_lock_digest: input.environment.dependency_lock.digest,
  }))
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
