#!/usr/bin/env bun
/**
 * SQ-8 / #8625 — GPU media-run closeout receipt validator.
 *
 * Offline by default: schema + privacy + artifact-check method law.
 * Optional --live-artifacts: gsutil/gcloud object existence for gs:// URIs.
 *
 * Usage:
 *   bun apps/sarah/scripts/gpu-media-run-closeout.mjs --receipt path.json
 *   bun apps/sarah/scripts/gpu-media-run-closeout.mjs --receipt path.json --live-artifacts
 *   bun apps/sarah/scripts/gpu-media-run-closeout.mjs --self-test
 */

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, statSync } from 'node:fs'
import { dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const SCHEMA_VERSION = 'openagents.sarah.gpu_media_run_closeout.v1'
export const ALLOWED_HOST_STATUS = new Set(['stopped', 'deleted', 'left_running'])
export const ALLOWED_CHECK_METHODS = new Set(['object_exists'])

/** Patterns that must never appear in a public-safe closeout receipt. */
const SECRETISH =
  /\b(api[_-]?key|secret|password|mnemonic|private[_-]?key|authorization\s*:|bearer\s+[a-z0-9._\-]{12,}|sk-[a-z0-9]{10,}|AIza[0-9A-Za-z_\-]{20,})\b/i

const EMAILISH = /\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b/i

/**
 * @typedef {{
 *   ok: boolean,
 *   errors: string[],
 *   warnings: string[],
 * }} ValidationResult
 */

/**
 * @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
const isRecord = (value) =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/**
 * Walk JSON for secret-shaped strings.
 * @param {unknown} node
 * @param {string} path
 * @param {string[]} errors
 */
const scanSecrets = (node, path, errors) => {
  if (typeof node === 'string') {
    if (SECRETISH.test(node)) {
      errors.push(`${path}: forbidden secret-shaped content`)
    }
    // Allow example.com / openagents.com docs emails only if we want — reject all for hard law
    if (EMAILISH.test(node) && !node.includes('example.com') && !node.endsWith('@openagents.com')) {
      // Prospect emails: reject personal domains in receipts
      if (!/@openagents\.com\b/i.test(node)) {
        errors.push(`${path}: email-like value (prospect PII risk)`)
      }
    }
    return
  }
  if (Array.isArray(node)) {
    node.forEach((item, i) => scanSecrets(item, `${path}[${i}]`, errors))
    return
  }
  if (isRecord(node)) {
    for (const [key, value] of Object.entries(node)) {
      if (SECRETISH.test(key)) {
        errors.push(`${path}.${key}: forbidden secret-shaped field name`)
      }
      scanSecrets(value, `${path}.${key}`, errors)
    }
  }
}

/**
 * @param {unknown} raw
 * @returns {ValidationResult}
 */
export const validateCloseoutReceipt = (raw) => {
  /** @type {string[]} */
  const errors = []
  /** @type {string[]} */
  const warnings = []

  if (!isRecord(raw)) {
    return { ok: false, errors: ['receipt must be a JSON object'], warnings }
  }

  if (raw.schemaVersion !== SCHEMA_VERSION) {
    errors.push(
      `schemaVersion must be ${SCHEMA_VERSION} (got ${String(raw.schemaVersion)})`,
    )
  }

  if (typeof raw.runId !== 'string' || raw.runId.trim().length === 0) {
    errors.push('runId is required (non-empty string)')
  }

  if (!Array.isArray(raw.issueRefs) || raw.issueRefs.length === 0) {
    errors.push('issueRefs must be a non-empty array of strings')
  } else if (!raw.issueRefs.every((r) => typeof r === 'string' && r.length > 0)) {
    errors.push('issueRefs entries must be non-empty strings')
  }

  const startedAt = typeof raw.startedAt === 'string' ? Date.parse(raw.startedAt) : NaN
  const endedAt = typeof raw.endedAt === 'string' ? Date.parse(raw.endedAt) : NaN
  if (Number.isNaN(startedAt)) errors.push('startedAt must be ISO-8601')
  if (Number.isNaN(endedAt)) errors.push('endedAt must be ISO-8601')
  if (!Number.isNaN(startedAt) && !Number.isNaN(endedAt) && endedAt < startedAt) {
    errors.push('endedAt must be >= startedAt')
  }

  if (!isRecord(raw.host)) {
    errors.push('host is required')
  } else {
    for (const key of ['name', 'project', 'zone', 'machineType', 'gpu']) {
      if (typeof raw.host[key] !== 'string' || raw.host[key].trim().length === 0) {
        errors.push(`host.${key} is required`)
      }
    }
  }

  if (!isRecord(raw.hostDisposition)) {
    errors.push('hostDisposition is required')
  } else {
    const status = raw.hostDisposition.status
    if (typeof status !== 'string' || !ALLOWED_HOST_STATUS.has(status)) {
      errors.push(
        `hostDisposition.status must be one of ${[...ALLOWED_HOST_STATUS].join('|')}`,
      )
    }
    if (status === 'left_running') {
      const reason = raw.hostDisposition.reason
      if (typeof reason !== 'string' || reason.trim().length === 0) {
        errors.push(
          'hostDisposition.reason is required when status is left_running',
        )
      }
    }
  }

  if (!Array.isArray(raw.artifacts) || raw.artifacts.length === 0) {
    errors.push('artifacts must be a non-empty array (media runs claim deliverables)')
  } else {
    raw.artifacts.forEach((art, i) => {
      if (!isRecord(art)) {
        errors.push(`artifacts[${i}] must be an object`)
        return
      }
      if (typeof art.uri !== 'string' || art.uri.trim().length === 0) {
        errors.push(`artifacts[${i}].uri is required`)
      } else if (
        !art.uri.startsWith('gs://') &&
        !art.uri.startsWith('file://') &&
        !isAbsolute(art.uri)
      ) {
        errors.push(
          `artifacts[${i}].uri must be gs://, file://, or an absolute path`,
        )
      }
      if (typeof art.kind !== 'string' || art.kind.trim().length === 0) {
        errors.push(`artifacts[${i}].kind is required`)
      }
    })
  }

  if (!Array.isArray(raw.artifactChecks) || raw.artifactChecks.length === 0) {
    errors.push(
      'artifactChecks must be a non-empty array (object_exists only — never log markers)',
    )
  } else {
    raw.artifactChecks.forEach((check, i) => {
      if (!isRecord(check)) {
        errors.push(`artifactChecks[${i}] must be an object`)
        return
      }
      if (typeof check.uri !== 'string' || check.uri.trim().length === 0) {
        errors.push(`artifactChecks[${i}].uri is required`)
      }
      if (check.method === 'log_marker') {
        errors.push(
          `artifactChecks[${i}].method is log_marker — forbidden (use object_exists)`,
        )
      } else if (
        typeof check.method !== 'string' ||
        !ALLOWED_CHECK_METHODS.has(check.method)
      ) {
        errors.push(
          `artifactChecks[${i}].method must be object_exists (got ${String(check.method)})`,
        )
      }
    })

    // Every artifact URI should have a matching object_exists check
    if (Array.isArray(raw.artifacts)) {
      const checked = new Set(
        raw.artifactChecks
          .filter((c) => isRecord(c) && c.method === 'object_exists')
          .map((c) => (isRecord(c) ? String(c.uri) : '')),
      )
      for (const art of raw.artifacts) {
        if (isRecord(art) && typeof art.uri === 'string' && !checked.has(art.uri)) {
          errors.push(
            `artifact ${art.uri} has no object_exists check (log markers are not enough)`,
          )
        }
      }
    }
  }

  if (!isRecord(raw.gcsIndex)) {
    errors.push('gcsIndex is required')
  } else if (raw.gcsIndex.updated !== true) {
    errors.push('gcsIndex.updated must be true')
  }

  if (!isRecord(raw.costEstimate)) {
    errors.push('costEstimate is required')
  } else {
    if (raw.costEstimate.currency !== 'USD') {
      errors.push('costEstimate.currency must be USD')
    }
    const hasHours =
      typeof raw.costEstimate.gpuHours === 'number' &&
      Number.isFinite(raw.costEstimate.gpuHours)
    const hasUsd =
      typeof raw.costEstimate.estimatedUsd === 'number' &&
      Number.isFinite(raw.costEstimate.estimatedUsd)
    const hasNotes =
      typeof raw.costEstimate.notes === 'string' &&
      raw.costEstimate.notes.trim().length > 0
    if (!hasHours && !hasUsd && !hasNotes) {
      errors.push(
        'costEstimate needs gpuHours and/or estimatedUsd and/or notes (silence is not allowed)',
      )
    }
  }

  if (!isRecord(raw.privacy)) {
    errors.push('privacy is required')
  } else if (
    raw.privacy.attestation !== 'no_secrets_no_prospect_pii'
  ) {
    errors.push(
      'privacy.attestation must be "no_secrets_no_prospect_pii"',
    )
  }

  scanSecrets(raw, 'receipt', errors)

  return { ok: errors.length === 0, errors, warnings }
}

/**
 * Live existence checks for gs:// and local paths.
 * @param {Record<string, unknown>} receipt
 * @returns {ValidationResult}
 */
export const checkLiveArtifacts = (receipt) => {
  /** @type {string[]} */
  const errors = []
  /** @type {string[]} */
  const warnings = []

  if (!Array.isArray(receipt.artifacts)) {
    return { ok: false, errors: ['no artifacts'], warnings }
  }

  for (const art of receipt.artifacts) {
    if (!isRecord(art) || typeof art.uri !== 'string') continue
    const uri = art.uri
    if (uri.startsWith('gs://')) {
      try {
        execFileSync('gsutil', ['-q', 'stat', uri], {
          stdio: ['ignore', 'pipe', 'pipe'],
          encoding: 'utf8',
        })
      } catch {
        // Fall back to gcloud storage if gsutil missing
        try {
          execFileSync(
            'gcloud',
            ['storage', 'objects', 'describe', uri, '--format=value(name)'],
            { stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8' },
          )
        } catch {
          errors.push(`live check failed: object not found or inaccessible: ${uri}`)
        }
      }
    } else {
      const path = uri.startsWith('file://') ? uri.slice('file://'.length) : uri
      if (!existsSync(path)) {
        errors.push(`live check failed: local path missing: ${path}`)
      } else {
        try {
          const st = statSync(path)
          if (!st.isFile() && !st.isDirectory()) {
            warnings.push(`live check: unusual file type for ${path}`)
          }
        } catch (e) {
          errors.push(`live check failed: cannot stat ${path}: ${String(e)}`)
        }
      }
    }
  }

  return { ok: errors.length === 0, errors, warnings }
}

/**
 * @param {string[]} argv
 */
export const parseArgs = (argv) => {
  /** @type {{ receipt: string | null, liveArtifacts: boolean, selfTest: boolean, help: boolean }} */
  const out = {
    receipt: null,
    liveArtifacts: false,
    selfTest: false,
    help: false,
  }
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--receipt' || a === '-r') {
      out.receipt = argv[++i] ?? null
    } else if (a === '--live-artifacts') {
      out.liveArtifacts = true
    } else if (a === '--self-test') {
      out.selfTest = true
    } else if (a === '--help' || a === '-h') {
      out.help = true
    }
  }
  return out
}

const HERE = dirname(fileURLToPath(import.meta.url))
const EXAMPLE = resolve(HERE, '../fixtures/gpu-media-run-closeout.example.json')

export const runSelfTest = () => {
  const example = JSON.parse(readFileSync(EXAMPLE, 'utf8'))
  const good = validateCloseoutReceipt(example)
  if (!good.ok) {
    console.error('self-test FAIL: example fixture invalid', good.errors)
    return 1
  }

  const logMarker = structuredClone(example)
  logMarker.artifactChecks = [
    { uri: example.artifacts[0].uri, method: 'log_marker' },
  ]
  const badMarker = validateCloseoutReceipt(logMarker)
  if (badMarker.ok) {
    console.error('self-test FAIL: log_marker should be rejected')
    return 1
  }

  const noReason = structuredClone(example)
  noReason.hostDisposition = { status: 'left_running' }
  const badReason = validateCloseoutReceipt(noReason)
  if (badReason.ok) {
    console.error('self-test FAIL: left_running without reason should fail')
    return 1
  }

  console.log('self-test OK')
  return 0
}

const main = () => {
  const args = parseArgs(process.argv.slice(2))
  if (args.help) {
    console.log(`Usage:
  bun scripts/gpu-media-run-closeout.mjs --receipt <path.json> [--live-artifacts]
  bun scripts/gpu-media-run-closeout.mjs --self-test
`)
    process.exit(0)
  }
  if (args.selfTest) {
    process.exit(runSelfTest())
  }
  if (!args.receipt) {
    console.error('error: --receipt <path> is required (or --self-test)')
    process.exit(2)
  }

  const path = resolve(args.receipt)
  if (!existsSync(path)) {
    console.error(`error: receipt not found: ${path}`)
    process.exit(2)
  }

  let raw
  try {
    raw = JSON.parse(readFileSync(path, 'utf8'))
  } catch (e) {
    console.error(`error: invalid JSON: ${String(e)}`)
    process.exit(2)
  }

  const result = validateCloseoutReceipt(raw)
  if (!result.ok) {
    console.error('CLOSEOUT INVALID')
    for (const err of result.errors) console.error(`  - ${err}`)
    process.exit(1)
  }

  if (args.liveArtifacts) {
    const live = checkLiveArtifacts(/** @type {Record<string, unknown>} */ (raw))
    if (!live.ok) {
      console.error('CLOSEOUT SCHEMA OK but LIVE ARTIFACT CHECKS FAILED')
      for (const err of live.errors) console.error(`  - ${err}`)
      process.exit(1)
    }
    for (const w of live.warnings) console.warn(`  warn: ${w}`)
  }

  console.log(
    `CLOSEOUT OK — runId=${String(/** @type {Record<string, unknown>} */ (raw).runId)} artifacts=${Array.isArray(raw.artifacts) ? raw.artifacts.length : 0}`,
  )
  process.exit(0)
}

// Only run CLI when executed directly (not when imported by tests)
const isMain =
  typeof Bun !== 'undefined'
    ? Boolean(Bun.main) &&
      resolve(String(Bun.main)) === resolve(fileURLToPath(import.meta.url))
    : process.argv[1] &&
      resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))

if (isMain) {
  main()
}
