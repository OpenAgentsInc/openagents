import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgePromptSuggestionsInput,
  projectForgePromptSuggestionsEvidence,
} from './prompt-suggestions-evidence'

const baseInput = {
  generatedAt: '2026-06-18T09:00:00.000Z',
  snapshotRef: 'prompt-suggestions-snapshot.public.work_1',
  versionRef: 'prompt-suggestions-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyFileSuggestion = {
  confidenceRefs: ['suggestion-confidence.public.high'],
  displayRefs: ['suggestion-display.public.file_name'],
  expirationRefs: ['suggestion-expiration.public.fresh'],
  freshness: 'fresh' as const,
  insertTextRefs: ['suggestion-insert.public.file_ref'],
  kind: 'file' as const,
  privacy: 'scoped_private' as const,
  privacyRefs: ['suggestion-privacy.public.current_workspace'],
  provenanceRefs: ['suggestion-provenance.public.workspace_index'],
  rankingRefs: ['suggestion-ranking.public.path_and_recency'],
  scopeRefs: ['suggestion-scope.public.current_workspace'],
  status: 'ready' as const,
  suggestionRef: 'suggestion.public.file_readme',
  validationRefs: ['suggestion-validation.public.file_exists'],
}

describe('Forge prompt suggestions evidence projection', () => {
  test('projects prompt suggestions as refs-only non-authoritative state', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [readyFileSuggestion],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      actions: 0,
      disabled: 0,
      ready: 1,
      scoped: 1,
      semantic: 0,
      stale: 0,
      suggestions: 1,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      actionExecutionAuthority: false,
      autocompleteStreamAuthority: false,
      commandExecutionAuthority: false,
      externalActionTriggerAuthority: false,
      permissionGrantAuthority: false,
      privateArtifactReadAuthority: false,
      privateFileReadAuthority: false,
      promptInsertionAuthority: false,
      promptSubmissionAuthority: false,
      rankingExecutionAuthority: false,
      semanticRoutingDecisionAuthority: false,
      settingsMutationAuthority: false,
      settlementAuthority: false,
      suggestionIndexingAuthority: false,
      toolInvocationAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing prompt suggestions evidence as empty', () => {
    const view = projectForgePromptSuggestionsEvidence({
      generatedAt: '2026-06-18T09:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks action suggestions that mix inserted text without separation refs', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [
        {
          actionRef: 'suggestion-action.public.open_issue',
          freshness: 'fresh',
          insertTextRefs: ['suggestion-insert.public.issue_prompt'],
          kind: 'follow_up_action',
          privacy: 'public_safe',
          rankingRefs: ['suggestion-ranking.public.semantic'],
          semanticSelectorRefs: ['semantic-selector.public.follow_up'],
          status: 'ready',
          suggestionRef: 'suggestion.public.follow_up',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.work_1:action-insert-separation-missing:suggestion.public.follow_up',
    )
  })

  test('blocks scoped suggestions without scope and privacy refs', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [
        {
          ...readyFileSuggestion,
          privacyRefs: [],
          scopeRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.work_1:scoped-suggestion-privacy-missing:suggestion.public.file_readme',
    )
  })

  test('blocks destructive or external actions without permission refs', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [
        {
          actionRef: 'suggestion-action.public.delete_branch',
          actionSeparationRefs: ['action-separation.public.insert_vs_execute'],
          destructiveActionRefs: ['destructive-action.public.delete_branch'],
          externalActionRefs: ['external-action.public.github_write'],
          freshness: 'fresh',
          kind: 'follow_up_action',
          privacy: 'public_safe',
          rankingRefs: ['suggestion-ranking.public.semantic'],
          semanticSelectorRefs: ['semantic-selector.public.follow_up'],
          status: 'ready',
          suggestionRef: 'suggestion.public.delete_branch',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.work_1:destructive-external-permission-missing:suggestion.public.delete_branch',
    )
  })

  test('blocks semantic suggestions without selector and non-keyword ranking refs', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [
        {
          freshness: 'fresh',
          kind: 'prompt_starter',
          privacy: 'public_safe',
          rankingRefs: ['suggestion-ranking.public.keyword_only'],
          semanticSelectorRefs: [],
          status: 'ready',
          suggestionRef: 'suggestion.public.prompt_starter',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.work_1:semantic-suggestion-ranking-evidence-missing:suggestion.public.prompt_starter',
    )
  })

  test('blocks stale and expired suggestions', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [
        {
          ...readyFileSuggestion,
          freshness: 'expired',
          status: 'expired',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.work_1:stale-or-expired-suggestion:suggestion.public.file_readme',
    )
  })

  test('surfaces disabled suggestions as typed state', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      entries: [
        {
          disablementRefs: ['suggestions-disabled.public.user_setting'],
          freshness: 'fresh',
          kind: 'slash_command',
          privacy: 'public_safe',
          status: 'disabled',
          suggestionRef: 'suggestion.public.disabled',
        },
      ],
    })

    expect(view.status).toBe('disabled')
    expect(view.blockerRefs).toEqual([])
    expect(view.counts.disabled).toBe(1)
  })

  test('blocks populated prompt suggestions without snapshot refs', () => {
    const view = projectForgePromptSuggestionsEvidence({
      entries: [readyFileSuggestion],
      generatedAt: '2026-06-18T09:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.no_snapshot:missing-prompt-suggestions-snapshot-ref',
    )
  })

  test('omits unsafe private suggestion material before projection', () => {
    const view = projectForgePromptSuggestionsEvidence({
      ...baseInput,
      blockerRefs: [
        'suggestion-blocker.public.safe',
        'raw prompt text /Users/christopher/prompt.md',
      ],
      entries: [
        {
          ...readyFileSuggestion,
          actionRef: 'suggestion-action.public.safe',
          actionSeparationRefs: ['action-separation.public.safe'],
          auditRefs: ['suggestion-audit.public.safe', 'unvalidated model output private'],
          displayRefs: ['suggestion-display.public.safe', 'repository private data'],
          insertTextRefs: ['suggestion-insert.public.safe', 'inserted text body secret'],
          permissionRefs: ['permission.public.safe'],
          provenanceRefs: ['suggestion-provenance.public.safe'],
          scopeRefs: ['suggestion-scope.public.safe', 'raw file /Users/christopher/file.ts'],
          validationRefs: ['suggestion-validation.public.safe'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.auditRefs).toEqual(['suggestion-audit.public.safe'])
    expect(view.entries[0]?.displayRefs).toEqual(['suggestion-display.public.safe'])
    expect(view.entries[0]?.insertTextRefs).toEqual(['suggestion-insert.public.safe'])
    expect(view.entries[0]?.scopeRefs).toEqual(['suggestion-scope.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-prompt-suggestions-blocker:work.public.work_1:unsafe-prompt-suggestions-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt text')
    expect(payload).not.toContain('unvalidated model output')
    expect(payload).not.toContain('repository private data')
    expect(payload).not.toContain('inserted text body')
    expect(payload).not.toContain('raw file')
    expect(payload).not.toContain('secret')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T09:00:00.000Z',
      promptSuggestionsEvidence: {
        entries: [readyFileSuggestion],
        generatedAt: '2026-06-18T09:01:00.000Z',
        snapshotRef: 'prompt-suggestions-snapshot.public.work_2',
        versionRef: 'prompt-suggestions-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgePromptSuggestionsInput(work)).toEqual({
      entries: [readyFileSuggestion],
      generatedAt: '2026-06-18T09:01:00.000Z',
      snapshotRef: 'prompt-suggestions-snapshot.public.work_2',
      versionRef: 'prompt-suggestions-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
