# Solver Agent

This document explains the Solver agent, a specialized autonomous agent designed to analyze, plan, and implement solutions for OpenAgents Projects issues.

## Overview

Solver runs in the OpenAgents Projects dashboard and is implemented as a Cloudflare Durable Object using their [Agents SDK](https://developers.cloudflare.com/agents/api-reference/). The agent interacts with users through a chat interface and works within the OpenAgents Projects data model:

- Teams have Projects
- Projects have Issues
- Issues have Implementation Steps and Comments

Each Solver is a unique instance with an ID format of `solver/{uuid-of-issue}`, with one Solver per issue.

## Architecture

The Solver architecture consists of several key components working together:

### Issue Page Interface

The main user interface is located at `/issues/{uuid-of-issue}` and provides:

- A chat interface for communicating with the agent
- Complete action history with tool usage
- Right sidebar with connection status, issue details, and agent controls

### State Management

The Solver extends BaseAgentState with specialized properties to track and maintain issue context:

```typescript
export interface SolverState extends BaseAgentState {
  messages: UIMessage[]; 
  currentIssue?: BaseIssue;
  currentProject?: BaseProject;
  currentTeam?: BaseTeam;
  implementationSteps?: ImplementationStep[];
  issueComments?: IssueComment[];
}
```

The state management system handles:

- Message history persistence
- Issue/project/team context tracking
- Implementation step progress
- Deep cloning of state objects to prevent reference issues
- State recovery from message history when context is lost
- Race condition handling in concurrent updates

### Message Handling

The WebSocket message handler processes various operation types:

- `get_system_prompt`: Retrieves the current system prompt with context
- `set_context`: Updates the issue/project/team context
- `observation`: Records agent observations during operation
- `status_update`: Handles status change notifications
- `command`: Processes specific agent commands
- `shared_infer`: Handles inference requests using Llama 4

The handler includes robust security features like redacting sensitive information in logs and validation of message data before processing.

## Core Files

The Solver implementation is distributed across several key files:

### Main Agent Implementation

`packages/agents/src/agents/solver/index.ts` defines the main Solver class that extends OpenAgent and implements:

- WebSocket message handling with type safety
- Context management and recovery mechanisms
- Inference request processing
- State serialization and deep cloning
- Comprehensive error handling and logging

### Base Agent Functionality

`packages/agents/src/common/open-agent.ts` provides the foundation for all agents in the system:

- Common functionality shared across different agent types
- Repository context and GitHub integration
- Shared inference using Cloudflare Workers AI
- Base WebSocket handling and state management
- Secure token management

### Dynamic Prompt Generation

`packages/agents/src/agents/solver/prompts.ts` handles system prompt generation:

- Creates dynamic prompts based on current state and context
- Configures behavior through temperature settings
- Incorporates comprehensive context including issue details, project information, team context, and implementation status
- Adapts behavior based on temperature:
  - Low temperature (<0.3): Focuses on precision and correctness
  - High temperature (>0.7): Provides more creative solutions

### Type Definitions

`packages/agents/src/agents/solver/types.ts` provides TypeScript interfaces:

- SolverState extending BaseAgentState with solver-specific properties
- SolverIssue extending BaseIssue for specialized functionality
- Type safety throughout the implementation

### Tool Implementation

`packages/agents/src/agents/solver/tools.ts` defines specialized tools:

- `getIssueDetails`: Fetches comprehensive issue information
- `updateIssueStatus`: Updates issue status with optional comments
- `createImplementationPlan`: Generates step-by-step solution plans

### Web Interface Components

The Solver agent's frontend interface is built from three main components that work together to provide a seamless user experience:

#### 1. Main Issue Page (`apps/website/app/routes/issues/$id.tsx`)

This component serves as the container for the entire issue view and is responsible for:

- Loading issue data from the database using the `loader` function
- Initializing the shared agent instance via the `useOpenAgent` hook
- Managing the overall layout with a two-column design
- Setting up the issue context and GitHub token handling
- Handling issue status updates and other metadata changes
- Synchronizing the Solver agent's state when the page loads

The main page automatically retrieves the GitHub token from local storage and passes it to child components, ensuring secure token handling without exposing credentials in the UI state.

```typescript
// Example of agent initialization in the issue page
const agent = useOpenAgent(issue.id, "solver");

// Passing the agent instance to child components
<SolverConnector
  issue={issue}
  agent={agent}
  githubToken={getGithubToken()}
  className="w-full h-full"
/>
```

#### 2. Chat Interface (`apps/website/app/components/agent/solver-connector.tsx`)

The chat interface handles direct user interaction with the agent and:

- Displays message history with proper formatting
- Provides a message input for user queries
- Handles message submission and response rendering
- Manages auto-scrolling behavior for new messages
- Shows appropriate UI states (connecting, disconnected, error)
- Implements context recovery if the agent loses state

The component includes special handling for agent communication:

```typescript
// Example of message handling in the chat interface
const handleSendMessage = async (message) => {
  // Add message to UI immediately
  agent.setMessages([...agent.messages, userMessage]);
  
  // Ensure context is available before sending
  if (contextMissing) {
    const contextMessage = {
      type: "set_context",
      issue: formattedIssue,
      project: formattedProject,
      team: formattedTeam
    };
    agent.sendRawMessage(contextMessage);
    await new Promise(resolve => setTimeout(resolve, 300));
  }
  
  // Send the inference request
  await agent.sendRawMessage({
    type: "shared_infer",
    params: { /* message parameters */ },
    context: { issue, project, team }
  });
};
```

#### 3. Sidebar Controls (`apps/website/app/components/agent/solver-controls.tsx`)

The sidebar controls provide user management of the agent and:

- Display connection status with visual indicators
- Offer connection/disconnection buttons
- Provide debugging tools for agent operations
- Allow viewing the system prompt
- Include testing buttons for agent features
- Handle error states and retries
- Show contextual information based on agent state

The component implements several agent control functions:

```typescript
// Example of agent connection in the controls component
const connectToSolver = async () => {
  setConnectionState('connecting');
  
  // First set the GitHub token
  await agent.setGithubToken(githubToken);
  
  // Set repository context
  await agent.setRepositoryContext(owner, repo, branch);
  
  // Send context message with issue data
  agent.sendRawMessage({
    type: "set_context",
    issue: formattedIssue,
    project: formattedProject,
    team: formattedTeam
  });
  
  // Send initial prompt and start inference
  await agent.handleSubmit(initialPrompt);
  await agent.infer(githubToken);
  
  setConnectionState('connected');
};
```

These components work together to create a cohesive user experience, with the parent issue page coordinating the agent instance that is shared between the chat interface and control sidebar.

## Agent Capabilities

The Solver agent provides a comprehensive set of capabilities for issue resolution:

1. **Analysis and Planning**
   - Analyzes issue descriptions and requirements
   - Plans implementation steps methodically
   - Researches existing codebase context

2. **Implementation**
   - Implements solutions through code modifications
   - Tests changes for issue resolution
   - Documents solutions and reasoning

3. **Progress Management**
   - Updates issue status and progress
   - Maintains context across interactions
   - Recovers from context loss

4. **Technical Robustness**
   - Handles temperature-based behavior adjustments
   - Implements proper error recovery
   - Maintains security best practices
   - Handles race conditions effectively
   - Provides comprehensive logging

## Implementation Details

The Solver uses several advanced techniques:

- **Inference**: Cloudflare Workers AI with Llama 4
- **State**: Persistent state through Durable Objects
- **TypeScript**: Strong typing for code quality and maintainability
- **WebSockets**: Real-time communication with clients
- **State Cloning**: Deep cloning to prevent reference issues
- **Context Recovery**: Rebuilding state from message history when needed
- **Security**: Proper token handling and sensitive data redaction

## Usage

When using the Solver agent through the issue chatbox:

1. Messages are appended to the agent's `messages` array as UIMessages
2. The agent processes these through its WebSocket handler
3. Responses are generated with appropriate context and tools

The agent maintains context across interactions and adapts behavior based on:
- Current issue state
- Project and team context
- Implementation progress
- Temperature settings
- Available tools
- Recent observations

### Interaction Guidelines

The Solver follows a methodical approach to issue resolution:

1. **Understanding Phase**
   - Analyzes issue requirements
   - Researches context and background
   - Plans implementation approach

2. **Implementation Phase**
   - Develops solution step-by-step
   - Tests changes thoroughly
   - Documents reasoning and decisions

3. **Context Recovery**
   - If state is lost, the agent can rebuild context from message history
   - Validates recovered state for consistency
   - Maintains continuity of operation

4. **Temperature Adaptation**
   - Lower temperature (<0.3): More precise and careful analysis
   - Higher temperature (>0.7): More creative problem-solving
   - Consistent validation regardless of temperature setting

## Tool Architecture and GitHub Token Handling

The Solver agent leverages a robust tool architecture that spans both base functionality and solver-specific capabilities. Tools are implemented using the Vercel AI SDK's `tool()` function, which provides type-safe parameter validation using Zod schemas.

### Tool Hierarchy

Tools are organized into two main categories:

1. **Base Agent Tools** (`packages/agents/src/common/tools/index.ts`)
   - Scheduling tools: `scheduleTask`, `listSystemSchedules`, `deleteSystemSchedule`
   - Repository context: `setRepositoryContext`
   - Utility tools: `getLocalTime`, `getWeatherInformation`

   These tools are available to all agent types and handle cross-cutting concerns that aren't specific to any particular agent use case.

2. **Solver-Specific Tools** (`packages/agents/src/agents/solver/tools.ts`)
   - Issue management: `getIssueDetails`, `updateIssueStatus`
   - Planning: `createImplementationPlan`

   These tools implement functionality specific to the Solver agent's role in managing issues and implementing solutions.

### GitHub Token Flow

The secure handling of GitHub tokens is a critical aspect of the Solver agent architecture. Here's how GitHub token authentication works:

1. **Token Origin**
   - The GitHub token originates from the user through the web interface
   - It's sent to the agent via a secure WebSocket connection when establishing a connection

2. **Token Storage**
   - The token is stored in the agent's state using the `setGithubToken` method
   - It's kept in the `githubToken` property of the `BaseAgentState` interface
   - This state is maintained by Cloudflare's Durable Object storage mechanism
   - The token is never persisted to disk or any external storage system
   - State is specific to the individual agent instance (one per issue)

3. **Token Usage in Tools**
   - When a GitHub API tool is invoked, it retrieves the token from the agent's state:
     ```typescript
     const agent = solverContext.getStore();
     const token = agent.state.githubToken;
     ```
   - The token is used in API requests with proper authorization headers:
     ```typescript
     const response = await fetch(url, {
       headers: {
         'Authorization': `Bearer ${token}`,
         'Accept': 'application/vnd.github.v3+json',
         'User-Agent': 'OpenAgents'
       }
     });
     ```
   - All token usage is scoped to the specific operations required by the tool

4. **Security Measures**
   - Tokens are redacted in all logs to prevent exposure:
     ```typescript
     const safeMessageForLogging = { ...parsedMessage };
     if (safeMessageForLogging.githubToken) {
       safeMessageForLogging.githubToken = "[REDACTED]";
     }
     ```
   - WebSocket connections use TLS encryption for secure transit
   - Tokens are never exposed to other agents or users
   - Token validation happens at the time of API requests
   - No token persistence beyond the agent's runtime state

5. **Token Lifecycle**
   - Tokens live only for the duration of the agent session
   - When an agent instance is stopped or evicted from memory, the token is discarded
   - New connections require re-authentication with a fresh token
   - No automatic token refresh is implemented; expired tokens must be replaced manually

The token flow ensures that GitHub credentials are securely handled while enabling the agent to perform authenticated operations against the GitHub API on behalf of the user.

## Development and Architecture Details

This section provides additional technical details for developers working with the Solver agent.

### OpenAgents Projects Ecosystem

The Solver agent is part of the broader OpenAgents Projects ecosystem, which includes:

- **Web Interface**: The primary user-facing application at `apps/website/`
- **Core Package**: Shared types and utilities at `packages/core/`
- **Agents Package**: This package, containing the implementation of all agents
- **Chat Server**: For handling more general chat interactions

Solver is one of several agent types, each designed for specific purposes:

1. **Solver Agent**: For analyzing and implementing solutions to project issues
2. **PR Reviewer Agent**: For code review (planned)
3. **Document Agent**: For knowledge management (planned)

These agents don't directly interact with each other, but share the same UI components, core functionality, and infrastructure.

### Durable Object Lifecycle

Solver agents are implemented as Cloudflare Durable Objects with these characteristics:

- **Lifetime**: Instances persist as long as they're actively used
- **Eviction**: After a period of inactivity (typically 24-72 hours), Cloudflare may evict the DO
- **State Persistence**: State stored in the DO survives until eviction
- **Context Loss**: Occurs primarily during code deployments or when instances are evicted and recreated

Context recovery is handled by:
1. Storing rich context in message history
2. Implementing logic to extract context from historical messages
3. Reconstructing state when context is detected as missing

In practice, context loss is relatively infrequent but can happen during deployments or after extended periods of inactivity.

### State Recovery Details

When context loss is detected (usually when key state properties are missing), the agent:

1. Searches the message history for context indicators like "issue" and "context"
2. Extracts structured information about the issue, project, and team
3. Rebuilds the state object with the extracted information
4. Validates the reconstructed state for consistency
5. Uses the recovered state for the current operation

This process happens automatically and is triggered by missing context in critical operations like inference or system prompt generation.

### Cloudflare Workers AI Configuration

The Solver agent uses Cloudflare Workers AI with these specifics:

- **Default Model**: `@cf/meta/llama-4-scout-17b-16e-instruct`
- **Context Window**: 32K tokens
- **Cost**: Free tier through Cloudflare Workers
- **Typical Latency**: 2-5 seconds for responses (varies by message complexity)
- **Temperature Control**: Set by the caller when needed, default 0.7
  - Can be dynamically adjusted per operation for different behavior characteristics

### Tool Extension

To add a new tool to the Solver agent:

1. Define the tool using the Vercel AI SDK's `tool()` function:
   ```typescript
   export const myNewTool = tool({
     description: "Description of what the tool does",
     parameters: z.object({
       param1: z.string().describe("Parameter description"),
       param2: z.number().optional().describe("Optional parameter")
     }),
     execute: async ({ param1, param2 }) => {
       // Implementation
       return { result: "Success", data: {...} };
     }
   });
   ```

2. Add the tool to the exported tools in either:
   - `packages/agents/src/common/tools/index.ts` for base tools
   - `packages/agents/src/agents/solver/tools.ts` for solver-specific tools

3. Reference the new tool in prompt generation if needed:
   ```typescript
   // In prompts.ts
   `You have access to the following tools:
   - existingTool: Description
   - myNewTool: Description of what the tool does`
   ```

All tools have access to the agent context through the `solverContext` AsyncLocalStorage instance.

### GitHub Token Scope Requirements

For the Solver agent to function properly with GitHub, the token provided by the user needs these scopes:

- `repo`: Full control of private repositories
  - `repo:status`: Access commit status
  - `repo_deployment`: Access deployment status
  - `public_repo`: Access public repositories
  - `repo:invite`: Access repository invitations

For issue-only operations (no code changes), a more limited scope is possible:
- `repo`: (Limited to issues, pull requests)
  - `read:packages`: Read packages
  - `read:user`: Read user information

The token is used for:
1. Fetching issue details from GitHub repositories
2. Updating issue statuses
3. Adding comments to issues
4. Accessing repository content for code context
5. (If configured) Making code changes via Pull Requests

### Development Environment

To develop and test the Solver agent locally:

1. **Setup Cloudflare Environment**:
   - Clone the repository
   - Install dependencies with `npm install`
   - Set up Cloudflare Wrangler with `npm i -g wrangler`
   - Login to Cloudflare with `wrangler login`

2. **Local Development**:
   - Start the development server with `wrangler dev`
   - Run the website with `npm run dev`
   - Connect to the local agent instance using the development tools

3. **Testing WebSocket Communication**:
   - Use the browser's developer tools to monitor WebSocket traffic
   - Develop against the mock agent in development mode before deploying

4. **Deployment**:
   - Publish to Cloudflare with `wrangler publish`
   - Note that deploying new code causes existing DO instances to restart, potentially losing in-memory state

### Monitoring and Debugging

Solver has extensive logging:

- **Console Logs**: Detailed debug information is sent to Cloudflare logs
- **State Snapshots**: Critical state is logged at key decision points
- **Error Tracking**: All errors are logged with stack traces
- **Message Tracing**: Agent messages include timestamps and request IDs

All GitHub tokens and sensitive information are redacted from logs automatically.

For local debugging, use:
- Browser developer tools to inspect WebSocket traffic
- Cloudflare dashboard for Worker logs
- `wrangler tail` for real-time log streaming

## Shared Inference System

The Solver agent uses a shared inference system implemented in the base `OpenAgent` class. This system provides a unified way for all agents to access AI models through Cloudflare Workers AI.

### Implementation Details

The shared inference system is implemented in the `sharedInfer` method of the `OpenAgent` class:

```typescript
async sharedInfer(props: InferProps): Promise<InferResponse> {
  // Default to Llama 4 Scout if no model specified
  const { 
    model = "@cf/meta/llama-4-scout-17b-16e-instruct", 
    messages, 
    system, 
    temperature = 0.7, 
    max_tokens = 1024, 
    top_p = 0.95 
  } = props;
  
  // Format conversation for LLM input
  let formattedMessages = [];
  
  // Use provided system prompt or generate one from agent state
  let systemPrompt = system || this.getSystemPrompt();
  formattedMessages.push({ role: "system", content: systemPrompt });
  
  // Add user and assistant messages
  messages.forEach(msg => {
    formattedMessages.push({ role: msg.role, content: msg.content });
  });
  
  // Call the Cloudflare Workers AI API
  const result = await this.env.AI.run(model, {
    messages: formattedMessages,
    temperature,
    max_tokens,
    top_p
  });
  
  // Process and return the response
  return {
    id: generateId(),
    content: extractResponseText(result),
    role: "assistant",
    timestamp: new Date().toISOString(),
    model: model
  };
}
```

### Key Features

1. **Model Access**: Provides access to Cloudflare's AI models, defaulting to Llama 4 Scout
2. **Dynamic System Prompts**: Automatically generates system prompts based on agent state
3. **Flexible Parameters**: Supports model-specific parameters like temperature and max tokens
4. **Context Awareness**: Includes agent context in system prompts
5. **Consistent Response Format**: Standardizes response format across different models
6. **Error Handling**: Gracefully handles inference errors with informative messages

### Usage in Solver Agent

The Solver agent uses the shared inference system in several ways:

1. **Initial Analysis**: When first connected to analyze issues
2. **User Interactions**: To respond to user messages in the chat interface
3. **Tool Integration**: Some tools may use inference for specific tasks
4. **System Prompt Generation**: To create dynamic prompts based on current context

When the user sends a message through the chat interface, this triggers a message flow that:

1. Formats the conversation history and user message
2. Ensures context is available for the system prompt
3. Calls `sharedInfer` with appropriate parameters
4. Processes the response and adds it to the message history
5. Displays the response to the user

### Advanced Configuration

For specialized use cases, the inference system can be configured with:

- Different model selections (e.g., smaller models for faster responses)
- Custom temperature settings (lower for precise answers, higher for creative ones)
- Token limits to control response length
- Custom system prompts for specific behaviors

For more implementation details, see the full documentation in `packages/agents/docs/shared-inference-implementation.md`.

## Additional Resources

- [Cloudflare Workers AI Documentation](https://developers.cloudflare.com/workers-ai/)
- [Llama 4 Model Information](https://ai.meta.com/llama/)