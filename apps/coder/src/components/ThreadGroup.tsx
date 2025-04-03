import React from 'react';
import { Thread } from '@openagents/core';
import { ThreadItem } from './ThreadItem';
import { useStableThread } from '../providers/StableThreadProvider';

interface ThreadGroupProps {
  label: string;
  threads: Thread[];
}

export const ThreadGroup = React.memo(function ThreadGroup({
  label,
  threads
}: ThreadGroupProps) {
  // Get stable handlers from context instead of props
  const { currentThreadId, handleSelectThread, handleDeleteThread, deletingThreadIds } = useStableThread();

  if (threads.length === 0) {
    return null;
  }

  return (
    <div
      data-sidebar="group"
      className="relative flex w-full min-w-0 flex-col p-2 rounded-lg transition-colors duration-200"
    >
      <div data-sidebar="group-label" className="flex h-8 shrink-0 select-none items-center rounded-md text-xs font-medium outline-none ring-sidebar-ring transition-[margin,opa] duration-200 ease-snappy focus-visible:ring-2 [&>svg]:size-4 [&>svg]:shrink-0 group-data-[collapsible=icon]:-mt-8 group-data-[collapsible=icon]:opacity-0 px-1.5 text-color-heading">
        <span>{label}</span>
      </div>

      <div data-sidebar="group-content" className="w-full text-sm">
        <ul data-sidebar="menu" className="flex w-full min-w-0 flex-col gap-2">
          {threads.map((thread) => (
            <ThreadItem
              key={thread.id}
              thread={thread}
              isSelected={thread.id === currentThreadId}
              onSelect={handleSelectThread}
              onDelete={handleDeleteThread}
              isDeleting={deletingThreadIds.has(thread.id)}
            />
          ))}
        </ul>
      </div>
    </div>
  );
});
