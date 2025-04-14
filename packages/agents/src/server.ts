import { Agent, routeAgentRequest, unstable_callable, type Connection, type Schedule, type WSMessage } from "agents"
import { type UIMessage, generateId, generateText, generateObject, type ToolSet } from "ai";
import { env } from "cloudflare:workers";
import { AsyncLocalStorage } from "node:async_hooks";
import type { UIPart } from "@openagents/core/src/chat/types";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import type { ToolContext } from "@openagents/core/src/tools/toolContext";
import { getFileContentsTool } from "@openagents/core/src/tools/github/getFileContents";
import { addIssueCommentTool } from "@openagents/core/src/tools/github/addIssueComment";
import { tools as availableTools } from "./tools";
import { getSystemPrompt } from "./prompts";
import type { CoderState, Task, FileNode } from "./types";
import { z } from "zod";

const openrouter = createOpenRouter({ apiKey: env.OPENROUTER_API_KEY })
const model = openrouter("google/gemini-2.5-pro-preview-03-25");

export const agentContext = new AsyncLocalStorage<Coder>();

// Zod schema for planning thoughts/scratchpad updates
const PlanningSchema = z.object({
  thought: z.string().describe("A concise thought or step in a plan related to the current task."),
  nextAction: z.string().optional().describe("A potential next immediate action or tool use."),
  questions: z.array(z.string()).optional().describe("Questions to ask the user or resolve internally."),
});

// Zod schema for summarizing file content for the codebase map
const FileSummarySchema = z.object({
  summary: z.string().describe("A brief summary of the file's purpose and key contents."),
  tags: z.array(z.string()).optional().describe("Keywords or tags describing the file's functionality (e.g., 'auth', 'api-route', 'database')."),
  exports: z.array(z.string()).optional().describe("Key functions, classes, or variables exported by the file."),
});

// Zod schema for defining a new task based on user request or analysis
const NewTaskSchema = z.object({
  description: z.string().describe("A clear, actionable description of the coding task."),
  priority: z.enum(["high", "medium", "low"]).optional().describe("Priority of the task."),
  subTasks: z.array(z.string()).optional().describe("Breakdown into smaller steps if applicable."),
});

/** * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends Agent<Env, CoderState> {
  initialState: CoderState = {
    messages: [],
    githubToken: undefined,
    currentRepoOwner: undefined,
    currentRepoName: undefined,
    currentBranch: undefined,
    codebase: {},
    scratchpad: '',
    tasks: [],
    observations: [],
    workingFilePath: undefined
  };
  tools: ToolSet = {};

  /**
   * Safely updates the agent's state by merging the provided partial state
   * with the existing state. Ensures ...this.state is always included.
   * @param partialState An object containing the state properties to update.
   */
  private updateState(partialState: Partial<CoderState>) {
    this.setState({
      ...this.state,
      ...partialState,
    });
  }

  async executeTask(description: string, task: Schedule<string>) {
    const newMessage = {
      id: generateId(),
      role: "user" as const,
      content: `This is a scheduled notice for you to now execute the following task: ${description}`,
      createdAt: new Date(),
      parts: [
        {
          type: "text" as const,
          text: `This is a scheduled notice for you to now execute the following task: ${description}`
        }
      ],
    };

    this.updateState({
      messages: [
        ...this.state.messages,
        newMessage
      ],
    });
    
    // now infer based on this message
    await this.infer();
  }

  onMessage(connection: Connection, message: WSMessage) {
    const parsedMessage = JSON.parse(message as string);
    console.log("IN ON MESSAGE AND HAVE PARSED MESSAGE", parsedMessage);
    const githubToken = parsedMessage.githubToken;
    
    // Store the githubToken in state, preserving other state
    this.updateState({
      githubToken
    });
    
    this.infer();
  }

  /**
   * Sets the current repository context
   */
  @unstable_callable({
    description: "Set the current repository context"
  })
  async setRepositoryContext(owner: string, repo: string, branch: string = 'main') {
    console.log(`Setting repository context to ${owner}/${repo} on branch ${branch}`);
    
    this.updateState({
      currentRepoOwner: owner,
      currentRepoName: repo,
      currentBranch: branch,
    });
    
    return { success: true, message: `Context set to ${owner}/${repo}:${branch}` };
  }

  /**
   * Adds a task to the agent's state
   */
  private addAgentTask(description: string) {
    const newTask: Task = {
      id: generateId(),
      description,
      status: 'pending',
      created: new Date(),
    };
    
    this.updateState({
      tasks: [...(this.state.tasks || []), newTask],
      observations: [...(this.state.observations || []), `New task added: ${description}`]
    });
    
    return newTask.id;
  }

  /**
   * Generates a structured task using AI and adds it to the agent's state
   */
  private async generateAndAddTask(prompt: string) {
    try {
      // Enhance prompt with current context
      let contextPrompt = `Based on the following request or user message, define a clear, actionable coding task: "${prompt.trim()}"\n\n`;
      
      // Add repository context if available
      if (this.state.currentRepoOwner && this.state.currentRepoName) {
        contextPrompt += `Repository: ${this.state.currentRepoOwner}/${this.state.currentRepoName}\n`;
        if (this.state.currentBranch) {
          contextPrompt += `Branch: ${this.state.currentBranch}\n`;
        }
      }
      
      // Add current file context if available
      if (this.state.workingFilePath) {
        contextPrompt += `Currently focused on file: ${this.state.workingFilePath}\n`;
      }
      
      // Add existing task context if available
      if (this.state.tasks && this.state.tasks.length > 0) {
        contextPrompt += `\nExisting tasks:\n`;
        this.state.tasks.slice(0, 3).forEach(task => {
          contextPrompt += `- ${task.description} (${task.status})\n`;
        });
        if (this.state.tasks.length > 3) {
          contextPrompt += `- ... (${this.state.tasks.length - 3} more tasks)\n`;
        }
      }
      
      // Add instruction for task quality
      contextPrompt += `\nCreate a SPECIFIC, ACTIONABLE coding task. Break it down into clear subtasks where appropriate.`;
      
      const { object: newTaskInfo } = await generateObject({
        model: model,
        schema: NewTaskSchema,
        prompt: contextPrompt
      });

      const newTask: Task = {
        id: generateId(),
        description: newTaskInfo.description,
        status: 'pending',
        created: new Date(),
        notes: newTaskInfo.subTasks ? [`Sub-tasks: ${newTaskInfo.subTasks.join(', ')}`] : [],
      };

      this.updateState({
        tasks: [...(this.state.tasks || []), newTask],
        observations: [...(this.state.observations || []), `New task generated: ${newTask.description}`]
      });
      return newTask.id;

    } catch (error) {
      console.error("Error generating task object:", error);
      this.addAgentObservation(`Failed to generate structured task for prompt: ${prompt}`);
      return null; // Indicate failure
    }
  }

  /**
   * Updates a task's status
   */
  private updateTaskStatus(taskId: string, status: Task['status'], notes?: string) {
    if (!this.state.tasks) return false;
    
    const updatedTasks = this.state.tasks.map(task => {
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
    
    this.updateState({
      tasks: updatedTasks,
      observations: [...(this.state.observations || []), `Task ${taskId} status changed to ${status}`]
    });
    
    return true;
  }

  /**
   * Updates the agent's scratchpad with AI-generated structured thoughts
   */
  private async updateAgentScratchpad(prompt: string) {
    try {
      // Get recent context to enrich the prompt
      const recentMessages = this.state.messages?.slice(-3) || [];
      const currentFile = this.state.workingFilePath || '';
      const currentRepo = this.state.currentRepoOwner && this.state.currentRepoName ? 
        `${this.state.currentRepoOwner}/${this.state.currentRepoName}` : '';
      
      // Build a context-rich prompt
      let contextPrompt = `Based on the current context and the goal "${prompt}", what is your immediate thought process, potential next action, or any clarifying questions?`;
      
      // Add file context if available
      if (currentFile) {
        contextPrompt += `\nCurrently focused on file: ${currentFile}`;
      }
      
      // Add repo context if available
      if (currentRepo) {
        contextPrompt += `\nWorking in repository: ${currentRepo}${this.state.currentBranch ? ` (branch: ${this.state.currentBranch})` : ''}`;
      }
      
      // Add recent message context
      if (recentMessages.length > 0) {
        contextPrompt += '\n\nRecent messages:';
        recentMessages.forEach(msg => {
          contextPrompt += `\n${msg.role}: ${msg.content?.substring(0, 100)}${msg.content && msg.content.length > 100 ? '...' : ''}`;
        });
      }
      
      // Generate a structured thought based on the enriched prompt
      const { object: planning } = await generateObject({
        model: model,
        schema: PlanningSchema,
        prompt: contextPrompt,
      });

      const timestamp = new Date().toISOString();
      const formattedThought = `${timestamp}: ${planning.thought}${planning.nextAction ? ` | Next: ${planning.nextAction}` : ''}${planning.questions && planning.questions.length > 0 ? ` | Questions: ${planning.questions.join(', ')}` : ''}`;

      this.updateState({
        scratchpad: this.state.scratchpad
          ? `${this.state.scratchpad}\n- ${formattedThought}`
          : `- ${formattedThought}`
      });

    } catch (error) {
      console.error("Error generating structured thought:", error);
      // Fallback to just adding the raw prompt
      const timestamp = new Date().toISOString();
      this.updateState({
        scratchpad: this.state.scratchpad
          ? `${this.state.scratchpad}\n- ${timestamp}: ${prompt}`
          : `- ${timestamp}: ${prompt}`
      });
    }
  }

  /**
   * Adds an observation to the agent's state
   */
  private addAgentObservation(observation: string) {
    this.updateState({
      observations: [...(this.state.observations || []), observation]
    });
  }

  /**
   * Updates information about a file or directory in the codebase structure with AI-generated summary
   */
  private async updateCodebaseStructure(path: string, content: string | null, nodeType: 'file' | 'directory' = 'file') {
    console.log(`[updateCodebaseStructure] Starting for path: ${path}, nodeType: ${nodeType}`);
    console.log(`[updateCodebaseStructure] Content length: ${content ? content.length : 'null'}`);
    console.log(`[updateCodebaseStructure] Current state keys: ${Object.keys(this.state || {}).join(', ')}`);
    console.log(`[updateCodebaseStructure] Codebase exists: ${!!this.state.codebase}`);
    
    let summary = null;
    if (nodeType === 'file' && content) {
      try {
        console.log(`[updateCodebaseStructure] Preparing to generate file summary`);
        
        // Enhance the prompt with repository context
        let contextPrompt = `Summarize the following code file located at '${path}'. `;
        
        // Add file extension info
        const fileExtension = path.split('.').pop()?.toLowerCase();
        if (fileExtension) {
          contextPrompt += `This is a ${fileExtension} file. `;
        }
        
        // Add repository context
        if (this.state.currentRepoOwner && this.state.currentRepoName) {
          contextPrompt += `File is from repository: ${this.state.currentRepoOwner}/${this.state.currentRepoName}. `;
        }
        
        // Add specific instructions based on file type
        if (fileExtension === 'js' || fileExtension === 'ts' || fileExtension === 'tsx') {
          contextPrompt += `Focus on identifying exports (functions, classes, constants), dependencies (imports), and the main purpose of this module. `;
        } else if (fileExtension === 'css' || fileExtension === 'scss') {
          contextPrompt += `Focus on the main UI components or themes being styled. `;
        } else if (fileExtension === 'json') {
          contextPrompt += `Focus on the configuration purpose and key settings. `;
        }
        
        // Add examples of good tag types
        contextPrompt += `Add tags like "component", "api", "utility", "state-management", "auth", "ui", "database", "config", etc. that best describe this file's role. `;
        
        // For debugging, shorten content significantly
        const contentForAI = content.substring(0, 2000); // Shorter than normal for quicker debugging
        contextPrompt += `\n\n\`\`\`\n${contentForAI}\n\`\`\``; 
        
        console.log(`[updateCodebaseStructure] Calling generateObject`);
        
        const { object } = await generateObject({
          model: model,
          schema: FileSummarySchema,
          prompt: contextPrompt
        });
        
        console.log(`[updateCodebaseStructure] generateObject returned result: ${!!object}`);
        console.log(`[updateCodebaseStructure] Generated summary: ${object?.summary?.substring(0, 50) || 'none'}`);
        
        summary = object;
      } catch (error) {
        console.error(`[updateCodebaseStructure] Error generating file summary for ${path}:`, error);
      }
    } else {
      console.log(`[updateCodebaseStructure] Skipping summary generation - not a file or no content`);
    }

    const structure = this.state.codebase?.structure || {};
    const existingNode = structure[path] || { type: nodeType, path };

    const updatedNode: FileNode = {
      ...existingNode, // Keep existing info like type and path
      type: nodeType, // Ensure type is set
      path: path, // Ensure path is set
      // Add generated summary and tags if available
      description: summary?.summary || existingNode.description || `Accessed at ${new Date().toISOString()}`,
      tags: summary?.tags || existingNode.tags || [],
    };

    // Update the codebase structure in the state
    this.updateState({
      codebase: {
        ...(this.state.codebase || { structure: {} }),
        structure: {
          ...structure,
          [path]: updatedNode
        }
      }
    });

    // Also add an observation
    this.addAgentObservation(summary
      ? `Analyzed file: ${path}. Purpose: ${summary.summary.substring(0, 30)}...`
      : `Accessed ${nodeType}: ${path}`
    );
  }

  /**
   * Sets the file currently being worked on
   */
  private setCurrentFile(filePath: string) {
    this.updateState({
      workingFilePath: filePath
    });
  }

  @unstable_callable({
    description: "Generate an AI response based on the current messages",
    streaming: true
  })
  async infer(githubToken?: string) {
    return agentContext.run(this, async () => {
      // Add initial planning thought
      await this.updateAgentScratchpad("Processing user request and planning response");

      // Use githubToken from state if not provided as parameter
      const token = githubToken || this.state.githubToken;

      // Get current state messages
      let messages = this.state.messages || [];

      // If there's more than 10 messages, take the first 3 and last 5
      if (messages.length > 10) {
        messages = messages.slice(0, 3).concat(messages.slice(-5));
        console.log("Truncated messages to first 3 and last 5", messages);
      }

      const toolContext: ToolContext = { githubToken: token }
      const tools = {
        get_file_contents: getFileContentsTool(toolContext),
        add_issue_comment: addIssueCommentTool(toolContext),
        ...availableTools
      }

      // Generate system prompt based on current state
      const systemPrompt = getSystemPrompt({
        state: this.state,
        model,
        temperature: 0.7
      });

      const result = await generateText({
        system: systemPrompt,
        model,
        messages,
        tools,
        maxTokens: 5000,
        temperature: 0.7,
        maxSteps: 5,
      });
      
      // Debug logging for result structure
      console.log("[Debug] Text response exists:", !!result.text);
      console.log("[Debug] Text response length:", result.text?.length || 0);
      console.log("[Debug] Tool calls exist:", !!result.toolCalls);
      console.log("[Debug] Tool calls length:", result.toolCalls?.length || 0);
      console.log("[Debug] Tool results exist:", !!result.toolResults);
      console.log("[Debug] Tool results length:", result.toolResults?.length || 0);
      console.log("[Debug] Finish reason:", result.finishReason || "unknown");
      
      // Log full tool calls structure if it exists
      if (result.toolCalls && result.toolCalls.length > 0) {
        console.log("[Debug] Tool calls:", JSON.stringify(result.toolCalls, null, 2));
      }
      
      // Log full tool results structure if it exists
      if (result.toolResults && result.toolResults.length > 0) {
        console.log("[Debug] Tool results:", JSON.stringify(result.toolResults, null, 2));
      }

      // Add observation for the response and analyze for potential tasks
      if (result.text) {
        const snippet = result.text.length > 50 
          ? `${result.text.substring(0, 50)}...` 
          : result.text;
          
        this.addAgentObservation(`Generated response: ${snippet}`);
        
        // Check if this is in response to a user message requiring a task
        if (messages.length > 0 && messages[messages.length - 1].role === 'user') {
          const lastUserMessage = messages[messages.length - 1].content || '';
          const taskIndicators = [
            'implement', 'create', 'build', 'fix', 'add', 'refactor', 'optimize', 
            'update', 'develop', 'design', 'setup', 'write'
          ];
          
          // Check if the user message suggests a coding task
          if (taskIndicators.some(indicator => 
            lastUserMessage.toLowerCase().includes(indicator)) &&
            (lastUserMessage.includes('code') || lastUserMessage.includes('function') || 
             lastUserMessage.includes('class') || lastUserMessage.includes('file') || 
             lastUserMessage.includes('component'))) {
            
            // Create a task based on the user's request
            await this.generateAndAddTask(lastUserMessage);
          }
        }
      }

      // Create message parts array to handle both text and tool calls
      const messageParts: UIPart[] = [];

      // Add text part if there is text content
      if (result.text) {
        messageParts.push({
          type: 'text' as const,
          text: result.text
        });
      }

      // Add tool calls and their results together
      if (result.toolCalls && result.toolCalls.length > 0) {
        for (const toolCall of result.toolCalls) {
          console.log(`[Infer Loop] Processing toolCall: ${toolCall.toolName}`);
          
          // @ts-ignore - toolCall type issue
          const toolResult = result.toolResults?.find(r => r.toolCallId === toolCall.toolCallId);
          console.log(`[Infer Loop] Found toolResult: ${!!toolResult}`);
          
          // Log the specific tool call details
          console.log(`[Infer Loop] Tool Call ID: ${(toolCall as any).toolCallId || 'undefined'}`);
          console.log(`[Infer Loop] Tool Name: ${toolCall.toolName || 'undefined'}`);
          console.log(`[Infer Loop] Tool Args: ${JSON.stringify(toolCall.args || {})}`);

          // Add the tool call
          messageParts.push({
            type: 'tool-invocation' as const,
            toolInvocation: {
              state: 'call' as const,
              // @ts-ignore - toolCall type issue
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName as "getWeatherInformation",
              args: toolCall.args
            }
          });

          // Add observation for tool usage
          this.addAgentObservation(`Used tool: ${toolCall.toolName} with args: ${JSON.stringify(toolCall.args)}`);

          // Immediately add its result if available
          if (toolResult) {
            console.log(`[Infer Loop] Tool Result ID: ${(toolResult as any).toolCallId || 'undefined'}`);
            console.log(`[Infer Loop] Tool Result Type: ${typeof toolResult.result}`);
            console.log(`[Infer Loop] Tool Result Length: ${typeof toolResult.result === 'string' ? toolResult.result.length : 'not a string'}`);
            
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'result' as const,
                // @ts-ignore - toolCall type issue
                toolCallId: toolCall.toolCallId,
                toolName: toolCall.toolName as "getWeatherInformation",
                args: toolCall.args,
                // @ts-ignore - toolResult type issue
                result: toolResult.result
              }
            });

            // Add observation for tool result
            const resultSnippet = typeof toolResult.result === 'string' && toolResult.result.length > 50 
              ? `${toolResult.result.substring(0, 50)}...` 
              : JSON.stringify(toolResult.result).substring(0, 50) + '...';
              
            this.addAgentObservation(`Tool result from ${toolCall.toolName}: ${resultSnippet}`);
            
            console.log(`[Infer Loop] Checking condition for updateCodebaseStructure for tool: ${toolCall.toolName}`);
            
            // Update codebase structure if it was a file contents tool
            if (toolCall.toolName === 'get_file_contents' && typeof toolCall.args === 'object' && toolResult.result) {
              console.log('[Infer Loop] Condition MET for updateCodebaseStructure');
              
              const args = toolCall.args as { path?: string };
              console.log(`[Infer Loop] Args path: ${args.path || 'undefined'}`);
              
              if (args.path) {
                console.log(`[Infer Loop] Calling updateCodebaseStructure for path: ${args.path}`);
                
                try {
                  // Call the enhanced method with the file content
                  await this.updateCodebaseStructure(args.path, toolResult.result as string, 'file');
                  // Also set current file
                  this.setCurrentFile(args.path);
                  console.log(`[Infer Loop] Successfully completed updateCodebaseStructure for ${args.path}`);
                } catch (error) {
                  console.error(`[Infer Loop] Error in updateCodebaseStructure: ${error}`);
                }
              } else {
                console.log('[Infer Loop] Missing path in args');
              }
            } else {
              console.log('[Infer Loop] Condition FAILED for updateCodebaseStructure');
              console.log(`  - toolName === 'get_file_contents': ${toolCall.toolName === 'get_file_contents'}`);
              console.log(`  - typeof args === 'object': ${typeof toolCall.args === 'object'}`);
              console.log(`  - toolResult.result is truthy: ${!!toolResult.result}`);
            }
          }
        }
      }

      // Add a thought about the interaction
      if (messages.length > 0 && messages[messages.length - 1].role === 'user') {
        const lastUserMessage = messages[messages.length - 1].content;
        await this.updateAgentScratchpad(`Analyzing message: ${lastUserMessage}`);
      }

      // Finally, update state with the new message
      this.updateState({
        messages: [
          ...messages,
          {
            id: generateId(),
            role: 'assistant' as const,
            content: result.text || '',
            createdAt: new Date(),
            parts: messageParts
          }
        ]
      });

      return {};
    });
  }
}

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env, { cors: true })) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;