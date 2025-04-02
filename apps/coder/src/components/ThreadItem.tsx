import React, { useCallback, useMemo, useRef } from 'react';
import { Thread } from '@openagents/core';
import { X } from 'lucide-react';

interface ThreadItemProps {
  thread: Thread;
  isSelected: boolean;
  onSelect: (threadId: string) => void;
  onDelete: (e: React.MouseEvent, threadId: string, threadTitle: string) => void;
  isDeleting?: boolean; // New prop to track deletion state
}

export const ThreadItem = React.memo(function ThreadItem({
  thread,
  isSelected,
  onSelect,
  onDelete,
  isDeleting = false // Default to false
}: ThreadItemProps) {
  // Create stable reference to thread ID and title to prevent rerenders
  const threadIdRef = useRef(thread.id);
  const threadTitleRef = useRef(thread.title);
  
  // Update refs if thread data changes
  if (threadIdRef.current !== thread.id) {
    threadIdRef.current = thread.id;
  }
  if (threadTitleRef.current !== thread.title) {
    threadTitleRef.current = thread.title;
  }
  
  // Create stable callback for thread selection
  const handleSelectThread = useCallback(() => {
    onSelect(threadIdRef.current);
    // Dispatch focus event when thread is selected
    window.dispatchEvent(new Event('focus-chat-input'));
  }, [onSelect]);
  
  // Create stable callback for input click
  const handleInputClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    onSelect(threadIdRef.current);
    // Dispatch focus event when thread is selected via the input
    window.dispatchEvent(new Event('focus-chat-input'));
  }, [onSelect]);
  
  // Create stable callback for delete button
  const handleDeleteClick = useCallback((e: React.MouseEvent) => {
    onDelete(e, threadIdRef.current, threadTitleRef.current || 'Untitled');
  }, [onDelete]);
  
  // Memoize class strings to prevent recalculation
  const threadItemClasses = useMemo(() => `group/link relative flex h-9 w-full items-center overflow-hidden rounded-lg px-2 py-1 text-sm outline-none 
    hover:bg-sidebar-accent hover:text-sidebar-accent-foreground 
    focus-visible:text-sidebar-accent-foreground focus-visible:ring-2 
    focus-visible:ring-sidebar-ring hover:focus-visible:bg-sidebar-accent 
    ${isSelected ? 'bg-sidebar-accent text-sidebar-accent-foreground before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md' : ''}
    ${isDeleting ? 'animate-delete-flash bg-red-500/20 dark:bg-red-800/30' : ''}`, [isSelected, isDeleting]);
  
  const inputClasses = useMemo(() => `hover:truncate-none h-full w-full rounded bg-transparent px-1 py-1 text-sm outline-none pointer-events-none cursor-pointer overflow-hidden truncate 
    ${isDeleting ? 'text-destructive font-semibold' : 'text-muted-foreground'}`, [isDeleting]);
  
  // Memoize display title
  const displayTitle = useMemo(() => thread.title || 'Untitled', [thread.title]);
  
  return (
    <span data-state="closed">
      <li data-sidebar="menu-item" className="group/menu-item relative">
        <a
          className={threadItemClasses}
          title={displayTitle}
          onClick={handleSelectThread}
        >
          <div className="relative flex w-full items-center">
            <div className="relative w-full">
              <input
                aria-label="Thread title"
                aria-describedby="thread-title-hint"
                aria-readonly="true"
                readOnly
                tabIndex={-1}
                className={inputClasses}
                title={displayTitle}
                type="text"
                value={displayTitle}
                onClick={handleInputClick}
              />
            </div>
            <div className="pointer-events-auto flex items-center justify-end text-muted-foreground opacity-0 group-hover/link:opacity-100 transition-opacity">
              <button
                className="rounded-md p-1.5 hover:bg-destructive/50 hover:text-destructive-foreground"
                tabIndex={-1}
                onClick={handleDeleteClick}
                aria-label="Delete thread"
                disabled={isDeleting}
              >
                <X className="size-4" />
              </button>
            </div>
          </div>
        </a>
      </li>
    </span>
  );
});