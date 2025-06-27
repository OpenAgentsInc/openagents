'use client';

import React, { useRef, useEffect } from 'react';
import { useChat } from '@ai-sdk/react';
import { Text, GridLines, Dots, FrameCorners, cx } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { ChatMessage } from '@/components/ChatMessage';
import { ChatInput } from '@/components/ChatInput';
import { TypingIndicator } from '@/components/TypingIndicator';
import type { UIMessage, Message } from '@/components/ChatMessage';
import type { ChatStatus } from '@/components/ChatInput';
import Link from 'next/link';
import { Rocket, FolderOpen } from 'lucide-react';

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
              <div className="text-center py-16 space-y-8">
                <div>
                  <Text className="text-lg font-mono text-cyan-500/40">Awaiting user input</Text>
                </div>
                
                {/* Quick Actions */}
                <div className="flex flex-col sm:flex-row gap-4 justify-center items-center">
                  <Link
                    href="/projects"
                    className={cx(
                      'flex items-center gap-3 px-6 py-3',
                      'bg-cyan-500/10 hover:bg-cyan-500/20',
                      'border border-cyan-500/30 hover:border-cyan-500/50',
                      'text-cyan-300 hover:text-cyan-200',
                      'transition-all duration-200',
                      'group'
                    )}
                  >
                    <FolderOpen size={20} className="group-hover:scale-110 transition-transform" />
                    <span className="font-mono uppercase tracking-wider" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>View Projects</span>
                  </Link>
                  
                  <Link
                    href="/projects/bitcoin-puns-website"
                    className={cx(
                      'flex items-center gap-3 px-6 py-3',
                      'bg-purple-500/10 hover:bg-purple-500/20',
                      'border border-purple-500/30 hover:border-purple-500/50',
                      'text-purple-300 hover:text-purple-200',
                      'transition-all duration-200',
                      'group'
                    )}
                  >
                    <Rocket size={20} className="group-hover:scale-110 transition-transform" />
                    <span className="font-mono uppercase tracking-wider" style={{ fontFamily: 'var(--font-berkeley-mono), monospace' }}>Try Demo Project</span>
                  </Link>
                </div>
                
                <div className="max-w-md mx-auto">
                  <Text className="text-xs text-gray-500 text-center">
                    Explore the project workspace with our demo projects or create your own AI-powered application
                  </Text>
                </div>
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