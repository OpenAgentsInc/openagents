import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PaneManager } from "@/panes/PaneManager";
import { Hotbar } from "@/components/hud/Hotbar";
import { usePaneStore } from "@/stores/pane";
import { TooltipProvider } from "@/components/ui/tooltip";

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
  const [newProjectPath, setNewProjectPath] = useState("");
  const [isDiscoveryLoading, setIsDiscoveryLoading] = useState(false);
  
  const { openChatPane, toggleMetadataPane } = usePaneStore();

  // Get current directory on mount
  useEffect(() => {
    invoke("get_current_directory").then((result: any) => {
      if (result.success && result.data) {
        setNewProjectPath(result.data);
      }
    }).catch(console.error);
  }, []);

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
        
        // Open a pane for the new session
        openChatPane(result.data, newProjectPath);
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

  const updateSessionInput = (sessionId: string, value: string) => {
    setSessions(prev => prev.map(s => 
      s.id === sessionId ? { ...s, inputMessage: value } : s
    ));
  };

  // Set up keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check if the user is typing in an input field
      const target = event.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') {
        return;
      }

      // Handle modifier + digit combinations
      const modifier = navigator.platform.toUpperCase().indexOf('MAC') >= 0
        ? event.metaKey // Mac uses Cmd key
        : event.ctrlKey; // Windows/Linux use Ctrl key

      if (!modifier) return;

      const digit = parseInt(event.key);
      if (isNaN(digit) || digit < 1 || digit > 9) return;

      event.preventDefault();

      // Call the appropriate function based on the digit
      switch (digit) {
        case 1:
          toggleMetadataPane();
          break;
        case 2:
          if (newProjectPath) {
            createSession();
          }
          break;
        case 8:
          console.log('Settings');
          break;
        case 9:
          console.log('Help');
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [toggleMetadataPane, newProjectPath, createSession]);

  // Set dark mode on mount
  useEffect(() => {
    document.documentElement.classList.add("dark");
  }, []);

  // Provide session data to child components through a context or prop drilling
  // For now, we'll use a global object (this should be replaced with proper state management)
  // Update immediately without useEffect to ensure real-time updates
  (window as any).__openagents_data = {
    claudeStatus,
    sessions,
    newProjectPath,
    isDiscoveryLoading,
    setNewProjectPath,
    createSession,
    sendMessage,
    updateSessionInput,
    stopSession,
  };

  return (
    <TooltipProvider>
      <div className="fixed inset-0 font-mono overflow-hidden">
        {/* Background with grid pattern */}
        <div 
          className="absolute inset-0 opacity-5"
          style={{
            backgroundImage: `
              linear-gradient(to right, #10b981 1px, transparent 1px),
              linear-gradient(to bottom, #10b981 1px, transparent 1px)
            `,
            backgroundSize: '50px 50px'
          }}
        />
        
        {/* Pane Manager */}
        <PaneManager />
        
        {/* Hotbar */}
        <Hotbar onNewChat={createSession} />
      </div>
    </TooltipProvider>
  );
}

export default App;