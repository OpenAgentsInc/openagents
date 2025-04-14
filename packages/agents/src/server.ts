import { Agent, routeAgentRequest, unstable_callable, type Connection, type Schedule, type WSMessage } from "agents"
import { type UIMessage, generateId, generateText, experimental_createMCPClient as createMCPClient, type ToolSet } from "ai";
import { env } from "cloudflare:workers";
import { AsyncLocalStorage } from "node:async_hooks";
import type { UIPart } from "@openagents/core/src/chat/types";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { type ToolContext } from "@openagents/core/src/tools/toolContext";
import { getFileContentsTool } from "@openagents/core/src/tools/github/getFileContents";
import { addIssueCommentTool } from "@openagents/core/src/tools/github/addIssueComment";
import { tools as availableTools } from "./tools";
import { getSystemPrompt } from "./prompts";
import { CoderState, Task, FileNode } from "./types";

const openrouter = createOpenRouter({ apiKey: env.OPENROUTER_API_KEY })
const model = openrouter("google/gemini-2.5-pro-preview-03-25");

export const agentContext = new AsyncLocalStorage<Coder>();

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

  async executeTask(description: string, task: Schedule<string>) {
    this.setState({
      ...this.state,
      messages: [
        ...this.state.messages,
        {
          id: generateId(),
          role: "user",
          content: `This is a scheduled notice for you to now execute the following task: ${description}`,
          createdAt: new Date(),
          parts: [
            {
              type: "text",
              text: `This is a scheduled notice for you to now execute the following task: ${description}`
            }
          ],
        },
      ],
    });
    // now infer based on this message
    await this.infer();
  }

  onMessage(connection: Connection, message: WSMessage) {
    const parsedMessage = JSON.parse(message as string);
    console.log("IN ON MESSAGE AND HAVE PARSED MESSAGE", parsedMessage);
    const githubToken = parsedMessage.githubToken;
    // Store the githubToken in state
    this.setState({
      ...this.state,
      githubToken
    });
    this.infer()
  }

  /**
   * Sets the current repository context
   */
  @unstable_callable({
    description: "Set the current repository context"
  })
  async setRepositoryContext(owner: string, repo: string, branch: string = 'main') {
    console.log(`Setting repository context to ${owner}/${repo} on branch ${branch}`);
    this.setState({
      ...this.state,
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
    
    this.setState({
      ...this.state,
      tasks: [...(this.state.tasks || []), newTask],
      observations: [...(this.state.observations || []), `New task added: ${description}`]
    });
    
    return newTask.id;
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
    
    this.setState({
      ...this.state,
      tasks: updatedTasks,
      observations: [...(this.state.observations || []), `Task ${taskId} status changed to ${status}`]
    });
    
    return true;
  }

  /**
   * Updates the agent's scratchpad for planning and thoughts
   */
  private updateAgentScratchpad(thought: string) {
    const timestamp = new Date().toISOString();
    const formattedThought = `${timestamp}: ${thought}`;
    
    this.setState({
      ...this.state,
      scratchpad: this.state.scratchpad 
        ? `${this.state.scratchpad}\n- ${formattedThought}` 
        : `- ${formattedThought}`
    });
  }

  /**
   * Adds an observation to the agent's state
   */
  private addAgentObservation(observation: string) {
    this.setState({
      ...this.state,
      observations: [...(this.state.observations || []), observation]
    });
  }

  /**
   * Updates information about a file or directory in the codebase structure
   */
  private updateCodebaseStructure(path: string, nodeInfo: Partial<FileNode>) {
    const structure = this.state.codebase?.structure || {};
    const existingNode = structure[path] || { type: nodeInfo.type || 'file', path };
    
    const updatedNode = {
      ...existingNode,
      ...nodeInfo
    };
    
    this.setState({
      ...this.state,
      codebase: {
        ...(this.state.codebase || {}),
        structure: {
          ...structure,
          [path]: updatedNode
        }
      }
    });
  }

  /**
   * Sets the file currently being worked on
   */
  private setCurrentFile(filePath: string) {
    this.setState({
      ...this.state,
      workingFilePath: filePath
    });
  }

  @unstable_callable({
    description: "Generate an AI response based on the current messages",
    streaming: true
  })
  async infer(githubToken?: string) {
    return agentContext.run(this, async () => {
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

      // Add observation for the response
      if (result.text) {
        const snippet = result.text.length > 50 
          ? `${result.text.substring(0, 50)}...` 
          : result.text;
          
        this.addAgentObservation(`Generated response: ${snippet}`);
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
          // @ts-ignore - toolCall type issue
          const toolResult = result.toolResults?.find(r => r.toolCallId === toolCall.toolCallId);

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
            
            // Update codebase structure if it was a file contents tool
            if (toolCall.toolName === 'get_file_contents' && typeof toolCall.args === 'object') {
              const args = toolCall.args as any;
              if (args.path) {
                this.setCurrentFile(args.path);
                this.updateCodebaseStructure(args.path, {
                  type: 'file', 
                  path: args.path,
                  description: `File retrieved at ${new Date().toISOString()}`
                });
              }
            }
          }
        }
      }

      // Update state with the new message containing all parts
      this.setState({
        ...this.state,
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
    })
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
