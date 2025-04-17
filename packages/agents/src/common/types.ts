// Common types shared between agents

import type { UIMessage } from "ai";

export interface AgentObservation {
  id: string;
  content: string;
  timestamp: Date;
  source?: string;
  metadata?: Record<string, any>;
}

// Define inference properties for the shared inference method
export interface InferProps {
  model: string;
  messages: UIMessage[];
  system?: string;
  temperature?: number;
  max_tokens?: number;
  top_p?: number;
  stream?: boolean;
}

// Define inference response type
export interface InferResponse {
  id: string;
  content: string;
  role: string;
  timestamp: string;
  model: string;
}

// Base agent state that all agents should include
export interface BaseAgentState {
  messages: any[];
  githubToken?: string;
  currentRepoOwner?: string;
  currentRepoName?: string;
  currentBranch?: string;
  observations?: string[];
  workingFilePath?: string;
  scratchpad?: string;
}

// Export type for Env
export type { Env } from './env';

// Import OpenAgent from its own file
export { OpenAgent } from './open-agent';
