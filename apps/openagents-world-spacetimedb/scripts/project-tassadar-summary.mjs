#!/usr/bin/env node
import { spawnSync } from 'node:child_process'

import {
  DEFAULT_BRIDGE_REF,
  DEFAULT_DATABASE,
  DEFAULT_TASSADAR_SOURCE_URL,
  assertNoDuplicateWorldEvents,
  assertWorldEventsAreSourced,
  buildTassadarProjectionPlan,
  reducerCounts,
} from './tassadar-summary-transform.mjs'

const defaults = {
  database: DEFAULT_DATABASE,
  gcloudInstance: 'spacetimedb-world-1',
  gcloudProject: 'openagentsgemini',
  gcloudZone: 'us-central1-a',
  server: 'local',
  sourceUrl: DEFAULT_TASSADAR_SOURCE_URL,
  spacetimeCli: '/stdb/bin/2.6.0/spacetimedb-cli',
}

const parseArgs = argv => {
  const options = { ...defaults, applyVm: false, json: false }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    const next = () => {
      index += 1
      if (index >= argv.length) {
        throw new Error(`missing value for ${arg}`)
      }
      return argv[index]
    }
    if (arg === '--apply-vm') options.applyVm = true
    else if (arg === '--json') options.json = true
    else if (arg === '--source-url') options.sourceUrl = next()
    else if (arg === '--database') options.database = next()
    else if (arg === '--server') options.server = next()
    else if (arg === '--spacetime-cli') options.spacetimeCli = next()
    else if (arg === '--gcloud-instance') options.gcloudInstance = next()
    else if (arg === '--gcloud-project') options.gcloudProject = next()
    else if (arg === '--gcloud-zone') options.gcloudZone = next()
    else if (arg === '--help') options.help = true
    else throw new Error(`unknown argument: ${arg}`)
  }
  return options
}

const printHelp = () => {
  console.log(`Usage: bun apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs [options]

Options:
  --apply-vm                  Apply the projection through gcloud SSH and VM-local spacetimedb-cli.
  --json                      Print the full reducer call plan.
  --source-url <url>          Public Tassadar summary URL.
  --database <name>           SpacetimeDB database name. Default: ${defaults.database}
  --server <name>             VM-local SpacetimeDB server nickname. Default: ${defaults.server}
  --spacetime-cli <path>      VM-local SpacetimeDB CLI path.
  --gcloud-instance <name>    GCE VM name. Default: ${defaults.gcloudInstance}
  --gcloud-project <name>     GCP project. Default: ${defaults.gcloudProject}
  --gcloud-zone <zone>        GCP zone. Default: ${defaults.gcloudZone}
`)
}

const shellQuote = value => `'${String(value).replaceAll("'", "'\\''")}'`

const reducerCommand = (options, database, call) =>
  [
    'sudo',
    '-u',
    'spacetimedb',
    shellQuote(options.spacetimeCli),
    'call',
    '-s',
    shellQuote(options.server),
    '-y',
    shellQuote(database),
    shellQuote(call.reducer),
    '--',
    ...call.args.map(arg => shellQuote(String(arg))),
  ].join(' ')

const applyPlanToVm = (plan, options) => {
  const successCall = {
    reducer: 'record_bridge_success',
    args: [DEFAULT_BRIDGE_REF, plan.sourceUrl],
  }
  const remoteCommand = [
    'set -euo pipefail',
    ...plan.calls.map(call => reducerCommand(options, plan.database, call)),
    reducerCommand(options, plan.database, successCall),
  ].join('\n')
  const result = spawnSync(
    'gcloud',
    [
      'compute',
      'ssh',
      options.gcloudInstance,
      '--project',
      options.gcloudProject,
      '--zone',
      options.gcloudZone,
      '--tunnel-through-iap',
      '--command',
      remoteCommand,
    ],
    { stdio: 'inherit' },
  )
  if (result.status !== 0) {
    throw new Error(`gcloud apply failed with status ${result.status}`)
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    printHelp()
    return
  }

  const response = await fetch(options.sourceUrl, {
    headers: { accept: 'application/json' },
  })
  if (!response.ok) {
    throw new Error(`failed to fetch ${options.sourceUrl}: ${response.status}`)
  }
  const summary = await response.json()
  const plan = buildTassadarProjectionPlan(summary, {
    database: options.database,
    sourceUrl: options.sourceUrl,
  })

  assertNoDuplicateWorldEvents(plan)
  assertWorldEventsAreSourced(plan)

  if (options.json) {
    console.log(JSON.stringify(plan, null, 2))
  } else {
    console.log(
      JSON.stringify(
        {
          applyVm: options.applyVm,
          callCount: plan.calls.length,
          database: plan.database,
          reducerCounts: reducerCounts(plan),
          runRef: plan.runRef,
          sourceGeneratedAt: plan.sourceGeneratedAt,
          sourceHash: plan.sourceHash,
          sourceUrl: plan.sourceUrl,
        },
        null,
        2,
      ),
    )
  }

  if (options.applyVm) {
    applyPlanToVm(plan, options)
  }
}

main().catch(error => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
