// Common types shared between agents

import { Agent } from "agents";
import { generateId } from "ai";
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

import type { Env } from './env';

// Base OpenAgent class that implements common functionality for all agents
export class OpenAgent<T extends BaseAgentState> extends Agent<Env, T> {
  // Provide default base state that can be extended by subclasses
  // Using a method instead of a property to ensure proper typing when inherited
  protected getBaseInitialState(): BaseAgentState {
    return {
      messages: [],
      githubToken: undefined,
      currentRepoOwner: undefined,
      currentRepoName: undefined,
      currentBranch: undefined,
      scratchpad: '',
      observations: [],
      workingFilePath: undefined
    };
  }
  
  constructor(ctx: any, env: Env) {
    super(ctx, env);
  }

  /**
   * Safely updates the agent's state by merging the provided partial state
   * with the existing state. Ensures ...this.state is always included.
   * @param partialState An object containing the state properties to update.
   */
  protected updateState(partialState: Partial<T>) {
    this.setState({
      ...this.state,
      ...partialState,
    });
    console.log('[updateState] Updated in-memory state via this.setState.');
  }

  /**
   * Sets the GitHub token for the agent
   */
  setGithubToken(token: string) {
    this.updateState({
      githubToken: token
    } as Partial<T>);

    console.log("GitHub token updated");
    return { success: true, message: "GitHub token updated" };
  }

  /**
   * Gets the GitHub token for the agent
   */
  getGithubToken() {
    return this.state.githubToken || "";
  }

  /**
   * Sets the current repository context
   */
  setRepositoryContext(owner: string, repo: string, branch: string = 'main') {
    console.log(`[setRepositoryContext] Setting context to ${owner}/${repo} on branch ${branch}`);

    this.updateState({
      currentRepoOwner: owner,
      currentRepoName: repo,
      currentBranch: branch
    } as Partial<T>);

    this.addAgentObservation(`Repository context set to ${owner}/${repo}:${branch}`);

    return { success: true, message: `Context set to ${owner}/${repo}:${branch}` };
  }

  /**
   * Sets the file currently being worked on
   */
  setCurrentFile(filePath: string) {
    this.updateState({
      workingFilePath: filePath
    } as Partial<T>);

    this.addAgentObservation(`Now working on file: ${filePath}`);
    
    return { success: true, message: `Current file set to ${filePath}` };
  }

  /**
   * Adds an observation to the agent's state
   */
  addAgentObservation(observation: string) {
    this.updateState({
      observations: [...(this.state.observations || []), observation]
    } as Partial<T>);
    
    return { success: true };
  }

  /**
   * Updates the agent's scratchpad with agent thoughts
   */
  protected updateScratchpad(thought: string) {
    const timestamp = new Date().toISOString();
    const formattedThought = `${timestamp}: ${thought}`;

    this.updateState({
      scratchpad: this.state.scratchpad
        ? `${this.state.scratchpad}\n- ${formattedThought}`
        : `- ${formattedThought}`
    } as Partial<T>);
    
    return { success: true };
  }
  
  /**
   * Gets the system prompt for the agent
   * This is a base implementation - agent subclasses should override to provide their specific system prompts
   */
  getSystemPrompt() {
    // Base system prompt that all agents can use
    const basePrompt = `You are an autonomous agent designed to assist with development tasks. 
You have access to a repository and can help with understanding code, implementing features, and fixing issues.

Current context:
${this.state.currentRepoOwner ? `Repository: ${this.state.currentRepoOwner}/${this.state.currentRepoName}` : 'No repository set'}
${this.state.currentBranch ? `Branch: ${this.state.currentBranch}` : ''}
${this.state.workingFilePath ? `Current file: ${this.state.workingFilePath}` : ''}`;

    return basePrompt;
  }
  
  /**
   * Shared inference method for all agents
   * This version just logs the input and returns a dummy response
   * @param props Inference properties including model, messages, and system prompt
   * @returns A dummy response object
   */
  async sharedInfer(props: InferProps): Promise<InferResponse> {
    const { model, messages, system, temperature = 0.7, max_tokens, top_p } = props;
    
    // Log the input parameters
    console.log("SHARED INFER CALLED WITH:", {
      model,
      messagesCount: messages.length,
      systemPromptLength: system ? system.length : 0,
      temperature,
      max_tokens,
      top_p
    });
    
    // For debugging, log the first and last message
    if (messages.length > 0) {
      console.log("First message:", messages[0]);
      console.log("Last message:", messages[messages.length - 1]);
    }
    
    // For now, just return a dummy response
    return {
      id: generateId(),
      content: `This is a dummy response from the ${model} model. The real implementation will connect to the AI provider.`,
      role: "assistant",
      timestamp: new Date().toISOString(),
      model: model
    };
  }
}
