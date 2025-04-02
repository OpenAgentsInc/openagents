import React, { createContext, useContext, useMemo } from 'react';
import { type UIMessage } from '@openagents/core';

// Create a context specifically for streaming messages
type StreamingMessageContextType = {
  messages: UIMessage[];
  isGenerating: boolean;
};

const StreamingMessageContext = createContext<StreamingMessageContextType | null>(null);

export const useStreamingMessages = () => {
  const context = useContext(StreamingMessageContext);
  if (!context) throw new Error('useStreamingMessages must be used within a StreamingMessageProvider');
  return context;
};

interface StreamingMessageProviderProps {
  messages: UIMessage[];
  isGenerating: boolean;
  children: React.ReactNode;
}

export const StreamingMessageProvider: React.FC<StreamingMessageProviderProps> = ({
  messages,
  isGenerating,
  children
}) => {
  // Process messages with full timestamp correction for the streaming component
  const processedMessages = useMemo(() => {
    // First ensure messages have parts
    const messagesWithParts: UIMessage[] = messages.map(msg => ({
      ...msg,
      parts: msg.parts || [{
        type: 'text' as const,
        text: msg.content
      }]
    }));
    
    // Check for timestamp collisions
    const timestampCounts: Record<number, number> = {};
    messagesWithParts.forEach(msg => {
      const timestamp = msg.createdAt?.getTime() || 0;
      timestampCounts[timestamp] = (timestampCounts[timestamp] || 0) + 1;
    });

    const hasCollisions = Object.values(timestampCounts).some(count => count > 1);

    // If no collisions, return messages as-is
    if (!hasCollisions) {
      return messagesWithParts;
    }

    // First organize by role to keep conversation flow
    const userMessages: UIMessage[] = [];
    const assistantMessages: UIMessage[] = [];

    messagesWithParts.forEach(msg => {
      if (msg.role === 'user') userMessages.push(msg);
      else assistantMessages.push(msg);
    });

    // Pair user messages with assistant responses
    const correctedMessages = [];
    let baseTime = Date.now() - (messages.length * 10000); // Start 10 seconds ago per message

    // If we have more user messages than assistant or vice versa, we need to handle that
    const maxLength = Math.max(userMessages.length, assistantMessages.length);

    for (let i = 0; i < maxLength; i++) {
      if (i < userMessages.length) {
        const userMsg = userMessages[i];
        userMsg.createdAt = new Date(baseTime);
        correctedMessages.push(userMsg);
        baseTime += 2000; // 2 second gap
      }

      if (i < assistantMessages.length) {
        const assistantMsg = assistantMessages[i];
        assistantMsg.createdAt = new Date(baseTime);
        correctedMessages.push(assistantMsg);
        baseTime += 3000; // 3 second gap
      }
    }

    return correctedMessages;
  }, [messages]);

  // Create the context value with our processed messages
  const contextValue = useMemo(() => ({
    messages: processedMessages,
    isGenerating
  }), [processedMessages, isGenerating]);

  return (
    <StreamingMessageContext.Provider value={contextValue}>
      {children}
    </StreamingMessageContext.Provider>
  );
};