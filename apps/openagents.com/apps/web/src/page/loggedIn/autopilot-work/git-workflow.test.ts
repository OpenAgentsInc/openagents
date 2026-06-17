import { describe, expect, test } from 'vitest'

import {
  buildForgeGitWorkflowInput,
  projectForgeGitWorkflow,
} from './git-workflow'
import type { AutopilotWorkProjection } from '../model'

const baseInput = {
  generatedAt: '2026-06-17T23:40:00.000Z',
  snapshotRef: 'git-workflow-snapshot.public.work_1',
  versionRef: 'git-workflow-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge Git and GitHub workflow projection', () => {
  test('projects public Git and GitHub workflow evidence as refs-only non-authoritative state', () => {
    const view = projectForgeGitWorkflow({
      ...baseInput,
      entries: [
        {
          branchRefs: ['branch.public.work_1.feature'],
          checkRefs: ['check.public.work_1.bun_test.pass'],
          commitRefs: ['commit.public.work_1.head'],
          diffRefs: ['diff.public.work_1.summary'],
          freshness: 'fresh',
          issueRefs: ['issue.public.5107'],
          policyRefs: ['policy.public.github.writeback'],
          prRefs: ['pr.public.work_1.draft'],
          repositoryRefs: ['repo.public.OpenAgentsInc.openagents'],
          reviewRefs: ['review.public.work_1.ready'],
          state: 'pr_ready',
          statusRefs: ['status.public.work_1.checks'],
          workflowRef: 'git-workflow.public.work_1.pr',
          worktreeRefs: ['worktree.public.work_1'],
          writebackRefs: ['writeback.public.work_1.draft_pr'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      blocked: 0,
      checksPending: 0,
      prReady: 1,
      reviewReady: 0,
      total: 1,
      writebackReady: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      branchCreateAuthority: false,
      checkRunAuthority: false,
      commitAuthority: false,
      deploymentAuthority: false,
      fileReadAuthority: false,
      gitExecutionAuthority: false,
      githubWriteAuthority: false,
      issueCommentAuthority: false,
      prCreateAuthority: false,
      publicClaimAuthority: false,
      reviewSubmitAuthority: false,
      settlementAuthority: false,
      shellExecutionAuthority: false,
      tagCreateAuthority: false,
      toolGrantAuthority: false,
      workerPayoutAuthority: false,
      writebackAuthority: false,
    })
  })

  test('treats missing Git workflow state as empty', () => {
    const view = projectForgeGitWorkflow({
      generatedAt: '2026-06-17T23:40:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.snapshotRef).toBeNull()
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale Git workflow evidence', () => {
    const view = projectForgeGitWorkflow({
      ...baseInput,
      entries: [
        {
          branchRefs: ['branch.public.stale'],
          checkRefs: ['check.public.stale'],
          diffRefs: ['diff.public.stale'],
          freshness: 'stale',
          state: 'pr_ready',
          workflowRef: 'git-workflow.public.stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-git-workflow-blocker:work.public.work_1:stale-git-workflow-evidence:git-workflow.public.stale',
    )
  })

  test('blocks PR-ready workflow without branch diff and check refs', () => {
    const view = projectForgeGitWorkflow({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          state: 'pr_ready',
          workflowRef: 'git-workflow.public.pr_missing',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-git-workflow-blocker:work.public.work_1:pr-ready-evidence-missing:git-workflow.public.pr_missing',
    )
  })

  test('blocks review-ready workflow without review or policy refs', () => {
    const view = projectForgeGitWorkflow({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          state: 'review_ready',
          workflowRef: 'git-workflow.public.review_missing',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-git-workflow-blocker:work.public.work_1:review-ready-evidence-missing:git-workflow.public.review_missing',
    )
  })

  test('blocks writeback-ready workflow without writeback or policy refs', () => {
    const view = projectForgeGitWorkflow({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          state: 'writeback_ready',
          workflowRef: 'git-workflow.public.writeback_missing',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-git-workflow-blocker:work.public.work_1:writeback-ready-evidence-missing:git-workflow.public.writeback_missing',
    )
  })

  test('blocks populated entries without snapshot refs', () => {
    const view = projectForgeGitWorkflow({
      generatedAt: '2026-06-17T23:40:00.000Z',
      entries: [
        {
          branchRefs: ['branch.public.no_snapshot'],
          checkRefs: ['check.public.no_snapshot'],
          diffRefs: ['diff.public.no_snapshot'],
          freshness: 'fresh',
          state: 'pr_ready',
          workflowRef: 'git-workflow.public.no_snapshot',
        },
      ],
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-git-workflow-blocker:work.public.no_snapshot:missing-git-workflow-snapshot-ref',
    )
  })

  test('omits unsafe private Git and GitHub material before projection', () => {
    const view = projectForgeGitWorkflow({
      ...baseInput,
      blockerRefs: [
        'git-workflow-blocker.public.safe',
        'git status /Users/christopher/openagents',
      ],
      entries: [
        {
          branchRefs: ['branch.public.safe', 'raw branch /Users/christopher/repo'],
          checkRefs: ['check.public.safe'],
          commitRefs: ['commit.public.safe', 'raw commit sk-private'],
          diffRefs: ['diff.public.safe', 'diff --git a/private.ts b/private.ts'],
          freshness: 'fresh',
          issueRefs: ['issue.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          prRefs: ['pr.public.safe', 'github comment private body'],
          repositoryRefs: ['repo.public.safe', 'https://github.com/private/repo'],
          reviewRefs: ['review.public.safe'],
          state: 'pr_ready',
          statusRefs: ['status.public.safe', 'raw status ./private'],
          workflowRef: 'git-workflow.public.safe',
          worktreeRefs: ['worktree.public.safe', '/Users/christopher/work/openagents'],
          writebackRefs: ['writeback.public.safe'],
        },
      ],
    })

    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.branchRefs).toEqual(['branch.public.safe'])
    expect(view.entries[0]?.diffRefs).toEqual(['diff.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-git-workflow-blocker:work.public.work_1:unsafe-git-workflow-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('git status')
    expect(payload).not.toContain('raw branch')
    expect(payload).not.toContain('raw commit')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('github comment')
    expect(payload).not.toContain('https://github.com/private')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-17T23:41:00.000Z',
      gitWorkflow: {
        entries: [
          {
            branchRefs: ['branch.public.work_2'],
            checkRefs: ['check.public.work_2'],
            diffRefs: ['diff.public.work_2'],
            freshness: 'fresh',
            state: 'pr_ready',
            workflowRef: 'git-workflow.public.work_2',
          },
        ],
        snapshotRef: 'git-workflow-snapshot.public.work_2',
        versionRef: 'git-workflow-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeGitWorkflowInput(work)).toEqual({
      entries: [
        {
          branchRefs: ['branch.public.work_2'],
          checkRefs: ['check.public.work_2'],
          diffRefs: ['diff.public.work_2'],
          freshness: 'fresh',
          state: 'pr_ready',
          workflowRef: 'git-workflow.public.work_2',
        },
      ],
      generatedAt: '2026-06-17T23:41:00.000Z',
      snapshotRef: 'git-workflow-snapshot.public.work_2',
      versionRef: 'git-workflow-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
