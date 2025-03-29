# Using the Enhanced `useChat` Hook

This document provides examples of how to use the newly enhanced `useChat` hook with Cloudflare Agents support.

## Basic Usage with Coder Agent

```tsx
import { useChat } from '@openagents/core/chat';

// Inside your component
const { messages, append, agentConnection } = useChat({
  // Connect to a specific CoderAgent instance
  agentId: 'coder-agent',
  agentName: 'my-project',
  
  // Optional project context
  agentOptions: {
    projectContext: {
      repoOwner: 'OpenAgentsInc',
      repoName: 'openagents',
      branch: 'main'
    }
  },
  
  // Get notification when agent connection changes
  onAgentConnectionChange: (connected) => {
    console.log(`Agent connection status: ${connected ? 'connected' : 'disconnected'}`);
  }
});

// Check if agent is connected
if (agentConnection.isConnected) {
  console.log('Connected to Coder Agent!');
}

// Send a message
const handleSendMessage = async (content) => {
  await append({
    role: 'user',
    content
  });
};
```

## Connecting to Multiple Coder Agents

The enhanced `useChat` hook enables connection to different agent instances, allowing for a many-to-many relationship between users and agents.

```tsx
import { useState } from 'react';
import { useChat } from '@openagents/core/chat';

function MultiAgentChat() {
  // Track the currently selected agent
  const [currentAgent, setCurrentAgent] = useState('default');
  
  // List of available agent instances
  const agents = [
    { id: 'default', name: 'Personal Assistant' },
    { id: 'project-A', name: 'Project A Coder' },
    { id: 'project-B', name: 'Project B Coder' },
  ];
  
  // Connect to the selected agent
  const { 
    messages, 
    append, 
    agentConnection,
    isAgentConnected
  } = useChat({
    agentId: 'coder-agent',
    agentName: currentAgent,
    agentServerUrl: 'https://agents.openagents.com'
  });
  
  // Switch between different agent instances
  const switchAgent = (agentId) => {
    setCurrentAgent(agentId);
  };
  
  return (
    <div>
      <div className="agent-selector">
        {agents.map(agent => (
          <button 
            key={agent.id}
            onClick={() => switchAgent(agent.id)}
            className={currentAgent === agent.id ? 'active' : ''}
          >
            {agent.name}
            {currentAgent === agent.id && isAgentConnected && ' (Connected)'}
          </button>
        ))}
      </div>
      
      <div className="chat-container">
        {messages.map(message => (
          <div key={message.id} className={`message ${message.role}`}>
            {message.content}
          </div>
        ))}
      </div>
      
      {/* Message input form */}
    </div>
  );
}
```

## Executing Commands through the Agent

The hook also supports executing commands through the agent instead of locally:

```tsx
import { useChat } from '@openagents/core/chat';

function AgentCommandExecutor() {
  const { 
    executeAgentCommand, 
    agentConnection 
  } = useChat({
    agentId: 'coder-agent',
    agentName: 'my-project'
  });
  
  const [result, setResult] = useState(null);
  
  const runCommand = async () => {
    if (!agentConnection.isConnected) {
      console.error('Agent not connected');
      return;
    }
    
    try {
      const commandResult = await executeAgentCommand('ls -la');
      setResult(commandResult);
    } catch (error) {
      console.error('Command execution failed:', error);
    }
  };
  
  return (
    <div>
      <button onClick={runCommand} disabled={!agentConnection.isConnected}>
        Run Command
      </button>
      
      {result && (
        <pre>{JSON.stringify(result, null, 2)}</pre>
      )}
    </div>
  );
}
```

## Hybrid Mode: Local Fallback

You can enable both agent-based and local command execution for hybrid scenarios:

```tsx
const { 
  messages, 
  append, 
  isAgentConnected,
  isCommandExecutionEnabled,
  executeAgentCommand 
} = useChat({
  // Agent configuration
  agentId: 'coder-agent',
  agentName: 'my-project',
  
  // Local command execution as fallback
  localCommandExecution: true,
  
  // Command execution options
  commandOptions: {
    // Local command execution options
    timeout: 5000,
    allowedCommands: ['git', 'ls', 'cat']
  }
});

// Function to execute a command with fallback
const runCommandWithFallback = async (command) => {
  try {
    // Try to execute via agent first
    if (isAgentConnected) {
      return await executeAgentCommand(command);
    }
    
    // Fall back to local execution
    if (isCommandExecutionEnabled) {
      return await safeExecuteCommand(command);
    }
    
    throw new Error('Neither agent nor local command execution is available');
  } catch (error) {
    console.error('Command execution failed:', error);
  }
};
```

## Testing Connection Status

The enhanced hook provides methods for testing both agent connection and command execution capabilities:

```tsx
const { testCommandExecution } = useChat({
  agentId: 'coder-agent',
  localCommandExecution: true
});

const testCapabilities = async () => {
  const testResults = await testCommandExecution();
  console.log('Agent connection:', testResults.agent.connected);
  console.log('Agent command execution:', testResults.agent.available);
  console.log('Local command execution:', testResults.local.available);
};
```