import { type UIMessage } from "ai";

// Define types for Solver state
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

export interface IssueComment {
  id: string;
  content: string;
  author: string;
  created: Date;
}