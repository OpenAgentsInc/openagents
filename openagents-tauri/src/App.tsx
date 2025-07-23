import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";

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

function App() {
  const [claudeStatus, setClaudeStatus] = useState<string>("Not initialized");
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [inputMessage, setInputMessage] = useState("");
  const [projectPath, setProjectPath] = useState("/Users/christopherdavid/Desktop/openagents");
  const [isLoading, setIsLoading] = useState(false);

  // Initialize Claude on mount
  useEffect(() => {
    console.log("App mounted, starting Claude discovery...");
    discoverClaude();
  }, []);

  // Poll for messages when session is active
  useEffect(() => {
    if (!sessionId) return;

    const interval = setInterval(async () => {
      await fetchMessages();
    }, 50); // Poll every 50ms for real-time updates

    return () => clearInterval(interval);
  }, [sessionId]);

  const discoverClaude = async () => {
    console.log("Starting Claude discovery...");
    setIsLoading(true);
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
    setIsLoading(false);
  };

  const createSession = async () => {
    if (!projectPath) {
      alert("Please enter a project path");
      return;
    }

    console.log("Creating session for project:", projectPath);
    setIsLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath,
      });
      console.log("Create session result:", result);
      if (result.success && result.data) {
        setSessionId(result.data);
        setMessages([]);
        console.log("Session created with ID:", result.data);
      } else {
        alert(`Error creating session: ${result.error}`);
        console.error("Session creation failed:", result.error);
      }
    } catch (error) {
      alert(`Error: ${error}`);
      console.error("Session creation error:", error);
    }
    setIsLoading(false);
  };

  const sendMessage = async () => {
    if (!sessionId || !inputMessage.trim()) {
      console.log("Cannot send message - sessionId:", sessionId, "message:", inputMessage);
      return;
    }

    // Immediately add user message to UI
    const userMessage: Message = {
      id: `user-${Date.now()}`,
      message_type: "user",
      content: inputMessage,
      timestamp: new Date().toISOString(),
    };
    setMessages(prev => [...prev, userMessage]);

    console.log("Sending message:", inputMessage, "to session:", sessionId);
    const messageToSend = inputMessage;
    setInputMessage(""); // Clear input immediately
    setIsLoading(true);
    
    try {
      const result = await invoke<CommandResult<void>>("send_message", {
        sessionId,
        message: messageToSend,
      });
      console.log("Send message result:", result);
      if (!result.success) {
        alert(`Error sending message: ${result.error}`);
        console.error("Send message failed:", result.error);
        // Remove the user message we optimistically added
        setMessages(prev => prev.filter(msg => msg.id !== userMessage.id));
      }
    } catch (error) {
      alert(`Error: ${error}`);
      console.error("Send message error:", error);
      // Remove the user message we optimistically added
      setMessages(prev => prev.filter(msg => msg.id !== userMessage.id));
    }
    setIsLoading(false);
  };

  const fetchMessages = async () => {
    if (!sessionId) return;

    try {
      const result = await invoke<CommandResult<Message[]>>("get_messages", {
        sessionId,
      });
      if (result.success && result.data) {
        console.log("Fetched messages from backend:", result.data.length);
        // Merge backend messages with local optimistic messages
        setMessages(prev => {
          const backendMessages = result.data || [];
          const optimisticUserMessages = prev.filter(msg => 
            msg.id.startsWith('user-') && msg.message_type === 'user'
          );
          
          // Remove optimistic messages that now exist in backend
          const filteredOptimistic = optimisticUserMessages.filter(optimistic => 
            !backendMessages.some(backend => 
              backend.message_type === 'user' && 
              backend.content === optimistic.content
            )
          );
          
          // Combine and sort by timestamp
          const combined = [...backendMessages, ...filteredOptimistic];
          combined.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
          
          return combined;
        });
      }
    } catch (error) {
      console.error("Error fetching messages:", error);
    }
  };

  const stopSession = async () => {
    if (!sessionId) return;

    setIsLoading(true);
    try {
      const result = await invoke<CommandResult<void>>("stop_session", {
        sessionId,
      });
      if (result.success) {
        setSessionId(null);
        setMessages([]);
      } else {
        alert(`Error stopping session: ${result.error}`);
      }
    } catch (error) {
      alert(`Error: ${error}`);
    }
    setIsLoading(false);
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

  return (
    <div className="min-h-screen bg-background p-8 font-mono overflow-x-hidden">
      <div className="mx-auto max-w-5xl space-y-6 w-full">
        <div className="text-center">
          <h1 className="text-3xl font-bold">OpenAgents</h1>
          <p className="text-muted-foreground">Claude Code Integration</p>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Status</CardTitle>
            <CardDescription>
              Session: {sessionId || "none"} • Messages: {messages.length} • {isLoading ? "Loading..." : "Ready"}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm">{claudeStatus}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Session Management</CardTitle>
            <CardDescription>
              {!sessionId ? "Start a new Claude Code session" : "Manage your active session"}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {!sessionId ? (
              <div className="flex gap-2">
                <Input
                  type="text"
                  value={projectPath}
                  onChange={(e) => setProjectPath(e.target.value)}
                  placeholder="Project path"
                  className="flex-1"
                />
                <Button onClick={createSession} disabled={isLoading}>
                  Create Session
                </Button>
              </div>
            ) : (
              <div className="space-y-4">
                <p className="text-sm text-muted-foreground">Active Session: {sessionId}</p>
                <Button onClick={stopSession} disabled={isLoading} variant="destructive">
                  Stop Session
                </Button>
              </div>
            )}
          </CardContent>
        </Card>

        {sessionId && (
          <Card className="h-[600px] flex flex-col overflow-hidden">
            <CardHeader>
              <CardTitle>Conversation</CardTitle>
              <CardDescription>
                Chat with Claude Code about your project
              </CardDescription>
            </CardHeader>
            <CardContent className="flex-1 flex flex-col overflow-hidden">
              <ScrollArea className="flex-1 border px-4 overflow-hidden max-w-full">
                <div className="pr-4 max-w-full overflow-hidden">
                  {messages.length === 0 ? (
                    <p className="text-sm text-muted-foreground">No messages yet. Send a message to start the conversation.</p>
                  ) : (
                    <div className="space-y-4 max-w-full">
                      {messages.map(renderMessage)}
                    </div>
                  )}
                </div>
              </ScrollArea>
              <Separator className="my-4" />
              <div className="flex gap-2">
                <Input
                  type="text"
                  value={inputMessage}
                  onChange={(e) => setInputMessage(e.target.value)}
                  onKeyPress={(e) => {
                    console.log("Key pressed:", e.key);
                    if (e.key === "Enter") {
                      console.log("Enter key detected, sending message...");
                      sendMessage();
                    }
                  }}
                  placeholder="Type your message..."
                  disabled={isLoading}
                  className="flex-1"
                />
                <Button onClick={sendMessage} disabled={isLoading || !inputMessage.trim()}>
                  Send
                </Button>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}

export default App;