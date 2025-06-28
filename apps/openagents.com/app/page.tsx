'use client';

import React, { useRef, useEffect } from 'react';
import { useChat } from '@ai-sdk/react';
import { Text, GridLines, Dots, FrameCorners, cx } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager';
import { ChatMessage } from '@/components/ChatMessage';
import { ChatInput } from '@/components/ChatInput';
import { TypingIndicator } from '@/components/TypingIndicator';
import { useAuth } from '@/hooks/useAuth';
import type { UIMessage, Message } from '@/components/ChatMessage';
import type { ChatStatus } from '@/components/ChatInput';
import Link from 'next/link';
import { Rocket, FolderOpen } from 'lucide-react';

const HomePage = (): React.ReactElement => {
  const { messages, input, handleInputChange, handleSubmit, isLoading } = useChat();
  const { isAuthenticated, signIn } = useAuth(); // Get signIn function
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
    <AppLayout showSidebar>
      <OnboardingOverlayManager
        minDesktopWidth={1024}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        {/* Background effects */}
        <div className="fixed inset-0 pointer-events-none">
          <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
          <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
        </div>

        <div className="relative z-10 flex flex-col h-full">
          {/* Messages container */}
          <div className="flex-1 flex items-center justify-center overflow-y-auto pb-32">
            <div className="w-full max-w-3xl px-8">
              {uiMessages.length === 0 ? (
                <div className="text-center space-y-8">
                  {/* Hero Section - ChatGPT style */}
                  <div className="space-y-2">
                    <Text className="text-3xl md:text-4xl font-semibold text-cyan-100/90" as="h1">
                      What's on your mind today?
                    </Text>
                    <Text className="text-base text-cyan-300/60">
                      Build and deploy apps instantly with AI
                    </Text>
                  </div>
                  
                  {/* Example prompts - ChatGPT style */}
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mt-12">
                    <button
                      onClick={() => handleInputChange({ target: { value: 'Build a Bitcoin Lightning payment app' } } as any)}
                      className={cx(
                        'text-left p-4 rounded-lg',
                        'bg-black/30 hover:bg-cyan-500/10',
                        'border border-cyan-500/20 hover:border-cyan-500/30',
                        'transition-all duration-200'
                      )}
                    >
                      <Text className="text-sm font-medium text-cyan-300">Bitcoin Lightning App</Text>
                      <Text className="text-xs text-cyan-500/60 mt-1">Create a payment app with invoices</Text>
                    </button>
                    
                    <button
                      onClick={() => handleInputChange({ target: { value: 'Create a modern dashboard with charts' } } as any)}
                      className={cx(
                        'text-left p-4 rounded-lg',
                        'bg-black/30 hover:bg-cyan-500/10',
                        'border border-cyan-500/20 hover:border-cyan-500/30',
                        'transition-all duration-200'
                      )}
                    >
                      <Text className="text-sm font-medium text-cyan-300">Analytics Dashboard</Text>
                      <Text className="text-xs text-cyan-500/60 mt-1">Build a dashboard with real-time data</Text>
                    </button>
                    
                    <button
                      onClick={() => handleInputChange({ target: { value: 'Build a blog with markdown support' } } as any)}
                      className={cx(
                        'text-left p-4 rounded-lg',
                        'bg-black/30 hover:bg-cyan-500/10',
                        'border border-cyan-500/20 hover:border-cyan-500/30',
                        'transition-all duration-200'
                      )}
                    >
                      <Text className="text-sm font-medium text-cyan-300">Blog Platform</Text>
                      <Text className="text-xs text-cyan-500/60 mt-1">Create a blog with markdown editor</Text>
                    </button>
                    
                    <button
                      onClick={() => handleInputChange({ target: { value: 'Make an e-commerce product page' } } as any)}
                      className={cx(
                        'text-left p-4 rounded-lg',
                        'bg-black/30 hover:bg-cyan-500/10',
                        'border border-cyan-500/20 hover:border-cyan-500/30',
                        'transition-all duration-200'
                      )}
                    >
                      <Text className="text-sm font-medium text-cyan-300">E-commerce Site</Text>
                      <Text className="text-xs text-cyan-500/60 mt-1">Build a product showcase page</Text>
                    </button>
                  </div>
                  
                  {/* Sign in CTA - smaller, more subtle */}
                  {!isAuthenticated && (
                    <div className="flex justify-center mt-8">
                      <button
                        onClick={signIn}
                        className={cx(
                          'px-6 py-2.5 rounded-lg',
                          'bg-cyan-500/10 hover:bg-cyan-500/20',
                          'border border-cyan-500/30 hover:border-cyan-500/50',
                          'text-sm text-cyan-300 hover:text-cyan-200',
                          'transition-all duration-200'
                        )}
                      >
                        Sign in with GitHub for more features
                      </button>
                    </div>
                  )}
                </div>
              ) : (
                <div className="space-y-4">
                  {uiMessages.map((message) => (
                    <ChatMessage 
                      key={message.id} 
                      message={message}
                    />
                  ))}
                  {isLoading && <TypingIndicator />}
                </div>
              )}
            </div>
          </div>

          {/* Input area - fixed to bottom, ChatGPT style */}
          <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black via-black/95 to-transparent pt-8 pb-4">
            <div className="max-w-3xl mx-auto px-8">
              <div className="relative">
                <ChatInput
                  ref={inputRef}
                  input={input}
                  onInputChange={handleTextareaChange}
                  onSubmit={onSubmit}
                  status={status}
                  placeholder="Ask me to build something amazing..."
                />
                <div className="text-center mt-2">
                  <Text className="text-xs text-cyan-500/40">
                    OpenAgents can make mistakes. Check important info.
                  </Text>
                </div>
              </div>
            </div>
          </div>
        </div>
      </OnboardingOverlayManager>
    </AppLayout>
  );
};

export default HomePage;