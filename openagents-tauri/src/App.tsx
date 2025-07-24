import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Frame } from "@/components/ui/frame";

interface Message {
  id: string;
  message_type: string;
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface Session {
  id: string;
  projectPath: string;
  messages: Message[];
  inputMessage: string;
  isLoading: boolean;
}

function App() {
  const [claudeStatus, setClaudeStatus] = useState<string>("Not initialized");
  const [sessions, setSessions] = useState<Session[]>([]);
  const [newProjectPath, setNewProjectPath] = useState("/Users/christopherdavid/Desktop/openagents");
  const [isDiscoveryLoading, setIsDiscoveryLoading] = useState(false);

  // Initialize Claude on mount
  useEffect(() => {
    console.log("App mounted, starting Claude discovery...");
    discoverClaude();
  }, []);

  // Poll for messages for all active sessions
  useEffect(() => {
    if (sessions.length === 0) return;

    const fetchAllMessages = async () => {
      await Promise.all(sessions.map(session => fetchMessages(session.id)));
    };

    // Initial fetch
    fetchAllMessages();

    const interval = setInterval(fetchAllMessages, 50); // Poll every 50ms for real-time updates

    return () => clearInterval(interval);
  }, [sessions.length]); // Only depend on length to avoid recreating interval

  const discoverClaude = async () => {
    console.log("Starting Claude discovery...");
    setIsDiscoveryLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("discover_claude");
      console.log("Discovery result:", result);
      if (result.success && result.data) {
        setClaudeStatus(`Claude found at: ${result.data}`);
        console.log("Claude binary found at:", result.data);
      } else {
        setClaudeStatus(`Error: ${result.error || "Unknown error"}`);
        console.error("Discovery failed:", result.error);
      }
    } catch (error) {
      setClaudeStatus(`Error: ${error}`);
      console.error("Discovery error:", error);
    }
    setIsDiscoveryLoading(false);
  };

  const createSession = async () => {
    if (!newProjectPath) {
      alert("Please enter a project path");
      return;
    }

    console.log("Creating session for project:", newProjectPath);
    setIsDiscoveryLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath: newProjectPath,
      });
      console.log("Create session result:", result);
      if (result.success && result.data) {
        const newSession: Session = {
          id: result.data,
          projectPath: newProjectPath,
          messages: [],
          inputMessage: "",
          isLoading: false,
        };
        setSessions(prev => [...prev, newSession]);
        console.log("Session created with ID:", result.data);
      } else {
        alert(`Error creating session: ${result.error}`);
        console.error("Session creation failed:", result.error);
      }
    } catch (error) {
      alert(`Error: ${error}`);
      console.error("Session creation error:", error);
    }
    setIsDiscoveryLoading(false);
  };

  const sendMessage = async (sessionId: string) => {
    const session = sessions.find(s => s.id === sessionId);
    if (!session || !session.inputMessage.trim()) {
      console.log("Cannot send message - session:", session, "message:", session?.inputMessage);
      return;
    }

    console.log("Sending message:", session.inputMessage, "to session:", sessionId);
    const messageToSend = session.inputMessage;
    
    // Immediately add user message to UI
    const userMessage: Message = {
      id: `user-${Date.now()}`,
      message_type: "user",
      content: messageToSend,
      timestamp: new Date().toISOString(),
    };
    
    // Clear input, add message, and set loading state
    setSessions(prev => prev.map(s => 
      s.id === sessionId 
        ? { ...s, messages: [...s.messages, userMessage], inputMessage: "", isLoading: true }
        : s
    ));
    
    try {
      const result = await invoke<CommandResult<void>>("send_message", {
        sessionId,
        message: messageToSend,
      });
      console.log("Send message result:", result);
      if (!result.success) {
        alert(`Error sending message: ${result.error}`);
        console.error("Send message failed:", result.error);
      }
    } catch (error) {
      alert(`Error: ${error}`);
      console.error("Send message error:", error);
    } finally {
      setSessions(prev => prev.map(s => 
        s.id === sessionId 
          ? { ...s, isLoading: false }
          : s
      ));
    }
  };

  const fetchMessages = async (sessionId: string) => {
    try {
      const result = await invoke<CommandResult<Message[]>>("get_messages", {
        sessionId,
      });
      if (result.success && result.data) {
        setSessions(prev => prev.map(session => {
          if (session.id !== sessionId) return session;
          
          const backendMessages = result.data || [];
          const currentMessages = session.messages;
          
          // Keep optimistic user messages that haven't appeared in backend yet
          const optimisticMessages = currentMessages.filter(msg => 
            msg.id.startsWith('user-') && 
            !backendMessages.some(backend => 
              backend.message_type === 'user' && 
              backend.content === msg.content
            )
          );
          
          // Combine optimistic messages with backend messages
          const allMessages = [...optimisticMessages, ...backendMessages];
          
          // Sort by timestamp to maintain order
          allMessages.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
          
          return { ...session, messages: allMessages };
        }));
      }
    } catch (error) {
      console.error("Error fetching messages:", error);
    }
  };

  const stopSession = async (sessionId: string) => {
    setSessions(prev => prev.map(s => 
      s.id === sessionId ? { ...s, isLoading: true } : s
    ));
    
    try {
      const result = await invoke<CommandResult<void>>("stop_session", {
        sessionId,
      });
      if (result.success) {
        setSessions(prev => prev.filter(s => s.id !== sessionId));
      } else {
        alert(`Error stopping session: ${result.error}`);
        setSessions(prev => prev.map(s => 
          s.id === sessionId ? { ...s, isLoading: false } : s
        ));
      }
    } catch (error) {
      alert(`Error: ${error}`);
      setSessions(prev => prev.map(s => 
        s.id === sessionId ? { ...s, isLoading: false } : s
      ));
    }
  };

  const renderMessage = (msg: Message) => {
    const messageTypeStyles: Record<string, string> = {
      user: "bg-primary/10 border-primary/20",
      assistant: "bg-secondary/10 border-secondary/20",
      tool_use: "bg-accent/10 border-accent/20",
      error: "bg-destructive/10 border-destructive/20 text-destructive",
      summary: "bg-muted/50 border-muted",
      thinking: "bg-muted/30 border-muted italic",
      system: "bg-muted/20 border-muted",
    };

    const style = messageTypeStyles[msg.message_type] || "bg-muted/10 border-muted";

    return (
      <div key={msg.id} className={`border p-4 overflow-hidden ${style}`}>
        <div className="text-xs font-semibold uppercase opacity-70 mb-1">
          {msg.message_type}
        </div>
        <div className="text-sm overflow-hidden">
          <div className="whitespace-pre-wrap break-all font-mono overflow-x-auto overflow-y-hidden">
            {msg.content}
          </div>
          {msg.tool_info && msg.tool_info.output && (
            <details className="mt-2">
              <summary className="cursor-pointer text-xs font-medium hover:underline">
                Tool Output
              </summary>
              <div className="mt-2 text-xs opacity-80 whitespace-pre-wrap break-all font-mono overflow-x-auto overflow-y-hidden">
                {msg.tool_info.output}
              </div>
            </details>
          )}
        </div>
      </div>
    );
  };

  // Set dark mode on mount
  useEffect(() => {
    document.documentElement.classList.add("dark");
  }, []);

  const updateSessionInput = (sessionId: string, value: string) => {
    setSessions(prev => prev.map(s => 
      s.id === sessionId ? { ...s, inputMessage: value } : s
    ));
  };

  return (
    <div className="fixed inset-0 p-8 font-mono overflow-hidden">
      <div className="h-full flex gap-6 overflow-hidden">
        {/* Metadata Panel */}
        <div className="w-80 flex flex-col gap-4 min-h-0">
          {/* Header */}
          <div className="text-center select-none p-4">
            <h1 className="text-xl font-bold mb-1">OpenAgents</h1>
            <p className="text-muted-foreground text-xs">Claude Code Commander</p>
          </div>

          {/* Status & Sessions */}
          <Frame className="p-4 flex-1 flex flex-col min-h-0">
            {/* Status Section */}
            <div className="space-y-2">
              <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Status</h3>
              <p className="text-xs text-muted-foreground">
                Sessions: {sessions.length} • {isDiscoveryLoading ? "Loading..." : "Ready"}
              </p>
              <p className="text-xs break-all">{claudeStatus}</p>
            </div>

            <Separator className="my-4" />

            {/* Sessions Section */}
            <div className="flex-1 flex flex-col space-y-4 min-h-0">
              <h3 className="text-sm font-semibold text-primary uppercase tracking-wide">Sessions</h3>
              
              {/* Create New Session */}
              <div className="space-y-2">
                <Input
                  type="text"
                  value={newProjectPath}
                  onChange={(e) => setNewProjectPath(e.target.value)}
                  placeholder="Project path"
                  className="text-xs"
                />
                <Button 
                  onClick={createSession} 
                  disabled={isDiscoveryLoading}
                  size="sm"
                  className="w-full"
                >
                  Create Session
                </Button>
              </div>

              <Separator />

              {/* Active Sessions List */}
              <div className="space-y-2 flex-1 overflow-y-auto">
                {sessions.length === 0 ? (
                  <p className="text-xs text-muted-foreground">No active sessions</p>
                ) : (
                  sessions.map((session) => (
                    <div key={session.id} className="p-2 border border-border/20 bg-muted/10">
                      <div className="flex justify-between items-start mb-1">
                        <p className="text-xs font-mono break-all flex-1">
                          {session.projectPath.split('/').pop()}
                        </p>
                        <Button
                          onClick={() => stopSession(session.id)}
                          disabled={session.isLoading}
                          variant="destructive"
                          size="sm"
                          className="h-5 px-2 text-xs ml-2"
                        >
                          ×
                        </Button>
                      </div>
                      <p className="text-xs text-muted-foreground truncate">
                        {session.messages.length} messages
                      </p>
                    </div>
                  ))
                )}
              </div>
            </div>
          </Frame>
        </div>

        {/* Chat Sessions Grid */}
        <div className="flex-1 grid grid-cols-1 gap-4 min-h-0 overflow-hidden" style={{
          gridTemplateColumns: sessions.length === 1 ? '1fr' : 
                              sessions.length === 2 ? '1fr 1fr' :
                              sessions.length === 3 ? '1fr 1fr 1fr' :
                              sessions.length >= 4 ? '1fr 1fr' : '1fr',
          gridAutoRows: 'minmax(0, 1fr)'
        }}>
          {sessions.length === 0 ? (
            <div className="flex items-center justify-center h-full">
              <div className="text-center">
                <p className="text-muted-foreground mb-4">No active sessions</p>
                <p className="text-sm text-muted-foreground">
                  Create a session to start chatting with Claude Code
                </p>
              </div>
            </div>
          ) : (
            sessions.map((session) => (
              <Frame key={session.id} className="flex flex-col min-h-0 overflow-hidden">
                {/* Chat Header */}
                <div className="p-4 border-b border-border flex-shrink-0">
                  <h3 className="text-sm font-semibold truncate">
                    {session.projectPath.split('/').pop()}
                  </h3>
                  <p className="text-xs text-muted-foreground">
                    {session.messages.length} messages
                  </p>
                </div>

                {/* Messages */}
                <div className="flex-1 min-h-0 overflow-hidden">
                  <ScrollArea className="h-full w-full">
                    <div className="px-4 py-4">
                      {session.messages.length === 0 ? (
                        <p className="text-sm text-muted-foreground">
                          No messages yet. Send a message to start the conversation.
                        </p>
                      ) : (
                        <div className="space-y-4">
                          {session.messages.map(renderMessage)}
                        </div>
                      )}
                    </div>
                  </ScrollArea>
                  
                  {/* Input */}
                  <div className="p-4 border-t border-border flex-shrink-0">
                    <div className="flex gap-2">
                      <Input
                        type="text"
                        value={session.inputMessage}
                        onChange={(e) => updateSessionInput(session.id, e.target.value)}
                        onKeyPress={(e) => {
                          if (e.key === "Enter") {
                            sendMessage(session.id);
                          }
                        }}
                        placeholder="Type your message..."
                        disabled={session.isLoading}
                        className="flex-1 text-sm"
                        autoFocus
                      />
                      <Button 
                        onClick={() => sendMessage(session.id)} 
                        disabled={session.isLoading || !session.inputMessage.trim()}
                        size="sm"
                      >
                        Send
                      </Button>
                    </div>
                  </div>
                </div>
              </Frame>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

export default App;