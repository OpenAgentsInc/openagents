#!/usr/bin/env node

import { execFile } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { promisify } from 'node:util'

const repoRoot = resolve(import.meta.dirname, '..')
const execFileAsync = promisify(execFile)
const deprecatedTranscriptPrefix = [
  'https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main',
  'docs/deprecated/transcripts',
].join('/')
const founderTranscriptUrl =
  'https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main/docs/transcripts/230.md'
const thisScriptPath = 'scripts/check-live-agent-doc-links.mjs'

const sourceFiles = [
  'docs/live/AGENTS.md',
  'apps/web/public/AGENTS.md',
  'docs/live/HEARTBEAT.md',
  'docs/live/RULES.md',
  'docs/live/skill.json',
]

const criticalUrls = [
  'https://openagents.com/AGENTS.md',
  'https://openagents.com/.well-known/openagents.json',
  'https://openagents.com/api/openapi.json',
  'https://openagents.com/HEARTBEAT.md',
  'https://openagents.com/RULES.md',
  'https://openagents.com/skill.json',
  founderTranscriptUrl,
]

const scannedFileExtensions = ['.json', '.js', '.mjs', '.md', '.ts', '.tsx']
const liveUrlUserAgent = 'openagents-agent-doc-link-check/1.0'
const liveFetchTimeoutMs = 45_000

const readRepoFile = async path => readFile(resolve(repoRoot, path), 'utf8')

const hasScannedExtension = path =>
  scannedFileExtensions.some(extension => path.endsWith(extension))

const trackedSourceFiles = async () => {
  // Resolve tracked files relative to repoRoot regardless of any inherited
  // GIT_DIR / GIT_WORK_TREE. A `git push` hook exports GIT_DIR, which would
  // otherwise make `git ls-files` enumerate from the true repo root and yield
  // paths that then resolve incorrectly against repoRoot. Stripping those env
  // vars and using `-C repoRoot` keeps this script correct in both the normal
  // chained `check:deploy` run and the pre-push hook context.
  const { GIT_DIR, GIT_WORK_TREE, ...cleanEnv } = process.env
  const { stdout } = await execFileAsync('git', ['-C', repoRoot, 'ls-files'], {
    cwd: repoRoot,
    env: cleanEnv,
  })

  return stdout
    .split('\n')
    .filter(path => path.length > 0)
    .filter(path => path !== thisScriptPath)
    .filter(hasScannedExtension)
}

const assertSourceLinks = async () => {
  const trackedFiles = await trackedSourceFiles()
  const trackedEntries = await Promise.all(
    trackedFiles.map(async path => [path, await readRepoFile(path)]),
  )
  const companionEntries = await Promise.all(
    sourceFiles.map(async path => [path, await readRepoFile(path)]),
  )
  const failures = []

  trackedEntries
    .filter(([, contents]) => contents.includes(deprecatedTranscriptPrefix))
    .forEach(([path]) => {
      failures.push(
        `${path} contains deprecated transcript prefix ${deprecatedTranscriptPrefix}`,
      )
    })

  const agentDoc =
    companionEntries.find(([path]) => path === 'docs/live/AGENTS.md')?.[1] ?? ''

  if (!agentDoc.includes(founderTranscriptUrl)) {
    failures.push(
      `docs/live/AGENTS.md must include founder transcript URL ${founderTranscriptUrl}`,
    )
  }

  if (failures.length > 0) {
    throw new Error(failures.join('\n'))
  }
}

const assertReadableBody = async (url, response) => {
  if (response.body === null) {
    throw new Error(`${url} did not expose a readable response body`)
  }

  const reader = response.body.getReader()

  try {
    const firstChunk = await reader.read()

    if (firstChunk.done || firstChunk.value.byteLength === 0) {
      throw new Error(`${url} returned an empty response body`)
    }
  } finally {
    await reader.cancel().catch(() => {})
  }
}

const fetchLiveUrl = async (url, { requireBody = false } = {}) => {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), liveFetchTimeoutMs)

  try {
    const response = await fetch(url, {
      headers: { 'user-agent': liveUrlUserAgent },
      redirect: 'follow',
      signal: controller.signal,
    })

    if (!response.ok) {
      throw new Error(`${url} returned ${response.status}`)
    }

    if (requireBody) {
      return response.text()
    }

    await assertReadableBody(url, response)
    return ''
  } catch (error) {
    if (requireBody && error instanceof Error && error.name === 'AbortError') {
      const { stdout } = await execFileAsync(
        'curl',
        [
          '-fsSL',
          '--max-time',
          String(Math.ceil(liveFetchTimeoutMs / 1000)),
          '-A',
          liveUrlUserAgent,
          url,
        ],
        { cwd: repoRoot, maxBuffer: 1024 * 1024 },
      )

      return stdout
    }

    throw error
  } finally {
    clearTimeout(timeout)
  }
}

const assertCriticalUrls = async () => {
  const results = []

  for (const url of criticalUrls) {
    try {
      const requiresFullText = url === founderTranscriptUrl
      const text = await fetchLiveUrl(url, { requireBody: requiresFullText })

      if (requiresFullText) {
        const hasExpectedTranscript =
          text.includes('Calling All Agents') &&
          text.includes('Pay the People')

        if (!hasExpectedTranscript) {
          results.push(`${url} did not return the Episode 230 transcript body`)
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      results.push(`${url} failed live check: ${message}`)
    }
  }

  const failures = results.filter(result => result !== null)

  if (failures.length > 0) {
    throw new Error(failures.join('\n'))
  }
}

await assertSourceLinks()
await assertCriticalUrls()

console.log('Live agent doc links checked')
