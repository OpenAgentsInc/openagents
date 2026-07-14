import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsAgentCorePath,
  OpenAgentsAgentCoreSha256,
  OpenAgentsAgentCoreUrl,
  OpenAgentsAgentOnboardingCanonicalPath,
  OpenAgentsAgentOnboardingLastUpdated,
  OpenAgentsAgentOnboardingSha256,
  OpenAgentsAgentOnboardingVersion,
} from './openagents-agent-onboarding'
import {
  handleOpenAgentsAgentOnboarding,
  handleOpenAgentsCompanionFile,
} from './openagents-agent-onboarding-routes'

const repoRoot = resolve(import.meta.dirname, '../../..')
const readLive = (name: string): string =>
  readFileSync(resolve(repoRoot, `docs/live/${name}`), 'utf8')
const readPublic = (name: string): string =>
  readFileSync(resolve(repoRoot, `apps/start/public/${name}`), 'utf8')

const liveAgentDocMarkdown = readLive('AGENTS.md')
const liveAgentCoreMarkdown = readLive('AGENTS-CORE.md')
const liveAssets: Fetcher = {
  fetch: (request: Request) => {
    const name = new URL(request.url).pathname.slice(1)
    const supported = new Set([
      'AGENTS.md',
      'AGENTS-CORE.md',
      'HEARTBEAT.md',
      'RULES.md',
      'skill.json',
    ])
    return Promise.resolve(
      supported.has(name)
        ? new Response(readLive(name))
        : new Response('missing', { status: 404 }),
    )
  },
} as unknown as Fetcher

const runRoute = (path: string, method = 'GET'): Promise<Response> =>
  Effect.runPromise(
    handleOpenAgentsAgentOnboarding(
      new Request(`https://openagents.com${path}`, { method }),
      liveAssets,
    ),
  )

const runCompanionRoute = (
  path: '/AGENTS-CORE.md' | '/HEARTBEAT.md' | '/RULES.md' | '/skill.json',
  method = 'GET',
): Promise<Response> =>
  Effect.runPromise(
    handleOpenAgentsCompanionFile(
      new Request(`https://openagents.com${path}`, { method }),
      liveAssets,
      path,
    ),
  )

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )
  return Array.from(new Uint8Array(digest))
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

describe('OpenAgents agent onboarding routes', () => {
  test('serves the current Workroom contract without advertising retired graphs', async () => {
    const response = await runRoute(OpenAgentsAgentOnboardingCanonicalPath)
    const markdown = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(markdown).toBe(liveAgentDocMarkdown)
    expect(OpenAgentsAgentOnboardingVersion).toBe('0.2.0')
    expect(OpenAgentsAgentOnboardingLastUpdated).toBe('2026-07-14')
    expect(markdown).toContain('Codex Workroom')
    expect(markdown).toContain('intentionally absent from active discovery')
    expect(markdown).toContain('free fallback allowed: false')
    expect(markdown).not.toContain('Autopilot Sites')
    expect(markdown).not.toMatch(/1000 sats|checkout demo|buy credits/i)
    expect(containsProviderSecretMaterial(markdown)).toBe(false)
  })

  test('keeps hashes and deployed assets aligned', async () => {
    await expect(sha256Hex(liveAgentDocMarkdown)).resolves.toBe(
      OpenAgentsAgentOnboardingSha256,
    )
    await expect(sha256Hex(liveAgentCoreMarkdown)).resolves.toBe(
      OpenAgentsAgentCoreSha256,
    )
    for (const name of [
      'AGENTS.md',
      'AGENTS-CORE.md',
      'HEARTBEAT.md',
      'RULES.md',
      'skill.json',
    ]) {
      expect(readPublic(name)).toBe(readLive(name))
    }
  })

  test('serves the compact core and metadata', async () => {
    const coreResponse = await runCompanionRoute(OpenAgentsAgentCorePath)
    const skillResponse = await runCompanionRoute('/skill.json')
    const core = await coreResponse.text()
    const metadata = (await skillResponse.json()) as {
      version: string
      openagents: { authority_notice: string }
    }

    expect(coreResponse.status).toBe(200)
    expect(OpenAgentsAgentCoreUrl).toBe('https://openagents.com/AGENTS-CORE.md')
    expect(core).toBe(liveAgentCoreMarkdown)
    expect(core).toContain('It never becomes free capacity')
    expect(metadata.version).toBe(OpenAgentsAgentOnboardingVersion)
    expect(metadata.openagents.authority_notice).toContain(
      'no formerly paid capacity becomes free capacity',
    )
  })

  test('rejects non-GET methods', async () => {
    const response = await runRoute(
      OpenAgentsAgentOnboardingCanonicalPath,
      'POST',
    )
    const companionResponse = await runCompanionRoute('/HEARTBEAT.md', 'POST')

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
    await expect(response.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
    expect(companionResponse.status).toBe(405)
  })
})
