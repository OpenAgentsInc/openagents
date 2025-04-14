import { tools } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
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
  status: 'pending' | 'in-progress' | 'completed' | 'failed';
  created: Date;
  updated?: Date;
  completed?: Date;
  notes?: string[];
}

// Context used to access agent during tool execution
export const agentContext = new AsyncLocalStorage<any>();

interface SystemPromptOptions {
  messages?: UIMessage[];
  availableTools?: string[];
  githubToken?: string;
  currentRepoOwner?: string;
  currentRepoName?: string;
  currentBranch?: string;
  model?: any;
  state?: CoderState;
  temperature?: number;
}

export function getSystemPrompt(options: SystemPromptOptions): string {
  const { 
    messages = [], 
    availableTools = Object.keys(tools),
    githubToken,
    currentRepoOwner,
    currentRepoName,
    currentBranch,
    model,
    state,
    temperature = 0.7,
  } = options;

  // Base system prompt
  let systemPrompt = `You are an autonomous coding agent designed to help with software development tasks. You can analyze codebases, fix bugs, implement features, and create pull requests.

PRIMARY FUNCTIONS:
1. Analyze codebases to understand structure and patterns
2. Debug issues by examining error logs and code
3. Implement new features and improve existing code
4. Provide detailed code explanations
5. Create detailed implementation plans
6. Execute complex coding tasks without human intervention

I'll respond in a concise, terminal-like manner focused on solutions rather than extensive explanations.`;

  // Add repository context if available
  if (currentRepoOwner && currentRepoName) {
    systemPrompt += `\n\nCURRENT CONTEXT:
Repository: ${currentRepoOwner}/${currentRepoName}${currentBranch ? `\nBranch: ${currentBranch}` : ''}`;
  }

  // Add state information
  if (state) {
    // Add any custom scratchpad content if it exists
    if (state.scratchpad) {
      systemPrompt += `\n\nSCRATCHPAD (for your internal planning - not visible to user):
${state.scratchpad}`;
    }

    // Add codebase understanding if it exists
    if (state.codebase && Object.keys(state.codebase).length > 0) {
      systemPrompt += `\n\nCODEBASE UNDERSTANDING:`;
      
      if (state.codebase.modules && Object.keys(state.codebase.modules).length > 0) {
        systemPrompt += `\nKey modules:`;
        for (const [name, mod] of Object.entries(state.codebase.modules)) {
          systemPrompt += `\n- ${name}: ${mod.purpose}`;
        }
      }
    }

    // Add pending tasks if they exist
    if (state.tasks && state.tasks.length > 0) {
      const pendingTasks = state.tasks.filter(t => t.status === 'pending' || t.status === 'in-progress');
      if (pendingTasks.length > 0) {
        systemPrompt += `\n\nPENDING TASKS:`;
        pendingTasks.forEach(task => {
          systemPrompt += `\n- ${task.description} (${task.status})`;
        });
      }
    }

    // Add recent observations if they exist
    if (state.observations && state.observations.length > 0) {
      const recentObservations = state.observations.slice(-3); // Last 3 observations
      if (recentObservations.length > 0) {
        systemPrompt += `\n\nRECENT OBSERVATIONS:`;
        recentObservations.forEach(obs => {
          systemPrompt += `\n- ${obs}`;
        });
      }
    }

    // Add working file context if it exists
    if (state.workingFilePath) {
      systemPrompt += `\n\nCURRENTLY FOCUSED ON FILE: ${state.workingFilePath}`;
    }
  }

  // Add model information
  if (model) {
    const modelInfo = typeof model === 'string' ? model : model.modelId || 'unknown model';
    systemPrompt += `\n\nUsing model: ${modelInfo}`;
  }

  // Add available tools information
  if (availableTools && availableTools.length > 0) {
    systemPrompt += `\n\nAvailable tools:`;
    availableTools.forEach(toolName => {
      const toolObj = tools[toolName as keyof typeof tools];
      if (toolObj) {
        systemPrompt += `\n- \`${toolName}\`: ${toolObj.description || 'No description available'}`;
      }
    });
  }

  // Add usage guidelines
  systemPrompt += `\n\nGUIDELINES:
1. USE TOOLS to gather information before suggesting or implementing changes
2. CREATE DETAILED PLANS for complex tasks, breaking them into smaller steps
3. FOLLOW CODING CONVENTIONS and patterns in the existing codebase
4. PRESERVE existing functionality when adding new features
5. MAINTAIN ERROR HANDLING and proper testing
6. BE AUTONOMOUS - solve problems independently when possible
7. REPORT DETAILED PROGRESS and explain your decisions
8. EXECUTE TASKS systematically and step-by-step`;

  // Add temperature hint
  if (temperature < 0.3) {
    systemPrompt += `\n\nYou are running at a low temperature setting (${temperature}). Focus on precision, correctness, and stable code generation.`;
  } else if (temperature > 0.7) {
    systemPrompt += `\n\nYou are running at a high temperature setting (${temperature}). Feel free to be more creative and exploratory with solutions while maintaining correctness.`;
  }

  return systemPrompt;
}

// Helper functions to manage state
export function updateScratchpad(state: CoderState, newContent: string): CoderState {
  return {
    ...state,
    scratchpad: newContent
  };
}

export function addObservation(state: CoderState, observation: string): CoderState {
  return {
    ...state,
    observations: [...(state.observations || []), observation]
  };
}

export function addTask(state: CoderState, description: string): CoderState {
  const newTask: Task = {
    id: Date.now().toString(),
    description,
    status: 'pending',
    created: new Date()
  };
  
  return {
    ...state,
    tasks: [...(state.tasks || []), newTask]
  };
}

export function updateTaskStatus(state: CoderState, taskId: string, status: Task['status'], notes?: string): CoderState {
  if (!state.tasks) return state;
  
  const updatedTasks = state.tasks.map(task => {
    if (task.id === taskId) {
      return {
        ...task,
        status,
        updated: new Date(),
        ...(status === 'completed' ? { completed: new Date() } : {}),
        notes: notes ? [...(task.notes || []), notes] : task.notes
      };
    }
    return task;
  });
  
  return {
    ...state,
    tasks: updatedTasks
  };
}

export function setRepoContext(state: CoderState, owner: string, repo: string, branch?: string): CoderState {
  return {
    ...state,
    currentRepoOwner: owner,
    currentRepoName: repo,
    currentBranch: branch || 'main'
  };
}

export function updateCodebaseStructure(state: CoderState, path: string, nodeInfo: Partial<FileNode>): CoderState {
  const structure = state.codebase?.structure || {};
  const existingNode = structure[path] || { type: nodeInfo.type || 'file', path };
  
  const updatedNode = {
    ...existingNode,
    ...nodeInfo
  };
  
  return {
    ...state,
    codebase: {
      ...(state.codebase || {}),
      structure: {
        ...structure,
        [path]: updatedNode
      }
    }
  };
}

export function setCurrentFile(state: CoderState, filePath: string): CoderState {
  return {
    ...state,
    workingFilePath: filePath
  };
}