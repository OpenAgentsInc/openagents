'use client';

import React, { useRef, useEffect, useState } from 'react';
import { useParams, useRouter } from 'next/navigation';
import { useQuery, useMutation } from 'convex/react';
import { api } from '@/convex/_generated/api';
import { Id } from '@/convex/_generated/dataModel';
import { Text, GridLines, Dots } from '@arwes/react';
import { AppLayout } from '@/components/AppLayout';
import { OnboardingOverlayManager } from '@/components/onboarding/OnboardingOverlayManager';
import { ChatMessage } from '@/components/ChatMessage';
import { ChatInput } from '@/components/ChatInput';
import { TypingIndicator } from '@/components/TypingIndicator';
import { useAuth } from '@/hooks/useAuth';
import type { UIMessage } from '@/components/ChatMessage';
import type { ChatStatus } from '@/components/ChatInput';

const ChatPage = (): React.ReactElement => {
  const params = useParams();
  const router = useRouter();
  const conversationId = params.id as Id<"conversations">;
  const { isAuthenticated } = useAuth();
  const inputRef = useRef<HTMLTextAreaElement>(null);
  
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  
  // Fetch conversation data
  const conversationData = useQuery(api.conversations.get, { id: conversationId });
  const addMessage = useMutation(api.conversations.addMessage);
  
  // Redirect if not authenticated
  useEffect(() => {
    if (!isAuthenticated) {
      router.push('/');
    }
  }, [isAuthenticated, router]);
  
  // Convert messages to UIMessage format
  const uiMessages: UIMessage[] = conversationData?.messages.map((message) => ({
    id: message._id,
    role: message.role,
    content: message.content,
    parts: [{ type: 'text' as const, text: message.content }]
  })) || [];
  
  const status: ChatStatus = isLoading ? 'streaming' : 'ready';
  
  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
  };
  
  const onSubmit = async () => {
    if (!input.trim()) return;
    
    const userMessage = input.trim();
    setInput('');
    setIsLoading(true);
    
    try {
      // Add user message to conversation
      await addMessage({
        conversationId,
        role: 'user',
        content: userMessage,
      });
      
      // Send to chat API for AI response
      const response = await fetch('/api/chat', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          messages: [...uiMessages, { role: 'user', content: userMessage }].map(m => ({
            role: m.role,
            content: m.content
          })),
        }),
      });
      
      if (!response.ok) throw new Error('Chat request failed');
      
      const reader = response.body?.getReader();
      const decoder = new TextDecoder();
      let assistantMessage = '';
      
      if (reader) {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          
          const chunk = decoder.decode(value);
          const lines = chunk.split('\n');
          
          for (const line of lines) {
            if (line.startsWith('data: ')) {
              const data = line.slice(6);
              if (data === '[DONE]') continue;
              
              try {
                const parsed = JSON.parse(data);
                if (parsed.choices?.[0]?.delta?.content) {
                  assistantMessage += parsed.choices[0].delta.content;
                }
              } catch (e) {
                // Ignore parse errors
              }
            }
          }
        }
      }
      
      // Add assistant message to conversation
      if (assistantMessage) {
        await addMessage({
          conversationId,
          role: 'assistant',
          content: assistantMessage,
        });
      }
      
    } catch (error) {
      console.error('Error sending message:', error);
    } finally {
      setIsLoading(false);
      // Refocus input after sending message
      setTimeout(() => {
        inputRef.current?.focus();
      }, 100);
    }
  };
  
  if (!conversationData) {
    return (
      <AppLayout showSidebar>
        <OnboardingOverlayManager
          minDesktopWidth={1024}
          desktopMessage="OpenAgents requires a desktop browser for the full development experience. Please use a device with a screen width of at least 1024px."
        >
          <div className="h-full flex items-center justify-center">
            <Text className="text-cyan-300/60">Loading conversation...</Text>
          </div>
        </OnboardingOverlayManager>
      </AppLayout>
    );
  }
  
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
          
          {/* Chat header */}
          <div className="border-b border-cyan-500/20 px-8 py-4">
            <Text className="text-lg font-semibold text-cyan-100/90">
              {conversationData.conversation.title}
            </Text>
          </div>
          
          {/* Messages area */}
          <div className="flex-1 overflow-y-auto">
            <div className="max-w-3xl mx-auto px-8 py-8">
              <div className="space-y-4">
                {uiMessages.map((message) => (
                  <ChatMessage 
                    key={message.id} 
                    message={message}
                  />
                ))}
                {isLoading && <TypingIndicator />}
              </div>
            </div>
          </div>

          {/* Input area - at bottom of flex container */}
          <div className="bg-black/80">
            <div className="max-w-3xl mx-auto px-4 py-2">
              <ChatInput
                ref={inputRef}
                input={input}
                onInputChange={handleInputChange}
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

export default ChatPage;