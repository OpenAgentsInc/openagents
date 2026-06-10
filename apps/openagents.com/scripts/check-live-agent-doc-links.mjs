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

const readRepoFile = async path => readFile(resolve(repoRoot, path), 'utf8')

const hasScannedExtension = path =>
  scannedFileExtensions.some(extension => path.endsWith(extension))

const trackedSourceFiles = async () => {
  const { stdout } = await execFileAsync('git', ['ls-files'], { cwd: repoRoot })

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

const fetchText = async url => {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 15_000)

  try {
    const response = await fetch(url, {
      headers: { 'user-agent': 'openagents-agent-doc-link-check/1.0' },
      redirect: 'follow',
      signal: controller.signal,
    })

    if (!response.ok) {
      throw new Error(`${url} returned ${response.status}`)
    }

    return response.text()
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      const { stdout } = await execFileAsync(
        'curl',
        [
          '-fsSL',
          '--max-time',
          '15',
          '-A',
          'openagents-agent-doc-link-check/1.0',
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
  const results = await Promise.all(
    criticalUrls.map(async url => {
      try {
        const text = await fetchText(url)

        if (url === founderTranscriptUrl) {
          const hasExpectedTranscript =
            text.includes('Calling All Agents') &&
            text.includes('Pay the People')

          if (!hasExpectedTranscript) {
            return `${url} did not return the Episode 230 transcript body`
          }
        }

        return null
      } catch (error) {
        return error instanceof Error ? error.message : String(error)
      }
    }),
  )
  const failures = results.filter(result => result !== null)

  if (failures.length > 0) {
    throw new Error(failures.join('\n'))
  }
}

await assertSourceLinks()
await assertCriticalUrls()

console.log('Live agent doc links checked')
