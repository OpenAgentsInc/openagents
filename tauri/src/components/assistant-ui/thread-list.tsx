import type { FC } from "react";
import { useState, useEffect, useRef } from "react";
import {
  ThreadListItemPrimitive,
  ThreadListPrimitive,
  useAssistantState,
  useAssistantRuntime,
} from "@openagentsinc/assistant-ui-runtime";
import { ArchiveIcon, PlusIcon } from "lucide-react";

import { Button } from "@openagentsinc/ui";
import { Badge } from "@openagentsinc/ui";
import { TooltipIconButton } from "@/components/assistant-ui/tooltip-icon-button";
import { Skeleton } from "@openagentsinc/ui";
import { useProjectStore } from "@/lib/project-store";
import { useUiStore } from "@/lib/ui-store";

// Get thread ID from assistant-ui context
function useThreadId() {
  return useAssistantState(({ threadListItem }) => threadListItem?.id);
}

export const ThreadList: FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const runtime = useAssistantRuntime();

  // Get threads from threadItems (not threads!)
  const threads = useAssistantState((s) => {
    console.log('[ThreadList] State threads object:', s.threads);
    return s.threads?.threadItems;
  });
  const currentThreadId = useAssistantState((s) => s.threads?.mainThreadId);

  // Use refs to always have current values in event handler
  const threadsRef = useRef(threads);
  const currentThreadIdRef = useRef(currentThreadId);

  useEffect(() => {
    threadsRef.current = threads;
    currentThreadIdRef.current = currentThreadId;
  }, [threads, currentThreadId]);

  console.log('[ThreadList] Render:', {
    threads: threads,
    threadsCount: threads?.length,
    threadsIsArray: Array.isArray(threads),
    threadIds: threads?.map(t => t.id),
    currentThreadId,
    hasSwitchToThread: !!runtime.switchToThread,
    runtimeKeys: Object.keys(runtime),
    threadsRefCurrent: threadsRef.current,
    threadsRefCount: threadsRef.current?.length,
  });

  useEffect(() => {
    const container = containerRef.current;
    console.log('[ThreadList] useEffect setup:', {
      hasContainer: !!container,
      threadsCount: threads?.length,
    });

    if (!container) {
      console.log('[ThreadList] No container ref, skipping keyboard listener');
      return;
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      console.log('[ThreadList] Key pressed:', {
        key: e.key,
        metaKey: e.metaKey,
        ctrlKey: e.ctrlKey,
        activeElement: document.activeElement?.tagName,
        activeElementClass: document.activeElement?.className,
        containerContainsActive: container.contains(document.activeElement),
      });

      if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;

      // Handle Cmd+Arrow (Mac) or Ctrl+Arrow (Windows/Linux) globally
      const hasModifier = e.metaKey || e.ctrlKey;

      if (!hasModifier) {
        // Without modifier, only work when focused in sidebar
        if (!container.contains(document.activeElement)) {
          console.log('[ThreadList] Not focused on sidebar and no modifier, ignoring');
          return;
        }
      }

      e.preventDefault();
      console.log('[ThreadList] Arrow key intercepted', { hasModifier });

      const currentThreads = threadsRef.current;
      const currentId = currentThreadIdRef.current;

      if (!currentThreads || currentThreads.length === 0) {
        console.log('[ThreadList] No threads available', { currentThreads });
        return;
      }

      const currentIndex = currentThreads.findIndex((t) => t.id === currentId);
      let nextIndex: number;

      if (e.key === "ArrowDown") {
        nextIndex = currentIndex + 1 < currentThreads.length ? currentIndex + 1 : 0;
      } else {
        nextIndex = currentIndex - 1 >= 0 ? currentIndex - 1 : currentThreads.length - 1;
      }

      const nextThread = currentThreads[nextIndex];
      console.log('[ThreadList] Switching thread:', {
        currentIndex,
        nextIndex,
        nextThreadId: nextThread?.id,
        hasSwitchMethod: !!runtime.switchToThread,
      });

      if (nextThread && runtime.switchToThread) {
        console.log('[ThreadList] Calling switchToThread:', nextThread.id);
        runtime.switchToThread(nextThread.id);
      } else {
        console.log('[ThreadList] Cannot switch - missing thread or method');
      }
    };

    console.log('[ThreadList] Adding window keydown listener');
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      console.log('[ThreadList] Removing window keydown listener');
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [runtime]);

  return (
    <ThreadListPrimitive.Root
      ref={containerRef}
      tabIndex={0}
      className="aui-root aui-thread-list-root flex flex-col items-stretch gap-1.5 focus:outline-none"
      onFocus={() => console.log('[ThreadList] Container focused')}
      onBlur={() => console.log('[ThreadList] Container blurred')}
    >
      <ThreadListNew />
      <ThreadListItems />
    </ThreadListPrimitive.Root>
  );
};

const ThreadListNew: FC = () => {
  const clearActiveProject = useProjectStore((s) => s.setActiveProject);
  const clearProjectView = useUiStore((s) => s.clearProjectView);
  const handleFocusRequest = () => {
    // Nudge focus to the composer after the runtime switches threads
    setTimeout(() => {
      window.dispatchEvent(new Event("openagents:focus-composer"));
    }, 80);
  };

  return (
    <ThreadListPrimitive.New asChild>
      <Button
        className="aui-thread-list-new flex items-center justify-start gap-1 rounded-[var(--radius-lg)] px-2.5 py-2 text-start hover:bg-muted data-active:bg-muted"
        variant="ghost"
        onClick={(e) => {
          // Ensure generic "New Thread" is not scoped to a project
          clearActiveProject(null);
          clearProjectView();
          handleFocusRequest();
        }}
      >
        <PlusIcon />
        New Thread
      </Button>
    </ThreadListPrimitive.New>
  );
};

const ThreadListItems: FC = () => {
  const isLoading = useAssistantState(({ threads }) => threads.isLoading);

  if (isLoading) {
    return <ThreadListSkeleton />;
  }

  return <ThreadListPrimitive.Items components={{ ThreadListItem }} />;
};

const ThreadListSkeleton: FC = () => {
  return (
    <>
      {Array.from({ length: 5 }, (_, i) => (
        <div
          key={i}
          role="status"
          aria-label="Loading threads"
          aria-live="polite"
          className="aui-thread-list-skeleton-wrapper flex items-center gap-2 rounded-[var(--radius-md)] px-3 py-2"
        >
          <Skeleton className="aui-thread-list-skeleton h-[22px] flex-grow" />
        </div>
      ))}
    </>
  );
};

const ThreadListItem: FC = () => {
  const [isHovered, setIsHovered] = useState(false);
  const threadId = useThreadId(); // Get thread ID from context
  const [isArchiving, setIsArchiving] = useState(false);
  const [threadSource, setThreadSource] = useState<string | null>(null);
  const [threadProjectId, setThreadProjectId] = useState<string | null>(null);
  const getProject = useProjectStore((s) => s.getProject);

  // Update metadata when thread ID or metadata changes
  useEffect(() => {
    const updateMetadata = () => {
      const metadata = (window as any).__threadMetadata as Map<string, { source?: string; isArchiving?: boolean; projectId?: string }> | undefined;
      if (!metadata || !threadId) return;

      const meta = metadata.get(threadId);
      const archiving = meta?.isArchiving || false;
      setIsArchiving(archiving);
      setThreadSource(meta?.source || null);
      setThreadProjectId(meta?.projectId || null);
    };

    updateMetadata();
    // Listen for metadata updates from runtime
    window.addEventListener('threadMetadataUpdated', updateMetadata);

    return () => {
      window.removeEventListener('threadMetadataUpdated', updateMetadata);
    };
  }, [threadId]);

  const threadProject = threadProjectId ? getProject(threadProjectId) : undefined;

  // Hide project-scoped threads from the main Chats list
  if (threadProjectId) {
    return null;
  }

  return (
    <ThreadListItemPrimitive.Root
      className="aui-thread-list-item flex items-center gap-2 rounded-[var(--radius-lg)] transition-all hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none data-active:bg-muted"
      style={isArchiving ? { backgroundColor: 'rgba(239, 68, 68, 0.2)' } : undefined}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <ThreadListItemPrimitive.Trigger className="aui-thread-list-item-trigger flex-grow px-3 py-2 text-start">
        <ThreadListItemTitle />
      </ThreadListItemPrimitive.Trigger>
      {threadProject && (
        <Badge variant="outline" className="text-xs mr-1 flex-shrink-0">
          {threadProject.name}
        </Badge>
      )}
      {threadSource && (
        <span className="text-xs text-muted-foreground/50 mr-2 flex-shrink-0">
          {threadSource === "claude-code" ? "claude" : threadSource === "codex" ? "codex" : threadSource}
        </span>
      )}
      <ThreadListItemArchive isVisible={isHovered} />
    </ThreadListItemPrimitive.Root>
  );
};

const ThreadListItemTitle: FC = () => {
  return (
    <span className="aui-thread-list-item-title text-sm text-foreground">
      <ThreadListItemPrimitive.Title fallback="New Chat" />
    </span>
  );
};

const ThreadListItemArchive: FC<{ isVisible: boolean }> = ({ isVisible }) => {
  return (
    <ThreadListItemPrimitive.Archive asChild>
      <TooltipIconButton
        className={`aui-thread-list-item-archive mr-3 ml-auto size-4 p-0 text-foreground hover:text-primary transition-opacity ${isVisible ? "opacity-100" : "opacity-0"}`}
        variant="ghost"
        tooltip="Archive thread"
      >
        <ArchiveIcon />
      </TooltipIconButton>
    </ThreadListItemPrimitive.Archive>
  );
};
