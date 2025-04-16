# Agent Architecture Refactoring

This document explains the architectural refactoring of the OpenAgents codebase to support multiple agent types and a more modular structure.

## Overview

The OpenAgents codebase has been refactored from a monolithic architecture to a modular agent framework that can support multiple specialized agent types. The original implementation had a single `server.ts` file containing all the Coder agent logic, which became difficult to maintain and extend. The new architecture separates agent-specific logic into dedicated modules, allowing for:

- Easy addition of new agent types
- Better code organization and maintenance
- Shared utilities and tools between agents
- Type-safe state management for each agent type
- Simplified testing of individual components

## Architecture Design

### Directory Structure

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
├── utils/                    # Utility functions
│   └── index.ts              # Common utilities
└── plugins/                  # Extensibility plugins
    ├── plugin-interface.ts   # Plugin interface definitions
    └── github-plugin.ts      # GitHub integration plugin
```

### Core Components

#### 1. Base Agent Class

A shared base `Agent` class provides common functionality that all agent types can inherit:

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
  
  // Methods for message handling
  addMessage(message: UIMessage): void {
    const state = this.getState();
    this.setState({
      messages: [...state.messages, message]
    } as Partial<TState>);
  }
  
  // Token management
  async setGithubToken(token: string): Promise<void> {
    this.setState({ githubToken: token } as Partial<TState>);
  }
  
  async getGithubToken(): Promise<string | undefined> {
    return this.getState().githubToken;
  }
  
  // Method that must be implemented by all agent types
  abstract infer(token?: string): Promise<AIResponse>;
}
```

#### 2. Agent Router

The main entry point in `server.ts` uses a routing mechanism to direct requests to the appropriate agent type:

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

The `routeAgentRequest` function parses the request URL, extracts the agent type from the path, and instantiates the appropriate agent class to handle the request.

#### 3. Agent State Management

Each agent type has its own state interface that extends a common base state:

```typescript
// Common base state interface
export interface BaseAgentState {
  messages: UIMessage[];
  githubToken?: string;
}

// Coder agent specific state
export interface CoderState extends BaseAgentState {
  currentFilePath?: string;
  editorContent?: string;
  diagnostics?: Diagnostic[];
  selectedRange?: Range;
}

// Solver agent specific state
export interface SolverState extends BaseAgentState {
  currentIssue?: Issue;
  currentRepoOwner?: string;
  currentRepoName?: string;
  currentBranch?: string;
  implementationSteps?: ImplementationStep[];
  observations?: string[];
  scratchpad?: string;
  workingFilePath?: string;
  issueComments?: IssueComment[];
}
```

The `AsyncLocalStorage` pattern is used to maintain isolated state for each request, preventing state leakage between concurrent requests.

## Agent Implementations

### 1. Coder Agent

The Coder agent was extracted from the original implementation, with functionality for:

- Code analysis and generation
- File operations and manipulation
- Task management
- Dependency analysis
- Version control integration

Key methods include:

```typescript
export class Coder extends Agent<Env, CoderState> {
  // Initialize state with default values
  initialState: CoderState = {
    messages: [],
    githubToken: undefined,
    diagnostics: [],
  };
  
  // File operations
  async readFile(path: string): Promise<string> { /* ... */ }
  async writeFile(path: string, content: string): Promise<void> { /* ... */ }
  
  // Code analysis
  async analyzeCode(code: string): Promise<CodeAnalysis> { /* ... */ }
  
  // Main inference method
  async infer(token?: string): Promise<AIResponse> {
    // 1. Get current state
    const state = this.getState();
    
    // 2. Set up context with relevant information
    const context = {
      messages: state.messages,
      currentFile: state.currentFilePath 
        ? await this.readFile(state.currentFilePath) 
        : undefined,
      diagnostics: state.diagnostics
    };
    
    // 3. Generate response using LLM
    const response = await this.generateResponse(context);
    
    // 4. Update state with response
    this.addMessage({
      id: generateId(),
      role: 'assistant',
      content: response.content
    });
    
    return response;
  }
}
```

### 2. Solver Agent

A new Solver agent was implemented for GitHub/Linear issue resolution, with:

- Issue state management
- Implementation plan generation
- Status tracking for implementation steps
- GitHub/Linear API integration
- Code change suggestions and execution

Key methods include:

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
  
  // Issue management
  async setCurrentIssue(issue: Issue): Promise<void> {
    this.setState({ currentIssue: issue });
    
    // Add initial observations about the issue
    this.addObservation(`Issue #${issue.number}: ${issue.title}`);
    this.addObservation(`Description: ${issue.description}`);
    if (issue.labels && issue.labels.length > 0) {
      this.addObservation(`Labels: ${issue.labels.join(', ')}`);
    }
  }
  
  async setRepositoryContext(owner: string, repo: string, branch: string = 'main'): Promise<void> {
    this.setState({
      currentRepoOwner: owner,
      currentRepoName: repo,
      currentBranch: branch
    });
    
    this.addObservation(`Repository context set to ${owner}/${repo}:${branch}`);
  }
  
  // Implementation plan management
  async createImplementationPlan(steps: string[]): Promise<string[]> {
    const implementationSteps = steps.map((step, index) => ({
      id: generateId(),
      description: step,
      status: 'pending' as ImplementationStep['status'],
      notes: '',
      created: new Date(),
      updated: new Date()
    }));
    
    this.setState({ implementationSteps });
    return implementationSteps.map(step => step.id);
  }
  
  async updateStepStatus(
    stepId: string, 
    status: ImplementationStep['status'], 
    notes?: string
  ): Promise<void> {
    const state = this.getState();
    const steps = state.implementationSteps || [];
    
    const updatedSteps = steps.map(step => 
      step.id === stepId 
        ? { 
            ...step, 
            status, 
            notes: notes || step.notes, 
            updated: new Date() 
          } 
        : step
    );
    
    this.setState({ implementationSteps: updatedSteps });
    this.addObservation(`Updated step status: ${status} ${notes ? `(${notes})` : ''}`);
  }
  
  // Helper methods
  private addObservation(observation: string): void {
    const state = this.getState();
    this.setState({
      observations: [...(state.observations || []), observation]
    });
  }
  
  // Main inference method
  async infer(token?: string): Promise<AIResponse> {
    // 1. Get current state
    const state = this.getState();
    
    // 2. Set up context with issue information
    const context = {
      messages: state.messages,
      issue: state.currentIssue,
      repository: state.currentRepoOwner && state.currentRepoName 
        ? `${state.currentRepoOwner}/${state.currentRepoName}` 
        : undefined,
      implementationSteps: state.implementationSteps || [],
      observations: state.observations || [],
      scratchpad: state.scratchpad || ''
    };
    
    // 3. Generate response using LLM
    const response = await this.generateResponse(context);
    
    // 4. Update state with response
    this.addMessage({
      id: generateId(),
      role: 'assistant',
      content: response.content
    });
    
    return response;
  }
}
```

## UI Integration

### Connecting to Agents from the Client

The `useOpenAgent` hook provides a convenient way to connect to agents from React components:

```typescript
// Enhanced with support for both Coder and Solver agents with the official Cloudflare Agents SDK
export function useOpenAgent(id: string, type: AgentType = "coder"): OpenAgent {
  const [state, setAgentState] = useState<AgentState>({ messages: [] })

  const cloudflareAgent = useAgent({
    name: `${type}-${id}`,
    agent: type,
    onStateUpdate: (state: AgentState) => {
      // update local state
      setAgentState(state)
    }
  })

  const handleSubmit = (message: string) => {
    cloudflareAgent.setState({
      messages: [...(state?.messages || []), {
        id: generateId(),
        role: 'user',
        content: message,
        parts: [{
          type: 'text',
          text: message
        }]
      }]
    })
  }

  const infer = async (token: string) => {
    return await cloudflareAgent.call('infer', [token])
  }

  const setGithubToken = async (token: string): Promise<void> => {
    await cloudflareAgent.call('setGithubToken', [token])
    return
  }

  const getGithubToken = async (): Promise<string> => {
    const result = await cloudflareAgent.call('getGithubToken')
    return result as string
  }
  
  const setCurrentIssue = async (issue: any): Promise<void> => {
    if (type === 'solver') {
      await cloudflareAgent.call('setCurrentIssue', [issue])
    }
    return
  }

  return {
    state,
    messages: state?.messages || [],
    setMessages: (messages) => cloudflareAgent.setState({ messages }),
    handleSubmit,
    infer,
    setGithubToken,
    getGithubToken,
    ...(type === 'solver' ? { setCurrentIssue } : {})
  };
}
```

### SolverConnector Component

A `SolverConnector` component was created to integrate the Solver agent with the issues UI:

```typescript
export function SolverConnector({ issue, githubToken }: SolverConnectorProps) {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isStartingSolver, setIsStartingSolver] = useState(false);

  // Format issue object for the solver agent
  const formattedIssue = {
    id: issue.id,
    number: parseInt(issue.identifier.replace(/[^\d]/g, '')),
    title: issue.title,
    description: issue.description || "",
    source: "github",
    status: issue.status.type === 'done' ? 'closed' : 'open',
    labels: issue.labels?.map((label: any) => label.name) || [],
    assignee: issue.assignee?.name,
    created: new Date(issue.createdAt),
    updated: issue.updatedAt ? new Date(issue.updatedAt) : undefined
  };

  // Use the OpenAgent hook to connect to the Solver agent
  const agent = useOpenAgent(`solver-${issue.id}`, "solver");

  // Handle connection to the Solver agent
  const connectToSolver = async () => {
    if (!githubToken) {
      setErrorMessage("GitHub token is required. Please set it in your account settings.");
      setConnectionState('error');
      return;
    }

    setConnectionState('connecting');
    setIsStartingSolver(true);

    try {
      // Set GitHub token for the agent
      await agent.setGithubToken(githubToken);
      
      // Set the current issue context
      if (agent.setCurrentIssue) {
        await agent.setCurrentIssue(formattedIssue);
      }
      
      // Set up the issue context with the agent
      await agent.handleSubmit(`I need help with issue ${issue.identifier}: "${issue.title}". Please analyze this issue and suggest a plan to solve it.`);
      
      // Start inference on the agent
      await agent.infer(githubToken);
      
      setConnectionState('connected');
    } catch (err) {
      console.error("Error connecting to Solver agent:", err);
      setErrorMessage(err instanceof Error ? err.message : "Failed to connect to the Solver agent. Please try again later.");
      setConnectionState('error');
    } finally {
      setIsStartingSolver(false);
    }
  };
  
  // Check if the connect button should be disabled
  const isConnectButtonDisabled = isStartingSolver || !githubToken;

  // Disconnect from the Solver agent
  const disconnectFromSolver = () => {
    // Reset messages
    agent.setMessages([]);
    setConnectionState('disconnected');
  };

  // Render UI with connection state and controls
  return (
    <Card className="mb-6">
      {/* Card content with connection status and controls */}
    </Card>
  );
}
```

## Development Notes

### Deployment Fixes

Several issues were fixed to enable successful deployment of the agents to Cloudflare Workers:

1. **Removed `unstable_callable` decorator**: The original implementation was using a Cloudflare Workers-specific decorator `@unstable_callable` which was causing deployment errors. This decorator was removed from the `infer` method in the `Coder` class.

2. **Direct integration with Cloudflare Agents SDK**: The `useOpenAgent` hook was implemented to use the official Cloudflare Agents SDK's `useAgent` hook for seamless integration with Durable Objects while preserving a consistent interface for components.

3. **Added Solver to Durable Objects**: The Wrangler configuration was updated to include Solver in the Durable Objects bindings with a separate migration (tag: "v2"), ensuring proper versioning and state management for the new agent type.

4. **Simplified request routing**: Updated server.ts to directly route requests to the appropriate Durable Object based on the agent type in the path.

After these changes, the agents can be successfully deployed to Cloudflare Workers without encountering the `unstable_callable is not defined` error.

### Fixed React Hydration Mismatch

A React hydration mismatch was fixed in the `SolverConnector` component:

The issue was that the `disabled` attribute on the `Button` component was being rendered differently on the server versus the client:
- Server: `disabled=""` (attribute with empty string value)
- Client: `disabled={undefined}` (boolean attribute)

Our initial approach using a ternary expression (`disabled={condition ? true : undefined}`) wasn't sufficient. Instead, we implemented a more robust solution that completely avoids the `disabled` prop by using conditional rendering:

```typescript
// Before (problematic approach)
<Button 
  onClick={connectToSolver}
  disabled={isStartingSolver || !githubToken}
>
  Connect to Solver
</Button>

// After (robust solution)
// First extract the condition to a variable
const isConnectButtonDisabled = isStartingSolver || !githubToken;

// Then use conditional rendering to show different button variants
{isConnectButtonDisabled ? (
  <Button
    variant="secondary"
    className="opacity-50 cursor-not-allowed"
  >
    Connect to Solver
  </Button>
) : (
  <Button
    onClick={connectToSolver}
  >
    Connect to Solver
  </Button>
)}
```

This approach completely avoids the hydration mismatch by:
1. Not using the `disabled` attribute at all
2. Rendering different button variants based on the disabled state
3. Using visual styling to indicate the disabled state
4. Only attaching the `onClick` handler to the enabled button

This pattern is more reliable for SSR applications and ensures consistent rendering between server and client.

## Contributing New Agent Types

To add a new agent type to the architecture:

1. Create a new directory in `src/agents/` for your agent type (e.g., `src/agents/researcher/`)

2. Implement the required files:
   - `index.ts`: Main agent class extending the base `Agent` class
   - `types.ts`: Type definitions for agent state
   - `prompts.ts`: Agent-specific prompts
   - `tools.ts`: Agent-specific tools

3. Update the agent type definition in `useOpenAgent.ts`:
   ```typescript
   type AgentType = 'coder' | 'solver' | 'your-new-agent-type';
   ```

4. Add any agent-specific methods to the `OpenAgent` interface and implementation:
   ```typescript
   export type OpenAgent = {
     // Common methods
     state: AgentState;
     messages: UIMessage[];
     setMessages: (messages: UIMessage[]) => void;
     handleSubmit: (message: string) => void;
     infer: (token: string) => Promise<any>;
     setGithubToken: (token: string) => Promise<void>;
     getGithubToken: () => Promise<string>;
     
     // Agent-specific methods (conditionally included)
     setCurrentIssue?: (issue: any) => Promise<void>;
     yourNewAgentMethod?: (arg: any) => Promise<void>;
   }
   
   // Then in the return statement:
   return {
     // Common methods...
     
     // Conditionally include agent-specific methods
     ...(type === 'solver' ? { setCurrentIssue } : {}),
     ...(type === 'your-new-agent-type' ? { yourNewAgentMethod } : {})
   };
   ```

5. Add your agent to the config in `server.ts`:
   ```typescript
   const agentConfig = {
     agents: {
       'coder': Coder,
       'solver': Solver,
       'your-new-agent-type': YourNewAgentClass
     },
     cors: true
   };
   ```

6. Create UI components for interacting with your agent, modeled after the `SolverConnector` component

## Future Work and Roadmap

### Short-term Improvements

- Enhance Solver agent capabilities for deeper issue analysis
- Add support for creating and merging pull requests
- Improve error handling and recovery mechanisms
- Add unit tests for each agent type

### Medium-term Goals

- Implement additional agent types:
  - Researcher: For exploring and summarizing documentation/APIs
  - Writer: For generating documentation and comments
  - Reviewer: For reviewing and suggesting improvements to code
- Create a unified interface for managing multiple agent types
- Add support for custom tool composition at runtime
- Implement more granular access control

### Long-term Vision

- Develop a plugin system for extending agent capabilities
- Support dynamic loading of tools and prompts
- Create a visual builder for constructing specialized agents
- Implement multi-agent collaboration for complex tasks
- Add support for continuous learning from user feedback

## Best Practices

When working with the agent architecture, follow these best practices:

1. **State Management**
   - Keep agent state minimal and focused
   - Use immutable patterns when updating state
   - Don't store large artifacts in state; use external storage

2. **Tool Design**
   - Make tools atomic and focused on a single responsibility
   - Design tools to be composable and reusable
   - Implement proper error handling and validation

3. **Security**
   - Never store sensitive tokens directly in agent state
   - Use proper token validation and access control
   - Implement rate limiting for external API calls

4. **Testing**
   - Write unit tests for individual tools
   - Create integration tests for agent workflows
   - Use mocks for external dependencies

5. **UI Integration**
   - Maintain a clear separation between UI and agent logic
   - Implement graceful error handling and recovery
   - Use optimistic updates for better user experience