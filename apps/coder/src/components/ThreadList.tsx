import React, { useState, useMemo, useEffect, useCallback } from 'react';
import { useThreads, Thread, useSettings } from '@openagents/core';
import { useThreadDeletion } from '@openagents/core/src/chat/useThreadDeletion';
import { format, subDays, isWithinInterval, startOfDay } from 'date-fns';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter
} from './ui/dialog';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { ThreadGroup } from './ThreadGroup';

interface ThreadListProps {
  currentThreadId: string;
  onSelectThread: (threadId: string) => void;
  onCreateThread: () => Promise<void>;
  onDeleteThread: (threadId: string) => void;
  onRenameThread?: (threadId: string, title: string) => void;
}

interface ThreadGroup {
  label: string;
  threads: Thread[];
}

export const ThreadList = React.memo(function ThreadList({
  currentThreadId,
  onSelectThread,
  onCreateThread,
  onDeleteThread,
  onRenameThread,
}: ThreadListProps) {
  // Use a more reasonable refresh interval (3 seconds instead of 300ms)
  const { threads, isLoading, error, refresh, deleteThread, createThread, updateThread } = useThreads({ refreshInterval: 3000 });
  const { settings, getPreference } = useSettings();

  // Dialog state
  const [isRenameDialogOpen, setIsRenameDialogOpen] = useState(false);
  const [threadToRename, setThreadToRename] = useState<Thread | null>(null);
  const [newTitle, setNewTitle] = useState('');
  
  // State to store the confirmation preference
  const [confirmThreadDeletion, setConfirmThreadDeletion] = useState(true);
  
  // Use the custom thread deletion hook
  const { pendingDeletes, handleDeleteWithToast } = useThreadDeletion({
    threads,
    refresh,
    createThread,
    updateThread,
    onDeleteThread,
    onSelectThread
  });
  
  // Load the confirmation preference
  useEffect(() => {
    const loadPreference = async () => {
      const shouldConfirm = await getPreference("confirmThreadDeletion", true);
      setConfirmThreadDeletion(shouldConfirm);
    };
    loadPreference();
  }, [getPreference, settings]);

  // Memoize date values to prevent recalculation on every render
  const now = useMemo(() => new Date(), []);
  const sevenDaysAgo = useMemo(() => subDays(now, 7), [now]);
  const thirtyDaysAgo = useMemo(() => subDays(now, 30), [now]);
  
  // Group threads by time periods - filter out pending deletes for optimistic UI
  const groupedThreads = useMemo(() => {
    const groups: ThreadGroup[] = [
      { label: 'Last 7 Days', threads: [] },
      { label: 'Last 30 Days', threads: [] },
      { label: 'Older', threads: [] }
    ];

    // Filter out threads that are pending deletion
    const filteredThreads = threads.filter(thread => !pendingDeletes.has(thread.id));

    filteredThreads.forEach(thread => {
      const threadDate = new Date(thread.createdAt);

      if (isWithinInterval(threadDate, { start: sevenDaysAgo, end: now })) {
        groups[0].threads.push(thread);
      } else if (isWithinInterval(threadDate, { start: thirtyDaysAgo, end: now })) {
        groups[1].threads.push(thread);
      } else {
        groups[2].threads.push(thread);
      }
    });

    return groups;
  }, [threads, pendingDeletes, sevenDaysAgo, thirtyDaysAgo, now]);

  // Memoize the delete handler to prevent recreation on every render
  const handleDeleteClick = useCallback((e: React.MouseEvent, threadId: string, threadTitle: string) => {
    e.stopPropagation();
    
    if (confirmThreadDeletion) {
      if (window.confirm('Are you sure you want to delete this chat?')) {
        handleDeleteWithToast(threadId, threadTitle);
      }
    } else {
      handleDeleteWithToast(threadId, threadTitle);
    }
  }, [confirmThreadDeletion, handleDeleteWithToast]);

  // Refresh once when component mounts
  useEffect(() => {
    refresh();
  }, []); // Empty dependency array ensures this only runs once

  // Only show loading state on initial load when no threads are available
  if (isLoading && threads.length === 0) {
    return (
      <div data-sidebar="content" className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto group-data-[collapsible=icon]:overflow-hidden small-scrollbar scroll-shadow relative pb-2">
        {/* Empty space with no visible loading text to prevent layout shifts */}
        <div className="py-4"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div data-sidebar="content" className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto group-data-[collapsible=icon]:overflow-hidden small-scrollbar scroll-shadow relative pb-2">
        <div className="py-4 text-center text-red-500">
          Error loading chats. Please try again.
        </div>
      </div>
    );
  }

  return (
    <div data-sidebar="content" className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto group-data-[collapsible=icon]:overflow-hidden small-scrollbar scroll-shadow relative pb-2">
      {groupedThreads.map((group) => (
        <ThreadGroup
          key={group.label}
          label={group.label}
          threads={group.threads}
          selectedThreadId={currentThreadId}
          onSelectThread={onSelectThread}
          onDeleteThread={handleDeleteClick}
        />
      ))}

      {/* Rename Dialog */}
      <Dialog open={isRenameDialogOpen} onOpenChange={setIsRenameDialogOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Rename Chat</DialogTitle>
          </DialogHeader>
          <div className="py-4">
            <Input
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              placeholder="Enter new title"
              className="w-full"
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter' && threadToRename && onRenameThread) {
                  onRenameThread(threadToRename.id, newTitle);
                  setIsRenameDialogOpen(false);
                }
              }}
            />
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setIsRenameDialogOpen(false)}
            >
              Cancel
            </Button>
            <Button
              onClick={() => {
                if (threadToRename && onRenameThread) {
                  onRenameThread(threadToRename.id, newTitle);
                  setIsRenameDialogOpen(false);
                }
              }}
            >
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
});