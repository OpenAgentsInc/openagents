import { describe, expect, test } from 'vitest'

import {
  buildForgeEditorIntegrationInput,
  projectForgeEditorIntegration,
} from './editor-integration'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T23:50:00.000Z',
  snapshotRef: 'editor-integration-snapshot.public.work_1',
  versionRef: 'editor-integration-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge IDE and editor integration projection', () => {
  test('projects public editor integration evidence as refs-only non-authoritative state', () => {
    const view = projectForgeEditorIntegration({
      ...baseInput,
      entries: [
        {
          commandRefs: ['editor-command.public.open_diff'],
          deepLinkRefs: ['deep-link.public.editor.diff'],
          diagnosticHandoffRefs: ['diagnostic-handoff.public.editor'],
          diagnosticRefs: ['diagnostic.public.editor.safe'],
          editorRefs: ['editor.public.vscode'],
          extensionRefs: ['extension.public.openagents'],
          fileOpenRefs: ['file-open.public.diff_summary'],
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.vscode',
          policyRefs: ['policy.public.editor.deep_link'],
          selectionRefs: ['selection.public.none'],
          state: 'ready',
          statusRefs: ['status.public.editor.connected'],
          workspaceRefs: ['workspace.public.openagents'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      connected: 0,
      disconnected: 0,
      ready: 1,
      total: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      deploymentAuthority: false,
      editorAutomationAuthority: false,
      editorCommandAuthority: false,
      extensionInstallAuthority: false,
      fileOpenAuthority: false,
      fileReadAuthority: false,
      fileWriteAuthority: false,
      publicClaimAuthority: false,
      selectionReadAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      toolExecutionAuthority: false,
      toolRoutingAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing editor integration state as empty', () => {
    const view = projectForgeEditorIntegration({
      generatedAt: '2026-06-17T23:50:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale editor evidence', () => {
    const view = projectForgeEditorIntegration({
      ...baseInput,
      entries: [
        {
          freshness: 'stale',
          integrationRef: 'editor-integration.public.stale',
          policyRefs: ['policy.public.editor.ready'],
          state: 'ready',
          workspaceRefs: ['workspace.public.openagents'],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-editor-integration-blocker:work.public.work_1:stale-editor-integration-evidence:editor-integration.public.stale',
    )
  })

  test('blocks ready editor state without workspace and policy refs', () => {
    const view = projectForgeEditorIntegration({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.missing_ready',
          state: 'ready',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-editor-integration-blocker:work.public.work_1:editor-readiness-evidence-missing:editor-integration.public.missing_ready',
    )
  })

  test('blocks deep links without policy refs', () => {
    const view = projectForgeEditorIntegration({
      ...baseInput,
      entries: [
        {
          deepLinkRefs: ['deep-link.public.editor.diff'],
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.no_policy',
          state: 'unknown',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-editor-integration-blocker:work.public.work_1:deep-link-policy-ref-missing:editor-integration.public.no_policy',
    )
  })

  test('blocks diagnostic handoff without diagnostic refs', () => {
    const view = projectForgeEditorIntegration({
      ...baseInput,
      entries: [
        {
          diagnosticHandoffRefs: ['diagnostic-handoff.public.editor'],
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.no_diagnostic',
          policyRefs: ['policy.public.editor.handoff'],
          state: 'unknown',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-editor-integration-blocker:work.public.work_1:diagnostic-handoff-ref-missing:editor-integration.public.no_diagnostic',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeEditorIntegration({
      generatedAt: '2026-06-17T23:50:00.000Z',
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.no_snapshot',
          policyRefs: ['policy.public.editor.ready'],
          state: 'ready',
          workspaceRefs: ['workspace.public.openagents'],
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-editor-integration-blocker:work.public.no_snapshot:missing-editor-integration-snapshot-ref',
    )
  })

  test('omits unsafe private editor material before projection', () => {
    const view = projectForgeEditorIntegration({
      ...baseInput,
      blockerRefs: [
        'editor-blocker.public.safe',
        'raw editor /Users/christopher/editor.log',
      ],
      entries: [
        {
          commandRefs: ['editor-command.public.safe', 'editor command private text'],
          deepLinkRefs: ['deep-link.public.safe', 'vscode://file/Users/christopher/app.ts'],
          diagnosticHandoffRefs: ['diagnostic-handoff.public.safe'],
          diagnosticRefs: ['diagnostic.public.safe'],
          editorRefs: ['editor.public.safe'],
          extensionRefs: ['extension.public.safe', 'raw extension sk-private'],
          fileOpenRefs: ['file-open.public.safe', 'raw file /Users/christopher/app.ts'],
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.safe',
          policyRefs: ['policy.public.safe', 'bearer token private'],
          selectionRefs: ['selection.public.safe', 'raw selection private buffer'],
          state: 'ready',
          statusRefs: ['status.public.safe'],
          workspaceRefs: ['workspace.public.safe', '/Users/christopher/work/openagents'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.deepLinkRefs).toEqual(['deep-link.public.safe'])
    expect(view.entries[0]?.fileOpenRefs).toEqual(['file-open.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-editor-integration-blocker:work.public.work_1:unsafe-editor-integration-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw editor')
    expect(payload).not.toContain('editor command')
    expect(payload).not.toContain('vscode://')
    expect(payload).not.toContain('raw extension')
    expect(payload).not.toContain('raw file')
    expect(payload).not.toContain('raw selection')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      editorIntegration: {
        entries: [
          {
            freshness: 'fresh',
            integrationRef: 'editor-integration.public.work_2',
            policyRefs: ['policy.public.work_2'],
            state: 'ready',
            workspaceRefs: ['workspace.public.work_2'],
          },
        ],
        snapshotRef: 'editor-integration-snapshot.public.work_2',
        versionRef: 'editor-integration-version.public.v2',
      },
      generatedAt: '2026-06-17T23:51:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeEditorIntegrationInput(work)).toEqual({
      entries: [
        {
          freshness: 'fresh',
          integrationRef: 'editor-integration.public.work_2',
          policyRefs: ['policy.public.work_2'],
          state: 'ready',
          workspaceRefs: ['workspace.public.work_2'],
        },
      ],
      generatedAt: '2026-06-17T23:51:00.000Z',
      snapshotRef: 'editor-integration-snapshot.public.work_2',
      versionRef: 'editor-integration-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
