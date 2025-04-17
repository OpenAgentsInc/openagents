import { type UIMessage } from "ai";
import type { BaseAgentState } from "../../common/types";
import { BaseIssue, ImplementationStep, IssueComment } from "@openagents/core";

// Define types for Solver state - extends base agent state
export interface SolverState extends BaseAgentState {
  messages: UIMessage[]; // Override to specify UIMessage type
  currentIssue?: BaseIssue;
  implementationSteps?: ImplementationStep[];
  issueComments?: IssueComment[];
}

// Use the solver-specific issue type to add any solver-specific fields
export interface SolverIssue extends BaseIssue {
  // If needed, add solver-specific issue fields here
  // For now, we can use the base issue type directly
}
