# OpenAgents Agent Architecture Guide

**Date:** April 16, 2025  
**Author:** Claude Code

## Overview

This document provides a comprehensive guide to the new modular agent architecture implemented in the OpenAgents platform. This architecture supports multiple specialized agent types through a standardized framework, enabling developers to create, extend, and deploy different agent capabilities.

## Core Architecture Concepts

### 1. Agent Base Class

All agents in the system extend the base `Agent<Env, State>` class provided by the `agents` package:

```typescript
export class Coder extends Agent<Env, CoderState> { /* ... */ }
export class Solver extends Agent<Env, SolverState> { /* ... */ }
```

This ensures a consistent interface for:
- State management
- Message handling
- Scheduling capabilities
- Tool integration

### 2. Agent Context Pattern

Each agent type maintains its own context using the `AsyncLocalStorage` pattern:

```typescript
export const agentContext = new AsyncLocalStorage<Coder>();
export const solverContext = new AsyncLocalStorage<Solver>();
```

This allows:
- Tracking the current agent instance during execution
- Accessing the agent state from tool implementations
- Maintaining separation between different agent types

### 3. Agent Request Routing

The entry point (`server.ts`) uses the `routeAgentRequest` function to direct incoming requests to the appropriate agent type:

```typescript
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const agentConfig = {
      agents: {
        'coder': Coder, // Maps to /agent/coder
        'solver': Solver // Maps to /agent/solver
      },
      cors: true
    };

    return (
      (await routeAgentRequest(request, env, agentConfig)) ||
      new Response("Agent not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
```

### 4. State Management

Each agent defines its own state interface:

```typescript
// Coder agent state
export interface CoderState {
  messages: UIMessage[];
  githubToken?: string;
  currentRepoOwner?: string;
  // ... other Coder-specific state
}

// Solver agent state
export interface SolverState {
  messages: UIMessage[];
  githubToken?: string;
  currentProblem?: Problem;
  // ... other Solver-specific state
}
```

The agent's state is updated using the `updateState` method, which ensures proper merging with existing state:

```typescript
private updateState(partialState: Partial<AgentState>) {
  this.setState({
    ...this.state,
    ...partialState,
  });
}
```

### 5. Tool Composition Pattern

Tools are composed at runtime based on the agent's needs:

```typescript
// In Coder.infer()
const toolContext: ToolContext = { githubToken: token }
const tools = {
  get_file_contents: getFileContentsTool(toolContext),
  add_issue_comment: addIssueCommentTool(toolContext),
  ...availableTools
}

// In Solver.infer()
const toolContext: ToolContext = { githubToken: token };
const tools = {
  ...solverTools,
  ...commonTools
};
```

This approach allows:
- Sharing common tools across agent types
- Adding agent-specific tools
- Contextual tool configuration

## Directory Structure

The modular architecture is reflected in the project's directory structure:

```
src/
│
├── agents/              # Agent implementations
│   ├── coder/           # Coder agent files
│   │   ├── index.ts     # Main Coder class
│   │   ├── prompts.ts   # Coder-specific prompts
│   │   ├── schemas.ts   # Coder-specific schemas
│   │   └── types.ts     # Coder-specific types
│   │
│   └── solver/          # Solver agent files
│       ├── index.ts     # Main Solver class
│       ├── prompts.ts   # Solver-specific prompts
│       ├── tools.ts     # Solver-specific tools
│       └── types.ts     # Solver-specific types
│
├── common/              # Shared resources
│   ├── config.ts        # Common configuration
│   ├── types.ts         # Shared types
│   └── tools/           # Shared tools
│       ├── index.ts     # Common tool definitions
│       └── github/      # GitHub-specific tools
│
├── server.ts            # Main entry point
└── index.ts             # Package exports
```

## Agent Implementation Guide

To implement a new agent type, follow these steps:

### 1. Create Agent Directory Structure

```bash
mkdir -p src/agents/your-agent-name
```

### 2. Define Agent State

Create a `types.ts` file for your agent:

```typescript
// src/agents/your-agent-name/types.ts
import { type UIMessage } from "ai";

export interface YourAgentState {
  messages: UIMessage[];
  // Add agent-specific state properties
}
```

### 3. Implement Agent Class

Create an `index.ts` file with your agent implementation:

```typescript
// src/agents/your-agent-name/index.ts
import { Agent, type Connection, type WSMessage } from "agents";
import { type UIMessage, generateId, generateText, type ToolSet } from "ai";
import { AsyncLocalStorage } from "node:async_hooks";
import type { Env } from "../../types";
import type { YourAgentState } from "./types";
import { getSystemPrompt } from "./prompts";
import { model } from "../../common/config";

export const yourAgentContext = new AsyncLocalStorage<YourAgent>();

export class YourAgent extends Agent<Env, YourAgentState> {
  initialState: YourAgentState = {
    messages: [],
    // Initialize other state properties
  };
  tools: ToolSet = {};
  
  constructor(ctx: any, env: Env) {
    super(ctx, env);
    this.ctx = ctx;
    console.log("[Constructor] YourAgent instance created.");
  }

  private updateState(partialState: Partial<YourAgentState>) {
    this.setState({
      ...this.state,
      ...partialState,
    });
  }

  async onMessage(connection: Connection, message: WSMessage) {
    // Handle incoming messages
  }

  @unstable_callable({
    description: "Generate a response based on the current messages",
    streaming: true
  })
  async infer() {
    return yourAgentContext.run(this, async () => {
      // Your agent's inference logic
    });
  }
}
```

### 4. Create Agent Prompts

Create a `prompts.ts` file for your agent's prompts:

```typescript
// src/agents/your-agent-name/prompts.ts
import { tools } from "../../common/tools";
import { type YourAgentState } from "./types";

interface SystemPromptOptions {
  state: YourAgentState;
  model?: any;
  temperature?: number;
}

export function getSystemPrompt(options: SystemPromptOptions): string {
  const { state, model, temperature = 0.7 } = options;
  
  // Build your agent's system prompt based on its state
  let systemPrompt = `You are an autonomous agent specialized in...`;
  
  // Add dynamic content based on state
  
  return systemPrompt;
}
```

### 5. Add Agent-Specific Tools (Optional)

If your agent needs specialized tools, create a `tools.ts` file:

```typescript
// src/agents/your-agent-name/tools.ts
import { tool } from "ai";
import { z } from "zod";
import { YourAgent, yourAgentContext } from "./index";

export const specializedTool = tool({
  description: "A specialized tool for your agent",
  parameters: z.object({
    // Define parameters
  }),
  execute: async (params) => {
    // Implement tool functionality
  }
});

export const yourAgentTools = {
  specializedTool,
  // Add other tools
};
```

### 6. Register Agent in Server

Update `server.ts` to include your new agent:

```typescript
// src/server.ts
import { YourAgent } from './agents/your-agent-name';

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const agentConfig = {
      agents: {
        'coder': Coder,
        'solver': Solver,
        'your-agent-name': YourAgent, // Add your agent
      },
      cors: true
    };

    return (
      (await routeAgentRequest(request, env, agentConfig)) ||
      new Response("Agent not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
```

### 7. Export Agent in Package Index

Update `index.ts` to export your agent:

```typescript
// src/index.ts
export { YourAgent } from './agents/your-agent-name';
```

## Best Practices

1. **State Separation**: Keep agent state interfaces separate and focused
2. **Tool Modularity**: Design tools to be either shared or agent-specific
3. **Consistent Naming**: Follow the established naming patterns
4. **Context Isolation**: Use the AsyncLocalStorage pattern for agent context
5. **Comprehensive Prompts**: Tailor system prompts to each agent's purpose
6. **Error Handling**: Implement robust error handling in tools and inference
7. **Documentation**: Document the agent's purpose, capabilities, and usage

## Common Pitfalls

1. **State Pollution**: Avoid adding state properties that don't belong to the agent
2. **Tool Context Leakage**: Ensure tools properly access the correct agent context
3. **Prompt Confusion**: Don't mix instructions for different agent types
4. **Message Handling**: Correctly parse and validate incoming messages
5. **Tool Duplication**: Avoid implementing the same tool multiple times

## Conclusion

The modular agent architecture provides a flexible and extensible framework for implementing different types of AI agents in the OpenAgents platform. By following this guide, developers can create specialized agents that leverage common infrastructure while providing unique capabilities for different use cases.