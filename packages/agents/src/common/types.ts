// Common types shared between agents

import { Agent } from "agents";
import { generateId } from "ai";

export interface AgentObservation {
  id: string;
  content: string;
  timestamp: Date;
  source?: string;
  metadata?: Record<string, any>;
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
}
