/**
 * Hook for managing threads via Convex
 * Replaces Tinyvex WebSocket queries for thread data
 */

import { useQuery, useMutation } from "convex/react";
import { api } from "../../../convex/_generated/api";
import type { Id } from "../../../convex/_generated/dataModel";

export function useConvexThreads() {
  // Query all threads (non-archived by default)
  const allThreads = useQuery(api.chat.getThreads);

  // Mutations for thread management
  const createThread = useMutation(api.chat.createThreadExtended);
  const updateThread = useMutation(api.chat.updateThread);
  const archiveThread = useMutation(api.chat.archiveThread);
  const deleteThread = useMutation(api.chat.deleteThread);
  const updateThreadTitle = useMutation(api.chat.updateThreadTitle);

  return {
    // Data
    threads: allThreads,
    isLoading: allThreads === undefined,

    // Mutations
    createThread: async (args: {
      title?: string;
      projectId?: Id<"projects">;
      source?: string;
      workingDirectory?: string;
    }) => {
      const threadId = await createThread(args);
      return threadId;
    },

    updateThread: async (
      threadId: Id<"threads">,
      updates: {
        title?: string;
        projectId?: Id<"projects">;
        archived?: boolean;
        workingDirectory?: string;
      }
    ) => {
      await updateThread({ threadId, ...updates });
    },

    archiveThread: async (threadId: Id<"threads">) => {
      await archiveThread({ threadId });
    },

    deleteThread: async (threadId: Id<"threads">) => {
      await deleteThread({ threadId });
    },

    updateThreadTitle: async (threadId: Id<"threads">, title: string) => {
      await updateThreadTitle({ threadId, title });
    },
  };
}

// Hook for a specific thread
export function useConvexThread(threadId: Id<"threads"> | undefined) {
  const thread = useQuery(
    api.chat.getThread,
    threadId ? { threadId } : "skip"
  );

  return {
    thread,
    isLoading: thread === undefined,
  };
}

// Hook for threads by project
export function useConvexProjectThreads(projectId: Id<"projects"> | undefined) {
  const threads = useQuery(
    api.chat.getThreadsByProject,
    projectId ? { projectId, includeArchived: false } : "skip"
  );

  return {
    threads,
    isLoading: threads === undefined,
  };
}
