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
  const { isAuthenticated, isLoading: authLoading, signIn } = useAuth();
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

  const handleSignIn = async () => {
    try {
      await signIn();
    } catch (error) {
      console.error('Sign-in failed:', error);
    }
  };

  const handleDemoComplete = (demo: any) => {
    console.log('Demo completed:', demo);
  };

  // Show loading state while auth is initializing
  if (authLoading) {
    return (
      <AppLayout>
        <div className="flex items-center justify-center h-full">
          <Text className="text-cyan-300 font-mono">Initializing...</Text>
        </div>
      </AppLayout>
    );
  }

  // Chat interface content (shown behind overlays when unauthenticated)
  const chatContent = (
    <>
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
                
                {/* Quick Actions - only show if authenticated */}
                {isAuthenticated && (
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
                )}
                
                <div className="max-w-md mx-auto">
                  <Text className="text-xs text-gray-500 text-center">
                    {isAuthenticated 
                      ? "Explore the project workspace with our demo projects or create your own AI-powered application"
                      : "Chat your apps into existence. Deploy to the edge in 60 seconds."
                    }
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

        {/* Input area - disable if not authenticated */}
        <div className={isAuthenticated ? '' : 'pointer-events-none opacity-50'}>
          <ChatInput
            ref={inputRef}
            input={input}
            onInputChange={handleTextareaChange}
            onSubmit={onSubmit}
            status={status}
          />
        </div>
      </div>
    </>
  );

  return (
    <AppLayout>
      <OnboardingOverlayManager
        isAuthenticated={isAuthenticated}
        minDesktopWidth={1024}
        showDemo={true}
        showSocialProof={true}
        onSignIn={handleSignIn}
        onDemoComplete={handleDemoComplete}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        {chatContent}
      </OnboardingOverlayManager>
    </AppLayout>
  );
};

export default HomePage;