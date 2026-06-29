import { describe, expect, test } from 'vitest'

import {
  PACK_C_WORKSPACE_AUTHORITY_VERSION,
  projectPackCWorkspaceAuthority,
} from './pack-c-workspace-authority'

describe('Pack C workspace authority projections', () => {
  const base = {
    allowedCommandIntentRefs: ['command-intent:test:pack-c'],
    allowedPathRefs: ['path-scope:repo:src', 'path-scope:repo:test'],
    approvalRefs: ['approval:workspace:pack-c'],
    commandIntentRef: 'command-intent:test:pack-c',
    evidenceRef: 'workspace-evidence:pack-c:pc3',
    expectedSandboxProfileRef: 'sandbox:openagents:pack-c',
    generatedAt: '2026-06-12T04:40:00.000Z',
    operationKind: 'verification' as const,
    redactionClass: 'public' as const,
    redactionReceiptRefs: ['redaction:workspace-evidence:pack-c:pc3'],
    requiresApproval: true,
    sandboxProfileRef: 'sandbox:openagents:pack-c',
    touchedPathRefs: ['path-scope:repo:src'],
    workspaceRef: 'workspace:openagents:pack-c',
  }

  test('allows public-safe workspace evidence with approval and redaction refs', () => {
    const projection = projectPackCWorkspaceAuthority(base)

    expect(projection).toEqual({
      allowedCommandIntentRefs: ['command-intent:test:pack-c'],
      allowedPathRefs: ['path-scope:repo:src', 'path-scope:repo:test'],
      approvalRefs: ['approval:workspace:pack-c'],
      blockerRefs: [],
      cancellationRef: null,
      commandIntentRef: 'command-intent:test:pack-c',
      evidenceRef: 'workspace-evidence:pack-c:pc3',
      expectedSandboxProfileRef: 'sandbox:openagents:pack-c',
      generatedAt: '2026-06-12T04:40:00.000Z',
      operationKind: 'verification',
      redactionClass: 'public',
      redactionReceiptRefs: ['redaction:workspace-evidence:pack-c:pc3'],
      requiresApproval: true,
      sandboxProfileRef: 'sandbox:openagents:pack-c',
      status: 'allowed',
      timeoutRef: null,
      touchedPathRefs: ['path-scope:repo:src'],
      workspaceAuthorityVersion: PACK_C_WORKSPACE_AUTHORITY_VERSION,
      workspaceRef: 'workspace:openagents:pack-c',
    })
  })

  test('denies out-of-scope paths, missing approval, command policy, sandbox mismatch, and redaction gaps', () => {
    const projection = projectPackCWorkspaceAuthority({
      ...base,
      allowedCommandIntentRefs: ['command-intent:build:pack-c'],
      approvalRefs: [],
      expectedSandboxProfileRef: 'sandbox:openagents:locked',
      redactionReceiptRefs: [],
      touchedPathRefs: ['path-scope:repo:docs'],
    })

    expect(projection.status).toBe('denied')
    expect(projection.blockerRefs).toEqual([
      'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:out-of-scope:path-scope:repo:docs',
      'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:missing-approval',
      'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:command-not-allowed',
      'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:sandbox-mismatch',
      'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:redaction-required',
    ])
  })

  test('denies timeout and cancellation evidence refs without hiding them', () => {
    const projection = projectPackCWorkspaceAuthority({
      ...base,
      cancellationRef: 'cancel:workspace-evidence:pack-c:pc3',
      timeoutRef: 'timeout:workspace-evidence:pack-c:pc3',
    })

    expect(projection).toMatchObject({
      status: 'denied',
      timeoutRef: 'timeout:workspace-evidence:pack-c:pc3',
      cancellationRef: 'cancel:workspace-evidence:pack-c:pc3',
      blockerRefs: [
        'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:timeout',
        'pack-c-workspace-authority-blocker:workspace-evidence:pack-c:pc3:cancelled',
      ],
    })
  })

  test('allows private/operator evidence without public redaction receipts', () => {
    const projection = projectPackCWorkspaceAuthority({
      ...base,
      redactionClass: 'operator',
      redactionReceiptRefs: [],
    })

    expect(projection.status).toBe('allowed')
    expect(projection.redactionReceiptRefs).toEqual([])
  })

  test('rejects raw shell logs, raw commands, local paths, private repo data, prompts, and credentials', () => {
    expect(() =>
      projectPackCWorkspaceAuthority({
        ...base,
        commandIntentRef: 'raw_command:bun test && rm -rf dist',
      }),
    ).toThrow(/raw shell, private repo, local path, or prompt material/)

    expect(() =>
      projectPackCWorkspaceAuthority({
        ...base,
        evidenceRef: 'raw_shell:vitest output',
      }),
    ).toThrow(/raw shell, private repo, local path, or prompt material/)

    expect(() =>
      projectPackCWorkspaceAuthority({
        ...base,
        workspaceRef: '/Users/christopherdavid/work/openagents',
      }),
    ).toThrow(/raw shell, private repo, local path, or prompt material/)

    expect(() =>
      projectPackCWorkspaceAuthority({
        ...base,
        touchedPathRefs: ['private_repo:customer-source'],
      }),
    ).toThrow(/raw shell, private repo, local path, or prompt material/)

    expect(() =>
      projectPackCWorkspaceAuthority({
        ...base,
        allowedPathRefs: ['raw prompt: inspect customer code'],
      }),
    ).toThrow(/raw shell, private repo, local path, or prompt material/)

    expect(() =>
      projectPackCWorkspaceAuthority({
        ...base,
        approvalRefs: ['ghp_1234567890abcdef1234567890abcdef'],
      }),
    ).toThrow(/provider credential material|stable Pack C workspace ref/)
  })
})
