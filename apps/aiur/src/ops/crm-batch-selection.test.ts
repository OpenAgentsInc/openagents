import { describe, expect, test } from 'vitest'

import {
  clearSelection,
  commandSubjectLine,
  selectAllIds,
  selectedCount,
  summarizeApproveResult,
  toggleId,
} from './crm-batch-selection'

describe('crm-batch-selection helpers', () => {
  test('toggleId adds and removes ids', () => {
    const empty = new Set<string>()
    const one = toggleId(empty, 'a')
    expect([...one]).toEqual(['a'])
    const none = toggleId(one, 'a')
    expect([...none]).toEqual([])
  })

  test('selectAllIds / clearSelection / selectedCount', () => {
    const all = selectAllIds(['a', 'b', 'c'])
    expect(selectedCount(all)).toBe(3)
    expect(selectedCount(clearSelection())).toBe(0)
  })

  test('summarizeApproveResult includes dispositions that are non-zero', () => {
    const line = summarizeApproveResult({
      batchRef: 'crm_batch_9',
      cappedCount: 1,
      executedCount: 2,
      failedCount: 1,
      notFoundCount: 0,
      notPendingCount: 0,
      requestedCount: 4,
    })
    expect(line).toContain('Batch crm_batch_9')
    expect(line).toContain('2/4 executed')
    expect(line).toContain('1 failed')
    expect(line).toContain('1 capped')
    expect(line).not.toContain('not found')
  })

  test('commandSubjectLine prefers template + channel', () => {
    expect(
      commandSubjectLine({ templateSlug: 'welcome', channel: 'resend' }),
    ).toBe('welcome via resend')
    expect(commandSubjectLine({})).toBe('— via unknown')
  })
})
