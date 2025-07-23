import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

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
    discoverClaude();
  }, []);

  // Poll for messages when session is active
  useEffect(() => {
    if (!sessionId) return;

    const interval = setInterval(async () => {
      await fetchMessages();
    }, 1000);

    return () => clearInterval(interval);
  }, [sessionId]);

  const discoverClaude = async () => {
    setIsLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("discover_claude");
      if (result.success && result.data) {
        setClaudeStatus(`Claude found at: ${result.data}`);
      } else {
        setClaudeStatus(`Error: ${result.error || "Unknown error"}`);
      }
    } catch (error) {
      setClaudeStatus(`Error: ${error}`);
    }
    setIsLoading(false);
  };

  const createSession = async () => {
    if (!projectPath) {
      alert("Please enter a project path");
      return;
    }

    setIsLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("create_session", {
        projectPath,
      });
      if (result.success && result.data) {
        setSessionId(result.data);
        setMessages([]);
      } else {
        alert(`Error creating session: ${result.error}`);
      }
    } catch (error) {
      alert(`Error: ${error}`);
    }
    setIsLoading(false);
  };

  const sendMessage = async () => {
    if (!sessionId || !inputMessage.trim()) return;

    setIsLoading(true);
    try {
      const result = await invoke<CommandResult<void>>("send_message", {
        sessionId,
        message: inputMessage,
      });
      if (result.success) {
        setInputMessage("");
      } else {
        alert(`Error sending message: ${result.error}`);
      }
    } catch (error) {
      alert(`Error: ${error}`);
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
        setMessages(result.data);
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
      user: "user-message",
      assistant: "assistant-message",
      tool_use: "tool-message",
      error: "error-message",
      summary: "summary-message",
      thinking: "thinking-message",
      system: "system-message",
    };

    const style = messageTypeStyles[msg.message_type] || "";

    return (
      <div key={msg.id} className={`message ${style}`}>
        <div className="message-type">{msg.message_type}</div>
        <div className="message-content">
          <pre>{msg.content}</pre>
          {msg.tool_info && msg.tool_info.output && (
            <details className="tool-output">
              <summary>Tool Output</summary>
              <pre>{msg.tool_info.output}</pre>
            </details>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="container">
      <h1>OpenAgents - Claude Code Integration Test</h1>

      <div className="status-section">
        <h2>Status</h2>
        <p>{claudeStatus}</p>
        <button onClick={discoverClaude} disabled={isLoading}>
          Rediscover Claude
        </button>
      </div>

      <div className="session-section">
        <h2>Session Management</h2>
        {!sessionId ? (
          <div>
            <input
              type="text"
              value={projectPath}
              onChange={(e) => setProjectPath(e.target.value)}
              placeholder="Project path"
              style={{ width: "400px" }}
            />
            <button onClick={createSession} disabled={isLoading}>
              Create Session
            </button>
          </div>
        ) : (
          <div>
            <p>Active Session: {sessionId}</p>
            <button onClick={stopSession} disabled={isLoading}>
              Stop Session
            </button>
          </div>
        )}
      </div>

      {sessionId && (
        <div className="chat-section">
          <h2>Conversation</h2>
          <div className="messages-container">
            {messages.length === 0 ? (
              <p>No messages yet. Send a message to start the conversation.</p>
            ) : (
              messages.map(renderMessage)
            )}
          </div>
          <div className="input-section">
            <input
              type="text"
              value={inputMessage}
              onChange={(e) => setInputMessage(e.target.value)}
              onKeyPress={(e) => e.key === "Enter" && sendMessage()}
              placeholder="Type your message..."
              disabled={isLoading}
              style={{ flex: 1 }}
            />
            <button onClick={sendMessage} disabled={isLoading || !inputMessage.trim()}>
              Send
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;