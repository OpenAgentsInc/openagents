import { useState, useEffect, useCallback, useRef } from 'react';
import { toast } from 'sonner';
import { Thread } from '../db/types';
import { UIMessage } from './types';
import { messageRepository, threadRepository } from '../db/repositories';
import { getDatabase } from '../db/database';

interface UseThreadDeletionProps {
  threads: Thread[];
  refresh: () => Promise<Thread[]>;
  createThread: (title?: string) => Promise<Thread>;
  updateThread: (threadId: string, updates: Partial<Thread>) => Promise<Thread | null>;
  onDeleteThread: (threadId: string) => void;
  onSelectThread: (threadId: string) => void;
}

export function useThreadDeletion({
  threads,
  refresh,
  createThread,
  updateThread,
  onDeleteThread,
  onSelectThread,
}: UseThreadDeletionProps) {
  // State for tracking pending deletes (optimistic UI)
  const [pendingDeletes, setPendingDeletes] = useState<Set<string>>(new Set());

  // Initialize repositories once
  const repositoriesInitialized = useRef(false);

  useEffect(() => {
    if (!repositoriesInitialized.current) {
      (async () => {
        try {
          const db = await getDatabase();
          await messageRepository.initialize(db);
          await threadRepository.initialize(db);
          repositoriesInitialized.current = true;
          console.log("Repositories initialized for thread deletion");
        } catch (error) {
          console.error("Error initializing repositories:", error);
        }
      })();
    }

    return () => {
      // Cleanup logic if needed
    };
  }, []);

  const handleDeleteWithToast = useCallback(async (threadId: string, threadTitle: string) => {
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

    // Optimistic update - add to pending deletes first
    setPendingDeletes(prev => new Set([...prev, threadId]));

    // Cache all messages from this thread
    let cachedMessages: UIMessage[] = [];
    if (repositoriesInitialized.current) {
      try {
        cachedMessages = await messageRepository.getMessagesByThreadId(threadId);
        console.log(`Cached ${cachedMessages.length} messages for potential thread restoration`);
      } catch (fetchError) {
        console.error("Error caching messages before deletion:", fetchError);
      }
    }

    // Delay actual deletion to allow for undo
    let deleteTimeoutId: NodeJS.Timeout;
    let undoClicked = false;

    // Show toast with undo option
    toast.info(`Chat deleted`, {
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

          // UNDO FUNCTIONALITY
          try {
            // Check if thread still exists or has been deleted
            const existingThreads = await refresh();
            const threadExists = existingThreads.some(t => t.id === threadId);

            if (!threadExists) {
              // Thread was deleted, need to recreate it
              console.log("Thread was already deleted, recreating with data:", threadData);

              // Initialize repositories if needed
              if (repositoriesInitialized.current) {
                try {
                  // Recreate the thread with the SAME ID to ensure message consistency
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
                  toast.success(`Chat "${threadData.title || 'Untitled'}" restored`);
                } catch (createError) {
                  console.error("Error recreating thread:", createError);

                  // Fallback: create a new thread if exact recreation fails
                  const newThread = await createThread(threadData.title);
                  onSelectThread(newThread.id);
                  toast.warning("Chat restored with a new ID (messages could not be recovered)");
                }
              } else {
                // If repositories not initialized, use fallback approach
                const newThread = await createThread(threadData.title);
                onSelectThread(newThread.id);
                toast.success("Chat restored");
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
    });

    // Delay the actual deletion to allow for undo
    deleteTimeoutId = setTimeout(() => {
      if (!undoClicked) {
        // Only delete if undo wasn't clicked
        onDeleteThread(threadId);
      }
    }, 300);
  }, [threads, refresh, createThread, updateThread, onDeleteThread, onSelectThread]);

  return {
    pendingDeletes,
    handleDeleteWithToast
  };
}
