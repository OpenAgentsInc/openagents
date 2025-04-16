import { type UIMessage } from "ai";

// Define types for Solver state
export interface SolverState {
  messages: UIMessage[];
  githubToken?: string;
  currentProblem?: Problem;
  steps?: SolutionStep[];
  observations?: string[];
  scratchpad?: string;
}

export interface Problem {
  id: string;
  description: string;
  type: 'math' | 'logic' | 'reasoning' | 'other';
  constraints?: string[];
  status: 'unsolved' | 'in-progress' | 'solved' | 'failed';
  created: Date;
  updated?: Date;
  completed?: Date;
}

export interface SolutionStep {
  id: string;
  description: string;
  content: string;
  type: 'assumption' | 'theorem' | 'definition' | 'calculation' | 'proof';
  verified: boolean;
  created: Date;
}