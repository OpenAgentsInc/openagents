import { useState, useEffect } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { AgentSyncHook, SyncState, SyncOptions, StartChatResponse } from '../types';

const INITIAL_STATE: SyncState = {
  isOnline: true,
  lastSyncId: 0,
  pendingChanges: 0,
};

export function useAgentSync(options: SyncOptions): AgentSyncHook {
  const [state, setState] = useState<SyncState>(INITIAL_STATE);

  // Monitor online status
  useEffect(() => {
    const handleOnline = () => setState(s => ({ ...s, isOnline: true }));
    const handleOffline = () => setState(s => ({ ...s, isOnline: false }));

    window.addEventListener('online', handleOnline);
    window.addEventListener('offline', handleOffline);

    return () => {
      window.removeEventListener('online', handleOnline);
      window.removeEventListener('offline', handleOffline);
    };
  }, []);

  const sendMessage = async (content: string, repos?: string[]): Promise<StartChatResponse> => {
    if (!content.trim()) {
      throw new Error('Message content cannot be empty');
    }

    const chatId = uuidv4();

    try {
      const response = await fetch('/api/start-repo-chat', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          id: chatId,
          message: content,
          repos: repos || [],
          scope: options.scope,
        }),
      });

      if (!response.ok) {
        throw new Error('Failed to send message');
      }

      const data = await response.json();
      return {
        id: chatId,
        initialMessage: data.initialMessage || content,
      };
    } catch (error) {
      console.error('Error sending message:', error);
      throw error;
    }
  };

  return {
    state,
    sendMessage,
  };
}