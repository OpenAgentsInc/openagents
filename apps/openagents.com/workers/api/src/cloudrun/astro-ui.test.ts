import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, test } from 'vitest'

import { assertAstroUiArtifactsExist, handleAstroUiRequest } from './astro-ui'

const temporaryDirectories: Array<string> = []

const makeAstroFixture = async (): Promise<string> => {
  const directory = await mkdtemp(path.join(tmpdir(), 'openagents-astro-ui-'))
  temporaryDirectories.push(directory)
  await mkdir(path.join(directory, '_astro'))
  await mkdir(path.join(directory, 'install'))
  await writeFile(
    path.join(directory, 'index.html'),
    '<!doctype html><title>OpenAgents Desktop</title><link rel="stylesheet" href="/astro/_astro/site.css">',
  )
  await writeFile(
    path.join(directory, 'install', 'index.html'),
    '<!doctype html><title>Install OpenAgents for Mac</title><a href="https://github.com/OpenAgentsInc/openagents/releases/download/openagents-desktop-v0.1.0-rc.12/OpenAgents-0.1.0-rc.12-arm64.dmg">Download for Mac</a>',
  )
  await writeFile(
    path.join(directory, '_astro', 'site.css'),
    'body{color:white}',
  )
  return directory
}

afterEach(async () => {
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map(directory => rm(directory, { recursive: true, force: true })),
  )
})

describe('Astro landing serving', () => {
  test('does not own non-Astro paths', async () => {
    const directory = await makeAstroFixture()
    for (const pathname of [
      '/',
      '/promises',
      '/api/openapi.json',
      '/astrology',
      '/installer',
    ]) {
      expect(
        await handleAstroUiRequest(
          new Request(`https://openagents.com${pathname}`),
          directory,
        ),
      ).toBeUndefined()
    }
  })

  test('serves the standalone Mac install page', async () => {
    const directory = await makeAstroFixture()

    for (const pathname of ['/install', '/install/']) {
      const response = await handleAstroUiRequest(
        new Request(`https://openagents.com${pathname}`),
        directory,
      )
      expect(response?.status).toBe(200)
      expect(response?.headers.get('content-type')).toContain('text/html')
      expect(await response!.text()).toContain('Download for Mac')
    }
  })

  test('serves the Astro document at both canonical root forms', async () => {
    const directory = await makeAstroFixture()
    assertAstroUiArtifactsExist(directory)

    for (const pathname of ['/astro', '/astro/']) {
      const response = await handleAstroUiRequest(
        new Request(`https://openagents.com${pathname}`),
        directory,
      )
      expect(response?.status).toBe(200)
      expect(response?.headers.get('content-type')).toContain('text/html')
      expect(await response!.text()).toContain('OpenAgents Desktop')
    }
  })

  test('serves hashed assets immutably and supports HEAD', async () => {
    const directory = await makeAstroFixture()
    const response = await handleAstroUiRequest(
      new Request('https://openagents.com/astro/_astro/site.css', {
        method: 'HEAD',
      }),
      directory,
    )
    expect(response?.status).toBe(200)
    expect(response?.headers.get('content-type')).toContain('text/css')
    expect(response?.headers.get('cache-control')).toContain('immutable')
    expect(await response!.text()).toBe('')
  })

  test('returns bounded errors for unsupported requests', async () => {
    const directory = await makeAstroFixture()
    const missing = await handleAstroUiRequest(
      new Request('https://openagents.com/astro/nope.js'),
      directory,
    )
    expect(missing?.status).toBe(404)

    const post = await handleAstroUiRequest(
      new Request('https://openagents.com/astro', { method: 'POST' }),
      directory,
    )
    expect(post?.status).toBe(405)
    expect(post?.headers.get('allow')).toBe('GET, HEAD')

    const installPost = await handleAstroUiRequest(
      new Request('https://openagents.com/install', { method: 'POST' }),
      directory,
    )
    expect(installPost?.status).toBe(405)
  })
})
