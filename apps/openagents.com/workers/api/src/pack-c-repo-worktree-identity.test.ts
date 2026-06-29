import { describe, expect, test } from 'vitest'

import {
  PACK_C_REPO_WORKTREE_IDENTITY_VERSION,
  normalizePackCBranchRef,
  projectPackCRepoWorktreeIdentity,
} from './pack-c-repo-worktree-identity'

describe('Pack C repo and worktree identity snapshots', () => {
  const base = {
    generatedAt: '2026-06-12T04:00:30.000Z',
    observedAt: '2026-06-12T04:00:00.000Z',
    repository: {
      caveatRefs: ['caveat:repo-scope:public-only'],
      dataScopeRefs: ['data-scope:repo:openagents:src'],
      defaultBranch: 'main',
      host: 'github.com',
      name: 'openagents',
      owner: 'OpenAgentsInc',
      pinnedCommitRef: '18aaf36d9',
      remoteDigestRef: 'remote-digest:sha256:openagents-public',
      repositoryRef: 'repo:github:OpenAgentsInc/openagents',
      trustTier: 'public' as const,
      visibility: 'public' as const,
    },
    staleAfterMs: 60_000,
    worktree: {
      baseCommitRef: '18aaf36d9',
      branchRef: 'pack-c/repo-identity',
      cleanliness: 'clean' as const,
      headCommitRef: 'e00ffeb3b',
      retentionPolicyRef: 'retention:worktree:short',
      sandboxProfileRef: 'sandbox:openagents:pack-c',
      worktreeRef: 'worktree:pack-c:pc1',
      workspaceRef: 'workspace:openagents:pack-c',
    },
  }

  test('projects a ready repository and worktree identity snapshot', () => {
    const projection = projectPackCRepoWorktreeIdentity(base)

    expect(projection).toEqual({
      ageMs: 30_000,
      blockerRefs: [],
      freshness: 'fresh',
      generatedAt: '2026-06-12T04:00:30.000Z',
      identityVersion: PACK_C_REPO_WORKTREE_IDENTITY_VERSION,
      observedAt: '2026-06-12T04:00:00.000Z',
      repository: {
        caveatRefs: ['caveat:repo-scope:public-only'],
        dataScopeRefs: ['data-scope:repo:openagents:src'],
        defaultBranch: 'main',
        host: 'github.com',
        name: 'openagents',
        owner: 'OpenAgentsInc',
        pinnedCommitRef: '18aaf36d9',
        remoteDigestRef: 'remote-digest:sha256:openagents-public',
        repositoryRef: 'repo:github:OpenAgentsInc/openagents',
        trustTier: 'public',
        visibility: 'public',
      },
      staleAt: '2026-06-12T04:01:00.000Z',
      status: 'ready',
      worktree: {
        baseCommitRef: '18aaf36d9',
        branchRef: 'pack-c/repo-identity',
        cleanliness: 'clean',
        headCommitRef: 'e00ffeb3b',
        retentionPolicyRef: 'retention:worktree:short',
        sandboxProfileRef: 'sandbox:openagents:pack-c',
        worktreeRef: 'worktree:pack-c:pc1',
        workspaceRef: 'workspace:openagents:pack-c',
      },
    })
  })

  test('reports stale snapshots without dropping refs', () => {
    const projection = projectPackCRepoWorktreeIdentity({
      ...base,
      generatedAt: '2026-06-12T04:05:00.000Z',
    })

    expect(projection).toMatchObject({
      ageMs: 300_000,
      freshness: 'stale',
      status: 'stale',
    })
    expect(projection.repository.repositoryRef).toBe(
      'repo:github:OpenAgentsInc/openagents',
    )
  })

  test('blocks incomplete repository and worktree identity', () => {
    const projection = projectPackCRepoWorktreeIdentity({
      ...base,
      repository: {
        ...base.repository,
        dataScopeRefs: [],
        defaultBranch: null,
        pinnedCommitRef: null,
        remoteDigestRef: null,
      },
      worktree: {
        ...base.worktree,
        baseCommitRef: null,
        cleanliness: 'unknown',
        headCommitRef: null,
        retentionPolicyRef: null,
        sandboxProfileRef: null,
      },
    })

    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toEqual([
      'pack-c-identity-blocker:repo:github:OpenAgentsInc/openagents:missing-default-branch',
      'pack-c-identity-blocker:repo:github:OpenAgentsInc/openagents:missing-pinned-commit',
      'pack-c-identity-blocker:repo:github:OpenAgentsInc/openagents:missing-remote-digest',
      'pack-c-identity-blocker:repo:github:OpenAgentsInc/openagents:missing-data-scope',
      'pack-c-identity-blocker:worktree:pack-c:pc1:missing-base-commit',
      'pack-c-identity-blocker:worktree:pack-c:pc1:missing-head-commit',
      'pack-c-identity-blocker:worktree:pack-c:pc1:missing-sandbox-profile',
      'pack-c-identity-blocker:worktree:pack-c:pc1:missing-retention-policy',
      'pack-c-identity-blocker:worktree:pack-c:pc1:unknown-cleanliness',
    ])
  })

  test('normalizes safe branch refs and rejects shell-shaped branch refs', () => {
    expect(normalizePackCBranchRef(' feature/pack-c_pc1.1 ')).toBe(
      'feature/pack-c_pc1.1',
    )

    expect(() => normalizePackCBranchRef('feature/pack-c;rm -rf /')).toThrow(
      /private repo, local path, or shell material|safe Git refs/,
    )
    expect(() => normalizePackCBranchRef('../escape')).toThrow(/safe Git refs/)
    expect(() => normalizePackCBranchRef('feature.lock')).toThrow(
      /safe Git refs/,
    )
  })

  test('rejects private remotes, local paths, credentials, raw prompts, and private repo content', () => {
    expect(() =>
      projectPackCRepoWorktreeIdentity({
        ...base,
        repository: {
          ...base.repository,
          repositoryRef: 'https://github.com/OpenAgentsInc/private-repo',
        },
      }),
    ).toThrow(/private repo, local path, or shell material/)

    expect(() =>
      projectPackCRepoWorktreeIdentity({
        ...base,
        worktree: {
          ...base.worktree,
          workspaceRef: '/Users/christopherdavid/work/openagents',
        },
      }),
    ).toThrow(/private repo, local path, or shell material/)

    expect(() =>
      projectPackCRepoWorktreeIdentity({
        ...base,
        repository: {
          ...base.repository,
          remoteDigestRef: 'ghp_1234567890abcdef1234567890abcdef',
        },
      }),
    ).toThrow(/provider credential material|stable Pack C ref/)

    expect(() =>
      projectPackCRepoWorktreeIdentity({
        ...base,
        repository: {
          ...base.repository,
          dataScopeRefs: ['raw prompt: fix customer code'],
        },
      }),
    ).toThrow(/private repo, local path, or shell material|stable Pack C ref/)

    expect(() =>
      projectPackCRepoWorktreeIdentity({
        ...base,
        repository: {
          ...base.repository,
          caveatRefs: ['private_content:customer-source-file'],
        },
      }),
    ).toThrow(/private repo, local path, or shell material/)
  })
})
