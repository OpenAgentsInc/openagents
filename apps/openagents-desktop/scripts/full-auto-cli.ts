/**
 * FA-H13 (#8886): a tiny argv CLI over the Full Auto local control API -- a
 * deliberately thin pass-through client of the OpenAPI surface Desktop main
 * serves. Prints the server's JSON verbatim; exits nonzero on any non-2xx.
 *
 * Usage (package script `pnpm --dir apps/openagents-desktop run full-auto`):
 *   node --import tsx scripts/full-auto-cli.ts list
 *   node --import tsx scripts/full-auto-cli.ts status <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts enable <threadRef> --workspace <path>
 *   node --import tsx scripts/full-auto-cli.ts disable <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts continue-now <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts resume <threadRef>
 *
 * FA-WIRE-01 (#8996): start/enable/run-start accept a repeatable
 * `--lane <laneRef[:accountRef]>` (order = rotation priority) plus
 * `--max-turns` / `--max-wall-clock-ms` guardrail flags; `resume` clears a
 * FA-GD-01 low-confidence pause. Status/turns output renders rotation and
 * pause state because the server projection now carries rotationHistory,
 * decisionHistory, guardrails, routingPolicy, and pausedReason.
 *   node --import tsx scripts/full-auto-cli.ts turns <threadRef>
 *   node --import tsx scripts/full-auto-cli.ts openapi
 *
 * FA-RUN-01 (#8969) durable run lifecycle commands:
 *   node --import tsx scripts/full-auto-cli.ts runs
 *   node --import tsx scripts/full-auto-cli.ts run-status <runRef>
 *   node --import tsx scripts/full-auto-cli.ts run-start --workspace <path> --title <t> --objective <o> --done <d> [--lane <l>] [--turn-cap <n>] [--autonomy]
 *
 * HANDS control-API wiring: run-start accepts `--autonomy` to enable the autonomy
 * core on the created run (objective selection #9172, host verification #9173,
 * plan brief #9174, churn detection #9175, initiative #9184). Default OFF (the
 * passive base loop); `--no-autonomy` is the explicit default.
 *   node --import tsx scripts/full-auto-cli.ts run-pause <runRef>
 *   node --import tsx scripts/full-auto-cli.ts run-resume <runRef>
 *   node --import tsx scripts/full-auto-cli.ts run-stop <runRef>
 *
 * FA-RPT-01 (#8988) run report/receipt pass-throughs (aliases: run-report,
 * run-receipt):
 *   node --import tsx scripts/full-auto-cli.ts report <runRef>
 *   node --import tsx scripts/full-auto-cli.ts receipt <runRef>
 * Options: --user-data <path> (or OPENAGENTS_DESKTOP_USER_DATA) when Desktop
 * runs against a non-default userData directory.
 */
import {
  buildFullAutoPolicyOptions,
  ControlUnavailableError,
  controlOperations,
  readControlConnection,
  resolveUserDataDir,
} from "./full-auto-control-client.ts"

const USAGE = `usage: full-auto-cli <command> [args] [--user-data <path>]
commands:
  lanes
  list
  status <threadRef>
  start --workspace <path> [--title <title>] [--lane <laneRef[:accountRef]>]... [--max-turns <n>] [--max-wall-clock-ms <n>]
  enable <threadRef> --workspace <path> [--lane <laneRef[:accountRef]>]... [--max-turns <n>] [--max-wall-clock-ms <n>]
  disable <threadRef>
  continue-now <threadRef>
  resume <threadRef>
  turns <threadRef>
  openapi
  runs
  run-status <runRef>
  run-start --workspace <path> --title <t> --objective <o> --done <d> [--lane <laneRef[:accountRef]>]... [--turn-cap <n>] [--max-turns <n>] [--max-wall-clock-ms <n>] [--autonomy]
  run-pause <runRef>
  run-resume <runRef>
  run-stop <runRef>
  report <runRef>
  receipt <runRef>

FA-WIRE-01 (#8996): --lane is repeatable; the order given is the rotation
priority. A trailing :accountRef pins the candidate to one account (the last
colon splits lane from account). A namespaced fleet lane ref with exactly one
colon -- "acp:<agent>" (grok-cli, cursor-agent) or "harness:<agent>"
(opencode, pi, goose) -- is kept whole, so pin its account with a SECOND colon,
e.g. "harness:opencode:<accountRef>".
--max-turns / --max-wall-clock-ms bind owner guardrails; resume clears a
low-confidence pause.

HANDS control-API wiring: run-start --autonomy enables the autonomy core on the
created run (objective selection, host verification, plan brief, churn
detection, initiative). Default OFF; --no-autonomy is the explicit default.`

const main = async (): Promise<void> => {
  const argv = [...process.argv.slice(2)]
  const takeOption = (name: string): string | undefined => {
    const index = argv.indexOf(name)
    if (index === -1) return undefined
    const value = argv[index + 1]
    argv.splice(index, 2)
    return value
  }
  /** HANDS control-API wiring: a valueless boolean flag (e.g. --autonomy). */
  const takeFlag = (name: string): boolean => {
    const index = argv.indexOf(name)
    if (index === -1) return false
    argv.splice(index, 1)
    return true
  }
  /** FA-WIRE-01 (#8996): --lane is repeatable; the order given is priority. */
  const takeRepeatedOption = (name: string): Array<string> => {
    const values: Array<string> = []
    for (let value = takeOption(name); value !== undefined; value = takeOption(name)) {
      values.push(value)
    }
    return values
  }
  const userData = takeOption("--user-data")
  const workspace = takeOption("--workspace")
  const title = takeOption("--title")
  const lanes = takeRepeatedOption("--lane")
  const objective = takeOption("--objective")
  const doneCondition = takeOption("--done")
  const turnCapRaw = takeOption("--turn-cap")
  const maxTurnsRaw = takeOption("--max-turns")
  const maxWallClockMsRaw = takeOption("--max-wall-clock-ms")
  // HANDS control-API wiring: the opt-in autonomy core for run-start. Default OFF
  // (the passive base loop). --no-autonomy is the explicit default and wins
  // over --autonomy if both are given.
  const autonomyFlag = takeFlag("--autonomy")
  const noAutonomyFlag = takeFlag("--no-autonomy")
  const autonomy = autonomyFlag && !noAutonomyFlag
  const [command, threadRef] = argv
  const parsePositiveInt = (name: string, raw: string | undefined): number | undefined => {
    if (raw === undefined) return undefined
    const parsed = Number.parseInt(raw, 10)
    if (!Number.isInteger(parsed) || parsed <= 0) {
      console.error(`${command}: --${name} must be a positive integer\n${USAGE}`)
      process.exit(2)
    }
    return parsed
  }
  const policy = buildFullAutoPolicyOptions({
    lanes,
    maxTurns: parsePositiveInt("max-turns", maxTurnsRaw),
    maxWallClockMs: parsePositiveInt("max-wall-clock-ms", maxWallClockMsRaw),
  })

  const connection = readControlConnection(resolveUserDataDir(userData))
  const operations = controlOperations(connection)

  const requireThreadRef = (): string => {
    if (threadRef === undefined || threadRef.length === 0) {
      console.error(`${command}: a <threadRef> argument is required\n${USAGE}`)
      process.exit(2)
    }
    return threadRef
  }
  const requireRunRef = (): string => {
    if (threadRef === undefined || threadRef.length === 0) {
      console.error(`${command}: a <runRef> argument is required\n${USAGE}`)
      process.exit(2)
    }
    return threadRef
  }
  const requireOption = (name: string, value: string | undefined): string => {
    if (value === undefined || value.length === 0) {
      console.error(`${command}: --${name} <value> is required\n${USAGE}`)
      process.exit(2)
    }
    return value
  }

  const result = command === "lanes"
    ? await operations.lanes()
    : command === "list"
    ? await operations.list()
    : command === "openapi"
    ? await operations.openapi()
    : command === "status"
    ? await operations.status(requireThreadRef())
    : command === "start"
    ? await (async () => {
        if (workspace === undefined || workspace.length === 0) {
          console.error(`start: --workspace <path> is required (start must NAME the workspace it expects)\n${USAGE}`)
          process.exit(2)
        }
        return operations.start(workspace, title, policy.lane, policy.options)
      })()
    : command === "enable"
    ? await (async () => {
        const ref = requireThreadRef()
        if (workspace === undefined || workspace.length === 0) {
          console.error(`enable: --workspace <path> is required (enable must NAME the workspace it expects)\n${USAGE}`)
          process.exit(2)
        }
        return operations.enable(ref, workspace, policy.lane, policy.options)
      })()
    : command === "disable"
    ? await operations.disable(requireThreadRef())
    : command === "continue-now"
    ? await operations.continueNow(requireThreadRef())
    : command === "resume"
    ? await operations.resume(requireThreadRef())
    : command === "turns"
    ? await operations.turns(requireThreadRef())
    : command === "runs"
    ? await operations.runsList()
    : command === "run-status"
    ? await operations.runStatus(requireRunRef())
    : command === "run-start"
    ? await operations.runsStart({
        workspaceRef: requireOption("workspace", workspace),
        title: requireOption("title", title),
        objective: requireOption("objective", objective),
        doneCondition: requireOption("done", doneCondition),
        ...(policy.lane === undefined ? {} : { lane: policy.lane }),
        ...(turnCapRaw === undefined ? {} : { turnCap: Number.parseInt(turnCapRaw, 10) }),
        ...(policy.options.routingPolicy === undefined ? {} : { routingPolicy: policy.options.routingPolicy }),
        ...(policy.options.guardrails === undefined ? {} : { guardrails: policy.options.guardrails }),
        ...(autonomy ? { autonomy: true } : {}),
      })
    : command === "run-pause"
    ? await operations.runPause(requireRunRef())
    : command === "run-resume"
    ? await operations.runResume(requireRunRef())
    : command === "run-stop"
    ? await operations.runStop(requireRunRef())
    : command === "report" || command === "run-report"
    ? await operations.runReport(requireRunRef())
    : command === "receipt" || command === "run-receipt"
    ? await operations.runReceipt(requireRunRef())
    : null

  if (result === null) {
    console.error(USAGE)
    process.exit(2)
  }
  console.log(JSON.stringify(result.body, null, 2))
  if (result.status < 200 || result.status >= 300) process.exit(1)
}

await main().catch(error => {
  if (error instanceof ControlUnavailableError) {
    console.error(error.message)
  } else {
    console.error(
      "full-auto-cli failed:",
      error instanceof Error ? error.message : String(error),
    )
  }
  process.exit(1)
})
