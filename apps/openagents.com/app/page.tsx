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
import { GitHubSignInCTA } from '@/components/mvp/atoms/HeroCallToAction';

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
    <AppLayout>
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
          <div className="flex-1 overflow-y-auto px-8 pb-32">
            <div className="space-y-4">
              {uiMessages.length === 0 ? (
                <div className="text-center py-16 space-y-8">
                  {/* Hero Section */}
                  <div className="space-y-4">
                    <Text className="text-3xl md:text-4xl font-bold text-cyan-300" as="h1">
                      Chat your apps into existence
                    </Text>
                    <Text className="text-lg md:text-xl text-cyan-400/80">
                      Deploy to the edge in 60 seconds. No credit card required.
                    </Text>
                  </div>
                  
                  {/* Quick Actions - always show now */}
                  <div className="flex flex-col sm:flex-row gap-4 justify-center items-center mt-8">
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
                  
                  {/* Sign in CTA */}
                  {!isAuthenticated && (
                    <div className="flex justify-center">
                      <GitHubSignInCTA onClick={signIn} />
                    </div>
                  )}
                  
                  {/* Helper text */}
                  <div className="max-w-md mx-auto">
                    <Text className="text-sm text-cyan-500/60 text-center font-mono">
                      Or start chatting below to explore
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

          {/* Input area - fixed to bottom */}
          <div className="fixed bottom-0 left-0 right-0 bg-black/90 backdrop-blur-sm border-t border-cyan-500/20 p-4 z-50">
            <div className="max-w-4xl mx-auto">
              <ChatInput
                ref={inputRef}
                input={input}
                onInputChange={handleTextareaChange}
                onSubmit={onSubmit}
                status={status}
              />
            </div>
          </div>
        </div>
      </OnboardingOverlayManager>
    </AppLayout>
  );
};

export default HomePage;