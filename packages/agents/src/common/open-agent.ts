// Base OpenAgent class that implements common functionality for all agents

import { Agent } from "agents";
import { generateId, generateText } from "ai";
import type { BaseAgentState, InferProps, InferResponse } from "./types";
import type { SolverState } from "../agents/solver/types";
import { createWorkersAI } from 'workers-ai-provider';
import { env } from "cloudflare:workers"

/**
 * Base OpenAgent class that implements common functionality for all agent types
 */
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
   * Uses Cloudflare Workers AI with Llama 4 to generate responses
   * @param props Inference properties including model, messages, and system prompt
   * @returns Response from the AI model
   */
  async sharedInfer(props: InferProps): Promise<InferResponse> {
    // Default to Llama 4 Scout if no model specified
    const { 
      model = "@cf/meta/llama-4-scout-17b-16e-instruct", 
      messages, 
      system, 
      temperature = 0.7, 
      max_tokens = 1024, 
      top_p = 0.95 
    } = props;
    
    // Log the input parameters with more details
    console.log("SHARED INFER CALLED WITH:", {
      model,
      messagesCount: messages.length,
      systemProvidedExternally: !!system,
      systemPromptLength: system ? system.length : 0,
      temperature,
      max_tokens,
      top_p
    });
    
    try {
      // Format messages for the chat completion format
      let formattedMessages = [];
      
      // Add system message - either the one provided or the agent's own system prompt
      // Important: this is where we get the system prompt if not provided
      let systemPrompt;
      if (system) {
        console.log("Using externally provided system prompt");
        systemPrompt = system;
      } else {
        console.log("Generating system prompt from agent state");
        // Cast to any to avoid TypeScript errors since we can't know the exact properties 
        // at this level in the class hierarchy
        const state = this.state as any;
        console.log("AGENT STATE DEBUG:", JSON.stringify({
          hasIssue: state.currentIssue ? true : false,
          hasProject: state.currentProject ? true : false,
          hasTeam: state.currentTeam ? true : false,
          issueSource: state.currentIssue ? state.currentIssue.source : null,
          issueTitle: state.currentIssue ? state.currentIssue.title : null
        }));
        
        systemPrompt = this.getSystemPrompt();
        
        // Log brief analysis of the generated system prompt
        console.log("SYSTEM PROMPT ANALYSIS:", {
          length: systemPrompt.length,
          hasIssueContext: systemPrompt.includes("CURRENT ISSUE"),
          hasProjectContext: systemPrompt.includes("PROJECT CONTEXT"),
          hasTeamContext: systemPrompt.includes("TEAM CONTEXT"),
          firstFewWords: systemPrompt.substring(0, 50) + "..."
        });
      }
      
      formattedMessages.push({
        role: "system",
        content: systemPrompt
      });
      
      // Add user and assistant messages from the conversation
      messages.forEach(msg => {
        formattedMessages.push({
          role: msg.role,
          content: msg.content
        });
      });
      
      // Log the formatted messages for debugging
      console.log(`Formatted ${formattedMessages.length} messages for AI inference`);
      
      // Use Workers AI binding to call the model
      // @ts-expect-error The Env type may not be correctly defined in TypeScript
      const result = await this.env.AI.run(model, {
        messages: formattedMessages,
        temperature,
        max_tokens,
        top_p
      });
      
      // The result structure depends on the model used
      // For LLM chat models, the result may be either a string or an object with a response field
      const responseText = typeof result === 'string' ? result : 
                         (result as any).response || 
                         (result as any).text || 
                         (result as any).generated_text || 
                         "No content generated";
      
      console.log("AI inference successful:", {
        responseLength: responseText.length,
        model: model
      });
      
      // Return a properly formatted response
      return {
        id: generateId(),
        content: responseText,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    } catch (error) {
      console.error("Error during AI inference:", error);
      
      // Return a response with the error message
      return {
        id: generateId(),
        content: `Error generating response with Llama 4: ${error instanceof Error ? error.message : String(error)}`,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    }
  }
}
