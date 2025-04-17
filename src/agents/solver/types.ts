import type { BaseIssue, ImplementationStep, IssueComment } from "@openagents/core";
// ... existing code ...

export type Issue = BaseIssue;

export interface SolverState {
  messages: any[];
  currentIssue?: Issue;
  implementationSteps?: ImplementationStep[];
  issueComments?: IssueComment[];
  githubToken?: string;
  currentRepoOwner?: string;
  currentRepoName?: string;
  currentBranch?: string;
  observations?: string[];
  workingFilePath?: string;
  scratchpad?: string;
}

export type { ImplementationStep };
