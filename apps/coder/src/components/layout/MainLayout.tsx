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
import { useChatState } from '@/providers/ChatStateProvider';

export const MainLayout = memo(function MainLayout() {
  const { 
    currentThreadId, 
    handleSelectThread,
    handleCreateThread,
    handleDeleteThread,
    handleRenameThread,
    threadListKey
  } = useChatState();
  
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

                <MessageArea />
                <ChatInputArea />
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
});