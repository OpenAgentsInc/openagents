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

The frontend components handle rendering and user interaction:

- `apps/website/app/routes/issues/$id.tsx`: Main issue page
- `apps/website/app/components/agent/solver-connector.tsx`: Chat interface
- `apps/website/app/components/agent/solver-controls.tsx`: Sidebar controls

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

## Additional Resources

For more details on the inference implementation, see `packages/agents/docs/shared-inference-implementation.md`.