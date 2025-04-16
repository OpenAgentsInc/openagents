# Agent Codebase Refactoring

**Date:** April 16, 2025  
**Author:** Claude Code  

## Overview

This document outlines the refactoring of the OpenAgents codebase to support multiple agent types through a modular architecture. The primary goal was to transform the monolithic `server.ts` file (which contained the entire Coder agent implementation) into a structured, maintainable codebase that can support both `Coder` and new `Solver` agent types.

## Motivation

The original codebase had several challenges:

1. **Monolithic Structure**: All agent code was contained in a single large file (`server.ts`), making it difficult to maintain and extend.
2. **Limited Support for Multiple Agents**: The architecture was designed around a single agent type (Coder).
3. **Code Reuse Issues**: Common utilities and tools were mixed with agent-specific logic.
4. **Difficult Extension Path**: Adding new agent types would require significant duplication of code.

## Solution Architecture

The refactoring involved creating a structured directory hierarchy and separating concerns:

```
src/
│
├── agents/              # Agent-specific implementations
│   ├── coder/           # Coder agent files
│   │   ├── index.ts     # Main Coder class implementation
│   │   ├── prompts.ts   # Coder-specific prompts
│   │   ├── schemas.ts   # Coder-specific Zod schemas
│   │   └── types.ts     # Coder-specific types
│   │
│   └── solver/          # Solver agent files
│       ├── index.ts     # Main Solver class implementation
│       ├── prompts.ts   # Solver-specific prompts
│       ├── tools.ts     # Solver-specific tools
│       └── types.ts     # Solver-specific types
│
├── common/              # Shared resources
│   ├── config.ts        # Common configuration (models, etc.)
│   ├── types.ts         # Shared type definitions
│   └── tools/           # Shared tools
│       ├── index.ts     # Common tool definitions
│       └── github/      # GitHub-specific tools
│           ├── getFileContents.ts
│           └── addIssueComment.ts
│
├── server.ts            # Main entry point for routing requests to agents
└── index.ts             # Package exports
```

## Implemented Changes

### 1. Directory Structure Creation

Created the new directory structure for organizing agent-specific and shared code:

```bash
mkdir -p /Users/christopherdavid/code/openagents/packages/agents/src/agents/coder
mkdir -p /Users/christopherdavid/code/openagents/packages/agents/src/agents/solver
mkdir -p /Users/christopherdavid/code/openagents/packages/agents/src/common/tools
```

### 2. Type Definitions

- **Agent-Specific Types**: Created separate type definitions for each agent
  - `agents/coder/types.ts`: Moved `CoderState`, `FileNode`, `Task` interfaces from original `types.ts`
  - `agents/solver/types.ts`: Added new `SolverState`, `Problem`, `SolutionStep` interfaces
- **Common Types**: Added `common/types.ts` for shared type definitions

### 3. Agent Implementations

- **Coder Agent**: Moved the entire Coder class from `server.ts` to `agents/coder/index.ts`
- **Solver Agent**: Created a new Solver agent implementation in `agents/solver/index.ts` with similar structure but adapted for mathematical/logical problem solving

### 4. Prompt Generation

- **Coder Prompts**: Moved the `getSystemPrompt` function to `agents/coder/prompts.ts`
- **Solver Prompts**: Created new `getSolverSystemPrompt` function in `agents/solver/prompts.ts` tailored for problem-solving

### 5. Schemas

- **Coder Schemas**: Extracted Zod schemas to `agents/coder/schemas.ts`
  - `PlanningSchema`: For structured thoughts
  - `FileSummarySchema`: For file content summarization
  - `NewTaskSchema`: For task generation

### 6. Tools Organization

- **Common Tools**: Moved shared tools to `common/tools/index.ts`
  - Basic utilities like `getWeatherInformation`, `getLocalTime`
  - General tools like `scheduleTask`, `listSystemSchedules`, `deleteSystemSchedule`
- **GitHub Tools**: Extracted GitHub-specific tools to dedicated files
  - `getFileContentsTool`: `common/tools/github/getFileContents.ts`
  - `addIssueCommentTool`: `common/tools/github/addIssueComment.ts`
- **Solver-Specific Tools**: Created new tools for the Solver agent in `agents/solver/tools.ts`
  - `evaluateExpression`: For evaluating mathematical expressions
  - `verifyProof`: For verifying mathematical/logical proofs

### 7. Configuration

- **Model Configuration**: Moved model setup to `common/config.ts`
  - `openrouter`: OpenRouter client initialization
  - `model`: Main model for complex tasks
  - `smallModel`: Smaller model for structured generation

### 8. Entry Point Refactoring

- **Server Entry Point**: Simplified `server.ts` to focus on agent routing
  - Configured `routeAgentRequest` with a map of agent types
  - Added support for both Coder and Solver agent types
- **Package Exports**: Created `index.ts` to expose both agents and server

## Key Design Decisions

1. **Agent Context Separation**: Each agent type has its own `AsyncLocalStorage` instance to maintain independent contexts
2. **Tool Composition**: Tools are composed at runtime based on agent type and needs
3. **Shared Configuration**: Models and shared configuration are centralized
4. **TypeScript Interface Segregation**: Created specific interfaces for each agent's state
5. **Modular Prompt Generation**: Each agent has its own prompt generation function tailored to its purpose

## Solver Agent Implementation

The new Solver agent was implemented with the following characteristics:

1. **Purpose**: Focus on mathematical, logical, and analytical problem-solving
2. **State Model**: Uses `SolverState` to track problems, steps, and solutions
3. **Tools**: Dedicated tools for evaluating expressions and verifying proofs
4. **Prompt Design**: System prompt emphasizes thorough analysis, showing work, and verification

## Next Steps

While the refactoring establishes the foundation for multiple agent types, there are several areas for improvement:

1. **TypeScript Error Resolution**: Address TypeScript errors identified during type checking
2. **Testing**: Develop unit and integration tests for both agent types
3. **Enhanced Tool Integration**: Further refine the tool interfaces for consistency
4. **Documentation**: Add JSDoc comments to all exported components
5. **UI Integration**: Update frontend components to support the selection of different agent types
6. **Support for Additional Agent Types**: Use this architecture to implement other specialized agents

## Conclusion

The refactored codebase now provides a solid foundation for supporting multiple agent types within the OpenAgents platform. By separating concerns and enforcing modularity, the architecture allows for easier maintenance and extension with new agent capabilities in the future.