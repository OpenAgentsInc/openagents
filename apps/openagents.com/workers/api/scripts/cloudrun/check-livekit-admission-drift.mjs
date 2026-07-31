#!/usr/bin/env node
// Detect divergence between the committed Cloud Run LiveKit admission state
// and the state the serving revision actually carries (EP263-LK H3, #9282).
//
// On 2026-07-31 production served SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED=true
// while this repository committed "false". `main` described a closed alpha
// that was open. Nothing detected it, because nothing compared them. This
// script is that comparison, and it is the documented operator step after any
// out-of-band `gcloud run services update --update-env-vars` on these keys.
//
// Read-only. It prints booleans and revision names only — never a secret.
//
// Usage:
//   node scripts/cloudrun/check-livekit-admission-drift.mjs [--json]
//     [--target production|staging] [--service NAME] [--revision NAME]
//
// Exit codes: 0 agree, 1 drift, 2 could not compare.

import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/** Keys whose committed value must match the serving revision exactly. */
export const LIVEKIT_ADMISSION_KEYS = Object.freeze([
  'SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED',
  'SARAH_LIVEKIT_PROVIDER_DISCONNECT_ACCEPTANCE_ENABLED',
])

/**
 * Parse the committed quoted-boolean env declarations. The Cloud Run env files
 * are flat `KEY: "value"` documents, so a full YAML parser is not warranted and
 * would add a dependency to a deploy-path script.
 *
 * @param {string} source
 * @returns {Record<string, string | undefined>}
 */
export const parseCommittedAdmissionState = (source) => {
  /** @type {Record<string, string | undefined>} */
  const parsed = {}
  for (const key of LIVEKIT_ADMISSION_KEYS) {
    const match = new RegExp(`^${key}: "([^"]*)"$`, 'mu').exec(source)
    parsed[key] = match?.[1]
  }
  return parsed
}

/**
 * @param {Record<string, string | undefined>} committed
 * @param {Record<string, string | undefined>} serving
 * @returns {ReadonlyArray<{ key: string, committed: string | undefined, serving: string | undefined }>}
 */
export const admissionDrift = (committed, serving) =>
  LIVEKIT_ADMISSION_KEYS.filter(
    (key) => (committed[key] ?? undefined) !== (serving[key] ?? undefined),
  ).map((key) => ({
    key,
    committed: committed[key],
    serving: serving[key],
  }))

const describe = (args) =>
  JSON.parse(
    execFileSync('gcloud', [...args, '--format=json'], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'inherit'],
    }),
  )

const readServingRevisionState = (service, region, project, revision) => {
  const revisionName =
    revision ??
    describe([
      'run',
      'services',
      'describe',
      service,
      `--project=${project}`,
      `--region=${region}`,
    ]).status?.latestReadyRevisionName

  if (typeof revisionName !== 'string' || revisionName.length === 0) {
    throw new Error(`could not resolve a ready revision for ${service}`)
  }

  const described = describe([
    'run',
    'revisions',
    'describe',
    revisionName,
    `--project=${project}`,
    `--region=${region}`,
  ])
  /** @type {Record<string, string | undefined>} */
  const serving = {}
  for (const entry of described.spec?.containers?.[0]?.env ?? []) {
    if (LIVEKIT_ADMISSION_KEYS.includes(entry.name)) {
      serving[entry.name] = entry.value
    }
  }
  return { revisionName, serving }
}

const main = () => {
  const argv = process.argv.slice(2)
  const flag = (name, fallback) => {
    const index = argv.indexOf(`--${name}`)
    return index >= 0 ? argv[index + 1] : fallback
  }
  const json = argv.includes('--json')
  const target = flag('target', 'production')
  const service = flag(
    'service',
    target === 'production'
      ? 'openagents-monolith'
      : 'openagents-monolith-staging',
  )
  const project = flag('project', process.env.OPENAGENTS_GCP_PROJECT ?? 'openagentsgemini')
  const region = flag('region', process.env.OPENAGENTS_GCP_REGION ?? 'us-central1')

  const committed = parseCommittedAdmissionState(
    readFileSync(
      fileURLToPath(new URL(`env-${target}.yaml`, import.meta.url)),
      'utf8',
    ),
  )

  let observed
  try {
    observed = readServingRevisionState(
      service,
      region,
      project,
      flag('revision', undefined),
    )
  } catch (error) {
    console.error(
      `FATAL: could not read the serving revision (${service}): ${error instanceof Error ? error.message : String(error)}`,
    )
    console.error(
      'Authenticate with the workspace automation config, for example: ' +
        'CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config',
    )
    process.exit(2)
  }

  const drift = admissionDrift(committed, observed.serving)
  const report = {
    schema: 'openagents.livekit_admission_drift.v1',
    service,
    revision: observed.revisionName,
    committed,
    serving: observed.serving,
    drift,
    ok: drift.length === 0,
  }

  if (json) {
    console.log(JSON.stringify(report, undefined, 2))
  } else {
    console.log(`service:  ${service}`)
    console.log(`revision: ${observed.revisionName}`)
    for (const key of LIVEKIT_ADMISSION_KEYS) {
      const agree = committed[key] === observed.serving[key]
      console.log(
        `${agree ? '  ok  ' : ' DRIFT'} ${key}: committed=${committed[key]} serving=${observed.serving[key]}`,
      )
    }
  }

  if (drift.length > 0) {
    console.error(
      '\nFATAL: the serving revision disagrees with the committed Cloud Run ' +
        'environment. Either commit the intended state and redeploy, or ' +
        'withdraw the out-of-band override. Do not leave `main` describing a ' +
        'production state that does not exist.',
    )
    process.exit(1)
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main()
}
