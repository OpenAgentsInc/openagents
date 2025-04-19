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
   - Extends BaseAgentState with solver-specific properties
   - Maintains messages array for chat history
   - Tracks current issue/project/team context
   - Includes implementation steps and observations
   - Uses scratchpad for agent's internal planning
   - Handles deep cloning of state objects to prevent reference issues
   - Maintains state update counter for debugging
   - Ensures proper serialization of non-primitive objects

3. **Message Handling**
   - Processes WebSocket messages for various operations:
     - get_system_prompt: Retrieves current system prompt
     - set_context: Updates issue/project/team context
     - observation: Records agent observations
     - status_update: Handles status changes
     - command: Processes specific commands
     - shared_infer: Handles inference requests
   - Supports system prompt requests
   - Handles context setting and updates
   - Manages observations and status updates
   - Processes inference requests using Llama 4
   - Includes extensive error handling and logging
   - Redacts sensitive information in logs

## Core Files

1. `packages/agents/src/agents/solver/index.ts`
   - Defines main Solver agent class
   - Implements WebSocket message handling
   - Manages state updates and context
   - Handles inference requests
   - Includes extensive logging for debugging
   - Implements context recovery mechanisms
   - Handles message validation and error reporting
   - Manages assistant responses and message history

2. `packages/agents/src/common/open-agent.ts`
   - Base OpenAgent class that Solver extends
   - Provides common functionality for all agents
   - Manages repository context and GitHub integration
   - Implements shared inference using Cloudflare Workers AI
   - Handles base state management

3. `packages/agents/src/agents/solver/prompts.ts`
   - Generates system prompts for the Solver agent
   - Includes comprehensive context in prompts:
     - Current issue details (title, number, status, description)
     - Project information (name, ID, color, icon)
     - Team context (name, ID, key)
     - Implementation steps with status
     - Recent observations (last 3)
     - Available tools and descriptions
     - Working file context
   - Adapts prompt based on temperature settings
   - Includes usage guidelines and methodical approach
   - Handles missing context gracefully

4. `packages/agents/src/agents/solver/types.ts`
   - Defines SolverState interface extending BaseAgentState
   - Includes typed properties:
     - messages: UIMessage[]
     - currentIssue?: BaseIssue
     - currentProject?: BaseProject
     - currentTeam?: BaseTeam
     - implementationSteps?: ImplementationStep[]
     - issueComments?: IssueComment[]
   - Defines SolverIssue interface for future extensions

5. `packages/agents/src/agents/solver/tools.ts`
   - Defines tools specific to Solver agent
   - Complements common tools from `src/common/tools`

6. `packages/agents/src/common/tools/index.ts`
   - Defines tools common to all OpenAgents

7. `packages/agents/src/common/types.ts`
   - Defines BaseAgentState shared by all agents

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

3. The agent can recover context from message history if state is lost

4. Temperature settings influence behavior:
   - Lower: More precise and careful
   - Higher: More creative while maintaining correctness