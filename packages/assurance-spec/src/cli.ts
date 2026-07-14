#!/usr/bin/env bun
/**
 * assurance-spec CLI (AT-1, docs/assurance/AGENT_TOOLING.md §2).
 *
 * Exit codes are the API: 0 success, 1 operation failure, 2 usage error,
 * 3 stale session. Every command takes --json for machine output; human
 * output stays terse. The new commands run the exact Effect handlers the MCP
 * server uses (src/handlers.ts): one implementation, two transports.
 */
import { resolve, relative } from "node:path"

import type { Effect } from "effect"

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
  proposeAssuranceSpec,
  runAssuranceSpecMcpServer,
  runTool,
  validateAssuranceSpec,
  type AssuranceToolError,
  type ToolFailure,
} from "./index.ts"

const usage = (): never => {
  console.error("usage:")
  console.error("  assurance-spec propose <file.product-spec.md> [--repo <dir>] [--out <file.assurance-spec.md>] [--inventory-out <file.json>] [--id <id>] [--title <title>] [--author <author>] [--force] [--json]")
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
    "--root", "--against", "--spec-digest", "--subject-digest",
    "--criterion", "--status", "--technique", "--claim",
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
  if (!(await Bun.file(inputAbsolute).exists())) {
    console.error(`ProductSpec does not exist: ${input}`)
    process.exit(1)
  }
  const repoFlag = flagValue(args, "--repo")
  const repositoryRoot = repoFlag === undefined ? undefined : resolve(repoFlag)
  const base = repositoryRoot ?? process.cwd()
  const productSpecPath = relative(base, inputAbsolute).replaceAll("\\", "/")
  const output = resolve(flagValue(args, "--out") ?? defaultOutput(input))
  if (await Bun.file(output).exists() && !args.includes("--force")) {
    console.error(`refusing to overwrite existing file: ${output}`)
    process.exit(1)
  }
  const repositoryInventory = repositoryRoot === undefined ? undefined : inventoryRepository(repositoryRoot)
  const result = proposeAssuranceSpec({
    productSpecPath,
    productSpecMarkdown: await Bun.file(inputAbsolute).text(),
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
  await Bun.write(output, result.markdown)
  const inventoryOut = flagValue(args, "--inventory-out")
  if (inventoryOut !== undefined) {
    await Bun.write(resolve(inventoryOut), `${JSON.stringify(result.document.environments.repository_inventory, null, 2)}\n`)
  }
  if (json) {
    printJson({ ok: true, output, adequacy: result.adequacy })
    return
  }
  console.log(`proposed ${output}`)
  console.log(`  ${result.adequacy.coverage.obligations} obligations · ${result.adequacy.coverage.needs_design} need design · ${result.adequacy.coverage.ready} ready`)
  console.log(`  repository ${result.document.environments.repository_inventory.state} · structural valid · design ready ${result.adequacy.design_ready ? "yes" : "no"} · execution authorized no`)
}

const validate = async (args: ReadonlyArray<string>): Promise<void> => {
  const json = jsonFlag(args)
  const paths = args.filter((arg) => !arg.startsWith("--"))
  if (paths.length === 0) usage()
  let failures = 0
  const results: Array<unknown> = []
  for (const path of paths) {
    const result = validateAssuranceSpec(await Bun.file(path).text())
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
  const validation = validateAssuranceSpec(await Bun.file(path).text())
  if (!validation.valid || validation.document === undefined) {
    for (const error of validation.errors) console.error(`${error.code}: ${error.message}`)
    process.exit(1)
  }
  const assessment = assessAssuranceSpec(parseAssuranceSpec(await Bun.file(path).text()))
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
    const file = Bun.file(resolve(against))
    if (!(await file.exists())) {
      return failWith({ ok: false, code: "file_not_found", message: `Session file does not exist: ${against}` }, json)
    }
    try {
      pin = JSON.parse(await file.text())
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
  if (out !== undefined) await Bun.write(resolve(out), `${JSON.stringify(report, null, 2)}\n`)
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
  const report = runOrExit(getCoverageLedgers({ path, ...rootArg(args) }), json)
  if (json) {
    printJson(report)
    return
  }
  console.log(`traceability ${report.criterion_traceability.traceable_criteria}/${report.criterion_traceability.total_criteria} criteria bound to obligations`)
  console.log(`execution ${report.execution.executed_obligations}/${report.execution.total_obligations} obligations executed (every observation is not_run; receipt source: ${report.execution.receipt_source})`)
  console.log(`frontier ${report.reachable_frontier.status}: ${report.reachable_frontier.reason}`)
}

const checklist = (args: ReadonlyArray<string>): void => {
  const [path] = positional(args, 1)
  if (path === undefined) usage()
  const json = jsonFlag(args)
  const criterion = flagValue(args, "--criterion")
  const report = runOrExit(
    getEvidenceChecklist({ path, ...rootArg(args), ...(criterion === undefined ? {} : { criterion_ref: criterion }) }),
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
  const audit = runOrExit(
    checkCompletionClaim({ path, ...rootArg(args), ...(claimText === undefined ? {} : { claim: claimText }) }),
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

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

const [command, ...args] = process.argv.slice(2)
if (command === "propose") await propose(args)
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
else if (command === "mcp") runAssuranceSpecMcpServer(flagValue(args, "--root"))
else usage()
