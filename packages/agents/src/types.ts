import { type UIMessage } from "ai";

// Define types for Coder state
export interface CoderState {
  messages: UIMessage[];
  githubToken?: string;
  currentRepoOwner?: string;
  currentRepoName?: string;
  currentBranch?: string;
  codebase?: CodebaseState;
  scratchpad?: string;
  tasks?: Task[];
  observations?: string[];
  workingFilePath?: string;
  isContinuousRunActive?: boolean;
}

// Type to track codebase understanding
export interface CodebaseState {
  structure?: Record<string, FileNode>;
  dependencies?: Record<string, string>;
  modules?: Record<string, ModuleDescription>;
}

export interface FileNode {
  type: 'file' | 'directory';
  path: string;
  children?: string[]; // For directories, list of child paths
  description?: string;
  tags?: string[];
  metadata?: {
    exports?: string[];
    dependencies?: string[];
    complexity?: 'low' | 'medium' | 'high';
    lastAnalyzed?: string;
    [key: string]: any; // Allow for additional metadata
  };
}

export interface ModuleDescription {
  name: string;
  purpose: string;
  dependencies: string[];
  apis: string[];
}

export interface Task {
  id: string;
  description: string;
  status: 'pending' | 'in-progress' | 'completed' | 'failed' | 'cancelled';
  created: Date;
  updated?: Date;
  completed?: Date;
  notes?: string[];
  scheduleId?: string;
  payload?: Record<string, any>;
  callbackMethodName?: string;
}
