# Solver Agent Implementation Guide

**Date:** April 16, 2025  
**Author:** Claude Code

## Overview

This document details the implementation of the Solver agent, a new agent type in the OpenAgents platform designed for resolving GitHub and Linear issues. The Solver agent is focused on analyzing issue descriptions, planning implementation steps, and working through the necessary code changes to fix issues.

## Agent Purpose & Capabilities

The Solver agent is designed to:

1. Analyze GitHub and Linear issue descriptions to understand requirements
2. Create step-by-step implementation plans for solving issues
3. Use the codebase context to understand how to implement solutions
4. Update issue statuses and add comments as progress is made
5. Implement actual code changes to fix issues
6. Document the approach and reasoning behind solutions

## Technical Implementation

### 1. State Model

The Solver agent's state is defined by the `SolverState` interface:

```typescript
export interface SolverState {
  messages: UIMessage[];
  githubToken?: string;
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

export interface Issue {
  id: string;
  number: number;
  title: string;
  description: string;
  source: 'github' | 'linear' | 'other';
  status: 'open' | 'in_progress' | 'review' | 'closed';
  url?: string;
  assignee?: string;
  labels?: string[];
  created: Date;
  updated?: Date;
  projectId?: string;
  teamId?: string;
}

export interface ImplementationStep {
  id: string;
  description: string;
  type: 'analysis' | 'research' | 'implementation' | 'testing' | 'documentation';
  status: 'pending' | 'in_progress' | 'completed' | 'skipped';
  notes?: string;
  created: Date;
  started?: Date;
  completed?: Date;
  dependsOn?: string[]; // IDs of steps this one depends on
  filePaths?: string[]; // Files involved in this step
}
```

This state model tracks:
- The conversation history with the user
- The current issue being solved
- Repository context
- Implementation steps with their statuses
- Agent observations and thoughts
- Working file path

### 2. Agent Methods

Key methods of the Solver agent include:

```typescript
class Solver extends Agent<Env, SolverState> {
  // Core state management methods
  updateState(partialState: Partial<SolverState>)
  async addAgentObservation(observation: string)
  private async updateScratchpad(thought: string)
  
  // Issue and context management
  async setCurrentIssue(issue: Issue)
  async setRepositoryContext(owner: string, repo: string, branch: string = 'main')
  async updateStepStatus(stepId: string, status: ImplementationStep['status'], notes?: string)
  async setCurrentFile(filePath: string)
  
  // WebSocket communication
  async onMessage(connection: Connection, message: WSMessage)
  
  // Main inference method
  async infer()
}
```

### 3. Specialized Tools

The Solver agent includes specialized tools for GitHub/Linear issue management:

```typescript
// Fetch issue details
export const getIssueDetails = tool({
  description: "Fetch details about an issue from GitHub or Linear",
  parameters: z.object({
    source: z.enum(["github", "linear"]),
    owner: z.string().optional(),
    repo: z.string().optional(),
    issueNumber: z.number(),
    teamId: z.string().optional()
  }),
  execute: async ({ source, owner, repo, issueNumber, teamId }) => {
    // Implementation for fetching issue details from GitHub/Linear
  }
});

// Update issue status
export const updateIssueStatus = tool({
  description: "Update the status of an issue in GitHub or Linear",
  parameters: z.object({
    source: z.enum(["github", "linear"]),
    owner: z.string().optional(),
    repo: z.string().optional(),
    issueNumber: z.number(),
    status: z.enum(["open", "in_progress", "review", "closed"]),
    comment: z.string().optional()
  }),
  execute: async ({ source, owner, repo, issueNumber, status, comment }) => {
    // Implementation for updating issue status
  }
});

// Create implementation plan
export const createImplementationPlan = tool({
  description: "Create a step-by-step implementation plan for solving an issue",
  parameters: z.object({
    issueTitle: z.string(),
    issueDescription: z.string(),
    repoOwner: z.string().optional(),
    repoName: z.string().optional()
  }),
  execute: async ({ issueTitle, issueDescription, repoOwner, repoName }) => {
    // Implementation for creating solution steps
  }
});
```

### 4. System Prompt

The Solver agent uses a specialized system prompt that emphasizes issue resolution:

```typescript
export function getSolverSystemPrompt(options: SystemPromptOptions): string {
  // ...

  // Base system prompt
  let systemPrompt = `You are an autonomous issue-solving agent designed to analyze, plan, and implement solutions for GitHub and Linear issues. You work methodically to resolve issues in software projects.

PRIMARY FUNCTIONS:
1. Analyze issue descriptions to understand requirements
2. Plan implementation steps for solving issues
3. Research existing code to understand the problem context
4. Implement solutions by modifying code
5. Test changes to ensure they fix the issue
6. Document the solution with clear explanations
7. Update issue status and add comments as you make progress`;

  // Add current issue context
  if (currentIssue) {
    systemPrompt += `\n\nCURRENT ISSUE:
#${currentIssue.number}: ${currentIssue.title}
Status: ${currentIssue.status}
Source: ${currentIssue.source}
${currentIssue.url ? `URL: ${currentIssue.url}` : ''}

Description:
${currentIssue.description}`;
  }
  
  // Add implementation steps if available
  if (implementationSteps && implementationSteps.length > 0) {
    systemPrompt += `\n\nIMPLEMENTATION PLAN:`;
    implementationSteps.forEach((step, index) => {
      systemPrompt += `\n${index + 1}. ${step.description} (${step.status}) - ${step.type}`;
      // ... step details ...
    });
  }

  // ...
  
  return systemPrompt;
}
```

## Solver Agent Workflow

The typical workflow for the Solver agent is:

1. **Issue Selection**: The user or system provides a GitHub/Linear issue to solve
2. **Analysis**: The agent analyzes the issue description to understand requirements
3. **Planning**: The agent creates a step-by-step implementation plan
4. **Research**: The agent explores the codebase to understand relevant components
5. **Implementation**: The agent makes code changes to solve the issue
6. **Testing**: The agent verifies the solution works as expected
7. **Documentation**: The agent documents the solution and explains its approach
8. **Status Update**: The agent updates the issue status and adds explanatory comments

## Integration with UI

The Solver agent can be integrated with the OpenAgents UI by:

1. **Issue Selection Interface**: Allow users to choose issues from GitHub/Linear
2. **Implementation Plan Display**: Show the step-by-step plan with progress
3. **Status Updates**: Display issue status changes and agent progress
4. **Code Changes View**: Show proposed or implemented code changes
5. **Agent Chat**: Allow users to interact with the agent during the solution process

## Usage Examples

### Example 1: Resolving a Bug Issue

```
User: Can you help me solve GitHub issue #123 in our repository?

Solver Agent:
I'll help you solve this issue. Let me fetch the details first.

[Agent uses getIssueDetails tool to fetch issue #123]

I've analyzed issue #123: "Fix button click event not firing on mobile devices"

Here's my implementation plan:
1. Analyze the current button implementation (pending)
2. Research mobile event handling best practices (pending)
3. Implement fix for mobile event listeners (pending)
4. Add tests for mobile interaction (pending)
5. Update documentation (pending)

I'll start by examining the button component. Let me know if you want me to modify any part of this plan.
```

### Example 2: Feature Implementation

```
User: Please implement the new feature described in Linear issue LIN-456

Solver Agent:
I'll help implement this feature. Let me fetch the issue details.

[Agent uses getIssueDetails tool to fetch Linear issue LIN-456]

I've analyzed issue LIN-456: "Add dark mode support to dashboard components"

I've created an implementation plan:
1. Analyze current theme implementation (pending)
2. Research existing color palette and dark variants (pending)
3. Implement dark mode theme variables (pending)
4. Update dashboard components to use theme variables (pending)
5. Add theme toggle controls (pending)
6. Test theme switching functionality (pending)
7. Document usage for other developers (pending)

Let's start with analyzing the current theme implementation. I'll update you as I make progress on each step.
```

## Differentiation from Coder Agent

While the Coder and Solver agents share some similarities, they have distinct purposes:

| Aspect | Coder Agent | Solver Agent |
|--------|-------------|--------------|
| **Primary Focus** | General coding assistance | Issue resolution |
| **Starting Point** | Direct user instructions | GitHub/Linear issues |
| **Workflow** | Free-form coding assistance | Structured issue solving steps |
| **Context** | Repository exploration | Issue-specific implementation |
| **Tools** | General coding tools | Issue management tools |
| **Output** | Code implementations | Issue solutions with status updates |

The key difference is that the Solver agent is specifically designed to work with ticket/issue tracking systems and follows a more structured problem-solving approach.

## Future Enhancements

1. **Linear API Integration**: Complete the Linear API integration for issue management
2. **Pull Request Creation**: Automatically create PRs with implemented solutions
3. **Test Generation**: Generate and run tests to verify issue fixes
4. **Cross-Issue Analysis**: Connect related issues and identify common patterns
5. **CI/CD Integration**: Trigger builds and test runs to validate solutions

## Conclusion

The Solver agent represents a specialized extension of the OpenAgents platform focused on resolving GitHub and Linear issues. By following this implementation guide, developers can further enhance and extend the Solver agent's capabilities to support efficient issue resolution workflows across software projects.