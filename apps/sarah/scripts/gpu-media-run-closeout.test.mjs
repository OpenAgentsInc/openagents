import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  SCHEMA_VERSION,
  validateCloseoutReceipt,
  runSelfTest,
} from './gpu-media-run-closeout.mjs'

const HERE = dirname(fileURLToPath(import.meta.url))
const EXAMPLE = resolve(HERE, '../fixtures/gpu-media-run-closeout.example.json')

const loadExample = () => JSON.parse(readFileSync(EXAMPLE, 'utf8'))

describe('SQ-8 GPU media-run closeout (#8625)', () => {
  test('example fixture validates', () => {
    const result = validateCloseoutReceipt(loadExample())
    expect(result.ok).toBe(true)
    expect(result.errors).toEqual([])
  })

  test('schema version is pinned', () => {
    expect(SCHEMA_VERSION).toBe('openagents.sarah.gpu_media_run_closeout.v1')
    const bad = loadExample()
    bad.schemaVersion = 'wrong'
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
    expect(result.errors.some((e) => e.includes('schemaVersion'))).toBe(true)
  })

  test('rejects log_marker artifact checks (the law that burned us twice)', () => {
    const bad = loadExample()
    bad.artifactChecks = [
      { uri: bad.artifacts[0].uri, method: 'log_marker' },
    ]
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
    expect(result.errors.some((e) => e.includes('log_marker'))).toBe(true)
  })

  test('rejects missing object_exists for a claimed artifact', () => {
    const bad = loadExample()
    bad.artifactChecks = [bad.artifactChecks[0]] // drop second
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
    expect(result.errors.some((e) => e.includes('no object_exists check'))).toBe(
      true,
    )
  })

  test('rejects left_running without reason', () => {
    const bad = loadExample()
    bad.hostDisposition = { status: 'left_running' }
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
    expect(result.errors.some((e) => e.includes('reason'))).toBe(true)
  })

  test('allows stopped without reason', () => {
    const good = loadExample()
    good.hostDisposition = { status: 'stopped' }
    const result = validateCloseoutReceipt(good)
    expect(result.ok).toBe(true)
  })

  test('rejects empty artifacts', () => {
    const bad = loadExample()
    bad.artifacts = []
    bad.artifactChecks = []
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
  })

  test('rejects gcsIndex.updated !== true', () => {
    const bad = loadExample()
    bad.gcsIndex = { updated: false }
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
  })

  test('rejects missing cost estimate substance', () => {
    const bad = loadExample()
    bad.costEstimate = { currency: 'USD' }
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
  })

  test('rejects wrong privacy attestation', () => {
    const bad = loadExample()
    bad.privacy = { attestation: 'trust_me' }
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
  })

  test('rejects secret-shaped content in notes', () => {
    const bad = loadExample()
    bad.notes = 'used api_key=sk-abcdefghijklmnopqrstuv'
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
    expect(result.errors.some((e) => e.includes('secret'))).toBe(true)
  })

  test('rejects prospect-like emails', () => {
    const bad = loadExample()
    bad.notes = 'spoke with founder@acme-corp.example.org'
    // example.org still matches EMAILISH - wait, I only allow example.com
    // acme-corp.example.org should fail
    const result = validateCloseoutReceipt(bad)
    expect(result.ok).toBe(false)
  })

  test('self-test harness passes', () => {
    expect(runSelfTest()).toBe(0)
  })
})
