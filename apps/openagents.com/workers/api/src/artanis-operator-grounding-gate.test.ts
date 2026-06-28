import { describe, expect, test } from 'vitest'

import {
  ARTANIS_GROUNDING_ADDENDUM_HEADER,
  enforceArtanisGroundingGate,
  evaluateArtanisGroundingGate,
  extractArtanisGroundingLookup,
  extractArtanisRunnableArtifacts,
  type ArtanisGroundingLookup,
} from './artanis-operator-grounding-gate'

describe('artanis grounding gate — runnable-artifact extraction', () => {
  test('extracts file paths, scripts (as commands), and API endpoints', () => {
    const reply = [
      'Run `bun apps/pylon/scripts/multi-session-campaign.ts --burst 4` to start.',
      'Then call POST /api/admin/khala/mint and read docs/roadmap.md.',
    ].join('\n')
    const artifacts = extractArtanisRunnableArtifacts(reply)
    const kinds = artifacts.map(a => `${a.kind}:${a.ref}`)
    expect(kinds).toContain('file_path:apps/pylon/scripts/multi-session-campaign.ts')
    expect(kinds).toContain('command:apps/pylon/scripts/multi-session-campaign.ts')
    expect(kinds).toContain('file_path:docs/roadmap.md')
    const endpoint = artifacts.find(a => a.kind === 'api_endpoint')
    expect(endpoint?.ref).toBe('/api/admin/khala/mint')
    expect(endpoint?.method).toBe('POST')
    const command = artifacts.find(a => a.kind === 'command')
    expect(command?.flags).toContain('--burst')
  })

  test('does not extract bare prose words without a path separator', () => {
    expect(extractArtanisRunnableArtifacts('I improved the index today.')).toEqual(
      [],
    )
    // "../" traversal-shaped tokens are never treated as artifacts.
    expect(
      extractArtanisRunnableArtifacts('see ../../etc/passwd.sh maybe'),
    ).toEqual([])
  })
})

describe('artanis grounding gate — lookup extraction from tool results', () => {
  test('repo_path_exists positive and negative', () => {
    const positive = extractArtanisGroundingLookup({
      toolName: 'repo_path_exists',
      rawArguments: '{"path":"apps/pylon/scripts/multi-session-campaign.ts"}',
      content: 'GROUNDED: "apps/pylon/scripts/multi-session-campaign.ts" EXISTS (file, 900 bytes) in OpenAgentsInc/openagents@main.',
    })
    expect(positive).toMatchObject({
      tool: 'repo_path_exists',
      ref: 'apps/pylon/scripts/multi-session-campaign.ts',
      result: 'positive',
    })
    const negative = extractArtanisGroundingLookup({
      toolName: 'repo_path_exists',
      rawArguments: '{"path":"scripts/distill_traces.ts"}',
      content: 'GROUNDING: "scripts/distill_traces.ts" does NOT exist in OpenAgentsInc/openagents@main. UNGROUNDED — do not present it as a real file/script/command; label it SPECULATIVE.',
    })
    expect(negative?.result).toBe('negative')
  })

  test('route_exists positive and negative', () => {
    const positive = extractArtanisGroundingLookup({
      toolName: 'route_exists',
      rawArguments: '{"method":"POST","path":"/api/v1/chat/completions"}',
      content: 'GROUNDED: POST "/api/v1/chat/completions" is a registered route in the OpenAPI registry. Registered methods: POST.',
    })
    expect(positive?.result).toBe('positive')
    const negative = extractArtanisGroundingLookup({
      toolName: 'route_exists',
      rawArguments: '{"method":"POST","path":"/api/admin/khala/mint"}',
      content: 'GROUNDING: "/api/admin/khala/mint" is NOT in the OpenAPI route registry. UNGROUNDED — do not present "POST /api/admin/khala/mint" as a real endpoint; label it SPECULATIVE.',
    })
    expect(negative?.result).toBe('negative')
  })

  test('read_repo_file success counts as path grounding, error does not', () => {
    const success = extractArtanisGroundingLookup({
      toolName: 'read_repo_file',
      rawArguments: '{"path":"docs/roadmap.md"}',
      content: '# Roadmap\nFirst priority is the #6316 track.',
    })
    expect(success?.result).toBe('positive')
    const missing = extractArtanisGroundingLookup({
      toolName: 'read_repo_file',
      rawArguments: '{"path":"docs/does-not-exist.md"}',
      content: '(file not found: "docs/does-not-exist.md")',
    })
    expect(missing?.result).toBe('negative')
  })

  test('non-grounding tools produce no lookup', () => {
    expect(
      extractArtanisGroundingLookup({
        toolName: 'get_network_stats',
        rawArguments: '{}',
        content: 'tokens served: 1B',
      }),
    ).toBeNull()
  })
})

describe('artanis grounding gate — evaluation + enforcement', () => {
  test('a fabricated script path is flagged SPECULATIVE (UNGROUNDED)', () => {
    const reply =
      'You can regenerate the traces with `bun scripts/distill_traces.ts --since 24h`.'
    const { reply: out, gate } = enforceArtanisGroundingGate({
      reply,
      lookups: [],
    })
    expect(gate.allGrounded).toBe(false)
    expect(gate.enforced).toBe(true)
    expect(out).toContain(ARTANIS_GROUNDING_ADDENDUM_HEADER)
    expect(out).toContain('scripts/distill_traces.ts')
    expect(out).toContain('SPECULATIVE')
    expect(
      gate.speculativeArtifacts.some(
        a => a.artifactRef === 'scripts/distill_traces.ts',
      ),
    ).toBe(true)
  })

  test('a fabricated API endpoint with a negative route lookup is gated', () => {
    const reply = 'I will mint via POST /api/admin/khala/mint.'
    const lookups: ReadonlyArray<ArtanisGroundingLookup> = [
      {
        tool: 'route_exists',
        ref: '/api/admin/khala/mint',
        result: 'negative',
        matchedFlags: [],
        matchedText: null,
      },
    ]
    const { gate } = enforceArtanisGroundingGate({ reply, lookups })
    expect(gate.allGrounded).toBe(false)
    const verdict = gate.evaluated.find(
      v => v.artifactRef === '/api/admin/khala/mint',
    )
    expect(verdict?.state).toBe('LOOKED_UP')
    expect(verdict?.grounded).toBe(false)
  })

  test('a verified path + route passes GROUNDED with no addendum', () => {
    const reply = [
      'Run `bun apps/pylon/scripts/multi-session-campaign.ts --burst 4`.',
      'It posts to POST /api/v1/chat/completions.',
    ].join('\n')
    const lookups: ReadonlyArray<ArtanisGroundingLookup> = [
      {
        tool: 'repo_path_exists',
        ref: 'apps/pylon/scripts/multi-session-campaign.ts',
        result: 'positive',
        matchedFlags: [],
        matchedText: null,
      },
      {
        tool: 'repo_grep',
        ref: 'apps/pylon/scripts/multi-session-campaign.ts',
        result: 'positive',
        matchedFlags: ['--burst'],
        matchedText: 'GROUNDED: /--burst/ matched 1 line(s)',
      },
      {
        tool: 'route_exists',
        ref: '/api/v1/chat/completions',
        result: 'positive',
        matchedFlags: [],
        matchedText: null,
      },
    ]
    const { reply: out, gate } = enforceArtanisGroundingGate({ reply, lookups })
    expect(gate.allGrounded).toBe(true)
    expect(gate.enforced).toBe(false)
    expect(out).toBe(reply)
    // The command artifact carries an attached S4 sub-verdict.
    const command = gate.evaluated.find(v => v.artifactKind === 'command')
    expect(command?.commandSourceVerified).not.toBeNull()
    expect(command?.commandSourceVerified?.satisfiedEvidence).toContain(
      'evidence://command/flag-verification',
    )
  })

  test('does not double-flag an artifact the reply already marked SPECULATIVE', () => {
    const reply =
      'There may be a script scripts/distill_traces.ts but I have not verified it exists (SPECULATIVE).'
    const { gate } = evaluateAndEnforce(reply)
    expect(gate.speculativeArtifacts).toEqual([])
    expect(gate.enforced).toBe(false)
  })

  test('a reply naming no runnable artifact is left untouched', () => {
    const reply = 'The burndown is healthy; three assignments merged today.'
    const gate = evaluateArtanisGroundingGate({ reply, lookups: [] })
    expect(gate.evaluated).toEqual([])
    expect(gate.allGrounded).toBe(true)
  })
})

const evaluateAndEnforce = (reply: string) =>
  enforceArtanisGroundingGate({ reply, lookups: [] })
