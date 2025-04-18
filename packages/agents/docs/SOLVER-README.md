# Solver Agent

This document explains the Solver agent, a specialized autonomous agent designed to analyze, plan, and implement solutions for OpenAgents Projects issues.

## Overview

Solver runs in the OpenAgents Projects dashboard and is implemented as a Cloudflare Durable Object using their [Agents SDK](https://developers.cloudflare.com/agents/api-reference/).

Basic data model for OA Projects:
- Teams have Projects
- Projects have Issues

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

3. **Message Handling**
   - Processes WebSocket messages for various operations
   - Supports system prompt requests
   - Handles context setting and updates
   - Manages observations and status updates
   - Processes inference requests using Llama 4

## Core Files

1. `packages/agents/src/agents/solver/index.ts`
   - Defines main Solver agent class
   - Implements WebSocket message handling
   - Manages state updates and context
   - Handles inference requests
   - Includes extensive logging for debugging

2. `packages/agents/src/common/open-agent.ts`
   - Base OpenAgent class that Solver extends
   - Provides common functionality for all agents
   - Manages repository context and GitHub integration
   - Implements shared inference using Cloudflare Workers AI
   - Handles base state management

3. `packages/agents/src/agents/solver/prompts.ts`
   - Generates system prompts for the Solver agent
   - Includes comprehensive context in prompts:
     - Current issue details
     - Project and team information
     - Implementation steps
     - Recent observations
     - Available tools
   - Adapts prompt based on temperature settings

4. `packages/agents/src/agents/solver/types.ts`
   - Defines SolverState interface
   - Extends BaseAgentState
   - Includes issue/project/team context
   - Defines implementation steps structure

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

## Implementation Details

- Uses Cloudflare Workers AI with Llama 4 for inference
- Maintains persistent state through Durable Objects
- Implements robust error handling and logging
- Supports temperature-based behavior adjustment
- Uses TypeScript for type safety
- Includes extensive debugging capabilities

## Additional Resources

- `packages/agents/docs/shared-inference-implementation.md`: Details on the inference method used by agents (may need updates)

## Usage

When messaging the agent through the issue chatbox, messages are appended to the agent's `messages` array as UIMessages from the AI SDK. The agent processes these through its WebSocket handler and can respond using various tools and capabilities.

The agent maintains context across interactions and can adapt its behavior based on the current state of the issue, project, and implementation progress.