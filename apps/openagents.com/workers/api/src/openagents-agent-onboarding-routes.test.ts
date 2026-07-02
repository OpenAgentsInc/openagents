import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsAgentCorePath,
  OpenAgentsAgentCoreSha256,
  OpenAgentsAgentCoreSourceRef,
  OpenAgentsAgentCoreUrl,
  OpenAgentsAgentOnboardingCanonicalPath,
  OpenAgentsAgentOnboardingLastUpdated,
  OpenAgentsAgentOnboardingSha256,
  OpenAgentsAgentOnboardingSourceRef,
  OpenAgentsAgentOnboardingVersion,
} from './openagents-agent-onboarding'
import {
  handleOpenAgentsAgentOnboarding,
  handleOpenAgentsCompanionFile,
} from './openagents-agent-onboarding-routes'

const repoRoot = resolve(import.meta.dirname, '../../..')
const liveAgentDocPath = resolve(repoRoot, 'docs/live/AGENTS.md')
const liveAgentCoreDocPath = resolve(repoRoot, 'docs/live/AGENTS-CORE.md')
const liveHeartbeatPath = resolve(repoRoot, 'docs/live/HEARTBEAT.md')
const liveRulesPath = resolve(repoRoot, 'docs/live/RULES.md')
const liveSkillJsonPath = resolve(repoRoot, 'docs/live/skill.json')
const publicAgentDocPath = resolve(repoRoot, 'apps/web/public/AGENTS.md')
const publicAgentCoreDocPath = resolve(
  repoRoot,
  'apps/web/public/AGENTS-CORE.md',
)
const publicHeartbeatPath = resolve(repoRoot, 'apps/web/public/HEARTBEAT.md')
const publicRulesPath = resolve(repoRoot, 'apps/web/public/RULES.md')
const publicSkillJsonPath = resolve(repoRoot, 'apps/web/public/skill.json')
const liveAgentDocMarkdown = readFileSync(liveAgentDocPath, 'utf8')
const liveAgentCoreMarkdown = readFileSync(liveAgentCoreDocPath, 'utf8')
// SURFACES.md holds the registration / owner-claim / hosted-search / campaign
// detail that was extracted out of the compact AGENTS.md; assert that content
// against the doc it now lives in.
const liveSurfacesMarkdown = readFileSync(
  resolve(repoRoot, 'docs/live/SURFACES.md'),
  'utf8',
)
const liveHeartbeatMarkdown = readFileSync(liveHeartbeatPath, 'utf8')
const liveRulesMarkdown = readFileSync(liveRulesPath, 'utf8')
const liveSkillJson = readFileSync(liveSkillJsonPath, 'utf8')
const publicAgentDocMarkdown = readFileSync(publicAgentDocPath, 'utf8')
const publicAgentCoreMarkdown = readFileSync(publicAgentCoreDocPath, 'utf8')
const publicHeartbeatMarkdown = readFileSync(publicHeartbeatPath, 'utf8')
const publicRulesMarkdown = readFileSync(publicRulesPath, 'utf8')
const publicSkillJson = readFileSync(publicSkillJsonPath, 'utf8')
const deprecatedTranscript230Url = [
  'https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main',
  'docs/deprecated/transcripts/230.md',
].join('/')

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return Array.from(new Uint8Array(digest))
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const liveAssets: Fetcher = {
  fetch: (request: Request) => {
    const path = new URL(request.url).pathname
    const bodyByPath: Record<string, string> = {
      '/AGENTS-CORE.md': liveAgentCoreMarkdown,
      '/AGENTS.md': liveAgentDocMarkdown,
      '/HEARTBEAT.md': liveHeartbeatMarkdown,
      '/RULES.md': liveRulesMarkdown,
      '/skill.json': liveSkillJson,
    }
    const body = bodyByPath[path]

    return Promise.resolve(
      body === undefined
        ? new Response('missing', { status: 404 })
        : new Response(body),
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

describe('OpenAgents agent onboarding routes', () => {
  test('serves canonical agent onboarding markdown with stable metadata', async () => {
    const response = await runRoute(OpenAgentsAgentOnboardingCanonicalPath)
    const markdown = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('content-type')).toBe(
      'text/markdown; charset=utf-8',
    )
    expect(markdown).toContain('# OpenAgents')
    expect(markdown).toBe(liveAgentDocMarkdown)
    expect(markdown).toContain(`version: ${OpenAgentsAgentOnboardingVersion}`)
    expect(OpenAgentsAgentOnboardingLastUpdated).toBe('2026-07-02')
    expect(markdown).toContain('Last updated: July 2, 2026')
    expect(markdown).toContain('https://openagents.com/AGENTS-CORE.md')
    expect(markdown.indexOf('https://openagents.com/AGENTS-CORE.md')).toBeLessThan(
      markdown.indexOf('# OpenAgents'),
    )
    expect(markdown).toContain(
      'Canonical URL: https://openagents.com/AGENTS.md',
    )
    expect(markdown).toContain('This document does not grant permissions')
    expect(markdown).toContain(
      'https://openagents.com/.well-known/openagents.json',
    )
    expect(markdown).toContain('https://openagents.com/api/openapi.json')
    expect(markdown).toContain(
      'https://raw.githubusercontent.com/OpenAgentsInc/openagents/refs/heads/main/docs/transcripts/230.md',
    )
    expect(markdown).not.toContain(deprecatedTranscript230Url)
    expect(markdown).toContain('/api/public/launch-dashboard')
    expect(markdown).toContain('Product Promises Forum')
    expect(markdown).toContain(
      'https://openagents.com/forum/f/product-promises',
    )
    expect(markdown).toContain('/api/forum/forums/product-promises/topics')
    expect(markdown).toContain(
      'https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml',
    )
    expect(markdown).toContain('strict GitHub')
    expect(markdown).toContain('bug form')
    expect(markdown).toContain('rejected by the form')
    expect(markdown).toContain('Your default first mission')
    expect(markdown).toContain('Live Public Surfaces')
    expect(markdown).toContain('Meaningful Work Without A Bearer Token')
    expect(markdown).toContain('Live Browser-Session Surfaces')
    expect(markdown).toContain('Live Programmatic Agent Surfaces')
    expect(liveSurfacesMarkdown).toContain('self-service registration is the normal path')
    expect(markdown).toContain('/api/agents/register')
    expect(liveSurfacesMarkdown).toContain('Owner claim is also live and optional')
    expect(liveSurfacesMarkdown).toContain('/api/agents/claims')
    expect(liveSurfacesMarkdown).toContain('An owner claim is optional for Forum speech')
    expect(liveSurfacesMarkdown).toContain('1000 sats promotional')
    expect(liveSurfacesMarkdown).toContain('reward is a separate campaign ledger')
    expect(liveSurfacesMarkdown).toContain('Nostr is planned')
    expect(markdown).toContain('/api/agents/proposals')
    expect(markdown).toContain('Public proposal intake is live')
    expect(markdown).toContain('https://openagents.com/HEARTBEAT.md')
    expect(markdown).toContain('https://openagents.com/RULES.md')
    expect(markdown).toContain('https://openagents.com/skill.json')
    expect(markdown).toContain('Autopilot Sites')
    expect(markdown).toContain('/api/customer-orders/{orderId}/site-revisions')
    expect(markdown).toContain('/api/customer-orders/{orderId}/site-feedback')
    expect(liveSurfacesMarkdown).toContain('Register an agent')
    expect(liveSurfacesMarkdown).toContain('Hosted Search For Registered Agents')
    expect(liveSurfacesMarkdown).toContain('/api/agents/search')
    expect(liveSurfacesMarkdown).toContain('/api/agents/search/payments/preview')
    expect(liveSurfacesMarkdown).toContain('/api/agents/search/payments/redeem')
    expect(liveSurfacesMarkdown).toContain('X-OpenAgents-Agent-Search-Entitlement')
    expect(liveSurfacesMarkdown).toContain('Every active registered agent token')
    expect(liveSurfacesMarkdown).toContain('node scripts/forum-void-smoke.mjs')
    expect(markdown).toContain('AGENTS.md remains guidance. Runtime authority')
    expect(markdown).toContain(
      'Include a fresh `Idempotency-Key` for every logical write',
    )
    expect(markdown).toContain('Planned Or Gated Surfaces')
    expect(markdown).not.toContain('proposal intake API is planned')
    expect(markdown).not.toContain(
      '- public self-service agent registration and owner claim;',
    )
    expect(markdown).not.toContain('OPENAGENTS_AGENT_REGISTRATION_SECRET')
    expect(markdown).toContain('Use "bitcoin" for the asset language')
    expect(markdown).not.toContain('provider_account')
    expect(markdown).not.toContain('runner_payload')
    expect(markdown).not.toContain('callback_token')
    expect(markdown).not.toContain('autopilot-omega')
    expect(markdown).not.toContain('Omega-hosted')
    expect(markdown).not.toContain('planned but not live yet: `HEARTBEAT.md`')
    expect(markdown).not.toContain('$1')
    expect(containsProviderSecretMaterial(markdown)).toBe(false)
  })

  test('keeps the exported sha256 aligned with docs/live/AGENTS.md', async () => {
    await expect(sha256Hex(liveAgentDocMarkdown)).resolves.toBe(
      OpenAgentsAgentOnboardingSha256,
    )
    await expect(sha256Hex(liveAgentCoreMarkdown)).resolves.toBe(
      OpenAgentsAgentCoreSha256,
    )
    expect(OpenAgentsAgentOnboardingSourceRef).toBe(
      'https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents.com/docs/live/AGENTS.md',
    )
    expect(OpenAgentsAgentCoreSourceRef).toBe(
      'https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents.com/docs/live/AGENTS-CORE.md',
    )
  })

  test('keeps the deployed public asset synced from docs/live/AGENTS.md', () => {
    expect(publicAgentDocMarkdown).toBe(liveAgentDocMarkdown)
    expect(publicAgentCoreMarkdown).toBe(liveAgentCoreMarkdown)
    expect(new TextEncoder().encode(liveAgentCoreMarkdown).byteLength).toBeLessThan(
      10_000,
    )
  })

  test('serves the compact core tier from the public asset bundle', async () => {
    const response = await runCompanionRoute(OpenAgentsAgentCorePath)
    const markdown = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(response.headers.get('content-type')).toBe(
      'text/markdown; charset=utf-8',
    )
    expect(OpenAgentsAgentCoreUrl).toBe('https://openagents.com/AGENTS-CORE.md')
    expect(markdown).toBe(liveAgentCoreMarkdown)
    expect(markdown).toContain('# OpenAgents Core Agent Instructions')
    expect(markdown).toContain('## Five-Step Start')
    expect(markdown).toContain('## Public Endpoints')
    expect(markdown).toContain('## Security Rules')
    expect(markdown).toContain('https://openagents.com/AGENTS.md')
    expect(containsProviderSecretMaterial(markdown)).toBe(false)
  })

  test('serves public companion files with matching versions and references', async () => {
    const heartbeatResponse = await runCompanionRoute('/HEARTBEAT.md')
    const rulesResponse = await runCompanionRoute('/RULES.md')
    const metadataResponse = await runCompanionRoute('/skill.json')
    const heartbeat = await heartbeatResponse.text()
    const rules = await rulesResponse.text()
    const metadataText = await metadataResponse.text()
    const metadata = JSON.parse(metadataText) as {
      openagents: {
        api_base: string
        core_instructions_url: string
        files: Record<string, string>
        omni_sdk_seed_url: string
        openapi_url: string
        requires: { bins: ReadonlyArray<string> }
      }
      version: string
    }

    expect(heartbeatResponse.status).toBe(200)
    expect(rulesResponse.status).toBe(200)
    expect(metadataResponse.status).toBe(200)
    expect(heartbeatResponse.headers.get('content-type')).toBe(
      'text/markdown; charset=utf-8',
    )
    expect(rulesResponse.headers.get('content-type')).toBe(
      'text/markdown; charset=utf-8',
    )
    expect(metadataResponse.headers.get('content-type')).toBe(
      'application/json; charset=utf-8',
    )
    expect(heartbeat).toBe(liveHeartbeatMarkdown)
    expect(rules).toBe(liveRulesMarkdown)
    expect(metadataText).toBe(liveSkillJson)
    expect(publicHeartbeatMarkdown).toBe(liveHeartbeatMarkdown)
    expect(publicRulesMarkdown).toBe(liveRulesMarkdown)
    expect(publicSkillJson).toBe(liveSkillJson)
    expect(metadata.version).toBe(OpenAgentsAgentOnboardingVersion)
    expect(metadata.openagents.api_base).toBe('https://openagents.com/api')
    expect(metadata.openagents.openapi_url).toBe(
      'https://openagents.com/api/openapi.json',
    )
    expect(metadata.openagents.omni_sdk_seed_url).toBe(
      'https://openagents.com/api/omni/sdk-seed',
    )
    expect(metadata.openagents.core_instructions_url).toBe(
      'https://openagents.com/AGENTS-CORE.md',
    )
    expect(metadata.openagents.requires.bins).toContain('curl')
    expect(Object.values(metadata.openagents.files).sort()).toEqual([
      'https://openagents.com/AGENTS-CORE.md',
      'https://openagents.com/AGENTS.md',
      'https://openagents.com/HEARTBEAT.md',
      'https://openagents.com/RULES.md',
    ])
    expect(heartbeat).toContain('/api/agents/home')
    expect(rules).toContain('Payment cannot buy private access')
    expect(
      containsProviderSecretMaterial(heartbeat + rules + metadataText),
    ).toBe(false)
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
