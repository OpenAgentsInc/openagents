import React from 'react';
import { Thread } from '@openagents/core';
import { ThreadItem } from './ThreadItem';

interface ThreadGroupProps {
  label: string;
  threads: Thread[];
  selectedThreadId: string;
  onSelectThread: (threadId: string) => void;
  onDeleteThread: (e: React.MouseEvent, threadId: string, threadTitle: string) => void;
  deletingThreadIds?: Set<string>; // New prop to track threads being deleted
}

export const ThreadGroup = React.memo(function ThreadGroup({
  label,
  threads,
  selectedThreadId,
  onSelectThread,
  onDeleteThread,
  deletingThreadIds = new Set()
}: ThreadGroupProps) {
  if (threads.length === 0) {
    return null;
  }
  
  return (
    <div data-sidebar="group" className="relative flex w-full min-w-0 flex-col p-2">
      <div data-sidebar="group-label" className="flex h-8 shrink-0 select-none items-center rounded-md text-xs font-medium outline-none ring-sidebar-ring transition-[margin,opa] duration-200 ease-snappy focus-visible:ring-2 [&>svg]:size-4 [&>svg]:shrink-0 group-data-[collapsible=icon]:-mt-8 group-data-[collapsible=icon]:opacity-0 px-1.5 text-color-heading">
        <span>{label}</span>
      </div>
      
      <div data-sidebar="group-content" className="w-full text-sm">
        <ul data-sidebar="menu" className="flex w-full min-w-0 flex-col gap-1">
          {threads.map((thread) => (
            <ThreadItem
              key={thread.id}
              thread={thread}
              isSelected={thread.id === selectedThreadId}
              onSelect={onSelectThread}
              onDelete={onDeleteThread}
              isDeleting={deletingThreadIds.has(thread.id)}
            />
          ))}
        </ul>
      </div>
    </div>
  );
});