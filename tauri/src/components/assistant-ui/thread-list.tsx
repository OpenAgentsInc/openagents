import type { FC } from "react";
import { useState, useEffect, useRef } from "react";
import {
  ThreadListItemPrimitive,
  ThreadListPrimitive,
  useAssistantState,
} from "@openagentsinc/assistant-ui-runtime";
import { ArchiveIcon, PlusIcon } from "lucide-react";
import { motion, LayoutGroup } from "framer-motion";

import { Button } from "@/components/ui/button";
import { TooltipIconButton } from "@/components/assistant-ui/tooltip-icon-button";
import { Skeleton } from "@/components/ui/skeleton";

// Get thread ID from assistant-ui context
function useThreadId() {
  return useAssistantState(({ threadListItem }) => threadListItem?.id);
}

export const ThreadList: FC = () => {
  return (
    <ThreadListPrimitive.Root className="aui-root aui-thread-list-root flex flex-col items-stretch gap-1.5">
      <ThreadListNew />
      <ThreadListItems />
    </ThreadListPrimitive.Root>
  );
};

const ThreadListNew: FC = () => {
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
        onClick={handleFocusRequest}
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

  return (
    <LayoutGroup>
      <ThreadListPrimitive.Items components={{ ThreadListItem }} />
    </LayoutGroup>
  );
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

  // Update metadata when thread ID or metadata changes
  useEffect(() => {
    const updateMetadata = () => {
      const metadata = (window as any).__threadMetadata as Map<string, { source?: string; isArchiving?: boolean }> | undefined;
      if (!metadata || !threadId) return;

      const meta = metadata.get(threadId);
      const archiving = meta?.isArchiving || false;
      setIsArchiving(archiving);
    };

    updateMetadata();
    // Listen for metadata updates from runtime
    window.addEventListener('threadMetadataUpdated', updateMetadata);

    return () => {
      window.removeEventListener('threadMetadataUpdated', updateMetadata);
    };
  }, [threadId]);

  return (
    <motion.div
      layoutId={threadId || undefined}
      layout
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{
        layout: { duration: 0.3, ease: "easeInOut" },
        opacity: { duration: 0.15 }
      }}
    >
      <ThreadListItemPrimitive.Root
        className="aui-thread-list-item flex items-center gap-2 rounded-[var(--radius-lg)] transition-all hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none data-active:bg-muted"
        style={isArchiving ? { backgroundColor: 'rgba(239, 68, 68, 0.2)' } : undefined}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        <ThreadListItemPrimitive.Trigger className="aui-thread-list-item-trigger flex-grow px-3 py-2 text-start">
          <ThreadListItemTitle />
        </ThreadListItemPrimitive.Trigger>
        <ThreadListItemArchive isVisible={isHovered} />
      </ThreadListItemPrimitive.Root>
    </motion.div>
  );
};

const ThreadListItemTitle: FC = () => {
  return (
    <span className="aui-thread-list-item-title text-sm">
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
