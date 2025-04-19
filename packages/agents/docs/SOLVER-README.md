# Solver Agent

This document explains the Solver agent, a specialized autonomous agent designed to analyze, plan, and implement solutions for OpenAgents Projects issues.

## Overview

Solver runs in the OpenAgents Projects dashboard and is implemented as a Cloudflare Durable Object using their [Agents SDK](https://developers.cloudflare.com/agents/api-reference/).

Basic data model for OA Projects:
- Teams have Projects
- Projects have Issues
- Issues have Implementation Steps and Comments

## Architecture

Each Solver is a unique instance with an ID format of `solver/{uuid-of-issue}`. There is one Solver per issue.

### Key Components

1. **Issue Page**
   - Located at `/issues/{uuid-of-issue}`
   - Features a chat interface with the agent
   - Shows complete action history with tools
   - Includes right sidebar with:
     - Connection status
     - Issue details
     - Agent controls

2. **State Management**
   - Extends BaseAgentState with solver-specific properties:
     - messages: Array of UIMessage objects for chat history
     - currentIssue: Current BaseIssue being worked on
     - currentProject: Project context information
     - currentTeam: Team context information
     - implementationSteps: Array of step objects
     - issueComments: Array of comment objects
   - Maintains messages array for chat history
   - Tracks current issue/project/team context
   - Includes implementation steps and observations
   - Uses scratchpad for agent's internal planning
   - Handles deep cloning of state objects to prevent reference issues
   - Maintains state update counter for debugging
   - Ensures proper serialization of non-primitive objects
   - Implements state recovery from message history
   - Validates state updates before applying changes

3. **Message Handling**
   - Processes WebSocket messages for various operations:
     - get_system_prompt: Retrieves current system prompt
     - set_context: Updates issue/project/team context
     - observation: Records agent observations
     - status_update: Handles status changes
     - command: Processes specific commands
     - shared_infer: Handles inference requests
   - Supports system prompt requests with context validation
   - Handles context setting and updates with validation
   - Manages observations and status updates
   - Processes inference requests using Llama 4
   - Includes extensive error handling and logging
   - Redacts sensitive information in logs
   - Implements context recovery from message history
   - Validates message data before processing
   - Maintains proper message ordering

## Core Files

1. `packages/agents/src/agents/solver/index.ts`
   - Defines main Solver agent class extending OpenAgent
   - Implements WebSocket message handling with type safety
   - Manages state updates and context with validation
   - Handles inference requests with context recovery
   - Includes extensive logging for debugging
   - Implements context recovery mechanisms
   - Handles message validation and error reporting
   - Manages assistant responses and message history
   - Uses AsyncLocalStorage for context management
   - Implements deep state cloning for stability
   - Provides type-safe state access methods
   - Includes comprehensive error handling
   - Supports temperature-based behavior adjustment
   - Maintains proper message ordering and state consistency

2. `packages/agents/src/common/open-agent.ts`
   - Base OpenAgent class that Solver extends
   - Provides common functionality for all agents
   - Manages repository context and GitHub integration
   - Implements shared inference using Cloudflare Workers AI
   - Handles base state management
   - Defines core agent capabilities
   - Implements shared tool handling
   - Provides base WebSocket handling

3. `packages/agents/src/agents/solver/prompts.ts`
   - Generates dynamic system prompts based on current state
   - Implements SystemPromptOptions interface for configuration:
     - state: SolverState for current context
     - model: Optional model information
     - temperature: Optional temperature setting (default 0.7)
   - Includes comprehensive context in prompts:
     - Current issue details (title, number, status, description)
     - Project information (name, ID, color, icon)
     - Team context (name, ID, key)
     - Implementation steps with status
     - Recent observations (last 3)
     - Available tools and descriptions
     - Working file context
   - Adapts prompt based on temperature settings:
     - Low temperature (<0.3): Focus on precision
     - High temperature (>0.7): More creative solutions
   - Includes usage guidelines and methodical approach
   - Handles missing context gracefully
   - Supports model-specific prompt adjustments
   - Implements proper error handling
   - Provides detailed logging for debugging
   - Maintains consistent prompt structure
   - Supports dynamic tool inclusion
   - Includes extensive console logging for debugging
   - Properly formats multi-line prompt sections
   - Handles undefined/null values safely
   - Supports rich context recovery

4. `packages/agents/src/agents/solver/types.ts`
   - Defines SolverState interface extending BaseAgentState:
     - messages: UIMessage[] for chat history
     - currentIssue?: BaseIssue for active issue
     - currentProject?: BaseProject for project context
     - currentTeam?: BaseTeam for team context
     - implementationSteps?: ImplementationStep[] for tracking progress
     - issueComments?: IssueComment[] for discussion history
   - Defines SolverIssue interface extending BaseIssue
   - Ensures type safety throughout the agent
   - Supports proper TypeScript inference
   - Enables IDE autocompletion
   - Facilitates code maintenance
   - Imports core types from @openagents/core
   - Uses optional properties for flexibility
   - Maintains strict type checking
   - Enables proper state management

5. `packages/agents/src/agents/solver/tools.ts`
   - Defines solver-specific tools using the AI SDK:
     - getIssueDetails: Fetches issue information from OpenAgents/GitHub
     - updateIssueStatus: Updates issue status with optional comments
     - createImplementationPlan: Generates step-by-step solution plans
   - Implements comprehensive error handling
   - Supports both OpenAgents and GitHub issues
   - Uses Zod for parameter validation
   - Maintains proper TypeScript types
   - Handles GitHub API integration
   - Supports status updates with comments
   - Implements proper state management
   - Uses solverContext for agent access
   - Provides detailed error messages
   - Supports async/await operations
   - Includes extensive logging
   - Handles API rate limiting
   - Maintains proper security practices
   - Supports proper error recovery

6. `packages/agents/src/common/tools/index.ts`
   - Defines tools common to all OpenAgents
   - Implements shared functionality
   - Provides base tool types

7. `packages/agents/src/common/types.ts`
   - Defines BaseAgentState shared by all agents
   - Implements core type definitions
   - Ensures consistency across agents

8. Web Interface Components:
   - `apps/website/app/routes/issues/$id.tsx`: Issue page
   - `apps/website/app/components/agent/solver-connector.tsx`: Chat interface
   - `apps/website/app/components/agent/solver-controls.tsx`: Sidebar controls

## Agent Capabilities

The Solver agent is designed to:
1. Analyze issue descriptions and requirements
2. Plan implementation steps methodically
3. Research existing codebase context
4. Implement solutions through code modifications
5. Test changes for issue resolution
6. Document solutions and reasoning
7. Update issue status and progress
8. Maintain context across interactions
9. Recover from context loss
10. Handle temperature-based behavior adjustments

## Implementation Details

- Uses Cloudflare Workers AI with Llama 4 for inference
- Maintains persistent state through Durable Objects
- Implements robust error handling and logging
- Supports temperature-based behavior adjustment:
  - Low temperature (<0.3): Focus on precision and correctness
  - High temperature (>0.7): More creative and exploratory solutions
- Uses TypeScript for type safety
- Includes extensive debugging capabilities
- Implements deep state cloning for stability
- Handles WebSocket communication securely
- Maintains message history with proper typing
- Supports context recovery from message history
- Implements proper state serialization
- Validates all state updates
- Handles race conditions in state updates
- Provides comprehensive logging
- Supports proper error reporting

## Additional Resources

- `packages/agents/docs/shared-inference-implementation.md`: Details on the inference method used by agents (may need updates)

## Usage

When messaging the agent through the issue chatbox, messages are appended to the agent's `messages` array as UIMessages from the AI SDK. The agent processes these through its WebSocket handler and can respond using various tools and capabilities.

The agent maintains context across interactions and can adapt its behavior based on:
- Current state of the issue
- Project and team context
- Implementation progress
- Temperature settings
- Available tools
- Recent observations
- Working file context
- Message history for recovery

### Guidelines for Interaction

1. The agent follows a methodical approach to issue resolution:
   - Understand requirements
   - Plan implementation
   - Research context
   - Implement solution
   - Test changes
   - Document reasoning

2. All changes are documented with:
   - Clear explanations
   - Implementation decisions
   - Status updates
   - Progress tracking
   - File modifications
   - Context information

3. The agent can recover context from message history if state is lost:
   - Searches message history for context
   - Rebuilds state from found context
   - Validates recovered state
   - Maintains operation continuity

4. Temperature settings influence behavior:
   - Lower (<0.3): More precise and careful
   - Higher (>0.7): More creative while maintaining correctness
   - Adapts prompts based on temperature
   - Maintains consistency in approach