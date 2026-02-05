import type { ToolCallMessagePartComponent } from '@assistant-ui/react';
import { useAuiState } from '@assistant-ui/react';
import { CheckIcon, ChevronDownIcon, ChevronUpIcon, XCircleIcon } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

type ApprovalResult = {
  status: 'approval_required' | 'approval_pending' | 'approval_rejected';
  approvalId: string;
  summary?: string;
  toolName?: string;
  toolInput?: unknown;
};

const isApprovalResult = (result: unknown): result is ApprovalResult => {
  if (!result || typeof result !== 'object') return false;
  const status = (result as { status?: unknown }).status;
  const approvalId = (result as { approvalId?: unknown }).approvalId;
  if (status !== 'approval_required' && status !== 'approval_pending' && status !== 'approval_rejected') {
    return false;
  }
  return typeof approvalId === 'string' && approvalId.trim().length > 0;
};

export const ToolFallback: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const threadId = useAuiState(({ threadListItem }) => threadListItem.id);
  const [isCollapsed, setIsCollapsed] = useState(true);
  const [approvalState, setApprovalState] = useState<{
    status: 'idle' | 'sending' | 'approved' | 'rejected' | 'error';
    error?: string;
  }>({ status: 'idle' });

  const isCancelled =
    status.type === 'incomplete' && status.reason === 'cancelled';
  const cancelledReason =
    isCancelled && status.error
      ? typeof status.error === 'string'
        ? status.error
        : JSON.stringify(status.error)
      : null;

  const approvalResult = isApprovalResult(result) ? result : null;

  if (approvalResult) {
    const summary =
      approvalResult.summary ||
      `Approve ${toolName.replace(/_/g, ' ')}?`;

    const canRespond =
      approvalResult.status !== 'approval_rejected' &&
      approvalState.status !== 'approved' &&
      approvalState.status !== 'rejected' &&
      approvalState.status !== 'sending';

    const handleDecision = async (decision: 'approved' | 'rejected') => {
      if (!threadId) {
        setApprovalState({ status: 'error', error: 'Missing thread id.' });
        return;
      }
      setApprovalState({ status: 'sending' });
      try {
        const response = await fetch('/approvals', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            approvalId: approvalResult.approvalId,
            decision,
            threadId,
          }),
        });
        if (!response.ok) {
          const text = await response.text();
          setApprovalState({
            status: 'error',
            error: text || 'Approval request failed.',
          });
          return;
        }
        setApprovalState({ status: decision });
      } catch (error) {
        setApprovalState({
          status: 'error',
          error: error instanceof Error ? error.message : 'Approval request failed.',
        });
      }
    };

    const heading =
      approvalResult.status === 'approval_rejected'
        ? 'Approval rejected'
        : approvalResult.status === 'approval_pending'
          ? 'Approval pending'
          : 'Approval required';

    return (
      <div className="aui-tool-fallback-root mb-4 flex w-full flex-col gap-3 rounded-lg border border-muted bg-muted/20 p-4">
        <div className="flex items-center justify-between gap-2">
          <div>
            <p className="text-sm font-semibold text-foreground">{heading}</p>
            <p className="text-xs text-muted-foreground">{summary}</p>
          </div>
          <span className="rounded-md border border-border bg-background px-2 py-0.5 text-xs font-mono text-muted-foreground">
            {approvalResult.approvalId.slice(0, 8)}
          </span>
        </div>
        <div className="text-xs text-muted-foreground">
          Tool: <span className="font-mono">{toolName}</span>
        </div>
        {approvalResult.status === 'approval_rejected' && (
          <div className="text-xs font-medium text-red-400">This request was rejected.</div>
        )}
        {approvalState.status === 'approved' && (
          <div className="text-xs font-medium text-green-400">Approved.</div>
        )}
        {approvalState.status === 'rejected' && (
          <div className="text-xs font-medium text-red-400">Rejected.</div>
        )}
        {approvalState.status === 'error' && (
          <div className="text-xs font-medium text-red-400">{approvalState.error}</div>
        )}
        {canRespond && (
          <div className="flex flex-wrap gap-2">
            <Button size="sm" onClick={() => handleDecision('approved')} disabled={approvalState.status === 'sending'}>
              {approvalState.status === 'sending' ? 'Sending…' : 'Approve'}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => handleDecision('rejected')}
              disabled={approvalState.status === 'sending'}
            >
              Reject
            </Button>
          </div>
        )}
        <p className="text-[11px] text-muted-foreground">
          After approving, send a follow-up message so the assistant can continue.
        </p>
      </div>
    );
  }

  return (
    <div
      className={cn(
        'aui-tool-fallback-root mb-4 flex w-full flex-col gap-3 rounded-lg border py-3',
        isCancelled && 'border-muted-foreground/30 bg-muted/30',
      )}
    >
      <div className="aui-tool-fallback-header flex items-center gap-2 px-4">
        {isCancelled ? (
          <XCircleIcon className="aui-tool-fallback-icon size-4 text-muted-foreground" />
        ) : (
          <CheckIcon className="aui-tool-fallback-icon size-4" />
        )}
        <p
          className={cn(
            'aui-tool-fallback-title grow',
            isCancelled && 'text-muted-foreground line-through',
          )}
        >
          {isCancelled ? 'Cancelled tool: ' : 'Used tool: '}
          <b>{toolName}</b>
        </p>
        <Button onClick={() => setIsCollapsed(!isCollapsed)}>
          {isCollapsed ? <ChevronUpIcon /> : <ChevronDownIcon />}
        </Button>
      </div>
      {!isCollapsed && (
        <div className="aui-tool-fallback-content flex flex-col gap-2 border-t pt-2">
          {cancelledReason && (
            <div className="aui-tool-fallback-cancelled-root px-4">
              <p className="aui-tool-fallback-cancelled-header font-semibold text-muted-foreground">
                Cancelled reason:
              </p>
              <p className="aui-tool-fallback-cancelled-reason text-muted-foreground">
                {cancelledReason}
              </p>
            </div>
          )}
          <div
            className={cn(
              'aui-tool-fallback-args-root px-4',
              isCancelled && 'opacity-60',
            )}
          >
            <pre className="aui-tool-fallback-args-value whitespace-pre-wrap">
              {argsText}
            </pre>
          </div>
          {!isCancelled && result !== undefined && (
            <div className="aui-tool-fallback-result-root border-t border-dashed px-4 pt-2">
              <p className="aui-tool-fallback-result-header font-semibold">
                Result:
              </p>
              <pre className="aui-tool-fallback-result-content whitespace-pre-wrap">
                {typeof result === 'string'
                  ? result
                  : JSON.stringify(result, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
