import { describe, expect, test } from 'vitest'

import {
  buildForgeDiagnosticsInput,
  projectForgeDiagnostics,
} from './diagnostics'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T20:30:00.000Z',
  snapshotRef: 'diagnostics-snapshot.public.work_1',
  versionRef: 'diagnostics-version.public.v1',
  workOrderRef: 'work.public.work_1',
  workspaceBoundaryRefs: ['workspace-boundary.public.openagents'],
}

describe('Forge diagnostics projection', () => {
  test('projects public diagnostics as refs-only non-authoritative state', () => {
    const view = projectForgeDiagnostics({
      ...baseInput,
      entries: [
        {
          diagnosticRef: 'diagnostic.public.tsc.no_emit',
          freshness: 'fresh',
          languageServerRef: 'language-server.public.typescript',
          policyRefs: ['policy.public.diagnostics.read_only'],
          remediationRefs: ['remediation.public.fix_types'],
          severity: 'error',
          sourceRefs: ['source.public.diagnostic.typecheck'],
        },
        {
          diagnosticRef: 'diagnostic.public.eslint.warning',
          freshness: 'fresh',
          languageServerRef: 'language-server.public.eslint',
          severity: 'warning',
          sourceRefs: ['source.public.diagnostic.lint'],
        },
      ],
      freshness: 'fresh',
      indexedAt: '2026-06-17T20:29:00.000Z',
      indexedAtRef: 'diagnostics-index.public.work_1',
      languageServerRefs: ['language-server.public.typescript'],
      policyRefs: ['policy.public.diagnostics.read_only'],
      remediationRefs: ['remediation.public.fix_types'],
      sourceRefs: ['source.public.diagnostic.typecheck'],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      errors: 1,
      hints: 0,
      info: 0,
      total: 2,
      warnings: 1,
    })
    expect(view.languageServerRefs).toEqual([
      'language-server.public.typescript',
      'language-server.public.eslint',
    ])
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      codeActionAuthority: false,
      deploymentAuthority: false,
      diagnosticsExecutionAuthority: false,
      editAuthority: false,
      fileReadAuthority: false,
      lspConfigurationAuthority: false,
      lspProcessAuthority: false,
      providerAuthority: false,
      publicClaimAuthority: false,
      retrievalRoutingAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing diagnostics as empty', () => {
    const view = projectForgeDiagnostics({
      generatedAt: '2026-06-17T20:30:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale diagnostics without refresh evidence', () => {
    const view = projectForgeDiagnostics({
      ...baseInput,
      diagnosticRefs: ['diagnostic.public.stale'],
      freshness: 'stale',
      languageServerRefs: ['language-server.public.typescript'],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-diagnostics-blocker:work.public.work_1:stale-diagnostics-refresh-evidence-missing',
    )
  })

  test('blocks diagnostics without workspace boundary or language server evidence', () => {
    const view = projectForgeDiagnostics({
      generatedAt: '2026-06-17T20:30:00.000Z',
      diagnosticRefs: ['diagnostic.public.no_boundary'],
      freshness: 'fresh',
      snapshotRef: 'diagnostics-snapshot.public.no_boundary',
      workOrderRef: 'work.public.no_boundary',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-diagnostics-blocker:work.public.no_boundary:missing-workspace-boundary-ref',
    )
    expect(view.blockerRefs).toContain(
      'forge-diagnostics-blocker:work.public.no_boundary:missing-language-server-evidence',
    )
  })

  test('blocks remediation refs without policy refs', () => {
    const view = projectForgeDiagnostics({
      ...baseInput,
      entries: [
        {
          diagnosticRef: 'diagnostic.public.fixable',
          freshness: 'fresh',
          languageServerRef: 'language-server.public.typescript',
          remediationRefs: ['remediation.public.code_action'],
          severity: 'error',
        },
      ],
      freshness: 'fresh',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-diagnostics-blocker:work.public.work_1:remediation-policy-missing:diagnostic.public.fixable',
    )
  })

  test('omits unsafe private diagnostic material before projection', () => {
    const view = projectForgeDiagnostics({
      ...baseInput,
      blockerRefs: [
        'diagnostics-blocker.public.safe',
        'raw diagnostic /Users/christopher/error.log',
      ],
      entries: [
        {
          blockerRefs: ['entry-blocker.public.safe', 'compiler stderr /Users/christopher'],
          diagnosticRef: 'diagnostic.public.safe',
          freshness: 'fresh',
          languageServerRef: 'language-server.public.safe',
          policyRefs: ['policy.public.safe', 'bearer token private'],
          remediationRefs: ['remediation.public.safe', 'raw source ./private.ts'],
          severity: 'warning',
          sourceRefs: ['source.public.safe', 'diagnostic message sk-private'],
        },
      ],
      freshness: 'fresh',
      languageServerRefs: ['language-server.public.safe'],
      sourceRefs: ['source.public.safe', 'private repo /Users/christopher/project'],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.sourceRefs).toEqual(['source.public.safe'])
    expect(view.entries[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.entries[0]?.remediationRefs).toEqual(['remediation.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-diagnostics-blocker:work.public.work_1:unsafe-diagnostics-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw diagnostic')
    expect(payload).not.toContain('compiler stderr')
    expect(payload).not.toContain('raw source')
    expect(payload).not.toContain('diagnostic message')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      diagnostics: {
        diagnosticRefs: ['diagnostic.public.work_2'],
        freshness: 'fresh',
        languageServerRefs: ['language-server.public.typescript'],
        snapshotRef: 'diagnostics-snapshot.public.work_2',
        versionRef: 'diagnostics-version.public.work_2',
        workspaceBoundaryRefs: ['workspace-boundary.public.work_2'],
      },
      generatedAt: '2026-06-17T20:31:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeDiagnosticsInput(work)).toEqual({
      diagnosticRefs: ['diagnostic.public.work_2'],
      freshness: 'fresh',
      generatedAt: '2026-06-17T20:31:00.000Z',
      languageServerRefs: ['language-server.public.typescript'],
      snapshotRef: 'diagnostics-snapshot.public.work_2',
      versionRef: 'diagnostics-version.public.work_2',
      workOrderRef: 'work.public.work_2',
      workspaceBoundaryRefs: ['workspace-boundary.public.work_2'],
    })
  })
})
