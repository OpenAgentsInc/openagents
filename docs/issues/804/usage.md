# Using the CoderAgent with WebSocket

This document explains how to connect to and use the CoderAgent over WebSocket now that the connection issue has been fixed.

## Client-Side Connection

Use the AgentClient from the Agents SDK to establish a WebSocket connection:

```typescript
import { AgentClient } from "agents/client";

// Create a new agent client - note that CoderAgent gets converted to lowercase
const client = new AgentClient({
  agent: "CoderAgent", // Will be automatically converted to "coderagent"
  name: "default",
  host: "agents.openagents.com"
});

// Set up event listeners
client.addEventListener("open", () => {
  console.log("Connected to CoderAgent");
});

client.addEventListener("message", (event) => {
  console.log("Received message:", event.data);
});

client.addEventListener("close", () => {
  console.log("Connection closed");
});

client.addEventListener("error", (error) => {
  console.error("Connection error:", error);
});

// Call methods on the agent
async function executeCommand(command) {
  try {
    const result = await client.call("executeCommand", [command]);
    return result;
  } catch (error) {
    console.error("Failed to execute command:", error);
    throw error;
  }
}
```

## Using the useChat Hook

For React applications, use the useChat hook which wraps the WebSocket connection:

```tsx
import { useChat } from "@/hooks/useChat";

function CoderAgentInterface() {
  const { 
    messages, 
    input, 
    handleInputChange, 
    handleSubmit, 
    isLoading,
    error 
  } = useChat({
    agent: "CoderAgent",
    roomId: "default"
  });

  if (error) {
    return <div>Error connecting to agent: {error.message}</div>;
  }

  return (
    <div className="chat-container">
      <div className="messages">
        {messages.map((message) => (
          <div key={message.id} className={`message ${message.role}`}>
            {message.content}
          </div>
        ))}
        {isLoading && <div className="loading">Agent is thinking...</div>}
      </div>
      
      <form onSubmit={handleSubmit}>
        <input
          value={input}
          onChange={handleInputChange}
          placeholder="Ask the coding agent..."
          disabled={isLoading}
        />
        <button type="submit" disabled={isLoading}>Send</button>
      </form>
    </div>
  );
}
```

## Available Methods

The CoderAgent supports the following methods:

1. **executeCommand(command: string)**:
   Execute a shell command in the agent's environment.

2. **analyzeCode(code: string, language: string)**:
   Analyze a code snippet for issues, patterns, and improvements.

3. **generateCode(prompt: string, language: string)**:
   Generate code based on a description.

4. **explainCode(code: string, language: string)**:
   Provide an explanation of what the given code does.

## Connection Troubleshooting

If you encounter connection issues:

1. Check that you're using the correct host (agents.openagents.com)
2. Verify that the agent name is "CoderAgent" (will be converted to "coderagent")
3. Check browser console for WebSocket connection errors
4. Ensure the server is properly deployed with the correct binding name