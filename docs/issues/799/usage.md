# Local Command Execution Usage Guide

The `useChat` hook has been extended to support local command execution for autonomous coding agents. This guide explains how to use this feature in your OpenAgents applications.

## Getting Started

The command execution functionality allows AI models to run shell commands on the user's machine. This is especially useful for coding agents that need to interact with the development environment (running tests, installing packages, etc.).

### Basic Usage

To enable local command execution, pass the `localCommandExecution` flag to the `useChat` hook:

```typescript
import { useChat } from '@openagents/core';

const chat = useChat({
  localCommandExecution: true,
  // Other options...
});
```

### Command Format

The AI model can execute commands by including them in the message format with special XML-like tags:

```
<execute-command>
npm test
</execute-command>
```

The system will automatically:
1. Detect these tags in messages
2. Execute the command in a secure manner
3. Capture the output
4. Replace the command tags with the execution results

### Command Execution Options

You can configure the command execution behavior:

```typescript
const chat = useChat({
  localCommandExecution: true,
  commandOptions: {
    // Working directory for commands
    cwd: '/path/to/project',
    
    // Timeout (in milliseconds)
    timeout: 30000, // 30 seconds
    
    // Additional environment variables
    env: {
      NODE_ENV: 'development'
    }
  }
});
```

### Monitoring Command Execution

You can get notified when commands are being executed:

```typescript
const chat = useChat({
  localCommandExecution: true,
  onCommandStart: (command) => {
    console.log(`Executing: ${command}`);
    // Update UI to show a loading indicator
  },
  onCommandComplete: (command, result) => {
    console.log(`Completed: ${command}`, result);
    // Update UI to show completion status
  }
});
```

## Security Considerations

Local command execution comes with security risks. The implementation includes several safeguards:

1. **Dangerous Command Blocking**: Commands that could potentially harm the system (e.g., `rm -rf /`) are blocked automatically.

2. **Timeout Enforcement**: Commands have a default timeout to prevent long-running processes.

3. **Controlled Environment**: Commands run with the same permissions as the application itself.

4. **Limited Context**: The command execution is isolated to protect sensitive information.

## Implementation Details

This feature is only available in Electron (desktop) environments where Node.js APIs are accessible. In web environments, the command execution attempt will gracefully fail with an error message.

The command execution is handled through the Electron IPC system, with the main process executing the command and returning the results to the renderer process.

## Example Usage

Here's a complete example of a chat application with command execution:

```tsx
import React, { useState } from 'react';
import { useChat } from '@openagents/core';

export const ChatWithCommands = () => {
  const [executing, setExecuting] = useState(false);
  
  const chat = useChat({
    localCommandExecution: true,
    onCommandStart: () => setExecuting(true),
    onCommandComplete: () => setExecuting(false)
  });
  
  return (
    <div>
      {executing && <div>Executing command...</div>}
      
      <div className="messages">
        {chat.messages.map(message => (
          <div key={message.id} className={message.role}>
            {message.content}
          </div>
        ))}
      </div>
      
      <input
        type="text"
        value={chat.input}
        onChange={(e) => chat.setInput(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            chat.append({
              role: 'user',
              content: chat.input
            });
            chat.setInput('');
          }
        }}
        placeholder="Type a message..."
      />
    </div>
  );
};
```

## System Prompt Example

To inform the AI model about command execution capabilities, include information in your system prompt:

```
You can execute commands on the user's machine using the following format:

<execute-command>
command here
</execute-command>

For example, you can run tests:

<execute-command>
npm test
</execute-command>

Please be cautious when executing commands. Always explain what a command will do before running it, especially for commands that modify files.
```