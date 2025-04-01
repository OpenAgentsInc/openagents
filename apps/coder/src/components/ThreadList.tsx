import React, { useState, useMemo } from 'react';
import { useThreads, Thread } from '@openagents/core';
import { Button } from './ui/button';
import { X, Plus } from 'lucide-react';
import { format, subDays, isWithinInterval, startOfDay } from 'date-fns';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter
} from './ui/dialog';
import { Input } from './ui/input';

interface ThreadListProps {
  currentThreadId: string;
  onSelectThread: (threadId: string) => void;
  onCreateThread: () => void;
  onDeleteThread: (threadId: string) => void;
  onRenameThread?: (threadId: string, title: string) => void;
  // onPinThread?: (threadId: string) => void;
}

interface ThreadGroup {
  label: string;
  threads: Thread[];
}

export function ThreadList({
  currentThreadId,
  onSelectThread,
  onCreateThread,
  onDeleteThread,
  onRenameThread,
  // onPinThread
}: ThreadListProps) {
  const { threads, isLoading, error } = useThreads({ refreshInterval: 5000 });
  const [isRenameDialogOpen, setIsRenameDialogOpen] = useState(false);
  const [threadToRename, setThreadToRename] = useState<Thread | null>(null);
  const [newTitle, setNewTitle] = useState('');

  // Group threads by time periods
  const groupedThreads = useMemo(() => {
    const now = new Date();
    const groups: ThreadGroup[] = [
      { label: 'Last 7 Days', threads: [] },
      { label: 'Last 30 Days', threads: [] },
      { label: 'Older', threads: [] }
    ];

    threads.forEach(thread => {
      const threadDate = new Date(thread.createdAt);

      if (isWithinInterval(threadDate, { start: subDays(now, 7), end: now })) {
        groups[0].threads.push(thread);
      } else if (isWithinInterval(threadDate, { start: subDays(now, 30), end: now })) {
        groups[1].threads.push(thread);
      } else {
        groups[2].threads.push(thread);
      }
    });

    return groups;
  }, [threads]);

  if (isLoading && threads.length === 0) {
    return (
      <div data-sidebar="content" className="flex min-h-0 flex-1 flex-col gap-2 overflow-auto group-data-[collapsible=icon]:overflow-hidden small-scrollbar scroll-shadow relative pb-2">
        <div className="py-4 text-center text-muted-foreground">
          Loading...
        </div>
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
      {groupedThreads.map((group, index) => (
        group.threads.length > 0 && (
          <div key={group.label} data-sidebar="group" className="relative flex w-full min-w-0 flex-col p-2">
            <div data-sidebar="group-label" className="flex h-8 shrink-0 select-none items-center rounded-md text-xs font-medium outline-none ring-sidebar-ring transition-[margin,opa] duration-200 ease-snappy focus-visible:ring-2 [&>svg]:size-4 [&>svg]:shrink-0 group-data-[collapsible=icon]:-mt-8 group-data-[collapsible=icon]:opacity-0 px-1.5 text-color-heading">
              <span>{group.label}</span>
            </div>
            <div data-sidebar="group-content" className="w-full text-sm">
              <ul data-sidebar="menu" className="flex w-full min-w-0 flex-col gap-1">
                {group.threads.map((thread) => (
                  <span key={thread.id} data-state="closed">
                    <li data-sidebar="menu-item" className="group/menu-item relative">
                      <a
                        className={`group/link relative flex h-9 w-full items-center overflow-hidden rounded-lg px-2 py-1 text-sm outline-none hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:text-sidebar-accent-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring hover:focus-visible:bg-sidebar-accent ${thread.id === currentThreadId ? 'bg-sidebar-accent text-sidebar-accent-foreground' : ''
                          }`}
                        title={thread.title || 'Untitled'}
                        onClick={() => {
                          onSelectThread(thread.id);
                          // Dispatch focus event when thread is selected
                          window.dispatchEvent(new Event('focus-chat-input'));
                        }}
                      >
                        <div className="relative flex w-full items-center">
                          <div className="relative w-full">
                            <input
                              aria-label="Thread title"
                              aria-describedby="thread-title-hint"
                              aria-readonly="true"
                              readOnly
                              tabIndex={-1}
                              className="hover:truncate-none h-full w-full rounded bg-transparent px-1 py-1 text-sm text-muted-foreground outline-none pointer-events-none cursor-pointer overflow-hidden truncate"
                              title={thread.title || 'Untitled'}
                              type="text"
                              value={thread.title || 'Untitled'}
                              onClick={(e) => {
                                e.stopPropagation();
                                onSelectThread(thread.id);
                                // Dispatch focus event when thread is selected via the input
                                window.dispatchEvent(new Event('focus-chat-input'));
                              }}
                            />
                          </div>
                          <div className="pointer-events-auto flex items-center justify-end text-muted-foreground opacity-0 group-hover/link:opacity-100 transition-opacity">
                            {/* Commented out pin button
                            {onPinThread && (
                              <button
                                className="rounded-md p-1.5 hover:bg-muted/40"
                                tabIndex={-1}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  onPinThread(thread.id);
                                }}
                                aria-label="Pin thread"
                              >
                                <Pin className="size-4" />
                              </button>
                            )}
                            */}
                            <button
                              className="rounded-md p-1.5 hover:bg-destructive/50 hover:text-destructive-foreground"
                              tabIndex={-1}
                              onClick={(e) => {
                                e.stopPropagation();
                                if (window.confirm('Are you sure you want to delete this chat?')) {
                                  onDeleteThread(thread.id);
                                }
                              }}
                              aria-label="Delete thread"
                            >
                              <X className="size-4" />
                            </button>
                          </div>
                        </div>
                      </a>
                    </li>
                  </span>
                ))}
              </ul>
            </div>
          </div>
        )
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
}
