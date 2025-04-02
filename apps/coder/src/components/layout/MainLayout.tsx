import React, { memo } from 'react';
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarFooter,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarInset
} from '@/components/ui/sidebar';
import { Link } from '@tanstack/react-router';
import { SettingsIcon } from 'lucide-react';
import ToggleTheme from '@/components/ToggleTheme';
import { AppHeader } from '@/components/AppHeader';
import { ModelHeader } from '@/components/ModelHeader';
import { ThreadList } from '@/components/ThreadList';
import { MessageArea } from '@/components/MessageArea';
import { ChatInputArea } from '@/components/ChatInputArea';
import { useThreadContext, useMessageContext, useInputContext } from '@/providers/ChatStateProvider';
import { StreamingMessageProvider } from '@/providers/StreamingMessageProvider';
import { StableInputProvider } from '@/providers/StableInputProvider';

export const MainLayout = memo(function MainLayout() {
  // Use thread context instead of the full chat state
  // This ensures this component won't rerender during message streaming
  const { 
    currentThreadId, 
    handleSelectThread,
    handleCreateThread,
    handleDeleteThread,
    handleRenameThread,
    threadListKey
  } = useThreadContext();
  
  // Get message data for streaming provider
  const { messages, isGenerating } = useMessageContext();
  
  // Get input data for stable input provider
  const inputContext = useInputContext();
  
  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-screen w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="mt-[30px] relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <AppHeader onCreateThread={handleCreateThread} />
              </SidebarHeader>

              <SidebarContent>
                <ThreadList
                  key={`thread-list-${threadListKey}`}
                  currentThreadId={currentThreadId ?? ''}
                  onSelectThread={handleSelectThread}
                  onDeleteThread={handleDeleteThread}
                  onRenameThread={handleRenameThread}
                  onCreateThread={handleCreateThread}
                />
              </SidebarContent>

              <SidebarFooter>
                <SidebarMenu>
                  <SidebarMenuItem className="flex justify-between items-center">
                    <Link to="/settings/models">
                      <SidebarMenuButton>
                        <SettingsIcon />
                        <span>Settings</span>
                      </SidebarMenuButton>
                    </Link>
                    <ToggleTheme />
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarFooter>
            </Sidebar>

            <SidebarInset>
              <div className="grid grid-rows-[auto_minmax(0,1fr)_auto] h-[calc(100vh-30px)]">
                <div className="border-y bg-background p-3 flex items-center justify-between z-10 h-14">
                  <ModelHeader />
                </div>

                {/* Wrap just the MessageArea in the streaming provider */}
                <StreamingMessageProvider messages={messages} isGenerating={isGenerating}>
                  <MessageArea />
                </StreamingMessageProvider>
                
                {/* Now wrap the ChatInputArea with a dedicated input provider */}
                <StableInputProvider 
                  input={inputContext.input} 
                  handleInputChange={inputContext.handleInputChange}
                  handleSubmit={inputContext.handleSubmit}
                  stop={inputContext.stop}
                  isGenerating={isGenerating}>
                  <ChatInputArea />
                </StableInputProvider>
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
});