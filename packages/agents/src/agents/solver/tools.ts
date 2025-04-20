import { tool } from "ai";
import { z } from "zod";
import { AsyncLocalStorage } from "node:async_hooks";
import type { BaseIssue } from "@openagents/core";
import { Solver } from "./index";

// Create an AsyncLocalStorage instance for solver context
export const solverContext = new AsyncLocalStorage<Solver>();

// Define tools for the solver agent

export const getIssueDetails = tool({
  description: "Fetches comprehensive issue information",
  parameters: z.object({
    owner: z.string().describe("Repository owner"),
    repo: z.string().describe("Repository name"),
    issueNumber: z.number().describe("Issue number"),
  }),
  execute: async ({ owner, repo, issueNumber }) => {
    console.log(`Fetching issue details for ${owner}/${repo}#${issueNumber}`);
    
    // Get the solver instance from context
    const agent = solverContext.getStore();
    if (!agent) {
      throw new Error("Solver context not available");
    }
    
    // If we have the issue already in state, use that
    if (agent.state.currentIssue && agent.state.currentIssue.number === issueNumber) {
      return agent.state.currentIssue;
    }
    
    // Mock implementation - in real code, this would fetch from GitHub API
    const mockIssue: BaseIssue = {
      id: `issue-${issueNumber}`,
      number: issueNumber,
      title: `Test Issue #${issueNumber}`,
      description: `This is a test issue for ${owner}/${repo}`,
      status: "open",
      assignee: "user1",
      labels: ["bug", "high-priority"],
      created: new Date().toISOString(),
      updated: new Date().toISOString()
    };
    
    return mockIssue;
  }
});

export const updateIssueStatus = tool({
  description: "Updates the status of an issue",
  parameters: z.object({
    owner: z.string().describe("Repository owner"),
    repo: z.string().describe("Repository name"),
    issueNumber: z.number().describe("Issue number"),
    status: z.string().describe("New status"),
    comment: z.string().optional().describe("Optional comment to add"),
  }),
  execute: async ({ owner, repo, issueNumber, status, comment }) => {
    console.log(`Updating issue status for ${owner}/${repo}#${issueNumber} to ${status}`);
    
    // Get the solver instance from context
    const agent = solverContext.getStore();
    if (!agent) {
      throw new Error("Solver context not available");
    }
    
    // In a real implementation, this would update GitHub via API
    if (agent.state.currentIssue && agent.state.currentIssue.number === issueNumber) {
      // Update the issue in state
      const updatedIssue = {
        ...agent.state.currentIssue,
        status,
        updatedAt: new Date().toISOString()
      };
      
      // Update the state
      agent.setState({
        ...agent.state,
        currentIssue: updatedIssue
      });
      
      return { 
        success: true, 
        message: `Issue #${issueNumber} status updated to ${status}`,
        addedComment: comment ? true : false 
      };
    }
    
    return { 
      success: false, 
      message: `Issue #${issueNumber} not found in current context` 
    };
  }
});

export const createImplementationPlan = tool({
  description: "Creates a step-by-step implementation plan for the current issue",
  parameters: z.object({
    steps: z.array(z.string()).optional().describe("Optional custom steps for the plan"),
  }),
  execute: async ({ steps }) => {
    console.log("Creating implementation plan");
    
    // Get the solver instance from context
    const agent = solverContext.getStore();
    if (!agent) {
      throw new Error("Solver context not available");
    }
    
    // Use provided steps or default template
    const implementationSteps = steps || [
      "Analyze requirements and context",
      "Research existing codebase",
      "Plan implementation approach",
      "Implement solution",
      "Test thoroughly",
      "Document changes and decisions"
    ];
    
    return { 
      success: true, 
      plan: {
        issueId: agent.state.currentIssue?.id || "unknown",
        steps: implementationSteps.map((step, index) => ({
          id: `step-${index + 1}`,
          description: step,
          status: "pending",
          order: index + 1
        }))
      }
    };
  }
});

// Export all tools as a record
export const solverTools = {
  getIssueDetails,
  updateIssueStatus,
  createImplementationPlan,
};

// Export the tool name type for type safety
export type SolverToolName = keyof typeof solverTools;