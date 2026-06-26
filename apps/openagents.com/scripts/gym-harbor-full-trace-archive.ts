#!/usr/bin/env bun

/**
 * Bundle a Harbor job directory and upload the full raw trace archive to the
 * operator-only Gym archive endpoint.
 *
 * This intentionally ships RAW PRIVATE EVIDENCE. Do not paste the resulting
 * tarball contents into issues, public docs, or public projections.
 */
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { lstat, mkdir, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'

export const DEFAULT_BASE_URL = 'https://openagents.com'
export const ARCHIVE_PATH = '/api/operator/gym/full-trace-archives'

type ParsedArgs = Readonly<{
  archiveRef?: string
  baseUrl: string
  captureStartedAt?: string
  dryRun: boolean
  jobDir: string
  jobRef: string
  json: boolean
  outDir: string
  runRef: string
}>

type ScriptIO = Readonly<{
  fetchImpl?: typeof fetch
  stderr?: Pick<typeof process.stderr, 'write'>
  stdout?: Pick<typeof process.stdout, 'write'>
}>

const valueFlags = new Set([
  'archive-ref',
  'base-url',
  'capture-started-at',
  'job-dir',
  'job-ref',
  'out-dir',
  'run-ref',
])
const booleanFlags = new Set(['dry-run', 'help', 'h', 'json'])
const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,239}$/

export const usage = () => `Usage:
  bun scripts/gym-harbor-full-trace-archive.ts \\
    --job-dir /tmp/khala-tb/khala-tb-1782410587 \\
    --run-ref run.gym.terminal_bench.khala-live \\
    --job-ref job.gym.harbor_terminal_bench.khala-tb-1782410587

Options:
  --archive-ref <ref>          Optional stable archive ref. Defaults server-side from SHA-256.
  --base-url <url>             Defaults to OPENAGENTS_BASE_URL or ${DEFAULT_BASE_URL}.
  --capture-started-at <iso>   Optional capture start timestamp.
  --job-dir <path>             Harbor job directory to archive.
  --job-ref <ref>              Public-safe job ref for D1 metadata.
  --out-dir <path>             Local archive output directory.
  --run-ref <ref>              Public-safe run ref for D1 metadata.
  --dry-run                    Create and hash the archive but do not upload.
  --json                       Print machine-readable output.
  --help                       Show this help.

Environment:
  OPENAGENTS_ADMIN_API_TOKEN   Required unless --dry-run.
  OPENAGENTS_BASE_URL          Optional base URL.
  GYM_HARBOR_JOB_DIR           Optional default for --job-dir.
  GYM_HARBOR_RUN_REF           Optional default for --run-ref.
  GYM_HARBOR_JOB_REF           Optional default for --job-ref.`

const canonicalFlagName = (name: string): string =>
  ({
    baseUrl: 'base-url',
    captureStartedAt: 'capture-started-at',
    h: 'help',
    jobDir: 'job-dir',
    jobRef: 'job-ref',
    outDir: 'out-dir',
    runRef: 'run-ref',
  })[name] ?? name

const redactSecrets = (text: string, token?: string): string => {
  const redacted = text
    .replace(/Bearer\s+[A-Za-z0-9._~+/-]+/g, 'Bearer <redacted>')
    .replace(
      /OPENAGENTS_ADMIN_API_TOKEN=[^\s]+/g,
      'OPENAGENTS_ADMIN_API_TOKEN=<redacted>',
    )
    .replace(/oa_admin_[A-Za-z0-9._-]+/g, 'oa_admin_<redacted>')

  return token === undefined || token.length < 3
    ? redacted
    : redacted.replaceAll(token, '<redacted:value>')
}

const flagValue = (
  flags: Map<string, string | boolean>,
  name: string,
): string | undefined => {
  const value = flags.get(name)
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : undefined
}

const requireRef = (name: string, value: string | undefined): string => {
  if (value === undefined || value.trim() === '') {
    throw new Error(`Missing ${name}.`)
  }
  if (!SAFE_REF_PATTERN.test(value.trim())) {
    throw new Error(`${name} must be a bounded public-safe ref token.`)
  }
  return value.trim()
}

const requirePath = (name: string, value: string | undefined): string => {
  if (value === undefined || value.trim() === '') {
    throw new Error(`Missing ${name}.`)
  }
  return value.trim()
}

export const parseArgs = (
  argv: ReadonlyArray<string>,
  env: NodeJS.ProcessEnv,
): ParsedArgs | Readonly<{ help: true }> => {
  const flags = new Map<string, string | boolean>()

  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index]
    if (!raw?.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${raw}`)
    }

    const name = canonicalFlagName(raw.slice(2))
    if (booleanFlags.has(name)) {
      flags.set(name, true)
      continue
    }
    if (!valueFlags.has(name)) {
      throw new Error(`Unknown option: ${raw}`)
    }

    const value = argv[index + 1]
    if (value === undefined || value.startsWith('--')) {
      throw new Error(`Missing value for ${raw}.`)
    }
    flags.set(name, value)
    index += 1
  }

  if (flags.get('help') === true) {
    return { help: true }
  }

  const jobDir = flagValue(flags, 'job-dir') ?? env.GYM_HARBOR_JOB_DIR
  const runRef = flagValue(flags, 'run-ref') ?? env.GYM_HARBOR_RUN_REF
  const jobRef = flagValue(flags, 'job-ref') ?? env.GYM_HARBOR_JOB_REF
  const archiveRef = flagValue(flags, 'archive-ref')
  if (archiveRef !== undefined && !SAFE_REF_PATTERN.test(archiveRef)) {
    throw new Error('--archive-ref must be a bounded public-safe ref token.')
  }

  return {
    ...(archiveRef === undefined ? {} : { archiveRef }),
    baseUrl:
      flagValue(flags, 'base-url') ??
      env.OPENAGENTS_BASE_URL ??
      DEFAULT_BASE_URL,
    ...(flagValue(flags, 'capture-started-at') === undefined
      ? {}
      : { captureStartedAt: flagValue(flags, 'capture-started-at') }),
    dryRun: flags.get('dry-run') === true,
    jobDir: requirePath('--job-dir', jobDir),
    jobRef: requireRef('--job-ref', jobRef),
    json: flags.get('json') === true,
    outDir:
      flagValue(flags, 'out-dir') ??
      join(tmpdir(), 'openagents-harbor-full-trace-archives'),
    runRef: requireRef('--run-ref', runRef),
  }
}

const safeFileSegment = (value: string): string => {
  const safe = value.replace(/[^A-Za-z0-9_.-]+/g, '_').replace(/^_+|_+$/g, '')
  return safe === '' ? 'archive' : safe.slice(0, 120)
}

const sha256File = async (path: string): Promise<string> => {
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(path)) {
    hash.update(chunk)
  }
  return hash.digest('hex')
}

const makeArchive = async (
  parsed: ParsedArgs,
): Promise<
  Readonly<{ archivePath: string; bytes: number; sha256: string }>
> => {
  const jobDir = resolve(parsed.jobDir)
  const jobDirStat = await lstat(jobDir)
  if (!jobDirStat.isDirectory()) {
    throw new Error(`--job-dir is not a directory: ${jobDir}`)
  }

  await mkdir(parsed.outDir, { recursive: true })
  const stamp = new Date()
    .toISOString()
    .replace(/[-:]/g, '')
    .replace(/\..+$/, 'Z')
  const archivePath = join(
    resolve(parsed.outDir),
    `${safeFileSegment(parsed.jobRef)}-${stamp}.tar.gz`,
  )
  const tar = spawnSync(
    'tar',
    ['-czf', archivePath, '-C', dirname(jobDir), basename(jobDir)],
    {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  )
  if (tar.status !== 0) {
    throw new Error(`tar failed: ${tar.stderr || tar.stdout}`)
  }

  const archiveStat = await stat(archivePath)
  return {
    archivePath,
    bytes: archiveStat.size,
    sha256: await sha256File(archivePath),
  }
}

const uploadArchive = async (
  parsed: ParsedArgs,
  archive: Readonly<{ archivePath: string; bytes: number; sha256: string }>,
  env: NodeJS.ProcessEnv,
  fetchImpl: typeof fetch,
) => {
  const token = env.OPENAGENTS_ADMIN_API_TOKEN
  if (token === undefined || token.trim() === '') {
    throw new Error('Missing OPENAGENTS_ADMIN_API_TOKEN.')
  }

  const url = new URL(ARCHIVE_PATH, parsed.baseUrl)
  const headers: Record<string, string> = {
    accept: 'application/json',
    authorization: `Bearer ${token}`,
    'content-type': 'application/gzip',
    'x-openagents-archive-bytes': String(archive.bytes),
    'x-openagents-archive-sha256': archive.sha256,
    'x-openagents-capture-completed-at': new Date().toISOString(),
    'x-openagents-job-ref': parsed.jobRef,
    'x-openagents-run-ref': parsed.runRef,
  }
  if (parsed.archiveRef !== undefined) {
    headers['x-openagents-archive-ref'] = parsed.archiveRef
  }
  if (parsed.captureStartedAt !== undefined) {
    headers['x-openagents-capture-started-at'] = parsed.captureStartedAt
  }

  const response = await fetchImpl(url, {
    body: Bun.file(archive.archivePath),
    headers,
    method: 'POST',
  })
  const text = await response.text()
  if (!response.ok) {
    throw new Error(`Upload failed (${response.status}): ${text}`)
  }
  return text === '' ? {} : JSON.parse(text)
}

export const runHarborFullTraceArchive = async (
  argv: ReadonlyArray<string>,
  env: NodeJS.ProcessEnv = process.env,
  io: ScriptIO = {},
): Promise<number> => {
  const stdout = io.stdout ?? process.stdout
  const stderr = io.stderr ?? process.stderr
  const fetchImpl = io.fetchImpl ?? fetch

  try {
    const parsed = parseArgs(argv, env)
    if ('help' in parsed) {
      stdout.write(`${usage()}\n`)
      return 0
    }

    const archive = await makeArchive(parsed)
    if (parsed.dryRun) {
      const result = {
        archivePath: archive.archivePath,
        bytes: archive.bytes,
        dryRun: true,
        jobRef: parsed.jobRef,
        runRef: parsed.runRef,
        sha256: archive.sha256,
      }
      stdout.write(
        parsed.json
          ? `${JSON.stringify(result, null, 2)}\n`
          : `Created archive ${archive.archivePath} (${archive.bytes} bytes, sha256 ${archive.sha256}); dry run, not uploaded.\n`,
      )
      return 0
    }

    const response = await uploadArchive(parsed, archive, env, fetchImpl)
    stdout.write(
      parsed.json
        ? `${JSON.stringify(response, null, 2)}\n`
        : `Uploaded Harbor full trace archive ${response.archive?.archiveRef ?? ''} (${archive.bytes} bytes, sha256 ${archive.sha256}).\n`,
    )
    return 0
  } catch (error) {
    const token = env.OPENAGENTS_ADMIN_API_TOKEN
    const message = error instanceof Error ? error.message : String(error)
    stderr.write(`${redactSecrets(message, token)}\n`)
    return 1
  }
}

if (import.meta.main) {
  process.exitCode = await runHarborFullTraceArchive(process.argv.slice(2))
}
