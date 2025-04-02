import React, { useState, useMemo, useEffect, useCallback } from 'react';
import { useThreads, Thread, useSettings, UIMessage } from '@openagents/core';
import { messageRepository, threadRepository } from '@openagents/core/src/db/repositories';
import { getDatabase } from '@openagents/core/src/db/database';
import { v4 as uuidv4 } from 'uuid';
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
import { toast } from 'sonner';

interface ThreadListProps {
  currentThreadId: string;
  onSelectThread: (threadId: string) => void;
  onCreateThread: () => Promise<void>; // Changed to Promise for optimistic updates
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
  // Use a very short refresh interval for immediate updates
  const { threads, isLoading, error, refresh, deleteThread, createThread, updateThread } = useThreads({ refreshInterval: 300 });
  const { settings, getPreference } = useSettings();

  // Immediately refresh when component mounts or is forced to re-render
  useEffect(() => {
    refresh();
  }, [refresh]);
  const [isRenameDialogOpen, setIsRenameDialogOpen] = useState(false);
  const [threadToRename, setThreadToRename] = useState<Thread | null>(null);
  const [newTitle, setNewTitle] = useState('');
  // Local state to track threads to be removed for optimistic updates
  const [pendingDeletes, setPendingDeletes] = useState<Set<string>>(new Set());
  // State to store the confirmation preference
  const [confirmThreadDeletion, setConfirmThreadDeletion] = useState(true);
  
  // Load the confirmation preference
  useEffect(() => {
    const loadPreference = async () => {
      const shouldConfirm = await getPreference("confirmThreadDeletion", true);
      setConfirmThreadDeletion(shouldConfirm);
    };
    loadPreference();
  }, [getPreference, settings]);
  // We don't need optimistic threads since we're handling that in the database layer now
  // This keeps the UI simpler and avoids duplicate entries

  // Group threads by time periods - filter out pending deletes for optimistic UI
  const groupedThreads = useMemo(() => {
    const now = new Date();
    const groups: ThreadGroup[] = [
      { label: 'Last 7 Days', threads: [] },
      { label: 'Last 30 Days', threads: [] },
      { label: 'Older', threads: [] }
    ];

    // Filter out threads that are pending deletion
    const filteredThreads = threads.filter(thread => !pendingDeletes.has(thread.id));

    filteredThreads.forEach(thread => {
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
  }, [threads, pendingDeletes]);

  // Only show loading state on initial load when no threads are available
  // Don't show loading state during refreshes to prevent UI flashing
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
                        className={`group/link relative flex h-9 w-full items-center overflow-hidden rounded-lg px-2 py-1 text-sm outline-none hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:text-sidebar-accent-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring hover:focus-visible:bg-sidebar-accent ${thread.id === currentThreadId ? 'bg-sidebar-accent text-sidebar-accent-foreground before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md' : ''
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
                                
                                // Function to handle thread deletion
                                const handleDeleteWithToast = async (threadId: string, threadTitle: string) => {
                                  // Cache the original thread details for recovery
                                  const deletedThread = threads.find(t => t.id === threadId);
                                  if (!deletedThread) {
                                    console.error("Could not find thread for deletion:", threadId);
                                    return;
                                  }
                                  
                                  // Save a copy of the thread data
                                  const threadData = {
                                    id: deletedThread.id,
                                    title: deletedThread.title,
                                    createdAt: deletedThread.createdAt,
                                    updatedAt: deletedThread.updatedAt,
                                    modelId: deletedThread.modelId,
                                    systemPrompt: deletedThread.systemPrompt
                                  };
                                  
                                  // Also cache all messages from this thread
                                  let cachedMessages: UIMessage[] = [];
                                  try {
                                    // Initialize message repository if needed
                                    try {
                                      const db = await getDatabase();
                                      await messageRepository.initialize(db);
                                    } catch (initError) {
                                      console.error("Error initializing database for message caching:", initError);
                                    }
                                    
                                    // Fetch and cache all messages from this thread
                                    cachedMessages = await messageRepository.getMessagesByThreadId(threadId);
                                    console.log(`Cached ${cachedMessages.length} messages for potential thread restoration`);
                                  } catch (fetchError) {
                                    console.error("Error caching messages before deletion:", fetchError);
                                  }
                                  
                                  // Optimistic update - add to pending deletes first
                                  setPendingDeletes(prev => new Set([...prev, threadId]));
                                  
                                  // Delay actual deletion to allow for undo
                                  let deleteTimeoutId: NodeJS.Timeout;
                                  let undoClicked = false;
                                  
                                  // Show toast with undo option
                                  toast.info(
                                    `Chat deleted`,
                                    {
                                      description: `"${threadTitle || 'Untitled'}" was deleted.`,
                                      action: {
                                        label: "Undo",
                                        onClick: async () => {
                                          // Mark that undo was clicked to prevent deletion
                                          undoClicked = true;
                                          
                                          // Clear the deletion timeout if it's still pending
                                          if (deleteTimeoutId) {
                                            clearTimeout(deleteTimeoutId);
                                          }
                                          
                                          // Remove from pending deletes to show in UI
                                          setPendingDeletes(prev => {
                                            const newSet = new Set([...prev]);
                                            newSet.delete(threadId);
                                            return newSet;
                                          });
                                          
                                          try {
                                            // Check if thread still exists or has been deleted
                                            const existingThreads = await refresh();
                                            const threadExists = existingThreads.some(t => t.id === threadId);
                                            
                                            if (!threadExists) {
                                              // Thread was deleted, need to recreate it
                                              console.log("Thread was already deleted, recreating with data:", threadData);
                                              
                                              // Initialize repositories
                                              try {
                                                const db = await getDatabase();
                                                await threadRepository.initialize(db);
                                                await messageRepository.initialize(db);
                                              } catch (initError) {
                                                console.error("Error initializing database for thread restoration:", initError);
                                              }
                                              
                                              // Recreate the thread with the SAME ID to ensure message consistency
                                              try {
                                                // Create thread with exact same ID and properties
                                                await threadRepository.createThread({
                                                  id: threadData.id,
                                                  title: threadData.title,
                                                  createdAt: threadData.createdAt,
                                                  updatedAt: threadData.updatedAt,
                                                  modelId: threadData.modelId,
                                                  systemPrompt: threadData.systemPrompt
                                                });
                                                
                                                // Restore all messages if we have them cached
                                                if (cachedMessages.length > 0) {
                                                  console.log(`Restoring ${cachedMessages.length} messages to thread ${threadData.id}`);
                                                  
                                                  // Reinsert messages one by one
                                                  for (const message of cachedMessages) {
                                                    try {
                                                      await messageRepository.createMessage({
                                                        ...message,
                                                        threadId: threadData.id
                                                      });
                                                    } catch (msgError) {
                                                      console.error(`Error restoring message ${message.id}:`, msgError);
                                                    }
                                                  }
                                                }
                                                
                                                // Switch to the restored thread
                                                onSelectThread(threadData.id);
                                                toast.success(`Chat "${threadData.title || 'Untitled'}" restored completely`);
                                              } catch (createError) {
                                                console.error("Error recreating thread:", createError);
                                                
                                                // Fallback: create a new thread if exact recreation fails
                                                const newThread = await createThread(threadData.title);
                                                onSelectThread(newThread.id);
                                                toast.warning("Chat restored with a new ID (messages could not be recovered)");
                                              }
                                            } else {
                                              // Thread still exists, just switch to it
                                              onSelectThread(threadId);
                                              toast.success("Operation canceled");
                                            }
                                            
                                            // Final refresh to update UI
                                            refresh();
                                          } catch (error) {
                                            console.error("Failed to restore thread:", error);
                                            toast.error("Failed to restore chat");
                                          }
                                        }
                                      },
                                      duration: 5000
                                    }
                                  );
                                  
                                  // Delay the actual deletion to allow for undo
                                  deleteTimeoutId = setTimeout(() => {
                                    if (!undoClicked) {
                                      // Only delete if undo wasn't clicked
                                      onDeleteThread(threadId);
                                    }
                                  }, 300);
                                };
                                
                                if (confirmThreadDeletion) {
                                  // Show confirmation dialog if preference is set
                                  if (window.confirm('Are you sure you want to delete this chat?')) {
                                    handleDeleteWithToast(thread.id, thread.title);
                                  }
                                } else {
                                  // Delete immediately with toast if confirmation is disabled
                                  handleDeleteWithToast(thread.id, thread.title);
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
