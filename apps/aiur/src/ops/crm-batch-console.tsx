import { useCallback, useEffect, useMemo, useState } from 'react'

import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

import {
  type CrmBatchApproveResult,
  type CrmBatchCommand,
  type CrmBatchQueueGroup,
  fetchCrmBatchQueue,
  postCrmBatchApprove,
} from './crm-batch-api-client'
import {
  clearSelection,
  commandSubjectLine,
  selectAllIds,
  selectedCount,
  summarizeApproveResult,
  toggleId,
} from './crm-batch-selection'

/**
 * OB-4 (#8561) operator batch approval surface.
 *
 * Lists pending `crm_contact_commands` (send_email drafts) grouped by day +
 * segment, lets the operator select a subset, and posts one batch-approve.
 * Upstream still approves+executes each command individually through
 * `approveAndExecuteCrmSendCommand` — this panel is batch UX only.
 *
 * Invariant `lead_gen_agent.no_send_without_approval_receipt.v1`: preserved.
 */
export function CrmBatchApprovalPanel() {
  const [groups, setGroups] = useState<ReadonlyArray<CrmBatchQueueGroup>>([])
  const [total, setTotal] = useState(0)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')
  const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined)
  const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set())
  const [approving, setApproving] = useState(false)
  const [lastResult, setLastResult] = useState<CrmBatchApproveResult | undefined>(
    undefined,
  )

  const reload = useCallback(async () => {
    setStatus('loading')
    setErrorMessage(undefined)
    const result = await fetchCrmBatchQueue({ status: 'proposed' })
    if (!result.ok) {
      setStatus('error')
      setErrorMessage(result.messageSafe)
      return
    }
    setGroups(result.value.queue.groups)
    setTotal(result.value.queue.total)
    setSelected(clearSelection())
    setStatus('ready')
  }, [])

  useEffect(() => {
    let cancelled = false
    void (async () => {
      const result = await fetchCrmBatchQueue({ status: 'proposed' })
      if (cancelled) return
      if (!result.ok) {
        setStatus('error')
        setErrorMessage(result.messageSafe)
        return
      }
      setGroups(result.value.queue.groups)
      setTotal(result.value.queue.total)
      setStatus('ready')
    })()
    return () => {
      cancelled = true
    }
  }, [])

  const allIds = useMemo(
    () => groups.flatMap(group => group.commands.map(command => command.id)),
    [groups],
  )

  const onToggle = (id: string) => {
    setSelected(prev => toggleId(prev, id))
  }

  const onSelectAll = () => {
    setSelected(selectAllIds(allIds))
  }

  const onClear = () => {
    setSelected(clearSelection())
  }

  const onApprove = async () => {
    const commandIds = [...selected]
    if (commandIds.length === 0 || approving) return
    setApproving(true)
    setLastResult(undefined)
    setErrorMessage(undefined)
    const result = await postCrmBatchApprove({ commandIds })
    setApproving(false)
    if (!result.ok) {
      setErrorMessage(result.messageSafe)
      return
    }
    setLastResult(result.value.result)
    await reload()
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>CRM draft batch approval</CardTitle>
        <CardDescription>
          Pending <code>send_email</code> drafts from{' '}
          <code>crm_contact_commands</code>, grouped by day + segment. Batch
          approve is UX only — each send still goes through the per-command
          approval gate and gets its own receipt (
          <code>no_send_without_approval_receipt</code>). Daily cap default is
          100/day (OB-1 ramp config still open). See #8561.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            data-testid="crm-batch-reload"
            disabled={status === 'loading' || approving}
            onClick={() => void reload()}
            size="sm"
            variant="secondary"
          >
            Reload
          </Button>
          <Button
            data-testid="crm-batch-select-all"
            disabled={allIds.length === 0 || approving}
            onClick={onSelectAll}
            size="sm"
            variant="ghost"
          >
            Select all ({allIds.length})
          </Button>
          <Button
            data-testid="crm-batch-clear"
            disabled={selectedCount(selected) === 0 || approving}
            onClick={onClear}
            size="sm"
            variant="ghost"
          >
            Clear
          </Button>
          <Button
            data-testid="crm-batch-approve"
            disabled={selectedCount(selected) === 0 || approving}
            onClick={() => void onApprove()}
            size="sm"
          >
            {approving
              ? 'Approving…'
              : `Approve selected (${selectedCount(selected)})`}
          </Button>
        </div>

        {status === 'loading' && (
          <p className="text-sm text-khala-text-faint">Loading queue…</p>
        )}
        {status === 'error' && (
          <p className="text-sm text-khala-danger" data-testid="crm-batch-error">
            {errorMessage ?? 'Failed to load queue.'}
          </p>
        )}
        {errorMessage !== undefined && status === 'ready' && (
          <p className="text-sm text-khala-danger" data-testid="crm-batch-error">
            {errorMessage}
          </p>
        )}
        {lastResult !== undefined && (
          <p
            className="text-sm text-khala-text"
            data-testid="crm-batch-last-result"
          >
            {summarizeApproveResult(lastResult)}
          </p>
        )}

        {status === 'ready' && (
          <div className="grid gap-4" data-testid="crm-batch-queue">
            <p className="text-sm text-khala-text-muted" data-testid="crm-batch-total">
              {total === 0
                ? 'No pending drafts.'
                : `${total} pending draft${total === 1 ? '' : 's'} across ${groups.length} group${groups.length === 1 ? '' : 's'}.`}
            </p>
            {groups.map(group => (
              <GroupBlock
                group={group}
                key={`${group.day}::${group.segmentRef}`}
                onToggle={onToggle}
                selected={selected}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function GroupBlock({
  group,
  selected,
  onToggle,
}: {
  group: CrmBatchQueueGroup
  selected: ReadonlySet<string>
  onToggle: (id: string) => void
}) {
  return (
    <div
      className="grid gap-1 border border-khala-border p-3"
      data-testid="crm-batch-group"
      data-day={group.day}
      data-segment={group.segmentRef}
    >
      <div className="mb-1 flex items-center justify-between text-sm text-khala-text">
        <span className="font-mono">
          {group.day} · {group.segmentRef}
        </span>
        <span className="text-khala-text-muted">
          {group.commands.length} draft{group.commands.length === 1 ? '' : 's'}
        </span>
      </div>
      <ul className="grid gap-0">
        {group.commands.map(command => (
          <CommandRow
            command={command}
            key={command.id}
            onToggle={onToggle}
            selected={selected.has(command.id)}
          />
        ))}
      </ul>
    </div>
  )
}

function CommandRow({
  command,
  selected,
  onToggle,
}: {
  command: CrmBatchCommand
  selected: boolean
  onToggle: (id: string) => void
}) {
  return (
    <li className="flex items-start gap-2 border-b border-khala-border py-1.5 text-sm text-khala-text last:border-b-0">
      <input
        aria-label={`Select ${command.id}`}
        checked={selected}
        className="mt-1"
        data-testid="crm-batch-command-checkbox"
        onChange={() => onToggle(command.id)}
        type="checkbox"
      />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span className="font-mono text-xs text-khala-text-muted">
            {command.id}
          </span>
          <span className="text-xs text-khala-text-faint">
            {command.createdAt.slice(0, 19).replace('T', ' ')}
          </span>
        </div>
        <div>{commandSubjectLine(command.payload)}</div>
        <div className="text-xs text-khala-text-muted">
          contact {command.contactId ?? '—'}
          {command.proposedByRef !== null
            ? ` · proposed by ${command.proposedByRef}`
            : ''}
        </div>
      </div>
    </li>
  )
}
