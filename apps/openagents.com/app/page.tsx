'use client';

import React, { useRef, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useMutation } from 'convex/react';
import { api } from '@/convex/_generated/api';
import { Text, GridLines, Dots, FrameCorners, cx } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager';
import { ChatMessage } from '@/components/ChatMessage';
import { ChatInput } from '@/components/ChatInput';
import { TypingIndicator } from '@/components/TypingIndicator';
import { useAuth } from '@/hooks/useAuth';
import { ButtonSimple } from '@/components/ButtonSimple';
import type { UIMessage, Message } from '@/components/ChatMessage';
import type { ChatStatus } from '@/components/ChatInput';
import Link from 'next/link';
import { Rocket, FolderOpen } from 'lucide-react';
import { Github } from 'iconoir-react';

const HomePage = (): React.ReactElement => {
  const router = useRouter();
  const { isAuthenticated, signIn } = useAuth();
  const inputRef = useRef<HTMLTextAreaElement>(null);
  
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  
  const createConversation = useMutation(api.conversations.create);
  const addMessage = useMutation(api.conversations.addMessage);

  const status: ChatStatus = isLoading ? 'streaming' : 'ready';

  const onSubmit = async () => {
    if (!input.trim()) return;
    
    const userMessage = input.trim();
    setInput('');
    setIsLoading(true);
    
    try {
      // Create a new conversation with the first message as title
      const conversationId = await createConversation({
        title: userMessage.slice(0, 100) // Use first 100 chars as title
      });
      
      // Add the user message
      await addMessage({
        conversationId,
        role: 'user',
        content: userMessage,
      });
      
      // Redirect to the conversation page
      router.push(`/chat/${conversationId}`);
      
    } catch (error) {
      console.error('Error creating conversation:', error);
      setIsLoading(false);
    }
  };

  const handleTextareaChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
  };

  return (
    <AppLayout showSidebar>
      <OnboardingOverlayManager
        minDesktopWidth={1024}
        desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
      >
        {/* Main chat container - fills available space */}
        <div className="relative z-10 h-full flex flex-col">
          {/* Background effects - only in main content area */}
          <div className="absolute inset-0 pointer-events-none">
            <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
            <Dots color="hsla(180, 50%, 50%, 0.02)" size={1} distance={30} />
          </div>
          
          {/* Floating GitHub login button */}
          {!isAuthenticated && (
            <div className="absolute top-4 right-4 z-20">
              <ButtonSimple 
                onClick={signIn}
                className="text-xs"
              >
                <Github width={14} height={14} />
                <span>Log in with GitHub</span>
              </ButtonSimple>
            </div>
          )}
          {/* Messages area */}
          <div className="flex-1 overflow-y-auto">
            <div className="h-full flex items-center justify-center px-8">
              <div className="w-full max-w-3xl text-center space-y-8">
                {/* Hero Section - ChatGPT style */}
                <div className="space-y-2">
                  <Text className="text-3xl md:text-4xl font-semibold text-cyan-100/90" as="h1">
                    What will you create today?
                  </Text>
                  <Text className="text-base text-cyan-300/60">
                    Build and deploy apps instantly with AI
                  </Text>
                </div>
                
                {/* Example prompts - ChatGPT style */}
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mt-12">
                  <button
                    onClick={() => setInput('Build a Bitcoin Lightning payment app')}
                    className={cx(
                      'text-left p-4 rounded-lg',
                      'bg-black/30 hover:bg-cyan-500/10',
                      'border border-cyan-500/20 hover:border-cyan-500/30',
                      'transition-all duration-200',
                      'cursor-pointer'
                    )}
                  >
                    <Text className="text-sm font-medium text-cyan-300">Bitcoin Lightning App</Text>
                    <Text className="text-xs text-cyan-500/60 mt-1">Create a payment app with invoices</Text>
                  </button>
                  
                  <button
                    onClick={() => setInput('Create a modern dashboard with charts')}
                    className={cx(
                      'text-left p-4 rounded-lg',
                      'bg-black/30 hover:bg-cyan-500/10',
                      'border border-cyan-500/20 hover:border-cyan-500/30',
                      'transition-all duration-200',
                      'cursor-pointer'
                    )}
                  >
                    <Text className="text-sm font-medium text-cyan-300">Analytics Dashboard</Text>
                    <Text className="text-xs text-cyan-500/60 mt-1">Build a dashboard with real-time data</Text>
                  </button>
                  
                  <button
                    onClick={() => setInput('Build a blog with markdown support')}
                    className={cx(
                      'text-left p-4 rounded-lg',
                      'bg-black/30 hover:bg-cyan-500/10',
                      'border border-cyan-500/20 hover:border-cyan-500/30',
                      'transition-all duration-200',
                      'cursor-pointer'
                    )}
                  >
                    <Text className="text-sm font-medium text-cyan-300">Blog Platform</Text>
                    <Text className="text-xs text-cyan-500/60 mt-1">Create a blog with markdown editor</Text>
                  </button>
                  
                  <button
                    onClick={() => setInput('Make an e-commerce product page')}
                    className={cx(
                      'text-left p-4 rounded-lg',
                      'bg-black/30 hover:bg-cyan-500/10',
                      'border border-cyan-500/20 hover:border-cyan-500/30',
                      'transition-all duration-200',
                      'cursor-pointer'
                    )}
                  >
                    <Text className="text-sm font-medium text-cyan-300">E-commerce Site</Text>
                    <Text className="text-xs text-cyan-500/60 mt-1">Build a product showcase page</Text>
                  </button>
                </div>
                
              </div>
            </div>
          </div>

          {/* Input area - at bottom of flex container */}
          <div className="bg-black/80">
            <div className="max-w-3xl mx-auto px-4 py-2">
              <ChatInput
                ref={inputRef}
                input={input}
                onInputChange={handleTextareaChange}
                onSubmit={onSubmit}
                status={status}
                placeholder="Message OpenAgents..."
              />
            </div>
          </div>
        </div>
      </OnboardingOverlayManager>
    </AppLayout>
  );
};

export default HomePage;