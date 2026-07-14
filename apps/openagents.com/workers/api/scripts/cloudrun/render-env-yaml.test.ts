import { describe, expect, test } from 'vitest'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'

const script = path.resolve(import.meta.dirname, 'render-env-yaml.ts')

const parseRenderedEnvironment = (source: string): Record<string, string> =>
  Object.fromEntries(
    source
      .trim()
      .split('\n')
      .map(line => {
        const separator = line.indexOf(': ')
        return [
          line.slice(0, separator),
          JSON.parse(line.slice(separator + 2)) as string,
        ]
      }),
  )

const renderEnvironment = async (
  target: 'production' | 'staging',
): Promise<Record<string, string>> => {
  const directory = await mkdtemp(path.join(tmpdir(), 'openagents-cloudrun-env-'))
  const output = path.join(directory, `${target}.yaml`)
  try {
    const rendered = spawnSync('bun', [script, target, output], {
      encoding: 'utf8',
    })
    expect(rendered.stderr).toBe('')
    expect(rendered.status).toBe(0)
    return parseRenderedEnvironment(await readFile(output, 'utf8'))
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
}

describe('Cloud Run env rendering', () => {
  // Sarah removed at owner direction 2026-07-10 (epic #8610): the renderer
  // must never re-emit SARAH_* mounts, and the monolith runtime vars stay.
  test('production renders monolith vars with no SARAH_* mounts', async () => {
    const environment = await renderEnvironment('production')

    expect(environment.OPENAGENTS_RUNTIME).toBe('cloudrun-monolith')
    expect(environment.OPENAGENTS_AUDIO_GATEWAY_URL).toBe(
      'wss://openagents-audio-edge-staging-ezxz4mgdsq-uc.a.run.app/v1/stream',
    )
    expect(environment.PORTAL_UI_DIR).toBe('/app/portal-ui')
    for (const key of Object.keys(environment)) {
      expect(key.startsWith('SARAH_')).toBe(false)
    }
  })

  test('staging renders monolith vars with no SARAH_* mounts', async () => {
    const environment = await renderEnvironment('staging')

    expect(environment.OPENAGENTS_RUNTIME).toBe('cloudrun-monolith')
    expect(environment.OPENAGENTS_AUDIO_GATEWAY_URL).toBe(
      'wss://openagents-audio-edge-staging-ezxz4mgdsq-uc.a.run.app/v1/stream',
    )
    expect(environment.PORTAL_UI_DIR).toBe('/app/portal-ui')
    for (const key of Object.keys(environment)) {
      expect(key.startsWith('SARAH_')).toBe(false)
    }
  })
})
