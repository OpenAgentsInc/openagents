#!/usr/bin/env bun
import { readFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import {
  buildGlmFleetDurabilityOperatorBundle,
  buildGlmFleetDurabilityOwnerArmedCommand,
  formatGlmFleetDurabilityOperatorReadme,
} from '../src/inference/glm-fleet-durability-operator'
import type { GlmFleetReadinessProjection } from '../src/inference/glm-fleet-readiness'

const args = process.argv.slice(2)

const option = (name: string): string | undefined => {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  const value = args[index + 1]
  return value === undefined || value.startsWith('--') ? undefined : value
}

const flag = (name: string): boolean => args.includes(name)

const readinessUrl =
  option('--readiness-url') ??
  Bun.env.KHALA_GLM_FLEET_READINESS_URL ??
  'https://openagents.com/v1/gateway/glm-fleet/readiness'
const readinessJson = option('--readiness-json')
const outputDir = option('--output-dir')

const help = (): string => [
  'Usage: bun run scripts/khala-glm-fleet-durability.ts [options]',
  '',
  'Public-safe #6311 GLM fleet durability evidence bundler.',
  'By default it fetches the live public readiness projection and exits 2 until acceptance is complete.',
  '',
  'Options:',
  '  --readiness-url <url>       Public readiness URL to fetch.',
  '  --readiness-json <file>     Use a saved public readiness projection instead of fetching.',
  '  --output-dir <dir>          Write public-safe bundle, readout, and README files.',
  '  --summary                   Print compact public readout JSON.',
  '  --print-owner-command       Print the owner-armed public-ref command template and exit 0.',
  '  --help',
  '',
  'Owner-armed command template:',
  '',
  buildGlmFleetDurabilityOwnerArmedCommand({ outputDir, readinessUrl }),
  '',
].join('\n')

if (flag('--help')) {
  process.stdout.write(help())
  process.exit(0)
}

if (flag('--print-owner-command')) {
  process.stdout.write(
    `${buildGlmFleetDurabilityOwnerArmedCommand({
      outputDir,
      readinessUrl,
    })}\n`,
  )
  process.exit(0)
}

const loadProjection = async (): Promise<GlmFleetReadinessProjection> => {
  if (readinessJson !== undefined) {
    return JSON.parse(await readFile(readinessJson, 'utf8'))
  }
  const response = await fetch(readinessUrl, {
    headers: { accept: 'application/json' },
  })
  const json = await response.json().catch(() => ({}))
  if (!response.ok) {
    throw new Error(`readiness_fetch_failed:${response.status}`)
  }
  return json as GlmFleetReadinessProjection
}

const projection = await loadProjection()
const bundle = buildGlmFleetDurabilityOperatorBundle({
  generatedAt: new Date().toISOString(),
  outputDir,
  projection,
  readinessUrl,
})

if (outputDir !== undefined) {
  await mkdir(outputDir, { recursive: true })
  await Promise.all([
    Bun.write(
      join(outputDir, 'glm-fleet-durability-readiness.public.json'),
      `${JSON.stringify(bundle.readiness, null, 2)}\n`,
    ),
    Bun.write(
      join(outputDir, 'glm-fleet-durability-operator-bundle.public.json'),
      `${JSON.stringify(bundle, null, 2)}\n`,
    ),
    Bun.write(
      join(outputDir, 'README.public.md'),
      formatGlmFleetDurabilityOperatorReadme(bundle),
    ),
  ])
}

process.stderr.write(
  [
    `[glm-fleet-durability] acceptance=${bundle.readiness.acceptanceStatus}`,
    `[glm-fleet-durability] serving=${bundle.readiness.servingStatus}`,
    `[glm-fleet-durability] missingOperatorInputs=${bundle.missingOperatorInputs.map(input => input.env).join(', ') || 'none'}`,
    '[glm-fleet-durability] liveRoutingChanged=false',
    ...(outputDir === undefined
      ? []
      : [`[glm-fleet-durability] wrotePublicSafeEvidenceDir=${outputDir}`]),
  ].join('\n') + '\n',
)

process.stdout.write(
  `${JSON.stringify(flag('--summary') ? bundle.readiness : bundle, null, 2)}\n`,
)

if (bundle.readiness.acceptanceStatus !== 'complete') {
  process.exitCode = 2
}
