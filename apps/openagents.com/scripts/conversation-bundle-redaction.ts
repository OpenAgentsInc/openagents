#!/usr/bin/env bun

/**
 * Builds public-safe conversation bundles for NIP-DS data sales.
 *
 * The exporter is intentionally standalone: it reads local transcript files,
 * projects only role/text into a deterministic bundle, refuses high-risk secret
 * material, and writes a manifest whose digest matches the NIP-DS listing x tag.
 */
import {
  datasetListingToTags,
  makeDatasetListing,
  sha256Hex,
} from '@openagentsinc/nip90'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { basename, join } from 'node:path'

type Role = 'assistant' | 'system' | 'tool' | 'user'
type Flags = Record<string, ReadonlyArray<string> | true>

type RedactionClass =
  | 'absolute_path'
  | 'email'
  | 'long_hex'
  | 'payment_or_wallet_ref'
  | 'url'

type RedactionCounts = Record<RedactionClass, number>

type ConversationRecord = {
  readonly role: Role
  readonly sequence: number
  readonly sourceRef: string
  readonly text: string
}

type ConversationBundle = {
  readonly schema: 'openagents.conversation_bundle.v1'
  readonly createdAt: string
  readonly redactionPolicy: 'deny_by_default_conversation_text_v1'
  readonly records: ReadonlyArray<ConversationRecord>
}

type ConversationManifest = {
  readonly schema: 'openagents.conversation_bundle_manifest.v1'
  readonly bundleDigest: string
  readonly bundleFile: string
  readonly bundleBytes: number
  readonly createdAt: string
  readonly nipDs: {
    readonly datasetKind: 'conversation_bundle'
    readonly listingDigest: string
    readonly listingTags: ReadonlyArray<ReadonlyArray<string>>
  }
  readonly recordCount: number
  readonly redaction: {
    readonly counts: RedactionCounts
    readonly policy: 'deny_by_default_conversation_text_v1'
    readonly refusedSecretClasses: ReadonlyArray<string>
  }
}

export class ConversationBundleRefusal extends Error {
  readonly code = 'conversation_bundle_refused'
  readonly findings: ReadonlyArray<string>

  constructor(findings: ReadonlyArray<string>) {
    super(`Conversation bundle refused: ${findings.join(', ')}`)
    this.name = 'ConversationBundleRefusal'
    this.findings = findings
  }
}

const usage = () => `Usage:
  bun apps/openagents.com/scripts/conversation-bundle-redaction.ts build --input PATH [--input PATH] --out-dir PATH --title TITLE --d SLUG

Options:
  --generated-at ISO_TIME  Deterministic timestamp for tests/reproducible bundles.
  --summary TEXT           Listing summary for the manifest NIP-DS projection.
`

const redactionClasses: ReadonlyArray<RedactionClass> = [
  'absolute_path',
  'email',
  'long_hex',
  'payment_or_wallet_ref',
  'url',
]

const emptyCounts = (): RedactionCounts =>
  Object.fromEntries(redactionClasses.map(name => [name, 0])) as RedactionCounts

const highRiskSecretPatterns: ReadonlyArray<readonly [string, RegExp]> = [
  [
    'api_key',
    /\b[A-Z0-9_]*(?:API[_-]?KEY|ACCESS[_-]?TOKEN|AUTH[_-]?TOKEN|SECRET|WEBHOOK[_-]?SECRET)\s*[:=]\s*['"]?[A-Za-z0-9._~+/\-]{8,}/i,
  ],
  ['bearer_token', /\bBearer\s+[A-Za-z0-9._~+/\-]{8,}/i],
  ['github_token', /\bgh[pousr]_[A-Za-z0-9_]{12,}\b/i],
  [
    'lightning_payment_material',
    /\b(?:lnbc|lntb|lnbcrt|lno1|lnurl)[a-z0-9]{12,}\b/i,
  ],
  ['mdk_agent_token', /\boa_agent_[A-Za-z0-9._~+/\-]{8,}\b/i],
  ['openai_key', /\bsk-[A-Za-z0-9_-]{12,}\b/],
  ['private_key', /-----BEGIN [A-Z ]*PRIVATE KEY-----/i],
  [
    'wallet_seed',
    /\b(?:mnemonic|recovery[_ -]?phrase|seed[_ -]?phrase)\s*[:=]\s*["']?[a-z]+(?:\s+[a-z]+){10,23}\b/i,
  ],
  ['xprv', /\bxprv[a-zA-Z0-9]{20,}\b/],
]

const countMatches = (text: string, pattern: RegExp): number => {
  const flags = pattern.flags.includes('g')
    ? pattern.flags
    : `${pattern.flags}g`
  return Array.from(text.matchAll(new RegExp(pattern.source, flags))).length
}

const replaceAndCount = (
  text: string,
  counts: RedactionCounts,
  name: RedactionClass,
  pattern: RegExp,
  replacement: string,
): string => {
  counts[name] += countMatches(text, pattern)
  return text.replace(pattern, replacement)
}

export const findHighRiskSecrets = (text: string): ReadonlyArray<string> =>
  highRiskSecretPatterns
    .filter(([, pattern]) => pattern.test(text))
    .map(([name]) => name)

export const sanitizePublicText = (
  input: string,
  counts: RedactionCounts,
): string => {
  let text = input.replace(/\s+/g, ' ').trim()
  text = replaceAndCount(
    text,
    counts,
    'email',
    /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi,
    '[redacted-email]',
  )
  text = replaceAndCount(
    text,
    counts,
    'absolute_path',
    /(?:\/Users|\/home|\/var\/folders)\/[^\s"'`]+/g,
    '[redacted-path]',
  )
  text = replaceAndCount(
    text,
    counts,
    'url',
    /https?:\/\/[^\s"'`]+/gi,
    '[redacted-url]',
  )
  text = replaceAndCount(
    text,
    counts,
    'payment_or_wallet_ref',
    /\b(?:wallet|payment|invoice|preimage|payout)[._:-][A-Za-z0-9._:-]{6,}\b/gi,
    '[redacted-payment-ref]',
  )
  text = replaceAndCount(
    text,
    counts,
    'long_hex',
    /\b[a-f0-9]{40,}\b/gi,
    '[redacted-hex]',
  )
  return text
}

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value)
  }
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`
  }
  const object = value as Record<string, unknown>
  return `{${Object.keys(object)
    .sort()
    .map(key => `${JSON.stringify(key)}:${canonicalJson(object[key])}`)
    .join(',')}}`
}

const parseFlags = (
  argv: ReadonlyArray<string>,
): { command: string; flags: Flags } => {
  const [command = 'help', ...rest] = argv
  const flags: Record<string, Array<string> | true> = {}

  for (let index = 0; index < rest.length; index++) {
    const arg = rest[index]
    if (!arg.startsWith('--')) continue
    const name = arg.slice(2)
    const next = rest[index + 1]
    if (next === undefined || next.startsWith('--')) {
      flags[name] = true
      continue
    }
    const existing = flags[name]
    flags[name] = Array.isArray(existing) ? [...existing, next] : [next]
    index++
  }

  return { command, flags }
}

const requireValues = (flags: Flags, name: string): ReadonlyArray<string> => {
  const value = flags[name]
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`Missing --${name}`)
  }
  return value
}

const requireValue = (flags: Flags, name: string): string =>
  requireValues(flags, name)[0] ?? ''

const optionalValue = (
  flags: Flags,
  name: string,
  fallback: string,
): string => {
  const value = flags[name]
  return Array.isArray(value) && value[0] ? value[0] : fallback
}

const normalizeRole = (value: unknown): Role | undefined =>
  value === 'assistant' ||
  value === 'system' ||
  value === 'tool' ||
  value === 'user'
    ? value
    : undefined

const textFromContent = (value: unknown): string | undefined => {
  if (typeof value === 'string') return value
  if (!Array.isArray(value)) return undefined
  const parts = value.flatMap(part => {
    if (typeof part === 'string') return [part]
    if (part && typeof part === 'object') {
      const object = part as Record<string, unknown>
      if (typeof object.text === 'string') return [object.text]
      if (typeof object.content === 'string') return [object.content]
    }
    return []
  })
  return parts.length > 0 ? parts.join('\n') : undefined
}

const extractRecordCandidate = (
  value: unknown,
): { role: Role; text: string } | undefined => {
  if (!value || typeof value !== 'object') return undefined
  const object = value as Record<string, unknown>
  const message =
    object.message && typeof object.message === 'object'
      ? (object.message as Record<string, unknown>)
      : object
  const role = normalizeRole(message.role ?? object.role ?? object.type)
  const text =
    textFromContent(message.content) ??
    textFromContent(object.content) ??
    textFromContent(message.text) ??
    textFromContent(object.text)

  if (!role || !text) return undefined
  return { role, text }
}

const parseInputRecords = (
  fileText: string,
): ReadonlyArray<{ role: Role; text: string }> => {
  const trimmed = fileText.trim()
  if (trimmed.length === 0) return []

  const parsedWhole = (() => {
    try {
      return JSON.parse(trimmed) as unknown
    } catch {
      return undefined
    }
  })()
  if (Array.isArray(parsedWhole)) {
    return parsedWhole.flatMap(value => {
      const candidate = extractRecordCandidate(value)
      return candidate ? [candidate] : []
    })
  }
  const wholeCandidate = extractRecordCandidate(parsedWhole)
  if (wholeCandidate) return [wholeCandidate]

  return trimmed.split(/\r?\n/).flatMap(line => {
    try {
      const candidate = extractRecordCandidate(JSON.parse(line) as unknown)
      return candidate ? [candidate] : []
    } catch {
      return []
    }
  })
}

export const buildConversationBundle = async (options: {
  readonly createdAt: string
  readonly inputs: ReadonlyArray<string>
  readonly title: string
  readonly d: string
  readonly summary: string
}): Promise<{
  readonly bundle: ConversationBundle
  readonly bundlePayload: string
  readonly digest: string
  readonly manifest: ConversationManifest
}> => {
  const counts = emptyCounts()
  const findings = new Set<string>()
  const records: Array<ConversationRecord> = []

  for (const [fileIndex, input] of options.inputs.entries()) {
    const fileText = await readFile(input, 'utf8')
    for (const finding of findHighRiskSecrets(fileText)) {
      findings.add(finding)
    }
    const candidates = parseInputRecords(fileText)
    for (const candidate of candidates) {
      const text = sanitizePublicText(candidate.text, counts)
      if (text.length === 0) continue
      records.push({
        role: candidate.role,
        sequence: records.length + 1,
        sourceRef: `input.${fileIndex + 1}`,
        text,
      })
    }
  }

  if (findings.size > 0) {
    throw new ConversationBundleRefusal([...findings].sort())
  }

  const bundle: ConversationBundle = {
    schema: 'openagents.conversation_bundle.v1',
    createdAt: options.createdAt,
    redactionPolicy: 'deny_by_default_conversation_text_v1',
    records,
  }
  const bundlePayload = canonicalJson(bundle)
  const digest = sha256Hex(bundlePayload)
  const listing = makeDatasetListing({
    d: options.d,
    title: options.title,
    x: digest,
    publishedAt: Math.floor(Date.parse(options.createdAt) / 1000),
    content: options.summary,
    summary: options.summary,
    datasetKind: 'conversation_bundle',
    mime: 'application/json',
    size: new TextEncoder().encode(bundlePayload).byteLength,
    access: 'paid',
    delivery: ['nip90', 'download'],
    topics: ['dataset', 'conversation-bundle', 'openagents'],
  })
  const listingTags = datasetListingToTags(listing).map(tag => [...tag])

  const manifest: ConversationManifest = {
    schema: 'openagents.conversation_bundle_manifest.v1',
    bundleDigest: digest,
    bundleFile: 'conversation-bundle.json',
    bundleBytes: new TextEncoder().encode(bundlePayload).byteLength,
    createdAt: options.createdAt,
    nipDs: {
      datasetKind: 'conversation_bundle',
      listingDigest: digest,
      listingTags,
    },
    recordCount: records.length,
    redaction: {
      counts,
      policy: 'deny_by_default_conversation_text_v1',
      refusedSecretClasses: highRiskSecretPatterns.map(([name]) => name),
    },
  }

  if (findHighRiskSecrets(bundlePayload).length > 0) {
    throw new ConversationBundleRefusal(['post_redaction_secret_survived'])
  }

  return { bundle, bundlePayload, digest, manifest }
}

export const writeConversationBundle = async (options: {
  readonly createdAt: string
  readonly d: string
  readonly inputs: ReadonlyArray<string>
  readonly outDir: string
  readonly summary: string
  readonly title: string
}) => {
  const built = await buildConversationBundle(options)
  await mkdir(options.outDir, { recursive: true })
  const bundleFile = join(options.outDir, built.manifest.bundleFile)
  const manifestFile = join(options.outDir, 'manifest.json')
  await writeFile(bundleFile, built.bundlePayload)
  await writeFile(manifestFile, `${canonicalJson(built.manifest)}\n`)
  return {
    ...built,
    bundleFile,
    manifestFile,
  }
}

const runBuild = async (flags: Flags) => {
  const outDir = requireValue(flags, 'out-dir')
  const title = requireValue(flags, 'title')
  const d = requireValue(flags, 'd')
  const inputs = requireValues(flags, 'input')
  const createdAt = optionalValue(
    flags,
    'generated-at',
    new Date().toISOString(),
  )
  const summary = optionalValue(
    flags,
    'summary',
    'Public-safe redacted conversation bundle prepared for NIP-DS sale.',
  )
  const result = await writeConversationBundle({
    createdAt,
    d,
    inputs,
    outDir,
    summary,
    title,
  })
  console.log(
    JSON.stringify(
      {
        ok: true,
        bundleFile: basename(result.bundleFile),
        digest: result.digest,
        manifestFile: basename(result.manifestFile),
        recordCount: result.manifest.recordCount,
      },
      null,
      2,
    ),
  )
}

if (import.meta.main) {
  const { command, flags } = parseFlags(process.argv.slice(2))
  if (command === 'build') {
    try {
      await runBuild(flags)
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      console.error(message)
      process.exit(1)
    }
  } else {
    console.log(usage())
    process.exit(command === 'help' || command === '--help' ? 0 : 1)
  }
}
