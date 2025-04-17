# Agent Architecture Refactoring

This document explains the architectural refactoring of the OpenAgents codebase to support multiple agent types and a more modular structure.

## Overview

The OpenAgents codebase has been refactored from a monolithic architecture to a modular agent framework that can support multiple specialized agent types. The original implementation had a single `server.ts` file containing all the Coder agent logic. The new architecture separates agent-specific logic into dedicated modules, allowing for:

- Easy addition of new agent types
- Better code organization and maintenance
- Shared utilities and tools between agents
- Type-safe state management for each agent type

## Directory Structure

The new directory structure is organized as follows:

```
packages/agents/src/
├── server.ts                 # Main entry point with agent routing
├── agents/                   # Agent-specific implementations
│   ├── coder/                # Coder agent implementation
│   │   ├── index.ts          # Main Coder agent class
│   │   ├── prompts.ts        # Coder-specific prompts
│   │   ├── tools.ts          # Coder-specific tools
│   │   └── types.ts          # Coder state and type definitions
│   └── solver/               # Solver agent implementation
│       ├── index.ts          # Main Solver agent class
│       ├── prompts.ts        # Solver-specific prompts
│       ├── tools.ts          # Solver-specific tools
│       └── types.ts          # Solver state and type definitions
├── common/                   # Shared functionality
│   ├── agent.ts              # Base Agent class
│   ├── tools/                # Common tools
│   │   └── index.ts          # Shared tool implementations
│   └── types.ts              # Shared type definitions
└── utils/                    # Utility functions
    └── index.ts              # Common utilities
```

## Key Components

### 1. `server.ts`

The main entry point has been simplified to route requests to the appropriate agent type using a routing function:

```typescript
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    const agentConfig = {
      agents: {
        'coder': Coder, // Map path '/agent/coder' to Coder class
        'solver': Solver // Map path '/agent/solver' to Solver class
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

### 2. Base Agent Class

A shared base `Agent` class provides common functionality:

```typescript
export abstract class Agent<TEnv, TState> {
  abstract initialState: TState;
  store = new AsyncLocalStorage<TState>();

  // Common methods used by all agents
  getState(): TState {
    return this.store.getStore() || this.initialState;
  }

  setState(partial: Partial<TState>): void {
    this.store.enterWith({ ...this.getState(), ...partial });
  }

  // Method that must be implemented by all agent types
  abstract infer(): Promise<AIResponse>;
}
```

### 3. Coder Agent

The Coder agent was extracted from the original implementation, with functionality for:
- Code analysis and generation
- File operations
- Task management
- Dependency analysis

### 4. Solver Agent

A new Solver agent was implemented for GitHub/Linear issue resolution, with:
- Issue state management
- Implementation plan generation
- Status tracking for implementation steps
- Integration with GitHub/Linear APIs

```typescript
export class Solver extends Agent<Env, SolverState> {
  initialState: SolverState = {
    messages: [],
    githubToken: undefined,
    currentRepoOwner: undefined,
    currentRepoName: undefined,
    currentBranch: undefined,
    scratchpad: '',
    observations: [],
    workingFilePath: undefined
  };

  // Methods for issue management
  async setCurrentIssue(issue: Issue) { /* ... */ }
  async updateStepStatus(stepId: string, status: ImplementationStep['status'], notes?: string) { /* ... */ }

  // Main inference method
  async infer() { /* ... */ }
}
```

### 5. UI Integration

A new `SolverConnector` component was created to integrate the Solver agent with the issues UI:

```typescript
export function SolverConnector({ issue, githubToken }: SolverConnectorProps) {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');

  // Connect to Solver agent
  const connectToSolver = async () => { /* ... */ };

  // Disconnect from Solver agent
  const disconnectFromSolver = () => { /* ... */ };

  // Render UI based on connection state
  return (
    <Card className="mb-6">
      {/* Card content with connection status and controls */}
    </Card>
  );
}
```

## Development Notes

### Fixed React Hydration Mismatch

A React hydration mismatch was fixed in the `SolverConnector` component:

The issue was that the `disabled` attribute on the `Button` component was being rendered differently on the server versus the client:
- Server: `disabled=""` (attribute with empty string value)
- Client: `disabled={true}` (boolean attribute)

This was fixed by using a ternary expression that converts boolean values to either `true` or `undefined`:

```typescript
// Before
<Button
  onClick={connectToSolver}
  disabled={isStartingSolver || !githubToken}
>

// After
<Button
  onClick={connectToSolver}
  disabled={isStartingSolver || !githubToken ? true : undefined}
>
```

This approach ensures that the attribute is rendered consistently in both server and client environments, preventing hydration mismatches.

## Adding New Agent Types

To add a new agent type:

1. Create a new directory in `src/agents/` for your agent type (e.g., `src/agents/researcher/`)
2. Implement the required files:
   - `index.ts`: Main agent class extending the base `Agent` class
   - `types.ts`: Type definitions for agent state
   - `prompts.ts`: Agent-specific prompts
   - `tools.ts`: Agent-specific tools
3. Add your agent to the config in `server.ts`
4. Create UI components for interacting with your agent

## Future Work

- Implement more specialized agent types (e.g., Researcher, Writer)
- Enhance issue resolution capabilities with more advanced analysis
- Add support for custom tool composition at runtime
- Create a unified interface for managing multiple agent types
