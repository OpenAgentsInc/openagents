#!/usr/bin/env node
import { spawnSync } from "node:child_process"
import { existsSync, mkdirSync, realpathSync } from "node:fs"
import { access, readFile, writeFile } from "node:fs/promises"
/**
 * assurance-spec CLI (AT-1, docs/assurance/AGENT_TOOLING.md §2).
 *
 * Exit codes are the API: 0 success, 1 operation failure, 2 usage error,
 * 3 stale session. Every command takes --json for machine output; human
 * output stays terse. The new commands run the exact Effect handlers the MCP
 * server uses (src/handlers.ts): one implementation, two transports.
 */
import { resolve, relative } from "node:path"

/** Resolve and realpath so macOS /tmp vs /private/tmp never invents `..` refs. */
const realResolve = (path: string): string => {
  const absolute = resolve(path)
  try {
    return realpathSync(absolute)
  } catch {
    return absolute
  }
}

import { Effect } from "effect"

import {
  ASSURANCE_SPEC_EXTENSION,
  assessAssuranceSpec,
  beginAssuranceSession,
  checkAssuranceSession,
  checkCompletionClaim,
  getCoverageLedgers,
  getEvidenceChecklist,
  getObligation,
  getObligationGraph,
  getObligations,
  getRepositoryInventory,
  ingestAgentRun,
  inventoryRepository,
  parseAssuranceSpec,
  prepareSemanticPlannerInput,
  proposeAssuranceSpec,
  readOwnedRunnerConfig,
  runAssuranceSpecMcpServer,
  runOwnedRunnerVerification,
  runSemanticPlannerProposal,
  runTool,
  validateAssuranceSpec,
  fixtureSemanticPlanner,
  decideReviewAdmission,
  buildAuthorityDecisionReceipt,
  admitAssuranceFrontmatter,
  authorityDecisionReceiptArtifact,
  batchCommandString,
  sha256Digest,
  type AssuranceToolError,
  type ProductSpecSubject,
  type ToolFailure,
  type OracleBatch,
  type BatchReproduction,
} from "./index.ts"

const file = (path: string) => ({
  exists: async (): Promise<boolean> => access(path).then(() => true, () => false),
  text: (): Promise<string> => readFile(path, "utf8"),
})

const usage = (): never => {
  console.error("usage:")
  console.error("  assurance-spec propose <file.product-spec.md> [--repo <dir>] [--out <file.assurance-spec.md>] [--inventory-out <file.json>] [--id <id>] [--title <title>] [--author <author>] [--force] [--json]")
  console.error("  assurance-spec observer propose <file.product-spec.md> --accepted-subject <pin.json> --planner fixture [--repo <dir>] [--out <file.assurance-spec.md>] [--id <id>] [--title <title>] [--author <author>] [--force] [--json]")
  console.error("  assurance-spec validate <file.assurance-spec.md> [...] [--json]")
  console.error("  assurance-spec coverage <file.assurance-spec.md> [--json]")
  console.error("  assurance-spec session begin <file.assurance-spec.md> [--root <dir>] [--json]")
  console.error("  assurance-spec session check <file.assurance-spec.md> (--against <session.json> | --spec-digest <hex> --subject-digest <hex>) [--root <dir>] [--json]")
  console.error("  assurance-spec inventory <repo-dir> [--out <file.json>] [--json]")
  console.error("  assurance-spec obligations <file.assurance-spec.md> [--criterion <id>] [--status ready|needs_design] [--technique <t>] [--root <dir>] [--json]")
  console.error("  assurance-spec obligation <file.assurance-spec.md> <obligation-id> [--root <dir>] [--json]")
  console.error("  assurance-spec graph <file.assurance-spec.md> [--root <dir>] [--json]")
  console.error("  assurance-spec ledgers <file.assurance-spec.md> [--root <dir>] [--json]")
  console.error("  assurance-spec checklist <file.assurance-spec.md> [--criterion <id>] [--root <dir>] [--json]")
  console.error("  assurance-spec claim <file.assurance-spec.md> [--claim <text>] [--root <dir>] [--json]")
  console.error("  assurance-spec agent-run ingest <file.agent-run.json> [--root <dir>] [--json]")
  console.error("  assurance-spec owned-runner <assurance/owned-runner.json> [--root <dir>] [--json]")
  console.error("  assurance-spec review-admit <file.assurance-spec.md> --reviewer <ref> --producer <ref> [--root <dir>] [--program <ref>] [--trigger <ref>] [--receipts-dir <dir>] [--evidence <path>]... [--scope-note <text>]... [--dry-run] [--json]")
  console.error("  assurance-spec mcp [--root <dir>]")
  console.error("")
  console.error("exit codes: 0 success · 1 operation failure · 2 usage error · 3 stale session")
  console.error("paths for session/obligation/ledger/checklist/claim commands resolve inside --root (default: current directory)")
  process.exit(2)
}

const flagValue = (args: ReadonlyArray<string>, flag: string): string | undefined => {
  const index = args.indexOf(flag)
  return index === -1 ? undefined : args[index + 1]
}

const jsonFlag = (args: ReadonlyArray<string>): boolean => args.includes("--json")

const printJson = (value: unknown): void => {
  console.log(JSON.stringify(value, null, 2))
}

const failWith = (failure: ToolFailure, json: boolean): never => {
  if (json) printJson(failure)
  else console.error(`${failure.code}: ${failure.message}`)
  process.exit(failure.code === "invalid_argument" ? 2 : 1)
}

const runOrExit = <A>(program: Effect.Effect<A, AssuranceToolError>, json: boolean): A => {
  const outcome = runTool(program)
  if (!outcome.ok) return failWith(outcome, json)
  return outcome.value
}

const positional = (args: ReadonlyArray<string>, count: number): ReadonlyArray<string> => {
  const found: string[] = []
  const flagsWithValues = new Set([
    "--repo", "--out", "--inventory-out", "--id", "--title", "--author",
    "--accepted-subject", "--planner",
    "--root", "--against", "--spec-digest", "--subject-digest",
    "--criterion", "--status", "--technique", "--claim",
    "--reviewer", "--producer", "--trigger", "--receipts-dir", "--program",
    "--evidence", "--scope-note",
  ])
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]!
    if (flagsWithValues.has(arg)) {
      index += 1
      continue
    }
    if (arg.startsWith("--")) continue
    found.push(arg)
    if (found.length === count) break
  }
  return found
}

const rootArg = (args: ReadonlyArray<string>): { root?: string } => {
  const root = flagValue(args, "--root")
  return root === undefined ? {} : { root }
}

// ---------------------------------------------------------------------------
// Existing commands (propose / validate / coverage)
// ---------------------------------------------------------------------------

const defaultOutput = (input: string): string => input.endsWith(".product-spec.md")
  ? `${input.slice(0, -".product-spec.md".length)}${ASSURANCE_SPEC_EXTENSION}`
  : `${input}${ASSURANCE_SPEC_EXTENSION}`

const propose = async (args: ReadonlyArray<string>): Promise<void> => {
  const input = args[0]
  if (input === undefined || input.startsWith("--")) usage()
  const json = jsonFlag(args)
  const inputAbsolute = resolve(input)
  if (!(await file(inputAbsolute).exists())) {
    console.error(`ProductSpec does not exist: ${input}`)
    process.exit(1)
  }
  const repoFlag = flagValue(args, "--repo")
  const repositoryRoot = repoFlag === undefined ? undefined : resolve(repoFlag)
  const base = repositoryRoot ?? process.cwd()
  const productSpecPath = relative(base, inputAbsolute).replaceAll("\\", "/")
  const output = resolve(flagValue(args, "--out") ?? defaultOutput(input))
  if (await file(output).exists() && !args.includes("--force")) {
    console.error(`refusing to overwrite existing file: ${output}`)
    process.exit(1)
  }
  const repositoryInventory = repositoryRoot === undefined ? undefined : inventoryRepository(repositoryRoot)
  const result = proposeAssuranceSpec({
    productSpecPath,
    productSpecMarkdown: await file(inputAbsolute).text(),
    ...(repositoryInventory === undefined ? {} : { repositoryInventory }),
    ...(flagValue(args, "--id") === undefined ? {} : { assuranceSpecId: flagValue(args, "--id")! }),
    ...(flagValue(args, "--title") === undefined ? {} : { title: flagValue(args, "--title")! }),
    ...(flagValue(args, "--author") === undefined ? {} : { author: flagValue(args, "--author")! }),
  })
  if (!result.ok) {
    if (json) printJson({ ok: false, diagnostics: result.diagnostics })
    else {
      console.error("AssuranceSpec proposal failed.")
      for (const diagnostic of result.diagnostics) console.error(`  ${diagnostic.code}: ${diagnostic.message}`)
    }
    process.exit(1)
  }
  await writeFile(output, result.markdown)
  const inventoryOut = flagValue(args, "--inventory-out")
  if (inventoryOut !== undefined) {
    await writeFile(resolve(inventoryOut), `${JSON.stringify(result.document.environments.repository_inventory, null, 2)}\n`)
  }
  if (json) {
    printJson({ ok: true, output, adequacy: result.adequacy })
    return
  }
  console.log(`proposed ${output}`)
  console.log(`  ${result.adequacy.coverage.obligations} obligations · ${result.adequacy.coverage.needs_design} need design · ${result.adequacy.coverage.ready} ready`)
  console.log(`  repository ${result.document.environments.repository_inventory.state} · structural valid · design ready ${result.adequacy.design_ready ? "yes" : "no"} · execution authorized no`)
}

/** Provider-free CLI exercise of the same injected semantic planner boundary. */
const observerPropose = async (args: ReadonlyArray<string>): Promise<void> => {
  const input = args[0]
  const acceptedSubjectPath = flagValue(args, "--accepted-subject")
  const plannerName = flagValue(args, "--planner")
  if (input === undefined || input.startsWith("--") || acceptedSubjectPath === undefined || plannerName === undefined) usage()
  if (plannerName !== "fixture") {
    console.error(`unsupported semantic planner: ${plannerName}; CLI supports only the deterministic fixture planner`)
    process.exit(2)
  }
  const json = jsonFlag(args)
  const inputAbsolute = resolve(input)
  const subjectFile = file(resolve(acceptedSubjectPath!))
  if (!(await file(inputAbsolute).exists()) || !(await subjectFile.exists())) {
    console.error("ProductSpec and accepted-subject pin must both exist.")
    process.exit(1)
  }
  let acceptedSubject: unknown
  try {
    acceptedSubject = JSON.parse(await subjectFile.text())
  } catch {
    console.error("invalid_semantic_planner_input: accepted-subject pin is not valid JSON")
    process.exit(1)
  }
  const repoFlag = flagValue(args, "--repo")
  const repositoryRoot = repoFlag === undefined ? undefined : resolve(repoFlag)
  const prepared = prepareSemanticPlannerInput({
    acceptedSubject: acceptedSubject as ProductSpecSubject,
    productSpecPath: relative(repositoryRoot ?? process.cwd(), inputAbsolute).replaceAll("\\", "/"),
    productSpecMarkdown: await file(inputAbsolute).text(),
    ...(repositoryRoot === undefined ? {} : { repositoryInventory: inventoryRepository(repositoryRoot) }),
  })
  if (!prepared.ok) {
    if (json) printJson({ ok: false, diagnostics: prepared.diagnostics })
    else for (const entry of prepared.diagnostics) console.error(`${entry.code}: ${entry.message}`)
    process.exit(1)
  }
  const result = Effect.runSync(runSemanticPlannerProposal(prepared.input, fixtureSemanticPlanner, {
    author: flagValue(args, "--author") ?? "Observer deterministic fixture planner",
    ...(flagValue(args, "--id") === undefined ? {} : { assuranceSpecId: flagValue(args, "--id")! }),
    ...(flagValue(args, "--title") === undefined ? {} : { title: flagValue(args, "--title")! }),
  }))
  if (!result.ok) {
    if (json) printJson({ ok: false, diagnostics: result.diagnostics })
    else for (const entry of result.diagnostics) console.error(`${entry.code}: ${entry.message}`)
    process.exit(1)
  }
  const output = resolve(flagValue(args, "--out") ?? defaultOutput(input))
  if (await file(output).exists() && !args.includes("--force")) {
    console.error(`refusing to overwrite existing file: ${output}`)
    process.exit(1)
  }
  await writeFile(output, result.markdown)
  if (json) {
    printJson({
      ok: true,
      output,
      planner_input_digest: result.plannerInput.input_digest,
      lifecycle_state: result.document.frontmatter.lifecycle_state,
      execution_authorized: false,
      adequacy: result.adequacy,
    })
  } else {
    console.log(`proposed ${output}`)
    console.log(`  semantic planner fixture · input ${result.plannerInput.input_digest}`)
    console.log(`  lifecycle proposed · ${result.adequacy.coverage.needs_design} need design · execution authorized no`)
  }
}

const validate = async (args: ReadonlyArray<string>): Promise<void> => {
  const json = jsonFlag(args)
  const paths = args.filter((arg) => !arg.startsWith("--"))
  if (paths.length === 0) usage()
  let failures = 0
  const results: Array<unknown> = []
  for (const path of paths) {
    const result = validateAssuranceSpec(await file(path).text())
    results.push({ path, valid: result.valid, errors: result.errors, warnings: result.warnings })
    if (result.valid) {
      if (!json) console.log(`ok ${path}`)
    } else {
      failures += 1
      if (!json) {
        console.error(`FAIL ${path}`)
        for (const error of result.errors) console.error(`  ${error.code}: ${error.message}`)
      }
    }
  }
  if (json) printJson(results)
  if (failures > 0) process.exit(1)
}

const coverage = async (args: ReadonlyArray<string>): Promise<void> => {
  const path = args[0]
  if (path === undefined || path.startsWith("--")) usage()
  const validation = validateAssuranceSpec(await file(path).text())
  if (!validation.valid || validation.document === undefined) {
    for (const error of validation.errors) console.error(`${error.code}: ${error.message}`)
    process.exit(1)
  }
  const assessment = assessAssuranceSpec(parseAssuranceSpec(await file(path).text()))
  if (jsonFlag(args)) {
    printJson(assessment)
    return
  }
  console.log(`${path}: ${assessment.coverage.ready}/${assessment.coverage.obligations} obligations ready; ${assessment.coverage.needs_design} need design`)
  for (const diagnostic of assessment.diagnostics) {
    console.log(`  ${diagnostic.severity} ${diagnostic.code}: ${diagnostic.message}`)
  }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

const sessionBegin = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const pin = runOrExit(beginAssuranceSession({ path, ...rootArg(args) }), json)
  if (json) {
    printJson(pin)
    return
  }
  console.log(`pinned ${pin.assurance_spec.path}@r${pin.assurance_spec.revision} ${pin.assurance_spec.document_digest}`)
  console.log(`  subject ${pin.subject.path}@r${pin.subject.revision} ${pin.subject.document_digest} (${pin.subject_binding})`)
  console.log(`  session ${pin.session_id} · ${pin.criterion_refs.length} criteria · stateless (store this pin; use --json to capture it)`)
}

const sessionCheck = async (args: ReadonlyArray<string>): Promise<void> => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const against = flagValue(args, "--against")
  const specDigest = flagValue(args, "--spec-digest")
  const subjectDigest = flagValue(args, "--subject-digest")
  if (against === undefined && (specDigest === undefined || subjectDigest === undefined)) usage()
  let pin: unknown
  if (against !== undefined) {
    const againstFile = file(resolve(against))
    if (!(await againstFile.exists())) {
      return failWith({ ok: false, code: "file_not_found", message: `Session file does not exist: ${against}` }, json)
    }
    try {
      pin = JSON.parse(await againstFile.text())
    } catch {
      return failWith({ ok: false, code: "invalid_session_pin", message: `Session file is not valid JSON: ${against}` }, json)
    }
  }
  const check = runOrExit(
    checkAssuranceSession({
      path,
      ...rootArg(args),
      ...(pin === undefined ? {} : { pin }),
      ...(specDigest === undefined ? {} : { spec_digest: specDigest }),
      ...(subjectDigest === undefined ? {} : { subject_digest: subjectDigest }),
    }),
    json,
  )
  if (json) printJson(check)
  else {
    console.log(`status ${check.status} · ${check.recommended_action}`)
    console.log(`  assurance_spec changed=${check.assurance_spec.changed} pinned=${check.assurance_spec.pinned_digest} current=${check.assurance_spec.current_digest ?? "-"}`)
    console.log(`  subject changed=${check.subject.changed} pinned=${check.subject.pinned_digest} current=${check.subject.current_digest ?? "-"}`)
    for (const error of check.errors) console.log(`  ${error.code}: ${error.message}`)
  }
  if (check.status !== "unchanged") process.exit(3)
}

// ---------------------------------------------------------------------------
// Inventory, obligations, ledgers, checklist, claim
// ---------------------------------------------------------------------------

const inventory = async (args: ReadonlyArray<string>): Promise<void> => {
  const [repoDir] = positional(args, 1)
  if (repoDir === undefined) usage()
  const json = jsonFlag(args)
  const report = runOrExit(getRepositoryInventory({ root: repoDir }), json)
  const out = flagValue(args, "--out")
  if (out !== undefined) await writeFile(resolve(out), `${JSON.stringify(report, null, 2)}\n`)
  if (json) {
    printJson(report)
    return
  }
  console.log(`repository ${report.repository_label}: ${report.state} · ${report.tracked_file_count} tracked files`)
  console.log(`  ${report.candidate_artifact_refs.length} candidate artifacts · ${report.declared_scripts.length} declared scripts · candidates are not proof`)
  for (const diagnostic of report.diagnostics) console.log(`  ${diagnostic}`)
}

const obligations = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const criterion = flagValue(args, "--criterion")
  const status = flagValue(args, "--status")
  const technique = flagValue(args, "--technique")
  const summaries = runOrExit(
    getObligations({
      path,
      ...rootArg(args),
      ...(criterion === undefined ? {} : { criterion_ref: criterion }),
      ...(status === undefined ? {} : { status }),
      ...(technique === undefined ? {} : { technique }),
    }),
    json,
  )
  if (json) {
    printJson(summaries)
    return
  }
  for (const summary of summaries) {
    console.log(`${summary.id} ${summary.design_status} [${summary.criterion_refs.join(",")}] technique=${summary.technique ?? "-"} environments=${summary.environment_refs.join(",") || "-"}`)
  }
  console.log(`${summaries.length} obligations`)
}

const obligation = (args: ReadonlyArray<string>): void => {
  const [path, obligationId] = positional(args, 2)
  if (path === undefined || obligationId === undefined) usage()
  const json = jsonFlag(args)
  const detail = runOrExit(getObligation({ path, obligation_id: obligationId, ...rootArg(args) }), json)
  if (json) {
    printJson(detail)
    return
  }
  console.log(`${detail.obligation.id}: ${detail.obligation.title} (${detail.design_status})`)
  console.log(`  criteria ${detail.obligation.criterion_refs.join(", ")}`)
  console.log(`  oracle ${detail.obligation.oracle?.statement ?? "unresolved"}`)
  console.log(`  falsifier ${detail.obligation.falsifier?.ref ?? "unresolved"}`)
  if (detail.unresolved_fields.length > 0) console.log(`  unresolved: ${detail.unresolved_fields.join(", ")}`)
}

const graph = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const report = runOrExit(getObligationGraph({ path, ...rootArg(args) }), json)
  if (json) {
    printJson(report)
    return
  }
  console.log(`designable_now (${report.designable_now.length}): ${report.designable_now.join(", ") || "-"}`)
  for (const entry of report.blocked) {
    console.log(`blocked ${entry.obligation_id} waits_on ${entry.waits_on.join(", ")}`)
  }
  for (const entry of report.gated) {
    console.log(`gated ${entry.obligation_id} gate=${entry.activation_gate}`)
  }
  console.log(`design_order: ${report.design_order.join(" -> ") || "-"}`)
  console.log(report.message)
}

const ledgers = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const evidenceIndex = flagValue(args, "--evidence-index")
  const report = runOrExit(getCoverageLedgers({ path, ...rootArg(args), ...(evidenceIndex === undefined ? {} : { evidence_index_path: evidenceIndex }) }), json)
  if (json) {
    printJson(report)
    return
  }
  console.log(`traceability ${report.criterion_traceability.traceable_criteria}/${report.criterion_traceability.total_criteria} criteria bound to obligations`)
  console.log(`execution ${report.execution.executed_obligations}/${report.execution.total_obligations} obligations executed (receipt source: ${report.execution.receipt_source})`)
  console.log(`frontier ${report.reachable_frontier.status}: ${report.reachable_frontier.reason}`)
}

const checklist = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const criterion = flagValue(args, "--criterion")
  const evidenceIndex = flagValue(args, "--evidence-index")
  const report = runOrExit(
    getEvidenceChecklist({ path, ...rootArg(args), ...(criterion === undefined ? {} : { criterion_ref: criterion }), ...(evidenceIndex === undefined ? {} : { evidence_index_path: evidenceIndex }) }),
    json,
  )
  if (json) {
    printJson(report)
    return
  }
  for (const entry of report.criteria) {
    console.log(entry.criterion_ref)
    for (const bound of entry.obligations) {
      const kinds = bound.required_kinds.length > 0 ? bound.required_kinds.join(",") : "undesigned"
      console.log(`  ${bound.obligation_id} evidence=${bound.evidence_state} kinds=${kinds} missing=${bound.missing.length} gaps=${bound.gaps.length}`)
    }
  }
}

const claim = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const claimText = flagValue(args, "--claim")
  const evidenceIndex = flagValue(args, "--evidence-index")
  const audit = runOrExit(
    checkCompletionClaim({ path, ...rootArg(args), ...(claimText === undefined ? {} : { claim: claimText }), ...(evidenceIndex === undefined ? {} : { evidence_index_path: evidenceIndex }) }),
    json,
  )
  if (json) {
    printJson(audit)
    return
  }
  console.log(`claim: ${audit.claim ?? "(none)"} · admission ${audit.admission_state} · subject ${audit.subject_binding}`)
  for (const entry of audit.obligations) {
    const axes = entry.axes
    console.log(`  ${entry.obligation_id} admission=${axes.admission} readiness=${axes.readiness} observation=${axes.observation} infrastructure=${axes.infrastructure} stability=${axes.stability} freshness=${axes.freshness} disposition=${axes.disposition} exception=${axes.exception}`)
  }
  console.log(audit.message)
}

const agentRunIngest = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const evidence = runOrExit(ingestAgentRun({ path, ...rootArg(args) }), json)
  if (json) {
    printJson(evidence)
    return
  }
  console.log(`${evidence.run_id}: ${evidence.run_status} · proof rung ${evidence.proof_rung}`)
  console.log(`  ProductSpec ${evidence.spec_pin.path}@r${evidence.spec_pin.spec_revision} · digest ${evidence.spec_pin.digest_status}`)
  console.log(`  ${evidence.claimed_items.length} claimed item statuses · producer == claimant · observation not promoted`)
  for (const gap of evidence.gaps) console.log(`  gap ${gap.code}: ${gap.message}`)
}

const ownedRunner = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const root = resolve(flagValue(args, "--root") ?? process.cwd())
  try {
    const receipt = runOwnedRunnerVerification(root, readOwnedRunnerConfig(root, path))
    if (json) printJson(receipt)
    else {
      console.log(`owned runner ${receipt.blocking_verdict} · ${receipt.specs.length} AssuranceSpecs`)
      for (const spec of receipt.specs) {
        console.log(`  ${spec.path} structural=${spec.structurally_valid} traceable=${spec.traceability?.traceable_criteria ?? "-"}/${spec.traceability?.total_criteria ?? "-"} executed=${spec.execution?.executed_obligations ?? "-"}/${spec.execution?.total_obligations ?? "-"}`)
      }
      console.log("  ledgers are informational and never a threshold gate")
      console.log("  execution authority: OpenAgents-owned runner; GitHub-hosted CI: disabled")
    }
    if (receipt.blocking_verdict === "fail") process.exit(1)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    if (json) printJson({ ok: false, code: "invalid_owned_runner_config", message })
    else console.error(`invalid_owned_runner_config: ${message}`)
    process.exit(1)
  }
}

// ---------------------------------------------------------------------------
// Independent-admission verifier (grant.independent_assurance)
// ---------------------------------------------------------------------------

const flagValues = (args: ReadonlyArray<string>, flag: string): ReadonlyArray<string> => {
  const found: string[] = []
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === flag && args[index + 1] !== undefined) {
      found.push(args[index + 1]!)
      index += 1
    }
  }
  return found
}

const parseVpCounts = (output: string): { passed?: number; failed?: number; files?: number } => {
  const result: { passed?: number; failed?: number; files?: number } = {}
  const tests = /Tests\s+(?:(\d+)\s+failed\s*\|\s*)?(\d+)\s+passed/.exec(output)
  if (tests !== null) {
    if (tests[1] !== undefined) result.failed = Number(tests[1])
    result.passed = Number(tests[2])
  }
  const files = /Test Files\s+(?:\d+\s+failed\s*\|\s*)?(\d+)\s+passed/.exec(output)
  if (files !== null) result.files = Number(files[1])
  return result
}

const spawnReproducer = (repoRoot: string) => (batch: OracleBatch): BatchReproduction => {
  const cwd = resolve(repoRoot, batch.cwd)
  const binary = resolve(cwd, batch.binary)
  if (!existsSync(binary)) {
    return { batch_id: batch.batch_id, ok: false, exit_code: -1, detail: `vp binary not found: ${binary}` }
  }
  const outcome = spawnSync(binary, ["test", "--run", "--root", batch.root, ...batch.file_args], {
    cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    env: { ...process.env, CI: "1", FORCE_COLOR: "0" },
  })
  const output = `${outcome.stdout ?? ""}\n${outcome.stderr ?? ""}`
  const counts = parseVpCounts(output)
  const exitCode = outcome.status ?? -1
  return {
    batch_id: batch.batch_id,
    ok: exitCode === 0 && (counts.failed ?? 0) === 0,
    exit_code: exitCode,
    ...(counts.passed === undefined ? {} : { tests_passed: counts.passed }),
    ...(counts.failed === undefined ? {} : { tests_failed: counts.failed }),
    ...(counts.files === undefined ? {} : { files: counts.files }),
  }
}

const defaultScopeNotes = (specRel: string): ReadonlyArray<string> => [
  "Admission means this revision overclaims no evidence tier. It is not a claim that all criteria pass.",
  "Smoke-gated, receipt-backed, and designed-only criteria stay unobserved and are not claimed observed.",
  "This admission grants no release, public-claim, or promise-transition authority (AuthorityDelegationSpec Law 6).",
  `Target: ${specRel}.`,
]

const reviewAdmit = async (args: ReadonlyArray<string>): Promise<void> => {
  const [specPathArg] = positional(args, 1)
  if (specPathArg === undefined) usage()
  const json = jsonFlag(args)
  const repoRoot = realResolve(flagValue(args, "--root") ?? process.cwd())
  const reviewer = flagValue(args, "--reviewer")
  const producer = flagValue(args, "--producer")
  if (reviewer === undefined || producer === undefined) {
    return failWith({
      ok: false,
      code: "invalid_argument",
      message: "review-admit requires --reviewer <ref> and --producer <ref> so independence is explicit.",
    }, json)
  }
  if (reviewer === producer) {
    return failWith({
      ok: false,
      code: "independence_violation",
      message: "reviewer and producer refs must be distinct (condition.independence).",
    }, json)
  }
  const trigger = flagValue(args, "--trigger") ?? "owner_directive.independent_admission"
  const program = flagValue(args, "--program") ?? "program.full_auto_release"
  const receiptsDir = flagValue(args, "--receipts-dir") ?? "docs/assurance/receipts"
  const dryRun = args.includes("--dry-run")
  const extraEvidence = flagValues(args, "--evidence")
  const scopeNotes = flagValues(args, "--scope-note")

  const specAbsolute = realResolve(specPathArg!)
  let specRel = relative(repoRoot, specAbsolute).replaceAll("\\", "/")
  if (specRel.startsWith("..") || specRel.startsWith("/")) {
    // Fail closed rather than emit an off-root relative path into a receipt.
    return failWith({
      ok: false,
      code: "invalid_argument",
      message: `AssuranceSpec path must resolve inside --root (${repoRoot}); got ${specAbsolute}`,
    }, json)
  }
  if (!(await file(specAbsolute).exists())) {
    return failWith({ ok: false, code: "file_not_found", message: `AssuranceSpec does not exist: ${specPathArg}` }, json)
  }
  const markdown = await file(specAbsolute).text()
  const validation = validateAssuranceSpec(markdown)
  if (!validation.valid) {
    return failWith({
      ok: false,
      code: "invalid_assurance_spec",
      message: "AssuranceSpec failed structural validation.",
      errors: validation.errors,
    }, json)
  }
  const document = parseAssuranceSpec(markdown)
  const targetDigest = sha256Digest(markdown)
  const startedAt = new Date().toISOString().replace(/\.\d+Z$/, "Z")

  const decision = decideReviewAdmission({
    document,
    fileExists: (path) => existsSync(resolve(repoRoot, path)),
    reproduce: spawnReproducer(repoRoot),
  })
  const settledAt = new Date().toISOString().replace(/\.\d+Z$/, "Z")

  const evidenceRefs = [specRel, ...extraEvidence]
  const notes = scopeNotes.length > 0 ? scopeNotes : defaultScopeNotes(specRel)
  const receipt = buildAuthorityDecisionReceipt({
    decision,
    targetRef: specRel,
    targetDigest,
    reviewerRef: reviewer,
    producerRef: producer,
    triggerRef: trigger,
    startedAt,
    settledAt,
    evidenceRefs,
    scopeNotes: notes,
    programRef: program,
  })
  const receiptArtifact = authorityDecisionReceiptArtifact(receipt)
  const receiptRel = `${receiptsDir}/${receipt.receipt_ref}.json`.replaceAll("\\", "/")
  const receiptAbsolute = resolve(repoRoot, receiptRel)

  let lifecycleFlipped = false
  if (decision.admit && !dryRun) {
    mkdirSync(resolve(repoRoot, receiptsDir), { recursive: true })
    await writeFile(receiptAbsolute, receiptArtifact.bytes.endsWith("\n") ? receiptArtifact.bytes : `${receiptArtifact.bytes}\n`)
    const flipped = admitAssuranceFrontmatter({
      markdown,
      reviewerRef: reviewer,
      receiptRef: receipt.receipt_ref,
      receiptPath: receiptRel,
      admittedAt: settledAt,
    })
    await writeFile(specAbsolute, flipped)
    lifecycleFlipped = true
  }

  if (json) {
    printJson({
      ok: true,
      admit: decision.admit,
      outcome: decision.outcome,
      lifecycle_flipped: lifecycleFlipped,
      dry_run: dryRun,
      receipt_ref: receipt.receipt_ref,
      receipt_path: decision.admit && !dryRun ? receiptRel : null,
      counts: decision.counts,
      executable_green: decision.executable_green,
      executable_failed: decision.executable_failed,
      reproductions: decision.reproductions,
      blockers: decision.blockers,
      receipt,
    })
  } else {
    console.log(`${decision.outcome.toUpperCase()} ${specRel}`)
    console.log(`  executable ${decision.executable_green}/${decision.counts.executable} green · smoke-gated ${decision.counts.smoke_gated} · receipt-backed ${decision.counts.receipt_backed} · designed-only ${decision.counts.designed_only}`)
    for (const batch of decision.batches) {
      const repro = decision.reproductions.find((entry) => entry.batch_id === batch.batch_id)
      console.log(`  batch ${batch.batch_id}: exit ${repro?.exit_code ?? "-"} ok=${repro?.ok ?? false} passed=${repro?.tests_passed ?? "-"} failed=${repro?.tests_failed ?? "-"} files=${repro?.files ?? "-"}`)
      console.log(`    ${batchCommandString(batch)}`)
    }
    if (decision.admit) {
      console.log(`  receipt ${receipt.receipt_ref}${lifecycleFlipped ? ` -> ${receiptRel}` : " (dry-run, not written)"}`)
      if (lifecycleFlipped) console.log(`  lifecycle_state flipped proposed -> admitted (admitted_by ${reviewer})`)
    } else {
      for (const blocker of decision.blockers) console.log(`  BLOCKED ${blocker.code}: ${blocker.message}`)
    }
  }
  if (!decision.admit) process.exit(1)
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

const [command, ...args] = process.argv.slice(2)
if (command === "propose") await propose(args)
else if (command === "observer") {
  const [subcommand, ...rest] = args
  if (subcommand === "propose") await observerPropose(rest)
  else usage()
}
else if (command === "validate") await validate(args)
else if (command === "coverage") await coverage(args)
else if (command === "session") {
  const [subcommand, ...rest] = args
  if (subcommand === "begin") sessionBegin(rest)
  else if (subcommand === "check") await sessionCheck(rest)
  else usage()
}
else if (command === "inventory") await inventory(args)
else if (command === "obligations") obligations(args)
else if (command === "obligation") obligation(args)
else if (command === "graph") graph(args)
else if (command === "ledgers") ledgers(args)
else if (command === "checklist") checklist(args)
else if (command === "claim") claim(args)
else if (command === "agent-run") {
  const [subcommand, ...rest] = args
  if (subcommand === "ingest") agentRunIngest(rest)
  else usage()
}
else if (command === "owned-runner") ownedRunner(args)
else if (command === "review-admit") await reviewAdmit(args)
else if (command === "mcp") runAssuranceSpecMcpServer(flagValue(args, "--root"))
else usage()
