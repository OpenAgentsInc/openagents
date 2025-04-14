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
  summary: z.string().describe("A brief summary of the file's purpose and key contents (1-3 sentences)."),
  tags: z.array(z.string()).describe("Keywords or tags describing the file's functionality (e.g., 'auth', 'api-route', 'database', 'component', 'utility')."),
  exports: z.array(z.string()).optional().describe("Key functions, classes, or variables exported by the file."),
  dependencies: z.array(z.string()).optional().describe("Important libraries, modules, or files this file depends on."),
  complexity: z.enum(["low", "medium", "high"]).optional().describe("Assessment of the file's complexity."),
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
        
        // Create a comprehensive prompt for generating a structured file summary
        let contextPrompt = `Analyze the following code file located at '${path}' and generate a structured summary. `;
        
        // Add file extension info
        const fileExtension = path.split('.').pop()?.toLowerCase();
        if (fileExtension) {
          contextPrompt += `This is a ${fileExtension} file. `;
        }
        
        // Add repository context
        if (this.state.currentRepoOwner && this.state.currentRepoName) {
          contextPrompt += `File is from repository: ${this.state.currentRepoOwner}/${this.state.currentRepoName}${this.state.currentBranch ? ` (branch: ${this.state.currentBranch})` : ''}. `;
        }
        
        // Add specific instructions based on file type
        contextPrompt += '\nPlease provide: \n';
        contextPrompt += '1. A concise summary (1-3 sentences) of the file\'s purpose and functionality\n';
        contextPrompt += '2. Appropriate tags categorizing this file (minimum 3 tags)\n';
        
        if (fileExtension === 'js' || fileExtension === 'ts' || fileExtension === 'tsx') {
          contextPrompt += '3. A list of key exports (functions, classes, components, constants)\n';
          contextPrompt += '4. Important dependencies (imports) this file relies on\n';
          contextPrompt += '5. Assess the code complexity (low/medium/high)\n';
        } else if (fileExtension === 'css' || fileExtension === 'scss') {
          contextPrompt += '3. Main UI components or elements being styled\n';
          contextPrompt += '4. Any dependencies like imported fonts or other style files\n';
          contextPrompt += '5. Assess the styling complexity (low/medium/high)\n';
        } else if (fileExtension === 'json') {
          contextPrompt += '3. Key configuration properties and their purpose\n';
          contextPrompt += '4. Related files or systems that likely use this configuration\n';
          contextPrompt += '5. Assess the configuration complexity (low/medium/high)\n';
        } else {
          contextPrompt += '3. Key elements defined in this file\n';
          contextPrompt += '4. Related files or systems\n';
          contextPrompt += '5. Assess the complexity (low/medium/high)\n';
        }
        
        // Add examples of good tag types
        contextPrompt += `\nUse appropriate tags from categories such as: "component", "api", "utility", "state-management", "auth", "ui", "database", "config", "tool", "model", "type-definition", "server", "client", "test", etc. Be specific to the file's role in the codebase.\n`;
        
        // Limit content length for faster processing
        const contentForAI = content.substring(0, 3000); // Using 3000 instead of 2000 for better context
        contextPrompt += `\n\`\`\`\n${contentForAI}\n\`\`\``;
        
        // Add additional constraint for consistent output
        contextPrompt += '\nKeep your summary focused on the technical details and main functionality, avoiding subjective judgments about code quality unless there are obvious issues.'; 
        
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
      // Add additional metadata from the summary
      metadata: {
        ...(existingNode.metadata || {}),
        ...(summary ? {
          exports: summary.exports || [],
          dependencies: summary.dependencies || [],
          complexity: summary.complexity || 'medium',
          lastAnalyzed: new Date().toISOString(),
        } : {})
      }
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
      
      // Log steps array information
      console.log("[Debug] Steps array exists:", !!result.steps);
      console.log("[Debug] Steps array length:", result.steps?.length || 0);
      if (result.steps && result.steps.length > 0) {
        console.log("[Debug] Steps array content:", JSON.stringify(result.steps, null, 2));
      }
      
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

      // Process tool calls and results from the steps array - but don't add to messageParts
      // We only process tools to update agent state (codebase, etc.)
      const toolInfoMap = new Map<string, { call: any; result?: any }>();

      if (result.steps && result.steps.length > 0) {
        console.log("[Consolidate] Processing steps to gather tool info...");
        for (const step of result.steps) {
          console.log(`[Consolidate] Processing step with type: ${step.stepType}, finishReason: ${step.finishReason}`);
          
          // Collect tool calls from this step
          if (step.toolCalls && step.toolCalls.length > 0) {
            for (const toolCall of step.toolCalls) {
              // @ts-ignore
              const toolCallId = toolCall.toolCallId;
              if (toolCallId && !toolInfoMap.has(toolCallId)) {
                // Only store the first call encountered for an ID
                toolInfoMap.set(toolCallId, { call: toolCall });
                console.log(`[Consolidate] Stored call for ${toolCallId} (${toolCall.toolName})`);
              }
            }
          }
          
          // Collect tool results from this step
          if (step.toolResults && step.toolResults.length > 0) {
            for (const toolResult of step.toolResults) {
              // @ts-ignore
              const toolCallId = toolResult.toolCallId;
              const existingInfo = toolInfoMap.get(toolCallId);
              if (toolCallId && existingInfo) {
                // Add the result to the existing call info
                existingInfo.result = toolResult;
                console.log(`[Consolidate] Added result for ${toolCallId}`);
              } else if (toolCallId) {
                // If result appears without a prior call (unlikely but possible)
                console.warn(`[Consolidate] Found toolResult for ${toolCallId} without a preceding call.`);
              }
            }
          }
        } // end loop through steps
        
        // Also look for results in the top-level result.toolResults
        if (result.toolResults && result.toolResults.length > 0) {
          for (const toolResult of result.toolResults) {
            // @ts-ignore
            const toolCallId = toolResult.toolCallId;
            const existingInfo = toolInfoMap.get(toolCallId);
            if (toolCallId && existingInfo) {
              // Add the result to the existing call info (if not already set)
              if (!existingInfo.result) {
                existingInfo.result = toolResult;
                console.log(`[Consolidate] Added result from top-level for ${toolCallId}`);
              }
            }
          }
        }
      } else {
        console.log("[Consolidate] No steps array found or it is empty.");
        
        // Fall back to the original approach if steps array is empty
        if (result.toolCalls && result.toolCalls.length > 0) {
          console.log("[Consolidate] Using original toolCalls array as fallback");
          for (const toolCall of result.toolCalls) {
            // @ts-ignore
            const toolCallId = toolCall.toolCallId;
            if (toolCallId) {
              toolInfoMap.set(toolCallId, { call: toolCall });
              
              // Try to find matching result in top-level results
              if (result.toolResults && result.toolResults.length > 0) {
                // @ts-ignore
                const toolResult = result.toolResults.find(r => r.toolCallId === toolCallId);
                if (toolResult) {
                  const info = toolInfoMap.get(toolCallId);
                  if (info) {
                    info.result = toolResult;
                  }
                }
              }
            }
          }
        }
      }

      // Process the tool calls to update state, but DON'T add to messageParts
      console.log(`[Process Tools] Processing ${toolInfoMap.size} consolidated tool invocations for state updates.`);
      for (const [toolCallId, info] of toolInfoMap.entries()) {
        const { call: toolCall, result: toolResult } = info;

        // Process tool call and result, updating state and messageParts
        if (toolCall) {
          // Add observation for tool usage
          this.addAgentObservation(`Used tool: ${toolCall.toolName} with args: ${JSON.stringify(toolCall.args)}`);
          
          // If we have a result, add it to messageParts and process it for state updates
          if (toolResult) {
            console.log(`[Process Tools] Adding 'result' part for ${toolCallId} to messageParts`);
            // Add the tool result part to messageParts
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'result' as const,
                toolCallId: toolCallId,
                toolName: toolCall.toolName as any,
                args: toolCall.args,
                result: toolResult.result
              }
            });
            
            // Add observation for tool result
            const resultSnippet = typeof toolResult.result === 'string' && toolResult.result.length > 50
              ? `${toolResult.result.substring(0, 50)}...`
              : JSON.stringify(toolResult.result).substring(0, 50) + '...';
            this.addAgentObservation(`Tool result from ${toolCall.toolName}: ${resultSnippet}`);

            // --- Update codebase logic ---
            console.log(`[Process Tools] Checking condition for updateCodebaseStructure for tool: ${toolCall.toolName}`);
            if (toolCall.toolName === 'get_file_contents' && typeof toolCall.args === 'object' && toolResult.result) {
              console.log('[Process Tools] Condition MET for updateCodebaseStructure');
              
              const args = toolCall.args as { path?: string };
              
              // Extract the base64 content correctly from the nested result object
              const fileContentBase64 = (toolResult.result as any)?.content;
              let fileContentDecoded: string | null = null;

              if (typeof fileContentBase64 === 'string') {
                console.log(`[Process Tools] Found base64 content string (length: ${fileContentBase64.length})`);
                try {
                  // Use Buffer for more robust decoding
                  // First, ensure no invalid characters (like newlines) - remove ALL whitespace
                  const cleanBase64 = fileContentBase64.replace(/\s/g, '');
                  fileContentDecoded = Buffer.from(cleanBase64, 'base64').toString('utf8');
                  console.log(`[Process Tools] Decoded file content successfully using Buffer (length: ${fileContentDecoded?.length || 0})`);
                } catch (e) {
                  console.error(`[Process Tools] Error decoding base64 content for ${args.path} using Buffer:`, e);
                  
                  // Try a fallback method if Buffer fails
                  try {
                    console.log(`[Process Tools] Attempting fallback decoding method...`);
                    // For Cloudflare Workers environment - different approach
                    const cleanBase64 = fileContentBase64.replace(/[\s\r\n]+/g, '');
                    const padded = cleanBase64.padEnd(cleanBase64.length + (4 - cleanBase64.length % 4) % 4, '=');
                    fileContentDecoded = atob(padded);
                    console.log(`[Process Tools] Fallback decoding succeeded (length: ${fileContentDecoded?.length || 0})`);
                  } catch (fallbackError) {
                    console.error(`[Process Tools] Fallback decoding also failed:`, fallbackError);
                    // Keep fileContentDecoded as null if all decoding fails
                  }
                }
              } else {
                console.log(`[Process Tools] File content in tool result was not a string or was missing.`);
              }

              console.log(`[Process Tools] Args path: ${args.path || 'undefined'}`);
              if (args.path && fileContentDecoded) {
                console.log(`[Process Tools] Calling updateCodebaseStructure for path: ${args.path}`);
                try {
                  // Pass the decoded content
                  await this.updateCodebaseStructure(args.path, fileContentDecoded, 'file');
                  this.setCurrentFile(args.path);
                  console.log(`[Process Tools] Successfully completed updateCodebaseStructure for ${args.path}`);
                } catch (error) {
                  console.error(`[Process Tools] Error in updateCodebaseStructure: ${error}`);
                }
              } else {
                console.log('[Process Tools] Missing path in args or no file content decoded');
              }
            }
            // --- End Update codebase logic ---
          } else {
            // If we only have the call (tool hasn't finished or result wasn't found), push the call part
            console.log(`[Process Tools] Adding 'call' part for ${toolCallId} to messageParts`);
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'call' as const,
                toolCallId: toolCallId,
                toolName: toolCall.toolName as any,
                args: toolCall.args
              }
            });
          }
        }
      } // end loop through consolidated map

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