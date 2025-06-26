'use client';

import React, { useRef, useEffect } from 'react';
import { useChat } from '@ai-sdk/react';
import { Text, GridLines, Dots } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { ChatMessage } from '@/components/ChatMessage';
import { ChatInput } from '@/components/ChatInput';
import { TypingIndicator } from '@/components/TypingIndicator';
import type { UIMessage, Message } from '@/components/ChatMessage';
import type { ChatStatus } from '@/components/ChatInput';

const HomePage = (): React.ReactElement => {
  const { messages, input, handleInputChange, handleSubmit, isLoading } = useChat();
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Convert messages to UIMessage format for compatibility
  const uiMessages: UIMessage[] = messages.map((message) => ({
    ...message,
    parts: message.parts || [{ type: 'text' as const, text: message.content }]
  }));

  const status: ChatStatus = isLoading ? 'streaming' : 'ready';

  const onSubmit = () => {
    if (input.trim()) {
      handleSubmit();
      // Refocus input after sending message
      setTimeout(() => {
        inputRef.current?.focus();
      }, 100);
    }
  };

  const handleTextareaChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    handleInputChange(e);
  };

  return (
    <AppLayout>
      {/* Background effects */}
      <div className="fixed inset-0 pointer-events-none">
        <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
        <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
      </div>

      <div className="relative z-10 flex flex-col h-full px-8">
        {/* Messages container */}
        <div className="flex-1 overflow-y-auto pt-6">
          <div className="space-y-4">
            {uiMessages.length === 0 ? (
              <div className="text-center text-cyan-500/40 py-16">
                <Text className="text-lg font-mono">Awaiting user input</Text>
              </div>
            ) : (
              <>
                {uiMessages.map((message) => (
                  <ChatMessage 
                    key={message.id} 
                    message={message}
                  />
                ))}
                {isLoading && <TypingIndicator />}
              </>
            )}
          </div>
        </div>

        {/* Input area */}
        <ChatInput
          ref={inputRef}
          input={input}
          onInputChange={handleTextareaChange}
          onSubmit={onSubmit}
          status={status}
        />
      </div>
    </AppLayout>
  );
};

export default HomePage;