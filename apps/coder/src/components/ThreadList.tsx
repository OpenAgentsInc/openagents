import React from 'react';
import { useThreads, Thread } from '@openagents/core';
import { Button } from './ui/button';
import { Trash2, Edit, Plus } from 'lucide-react';
import { format } from 'date-fns';

interface ThreadListProps {
  currentThreadId: string;
  onSelectThread: (threadId: string) => void;
  onCreateThread: () => void;
  onDeleteThread: (threadId: string) => void;
  onRenameThread?: (threadId: string, title: string) => void;
}

export function ThreadList({ 
  currentThreadId, 
  onSelectThread,
  onCreateThread,
  onDeleteThread,
  onRenameThread 
}: ThreadListProps) {
  const { threads, isLoading, error } = useThreads({ refreshInterval: 5000 });

  if (isLoading && threads.length === 0) {
    return (
      <div className="p-4">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-lg font-medium">Chats</h2>
          <Button 
            size="sm" 
            variant="outline"
            onClick={onCreateThread}
            className="flex gap-1 items-center"
          >
            <Plus size={16} />
            <span>New</span>
          </Button>
        </div>
        <div className="py-4 text-center text-muted-foreground">
          Loading...
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-4">
        <div className="flex justify-between items-center mb-4">
          <h2 className="text-lg font-medium">Chats</h2>
          <Button 
            size="sm" 
            variant="outline"
            onClick={onCreateThread}
            className="flex gap-1 items-center"
          >
            <Plus size={16} />
            <span>New</span>
          </Button>
        </div>
        <div className="py-4 text-center text-red-500">
          Error loading chats. Please try again.
        </div>
      </div>
    );
  }

  const formatDate = (timestamp: number) => {
    try {
      return format(new Date(timestamp), 'MMM d, yyyy');
    } catch (e) {
      return 'Unknown date';
    }
  };

  return (
    <div className="p-4">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-lg font-medium">Chats</h2>
        <Button 
          size="sm" 
          variant="outline"
          onClick={onCreateThread}
          className="flex gap-1 items-center"
        >
          <Plus size={16} />
          <span>New</span>
        </Button>
      </div>

      {threads.length === 0 ? (
        <div className="py-4 text-center text-muted-foreground">
          No chats yet. Start a new conversation.
        </div>
      ) : (
        <ul className="space-y-2">
          {threads.map((thread: Thread) => (
            <li 
              key={thread.id}
              className={`
                p-2 rounded-md cursor-pointer
                hover:bg-muted/80 transition-colors
                flex justify-between items-center group
                ${thread.id === currentThreadId ? 'bg-muted' : ''}
              `}
              onClick={() => onSelectThread(thread.id)}
            >
              <div className="flex-1 truncate">
                <div className="font-medium truncate">{thread.title || 'Untitled'}</div>
                <div className="text-xs text-muted-foreground">
                  {formatDate(thread.updatedAt)}
                </div>
              </div>
              
              <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                {onRenameThread && (
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-8 w-8"
                    onClick={(e) => {
                      e.stopPropagation();
                      const newTitle = window.prompt('Enter new title:', thread.title);
                      if (newTitle) {
                        onRenameThread(thread.id, newTitle);
                      }
                    }}
                  >
                    <Edit size={16} />
                  </Button>
                )}
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 text-red-500 hover:text-red-600 hover:bg-red-100"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (window.confirm('Are you sure you want to delete this chat?')) {
                      onDeleteThread(thread.id);
                    }
                  }}
                >
                  <Trash2 size={16} />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}