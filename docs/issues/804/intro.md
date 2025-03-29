# Issue 804: Extend useChat to Connect to Cloudflare Agents

## Understanding the Issue

Issue 804 is focused on enhancing the `useChat` hook to support Cloudflare Agents, particularly the CoderAgent. Currently, the `useChat` hook in the OpenAgents project is used for local chat interactions and command execution, but it doesn't support connecting to remote agents built with the Cloudflare Agents SDK.

The primary goal is to extend the existing `useChat` hook to:

1. Accept an agent ID/name parameter to identify which agent to connect to
2. Connect to the specified Cloudflare Agent using the Agents SDK client
3. Fetch and display initial chat history from the agent
4. Route chat interactions to the appropriate agent (vs local processing)
5. Support hybrid operation with both local command execution and agent-based functionality

This is a sub-task of the larger "Overnight Autonomous Coding Agent MVP" (issue #796), which aims to create an autonomous coding agent capable of completing pull requests without human intervention.

## Technical Analysis

After reviewing the codebase and the Cloudflare Agents SDK, I've identified the following key components:

1. **Current useChat Hook**: Defined in `packages/core/src/chat/useChat.ts`
   - Currently supports local chat interactions and command execution
   - Uses Vercel AI SDK's `useChat` under the hood
   - Has special handling for command execution within messages

2. **CoderAgent**: Defined in `packages/agents/src/coder-agent.ts`
   - Extends the Cloudflare `AIChatAgent` class
   - Has project context management
   - Uses AI to respond to coding-related requests
   - Can execute tools for various coding tasks

3. **Cloudflare Agents SDK**: Available via the `agents` npm package
   - Provides `AgentClient` for client-side connection to agents
   - Offers React hooks like `useAgent` and `useAgentChat` for agent integration
   - Supports state synchronization, WebSocket communication, and tool execution

## Implementation Plan

I'll extend the `useChat` hook to support Cloudflare Agents through the following steps:

1. **Update TypeScript interfaces**:
   - Extend `UseChatWithCommandsOptions` to include `agentId` and `agentOptions`
   - Add new types for agent-specific operations

2. **Implement agent connection logic**:
   - Add conditional logic to connect to specified agent if `agentId` is provided
   - Utilize the Cloudflare Agents SDK's `AgentClient` or `useAgent` hook
   - Set up WebSocket connection for real-time communication

3. **Add message handling**:
   - Fetch initial messages from the agent's history
   - Route new messages to the appropriate destination (local or agent)
   - Handle streaming responses from the agent

4. **Support tool execution**:
   - Enable execution of tools via the agent
   - Maintain compatibility with local command execution when needed

5. **Ensure state synchronization**:
   - Keep local and agent states synchronized
   - Provide clean fallback when agent connection is unavailable

## Dependencies and Requirements

This implementation depends on:
- Cloudflare Agents SDK (`agents` package)
- Vercel AI SDK (already in use)
- PartySocket library (used by Agents SDK for WebSocket communication)

The implementation must maintain backward compatibility with the existing `useChat` functionality while adding new capabilities for agent integration.

## Testing Approach

I'll test the implementation by:
1. Verifying connection to a Cloudflare Agent
2. Testing message sending and receiving
3. Ensuring history is properly synchronized
4. Validating tool execution through the agent
5. Confirming graceful fallback to local processing when needed

I will now await your review before proceeding with the actual implementation.