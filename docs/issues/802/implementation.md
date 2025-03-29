# Implementing Cloudflare Agents with CoderAgent

This document outlines the implementation steps for integrating the Cloudflare Agents SDK and creating a specialized CoderAgent for the OpenAgents platform.

## Overview

The implementation follows a service-oriented architecture where:

1. The `agents` package serves as a standalone Cloudflare Worker service
2. The `chatserver` app communicates with the agents service via service bindings
3. The `coder` desktop app communicates with the chatserver
4. The specialized `CoderAgent` provides coding assistance capabilities

## Implementation Components

### 1. Agents Service (packages/agents)

The agents service is a Cloudflare Worker that hosts multiple agent implementations:

- `Chat` - The standard chat agent (from the starter template)
- `CoderAgent` - Our specialized agent for coding tasks

The service is configured to run as a Worker with Durable Objects for state persistence.

#### Key Files

- `src/server.ts` - The main worker entry point that routes requests to the appropriate agent
- `src/coder-agent.ts` - Implementation of the CoderAgent class
- `src/coder-tools.ts` - Specialized tools for coding tasks
- `wrangler.jsonc` - Worker configuration

### 2. ChatServer Integration (apps/chatserver)

The chatserver is updated to communicate with the agents service using service bindings.

#### Key Changes

- Added service binding in `wrangler.jsonc`
- Created a new `/coder` endpoint that forwards requests to the CoderAgent
- Set up proper streaming of responses back to clients

### 3. Coder Agent Implementation

The CoderAgent is a specialized agent that provides coding assistance:

- Project context management (repository, branch, etc.)
- Code analysis and generation tools
- Repository management tools
- Command execution capabilities

#### Features

- **Project Context**: The agent maintains context about the current repository, branch, and path
- **GitHub Integration**: Connects with the existing MCP GitHub implementation
- **Code Operations**: File reading, searching, and creation
- **Command Execution**: Running shell commands with human approval
- **Repository Management**: PR creation and management

## Integration Flow

1. User makes a request from the coder desktop app
2. Request is sent to the chatserver's `/coder` endpoint
3. Chatserver forwards the request to the agents service using service binding
4. Agents service routes to the CoderAgent based on the path
5. CoderAgent processes the request, potentially using tools like GitHub integration
6. Response is streamed back through the chain to the user

## Tool Categories

The CoderAgent implements these tool categories:

1. **Repository Tools**
   - `getRepositoryInfo` - Gets basic repository information
   - `setProjectContext` - Sets the repository context for the agent

2. **File Operations**
   - `getFileContents` - Gets the contents of a file from a repository
   - `searchCode` - Searches for code in a repository
   - `createFile` - Creates a new file (requires human confirmation)

3. **Development Operations**
   - `runCommand` - Executes a shell command (requires human confirmation)
   - `createPullRequest` - Creates a pull request (requires human confirmation)

## MCP Integration

The tools are designed to integrate with our existing MCP (Model Context Protocol) implementation:

- Tools will use the MCP client to communicate with GitHub
- Authentication is passed through from the original request
- Tools can be extended to support other MCP services

## Human-in-the-Loop Approval

Some operations require human approval before execution:

1. `runCommand` - Executing shell commands
2. `createFile` - Creating or modifying files
3. `createPullRequest` - Creating pull requests

These tools implement the confirmation flow using the Agents SDK's tool confirmation mechanism.

## Configuration

The service is configured using `wrangler.jsonc`:

- Durable Objects for state persistence
- AI binding for accessing AI models
- Environment variables for configuration
- Smart placement for performance

## Next Steps

1. Clean up the agents package by removing frontend components
2. Set up service binding in the chatserver
3. Implement the actual MCP integration in the tools
4. Add comprehensive testing
5. Deploy the worker service