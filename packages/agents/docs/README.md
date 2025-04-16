# OpenAgents Agent Framework

The OpenAgents Agent Framework provides a modular and extensible architecture for building specialized AI agents that can perform various tasks through a unified interface.

## Documentation Index

- [Agent Architecture Refactoring](./agent-architecture-refactoring.md) - Overview of the modular agent architecture design
- [Agent Implementation Guide](./agent-implementation-guide.md) - Step-by-step guide for implementing new agent types

## Quick Start

### Using an Agent

The framework provides multiple ways to interact with agents:

#### 1. Through the React Hooks (Recommended)

```tsx
import { useOpenAgent } from "@openagents/core/agents/useOpenAgent";

function YourComponent() {
  // Connect to the Coder agent
  const agent = useOpenAgent("unique-session-id", "coder");
  
  // Send a message to the agent
  const handleSendMessage = async () => {
    await agent.handleSubmit("Please help me implement a React component");
    await agent.infer(githubToken);
  };
  
  return (
    <div>
      {/* Display agent messages */}
      {agent.messages.map(message => (
        <div key={message.id}>
          <strong>{message.role}:</strong> {message.content}
        </div>
      ))}
      
      {/* UI for sending messages */}
      <button onClick={handleSendMessage}>Send Message</button>
    </div>
  );
}
```

#### 2. Through the REST API

```bash
# Connect to the Coder agent
curl -X POST https://agents.openagents.com/agent/coder/infer \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Token: your-github-token" \
  -d '{
    "messages": [
      {
        "role": "user",
        "content": "Please help me implement a React component"
      }
    ]
  }'
```

### Available Agents

The framework currently includes the following agent types:

1. **Coder Agent**: Focused on code generation, analysis, and file operations
   - Path: `/agent/coder`
   - Features: Code analysis, file operations, task management, dependency analysis

2. **Solver Agent**: Specialized in GitHub/Linear issue resolution
   - Path: `/agent/solver`
   - Features: Issue analysis, implementation planning, status tracking, PR creation

## Agent Architecture

The OpenAgents framework uses a modular architecture with the following components:

1. **Base Agent Class**: Provides common functionality for all agent types
2. **Agent-Specific Implementations**: Specialized logic for each agent type
3. **Tools**: Reusable functions that agents can use to perform tasks
4. **State Management**: Type-safe state management using AsyncLocalStorage

```
packages/agents/src/
├── server.ts                 # Main entry point with agent routing
├── agents/                   # Agent-specific implementations
│   ├── coder/                # Coder agent implementation
│   └── solver/               # Solver agent implementation
├── common/                   # Shared functionality
│   ├── agent.ts              # Base Agent class
│   ├── tools/                # Common tools
│   └── types.ts              # Shared type definitions
└── utils/                    # Utility functions
```

## Extending the Framework

To create a new agent type:

1. Create a new directory in `src/agents/` for your agent type
2. Implement the required files following the [Agent Implementation Guide](./agent-implementation-guide.md)
3. Add your agent to the configuration in `server.ts`
4. Update the `useOpenAgent` hook to support your agent type

## Contributing

We welcome contributions to the OpenAgents framework! To contribute:

1. Fork the repository
2. Create a feature branch (`git checkout -b my-new-feature`)
3. Commit your changes (`git commit -am 'Add some feature'`)
4. Push to the branch (`git push origin my-new-feature`)
5. Create a new Pull Request

Please ensure your code follows our coding standards and includes appropriate tests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.