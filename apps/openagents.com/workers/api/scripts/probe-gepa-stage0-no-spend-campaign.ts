#!/usr/bin/env bun

import { readFile } from 'node:fs/promises'

import {
  projectProbeGepaStage0LiveCampaignSmoke,
  projectProbeGepaStage0LivePylonPreflight,
} from '../src/probe-gepa-stage0-live-campaign-smoke'

const usage = `Usage:
  bun scripts/probe-gepa-stage0-no-spend-campaign.ts --preflight [--base-url https://openagents.com] [--pylon-ref REF ...]
  bun scripts/probe-gepa-stage0-no-spend-campaign.ts --bundle stage0-bundle.json [--preflight] [--base-url https://openagents.com] [--pylon-ref REF ...]

The bundle JSON shape is the input accepted by projectProbeGepaStage0NoSpendCampaign:
{ "campaignRef": "...", "coordinatorImports": [...], "probeCloseoutImportRefs": [...],
  "psionicImportDryRunRefs": [...], "artanisSummaryRefs": [...] }
`

type ParsedArgs = Readonly<{
  baseUrl: string
  bundlePath: string | null
  preflight: boolean
  pylonRefs: ReadonlyArray<string>
}>

const parseArgs = (argv: ReadonlyArray<string>): ParsedArgs => {
  const pylonRefs: Array<string> = []
  let baseUrl = process.env.OPENAGENTS_BASE_URL?.trim() || 'https://openagents.com'
  let bundlePath: string | null = null
  let preflight = false

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]

    if (arg === '--help' || arg === '-h') {
      process.stdout.write(usage)
      process.exit(0)
    }

    if (arg === '--base-url') {
      baseUrl = argv[index + 1]?.trim() || ''
      index += 1
      continue
    }

    if (arg === '--bundle') {
      bundlePath = argv[index + 1]?.trim() || null
      index += 1
      continue
    }

    if (arg === '--preflight') {
      preflight = true
      continue
    }

    if (arg === '--pylon-ref') {
      const pylonRef = argv[index + 1]?.trim()
      if (pylonRef !== undefined && pylonRef !== '') {
        pylonRefs.push(pylonRef)
      }
      index += 1
      continue
    }

    throw new Error(`Unknown argument: ${arg}`)
  }

  if (baseUrl === '') {
    throw new Error('--base-url must not be empty.')
  }

  if (bundlePath === null && !preflight) {
    preflight = true
  }

  return { baseUrl, bundlePath, preflight, pylonRefs }
}

const fetchPublicPylons = async (
  baseUrl: string,
): Promise<ReadonlyArray<unknown>> => {
  const url = new URL('/api/pylons', baseUrl)
  url.searchParams.set('cb', `probe-gepa-stage0-${Date.now()}`)
  const response = await fetch(url)
  const json = await response.json() as { pylons?: unknown }

  if (!response.ok || !Array.isArray(json.pylons)) {
    throw new Error(`Unable to read public Pylon list (${response.status}).`)
  }

  return json.pylons
}

try {
  const parsed = parseArgs(process.argv.slice(2))
  const preflight = parsed.preflight
    ? projectProbeGepaStage0LivePylonPreflight({
        candidates: await fetchPublicPylons(parsed.baseUrl),
        selectedPylonRefs: parsed.pylonRefs,
      })
    : null
  const result = parsed.bundlePath === null
    ? preflight
    : projectProbeGepaStage0LiveCampaignSmoke({
        campaignInput: JSON.parse(await readFile(parsed.bundlePath, 'utf8')),
        preflight,
      })

  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
  process.exitCode = result?.state === 'green' ? 0 : 2
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
  process.stderr.write(usage)
  process.exitCode = 1
}
