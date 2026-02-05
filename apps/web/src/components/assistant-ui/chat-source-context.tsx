'use client';

import React, { createContext, useCallback, useContext, useState } from 'react';

const CHAT_SOURCE_HEADER = 'X-Chat-Source';

export type ChatSource = 'local-fallback' | null;

type ChatSourceContextValue = {
  lastSource: ChatSource;
  setLastSource: (source: ChatSource) => void;
  /** Custom fetch that reads X-Chat-Source and updates lastSource. */
  createFetchWithSourceTracking: () => (url: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
};

const ChatSourceContext = createContext<ChatSourceContextValue | null>(null);

export function ChatSourceProvider({ children }: { children: React.ReactNode }) {
  const [lastSource, setLastSource] = useState<ChatSource>(null);

  const createFetchWithSourceTracking = useCallback(() => {
    return async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const res = await fetch(url, init);
      const source = res.headers.get(CHAT_SOURCE_HEADER);
      if (source === 'local-fallback') {
        setLastSource(source);
      }
      return res;
    };
  }, []);

  const value: ChatSourceContextValue = {
    lastSource,
    setLastSource,
    createFetchWithSourceTracking,
  };

  return (
    <ChatSourceContext.Provider value={value}>
      {children}
    </ChatSourceContext.Provider>
  );
}

export function useChatSource(): ChatSourceContextValue {
  const ctx = useContext(ChatSourceContext);
  if (!ctx) {
    return {
      lastSource: null,
      setLastSource: () => {},
      createFetchWithSourceTracking: () => fetch,
    };
  }
  return ctx;
}
