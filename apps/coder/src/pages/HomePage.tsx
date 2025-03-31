import React, { useCallback, useEffect, useState } from "react";
// Restored persistence with our wrapper
import { usePersistentChat, Thread } from "@openagents/core";
import { testLocalApi } from "../helpers/ipc/fetch/test-api";
import { MessageInput } from "@/components/ui/message-input";
import { MessageList } from "@/components/ui/message-list";
import { Chat, ChatForm } from "@/components/ui/chat";
import { ThreadList } from "@/components/ThreadList";
import ToggleTheme from "@/components/ToggleTheme";
import { Badge } from "@/components/ui/badge";
import {
  Sidebar,
  SidebarContent,
  SidebarHeader,
  SidebarProvider,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarFooter,
  SidebarGroup,
  SidebarInset
} from "@/components/ui/sidebar";
import { MessageSquareIcon, SettingsIcon, HelpCircleIcon } from "lucide-react";

export default function HomePage() {
  const [apiStatus, setApiStatus] = useState<string | null>(null);
  const [debugInfo, setDebugInfo] = useState<string | null>(null);

  // Function to test the local API
  const handleTestLocalApi = async () => {
    setApiStatus('Testing...');
    setDebugInfo(null);
    
    // First check if window.electron exists
    if (typeof window.electron === 'undefined') {
      setApiStatus('Failed: window.electron undefined ❌');
      setDebugInfo('Preload script not loading correctly');
      return;
    }
    
    // Check if testIpc exists
    if (typeof window.electron.testIpc !== 'function') {
      setApiStatus('Failed: testIpc missing ❌');
      setDebugInfo('IPC bridge not set up correctly');
      return;
    }
    
    // Test basic IPC
    try {
      const ipcResult = await window.electron.testIpc();
      if (ipcResult.success) {
        setDebugInfo(`IPC working: ${ipcResult.message}`);
      } else {
        setDebugInfo(`IPC returned: ${JSON.stringify(ipcResult)}`);
      }
    } catch (error) {
      setApiStatus('Failed: IPC error ❌');
      setDebugInfo(`IPC error: ${error instanceof Error ? error.message : String(error)}`);
      return;
    }
    
    // Test fetch API
    if (typeof window.electron.fetch !== 'function') {
      setApiStatus('Failed: fetch missing ❌');
      setDebugInfo('electron.fetch not available');
      return;
    }
    
    const success = await testLocalApi();
    setApiStatus(success ? 'Connected ✅' : 'Failed ❌');
    
    // Don't reset status since it's helpful for debugging
  };

  // Define a custom fetch function that will use Electron IPC in Electron 
  // and fall back to standard fetch in browser environments
  const customFetch = useCallback(async (url: string | URL | Request, init?: RequestInit) => {
    console.log('Custom fetch called with:', url);
    
    // For Request objects, we need to extract the URL
    let urlStr: string;
    if (url instanceof Request) {
      urlStr = url.url;
      console.log('URL extracted from Request:', urlStr);
    } else if (url instanceof URL) {
      urlStr = url.toString();
    } else {
      urlStr = url;
    }
    
    try {
      // If it's a relative URL and we're inside electron, handle it specially
      if (typeof urlStr === 'string' && urlStr.startsWith('/') && typeof window !== 'undefined' && window.electron?.fetch) {
        console.log('Handling relative URL with electron.fetch:', urlStr);
        try {
          // For relative URLs, pass them directly to electron.fetch which knows how to handle them
          const response = await window.electron.fetch(urlStr, init);
          console.log('electron.fetch response received:', response.status);
          
          // Verify response has text method
          if (typeof response.text !== 'function') {
            console.error('Response missing text method - creating new Response object');
            
            // Read the response body if possible
            let bodyContent = '';
            try {
              if (response.body) {
                const reader = response.body.getReader();
                const decoder = new TextDecoder();
                const { value } = await reader.read();
                bodyContent = decoder.decode(value);
              } else if (typeof response.bodyUsed === 'boolean' && !response.bodyUsed) {
                bodyContent = await (response as any).text?.() || '';
              }
            } catch (e) {
              console.error('Error reading response body:', e);
              // Use any available properties from the original response
              bodyContent = response.body?.toString() || response.toString() || '';
            }
            
            // Create a new Response with a Blob body
            const bodyBlob = new Blob([bodyContent], { 
              type: response.headers?.get('content-type') || 'text/plain' 
            });
            
            return new Response(bodyBlob, {
              status: response.status,
              statusText: response.statusText,
              headers: response.headers,
            });
          }
          
          return response;
        } catch (error) {
          console.error('electron.fetch error with relative URL:', error);
          throw error;
        }
      }
      
      // Use electron.fetch if available (in Electron environment)
      if (typeof window !== 'undefined' && window.electron?.fetch) {
        console.log('Using electron.fetch for request');
        try {
          const response = await window.electron.fetch(url, init);
          
          // Verify response has text method
          if (typeof response.text !== 'function') {
            console.error('Response missing text method - creating new Response object');
            
            // Read the response body if possible
            let bodyContent = '';
            try {
              if (response.body) {
                const reader = response.body.getReader();
                const decoder = new TextDecoder();
                const { value } = await reader.read();
                bodyContent = decoder.decode(value);
              } else if (typeof response.bodyUsed === 'boolean' && !response.bodyUsed) {
                bodyContent = await (response as any).text?.() || '';
              }
            } catch (e) {
              console.error('Error reading response body:', e);
              // Use any available properties from the original response
              bodyContent = response.body?.toString() || response.toString() || '';
            }
            
            // Create a new Response with a Blob body
            const bodyBlob = new Blob([bodyContent], { 
              type: response.headers?.get('content-type') || 'text/plain' 
            });
            
            return new Response(bodyBlob, {
              status: response.status,
              statusText: response.statusText,
              headers: response.headers,
            });
          }
          
          return response;
        } catch (error) {
          console.error('electron.fetch error:', error);
          // Fall back to standard fetch on error
          console.log('Falling back to standard fetch after electron.fetch error');
          return fetch(url, init);
        }
      } else {
        // Fall back to standard fetch (in browser or if electron.fetch fails)
        console.log('Falling back to standard fetch (electron.fetch not available)');
        return fetch(url, init);
      }
    } catch (finalError) {
      console.error('Fatal fetch error:', finalError);
      // Create a synthetic response for fatal errors to avoid breaking the UI
      return new Response(JSON.stringify({
        error: finalError instanceof Error ? finalError.message : String(finalError)
      }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      });
    }
  }, []);
  
  // Use the persistence layer with the correct configuration
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit,
    isLoading: isGenerating,
    stop,
    currentThreadId,
    switchThread,
    createNewThread,
    deleteThread,
    updateThread,
  } = usePersistentChat({
    // Use local API endpoint
    api: "/api/chat",
    // Pass our custom fetch function directly
    fetch: customFetch,
    // Configuration that we know works
    streamProtocol: 'data',
    body: {
      model: "claude-3-5-sonnet-20240620"
    },
    headers: {
      'Content-Type': 'application/json',
      'Accept': 'text/event-stream',
    },
    // Enable persistence
    persistenceEnabled: true,
    maxSteps: 10,
    // Event handlers
    onResponse: (response) => { 
      console.log('Chat response received:', response);
    },
    onFinish: (message) => { 
      console.log('Chat finished with message:', message);
    },
    onThreadChange: (threadId: string) => { 
      console.log('Thread changed to:', threadId); 
    },
    // Handle errors
    onError: (error) => {
      console.error('Chat error:', error);
    }
  });


  const handleCreateThread = useCallback(() => {
    createNewThread();
  }, [createNewThread]);

  const handleSelectThread = useCallback((threadId: string) => {
    switchThread(threadId);
  }, [switchThread]);

  const handleDeleteThread = useCallback((threadId: string) => {
    deleteThread(threadId);
  }, [deleteThread]);

  const handleRenameThread = useCallback((threadId: string, title: string) => {
    updateThread(threadId, title);
  }, [updateThread]);

  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-full w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="relative flex h-full w-full flex-row overflow-hidden">
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <div className="flex items-center h-full justify-between px-3">
                  <span className="flex items-center text-sm font-semibold">
                    OpenAgents Coder
                    <Badge
                      variant="outline"
                      className="text-[10px] px-1 py-0 ml-1 mt-[1px]"
                    >
                      v0.0.1
                    </Badge>
                  </span>
                </div>
              </SidebarHeader>

              <SidebarContent>
                <SidebarGroup>
                  <ThreadList
                    currentThreadId={currentThreadId ?? ''}
                    onSelectThread={handleSelectThread}
                    onDeleteThread={handleDeleteThread}
                    onRenameThread={handleRenameThread}
                    onCreateThread={handleCreateThread}
                  />
                </SidebarGroup>
              </SidebarContent>

              <SidebarFooter>
                <div className="px-3 py-2">
                  <div className="text-xs text-muted-foreground">
                    <ToggleTheme />
                  </div>
                </div>
              </SidebarFooter>
            </Sidebar>

            <SidebarInset>
              <div className="grid grid-rows-[auto_1fr_auto] h-screen">
                <div className="border-y bg-background p-3 flex items-center justify-between z-10 h-14">
                  <div className="flex items-center gap-2 overflow-hidden">
                    <button
                      aria-label="Model selector"
                      type="button"
                      className="select-none group flex cursor-pointer items-center gap-1 rounded-lg py-1.5 px-3 text-sm hover:bg-muted overflow-hidden whitespace-nowrap"
                    >
                      <div>
                        Claude 3.5 Sonnet
                      </div>
                      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" className="icon-md">
                        <path fillRule="evenodd" clipRule="evenodd" d="M5.29289 9.29289C5.68342 8.90237 6.31658 8.90237 6.70711 9.29289L12 14.5858L17.2929 9.29289C17.6834 8.90237 18.3166 8.90237 18.7071 9.29289C19.0976 9.68342 19.0976 10.3166 18.7071 10.7071L12.7071 16.7071C12.5196 16.8946 12.2652 17 12 17C11.7348 17 11.4804 16.8946 11.2929 16.7071L5.29289 10.7071C4.90237 10.3166 4.90237 9.68342 5.29289 9.29289Z" fill="currentColor" />
                      </svg>
                    </button>
                    <div className="flex items-center ml-auto">
                      <button
                        onClick={handleTestLocalApi}
                        className="select-none flex cursor-pointer items-center gap-1 rounded-lg py-1.5 px-3 text-xs bg-primary text-primary-foreground hover:bg-primary/90"
                      >
                        Test Local API
                      </button>
                      {apiStatus && (
                        <span className="ml-2 text-xs">{apiStatus}</span>
                      )}
                      {debugInfo && (
                        <div className="ml-2 text-xs p-1 bg-muted rounded-sm">{debugInfo}</div>
                      )}
                    </div>
                  </div>
                </div>

                <div className="overflow-y-auto">
                  <div className="h-full p-4">
                    <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">

                      <MessageList
                        messages={messages}
                        isTyping={isGenerating}
                      />
                    </div>
                  </div>
                </div>

                <div className="border-t bg-background p-4">
                  <div className="mx-auto md:max-w-3xl lg:max-w-[40rem] xl:max-w-[48rem]">
                    <ChatForm
                      isPending={isGenerating}
                      handleSubmit={handleSubmit}
                      className="relative"
                    >
                      {({ files, setFiles }) => (
                        <MessageInput
                          value={input}
                          onChange={handleInputChange}
                          allowAttachments
                          files={files}
                          setFiles={setFiles}
                          stop={stop}
                          isGenerating={isGenerating}
                        />
                      )}
                    </ChatForm>
                    <div className="mt-2 text-center text-xs text-muted-foreground">
                      <div>Coder will make mistakes. Commit to git regularly.</div>
                    </div>
                  </div>
                </div>
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
}
