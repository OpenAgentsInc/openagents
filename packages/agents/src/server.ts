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
const smallModel = openrouter("openai/gpt-4o-mini");

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
    workingFilePath: undefined,
    isContinuousRunActive: false
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

  async executeTask(payload: Record<string, any>, task: Schedule<Record<string, any>>) {
    const description = payload.description || "Scheduled task executed";
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
  
  /**
   * Decides the next action for the continuous run, schedules it,
   * and reschedules itself.
   */
  public async continueInfer(payload?: any) {
    // STATE LOGGING
    console.log(`[continueInfer STATE CHECK] Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}, Active: ${this.state.isContinuousRunActive}`);
    
    console.log(`[continueInfer] Cycle start. Active: ${this.state.isContinuousRunActive}. Payload: ${JSON.stringify(payload)}`);
    if (!this.state.isContinuousRunActive) {
      console.log(`[continueInfer] Run inactive. Stopping.`);
      return;
    }

    try {
      // --- Decide NEXT SINGLE Action ---
      const nextAction = this.planNextExplorationStep();

      if (nextAction) {
        console.log(`[continueInfer] Planning next action: ${nextAction.type} - ${nextAction.path || nextAction.description}`);

        // Schedule the *specific* action using its own method and payload
        // Use a short delay for the action itself (e.g., 5 seconds)
        await this.schedule(5, // Short delay to execute the action soon
          nextAction.type === 'listFiles' ? 'scheduledListFiles' : 'scheduledSummarizeFile',
          nextAction.payload
        );
        this.addAgentObservation(`Scheduled next action: ${nextAction.type} for ${nextAction.path || 'N/A'}`);
      } else {
        console.log("[continueInfer] No further exploration steps planned for now.");
        // Add generic observation about continuing exploration
        this.addAgentObservation("No specific exploration step found. Waiting for next planning cycle.");
        
        // We don't schedule any immediate action in this case
        // Just let the next planning cycle happen after the standard delay
      }

      // --- Reschedule continueInfer for the *next planning cycle* ---
      const planningIntervalSeconds = 120; // Longer interval (2 minutes) for planning the next step
      console.log(`[continueInfer] Rescheduling planning cycle in ${planningIntervalSeconds} seconds.`);
      await this.schedule(planningIntervalSeconds, 'continueInfer', { reason: 'next planning cycle' });

    } catch (error) {
      console.error("[continueInfer] Error during planning or scheduling:", error);
      // Reschedule self even on error
      if (this.state.isContinuousRunActive) {
        const errorDelaySeconds = 300;
        console.log(`[continueInfer] Rescheduling planning cycle after error in ${errorDelaySeconds} seconds.`);
        await this.schedule(errorDelaySeconds, 'continueInfer', { reason: 'error recovery planning' });
      }
    }
  }
  
  /**
   * Method to determine the next exploration step.
   */
  private planNextExplorationStep(): { type: 'listFiles' | 'summarizeFile'; path?: string; description?: string; payload: any } | null {
    console.log("[planNextExplorationStep] Deciding next step...");
    
    // CHECK FOR REPOSITORY CONTEXT
    if (!this.state.currentRepoOwner || !this.state.currentRepoName) {
      console.warn("[planNextExplorationStep] Repository context (owner/name) not set. Cannot plan file/dir actions.");
      // Add an observation asking the user to set context
      this.addAgentObservation("Please set the repository context using 'setRepositoryContext' before starting exploration.");
      // Since we can't proceed without repo context, consider stopping the continuous run
      this.stopContinuousRun().catch(e => console.error("Error stopping continuous run after missing repo context:", e));
      return null; // Cannot plan without context
    }
    
    // Get current state of exploration
    const codebaseStructure = this.state.codebase?.structure || {};
    const filesExplored = Object.values(codebaseStructure).filter(file => file.type === 'file');
    const directoriesExplored = Object.values(codebaseStructure).filter(file => file.type === 'directory');
    
    console.log(`[planNextExplorationStep] Files explored: ${filesExplored.length}, Directories explored: ${directoriesExplored.length}`);
    
    // First priority: Check if root directory has been listed
    if (!codebaseStructure['/']) {
      console.log("[planNextExplorationStep] Planning: List root directory");
      return {
        type: 'listFiles',
        path: '/',
        description: 'List repository root directory',
        payload: {
          path: '/',
          owner: this.state.currentRepoOwner,
          repo: this.state.currentRepoName,
          branch: this.state.currentBranch || 'main'
        }
      };
    }
    
    // Second priority: List key directories that haven't been explored yet
    const importantDirectories = ['src', 'packages', 'lib', 'docs', 'app'];
    for (const dir of importantDirectories) {
      if (!codebaseStructure[dir] && !codebaseStructure[`/${dir}`]) {
        const path = dir.startsWith('/') ? dir : `/${dir}`;
        console.log(`[planNextExplorationStep] Planning: List important directory '${path}'`);
        return {
          type: 'listFiles',
          path,
          description: `List '${path}' directory`,
          payload: {
            path,
            owner: this.state.currentRepoOwner,
            repo: this.state.currentRepoName,
            branch: this.state.currentBranch || 'main'
          }
        };
      }
    }
    
    // Third priority: Find a file to summarize that hasn't been well-analyzed
    const fileToSummarize = Object.values(codebaseStructure)
      .find(file => 
        file.type === 'file' && 
        (!file.description || file.description === `Accessed at ${new Date().toISOString()}`)
      );
    
    if (fileToSummarize) {
      console.log(`[planNextExplorationStep] Planning: Summarize file '${fileToSummarize.path}'`);
      return {
        type: 'summarizeFile',
        path: fileToSummarize.path,
        description: `Summarize file '${fileToSummarize.path}'`,
        payload: {
          path: fileToSummarize.path,
          owner: this.state.currentRepoOwner,
          repo: this.state.currentRepoName,
          branch: this.state.currentBranch || 'main'
        }
      };
    }
    
    // Fourth priority: Explore subdirectories of already listed directories
    for (const dir of directoriesExplored) {
      // Look for important subfolders like 'src', 'components', etc.
      const dirPath = dir.path;
      const importantSubdirs = ['components', 'utils', 'hooks', 'pages', 'api', 'lib', 'services'];
      
      for (const subdir of importantSubdirs) {
        const subdirPath = dirPath.endsWith('/') 
          ? `${dirPath}${subdir}` 
          : `${dirPath}/${subdir}`;
        
        if (!codebaseStructure[subdirPath]) {
          console.log(`[planNextExplorationStep] Planning: List subdirectory '${subdirPath}'`);
          return {
            type: 'listFiles',
            path: subdirPath,
            description: `List '${subdirPath}' directory`,
            payload: {
              path: subdirPath,
              owner: this.state.currentRepoOwner,
              repo: this.state.currentRepoName,
              branch: this.state.currentBranch || 'main'
            }
          };
        }
      }
    }
    
    console.log("[planNextExplorationStep] No specific next step found.");
    return null; // No specific action decided for now
  }
  
  /**
   * Fetches directory contents from GitHub API
   * @private Helper method to fetch directory listing
   */
  private async fetchDirectoryContents(path: string, owner: string, repo: string, branch: string = 'main'): Promise<any[] | null> {
    console.log(`[fetchDirectoryContents] Fetching directory: ${path} from ${owner}/${repo}:${branch}`);
    
    if (!this.state.githubToken) {
      console.error("[fetchDirectoryContents] No GitHub token available");
      return null;
    }
    
    const token = this.state.githubToken;
    const normalizedPath = path.startsWith('/') ? path.substring(1) : path;
    
    try {
      const url = `https://api.github.com/repos/${owner}/${repo}/contents/${normalizedPath}?ref=${branch}`;
      console.log(`[fetchDirectoryContents] Making API request to: ${url}`);
      
      const response = await fetch(url, {
        headers: {
          'Authorization': `token ${token}`,
          'Accept': 'application/vnd.github.v3+json',
          'User-Agent': 'OpenAgents'
        }
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        console.error(`[fetchDirectoryContents] GitHub API error: ${response.status} ${response.statusText}`, errorText);
        return null;
      }
      
      const data = await response.json();
      
      // Check if it's an array (directory listing) or object (single file)
      if (Array.isArray(data)) {
        console.log(`[fetchDirectoryContents] Successfully fetched directory with ${data.length} items`);
        return data;
      } else {
        console.error(`[fetchDirectoryContents] Expected directory listing, got single file response`);
        return null;
      }
    } catch (error) {
      console.error(`[fetchDirectoryContents] Error fetching directory contents:`, error);
      return null;
    }
  }
  
  /**
   * Fetches and decodes file content from GitHub API
   * @private Helper method to fetch and decode file content
   */
  private async fetchFileContent(path: string, owner: string, repo: string, branch: string = 'main'): Promise<string | null> {
    console.log(`[fetchFileContent] Fetching file: ${path} from ${owner}/${repo}:${branch}`);
    
    if (!this.state.githubToken) {
      console.error("[fetchFileContent] No GitHub token available");
      return null;
    }
    
    const token = this.state.githubToken;
    const normalizedPath = path.startsWith('/') ? path.substring(1) : path;
    
    try {
      const url = `https://api.github.com/repos/${owner}/${repo}/contents/${normalizedPath}?ref=${branch}`;
      console.log(`[fetchFileContent] Making API request to: ${url}`);
      
      const response = await fetch(url, {
        headers: {
          'Authorization': `token ${token}`,
          'Accept': 'application/vnd.github.v3+json',
          'User-Agent': 'OpenAgents'
        }
      });
      
      if (!response.ok) {
        const errorText = await response.text();
        console.error(`[fetchFileContent] GitHub API error: ${response.status} ${response.statusText}`, errorText);
        return null;
      }
      
      const data = await response.json();
      
      // Check if it's a file (has content property)
      if (data.content && data.encoding === 'base64') {
        console.log(`[fetchFileContent] Successfully fetched file content, decoding from base64`);
        
        try {
          // Use the same robust decoding approach as in the tool processing
          const cleanBase64 = data.content.replace(/\s/g, '');
          const binaryStr = Buffer.from(cleanBase64, 'base64');
          const decodedContent = new TextDecoder().decode(binaryStr);
          console.log(`[fetchFileContent] Successfully decoded content (length: ${decodedContent.length})`);
          return decodedContent;
        } catch (e) {
          console.error(`[fetchFileContent] Error decoding with TextDecoder:`, e);
          
          // Fallback to simpler method
          try {
            const cleanBase64 = data.content.replace(/[\s\r\n]+/g, '');
            const decodedContent = Buffer.from(cleanBase64, 'base64').toString('utf8');
            console.log(`[fetchFileContent] Alternative decode succeeded (length: ${decodedContent.length})`);
            return decodedContent;
          } catch (fallbackError) {
            console.error(`[fetchFileContent] All decode methods failed:`, fallbackError);
            return null;
          }
        }
      } else {
        console.error(`[fetchFileContent] Expected file with content, got:`, typeof data);
        return null;
      }
    } catch (error) {
      console.error(`[fetchFileContent] Error fetching file content:`, error);
      return null;
    }
  }

  /**
   * Method specifically scheduled to list files for a path.
   * Only performs the directory listing operation, without calling infer().
   */
  public async scheduledListFiles(payload: { path: string, owner?: string, repo?: string, branch?: string }) {
    // STATE LOGGING
    console.log(`[scheduledListFiles STATE CHECK] Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}`);
    
    console.log(`[scheduledListFiles] Executing for path: ${payload.path}`);
    const { path, owner, repo, branch } = payload;
    
    // Check if payload has all required fields
    if (!path) {
      console.error("[scheduledListFiles] Missing path in payload.");
      return;
    }
    
    // If owner/repo missing in payload, try to get from state
    const effectiveOwner = owner || this.state.currentRepoOwner;
    const effectiveRepo = repo || this.state.currentRepoName;
    const effectiveBranch = branch || this.state.currentBranch || 'main';
    
    if (!effectiveOwner || !effectiveRepo) {
      console.error("[scheduledListFiles] Missing owner or repo in both payload and state.");
      this.addAgentObservation("Cannot list files: Repository owner/name not provided. Please set repository context first.");
      return;
    }
    
    try {
      // Add an observation about listing files
      this.addAgentObservation(`Listing files for: ${path}`);
      
      // Directly fetch directory contents using GitHub API
      const listing = await this.fetchDirectoryContents(
        path, 
        effectiveOwner, 
        effectiveRepo, 
        effectiveBranch
      );
      
      if (listing === null) {
        throw new Error(`Failed to fetch directory contents for ${path}`);
      }
      
      // Update the codebase structure for the directory
      this.updateCodebaseStructure(path, null, 'directory');
      
      // Process each item in the directory listing
      for (const item of listing) {
        const itemPath = path.endsWith('/') ? `${path}${item.name}` : `${path}/${item.name}`;
        const itemType = item.type === 'dir' ? 'directory' : 'file';
        
        // Add an entry in the codebase structure for each item
        // For files, we just add a basic entry - they'll be summarized later if needed
        this.updateCodebaseStructure(itemPath, null, itemType);
      }
      
      // Add an observation with the results
      this.addAgentObservation(`Listed ${listing.length} items in directory ${path}`);
      console.log(`[scheduledListFiles] Successfully processed directory ${path} with ${listing.length} items`);
      
    } catch(e) {
      console.error(`[scheduledListFiles] Error listing ${path}:`, e);
      this.addAgentObservation(`Error listing files for ${path}: ${e.message}`);
    }
  }
  
  /**
   * Method specifically scheduled to summarize a file.
   * Only performs the file fetching and summarization, without calling infer().
   */
  public async scheduledSummarizeFile(payload: { path: string, owner?: string, repo?: string, branch?: string }) {
    // STATE LOGGING
    console.log(`[scheduledSummarizeFile STATE CHECK] Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}`);
    
    console.log(`[scheduledSummarizeFile] Executing for path: ${payload.path}`);
    const { path, owner, repo, branch } = payload;
    
    // Check if payload has all required fields
    if (!path) {
      console.error("[scheduledSummarizeFile] Missing path in payload.");
      return;
    }
    
    // If owner/repo missing in payload, try to get from state
    const effectiveOwner = owner || this.state.currentRepoOwner;
    const effectiveRepo = repo || this.state.currentRepoName;
    const effectiveBranch = branch || this.state.currentBranch || 'main';
    
    if (!effectiveOwner || !effectiveRepo) {
      console.error("[scheduledSummarizeFile] Missing owner or repo in both payload and state.");
      this.addAgentObservation("Cannot summarize file: Repository owner/name not provided. Please set repository context first.");
      return;
    }
    
    try {
      // Add an observation about summarizing the file
      this.addAgentObservation(`Summarizing file: ${path}`);
      
      // Directly fetch file content using GitHub API
      const fileContent = await this.fetchFileContent(
        path, 
        effectiveOwner, 
        effectiveRepo, 
        effectiveBranch
      );
      
      if (fileContent === null) {
        throw new Error(`Failed to fetch content for ${path}`);
      }
      
      // Update the codebase structure with the file content
      // This will trigger the summary generation via generateObject
      await this.updateCodebaseStructure(path, fileContent, 'file');
      
      // Set as current file to help with context
      this.setCurrentFile(path);
      
      // Add success observation
      this.addAgentObservation(`Successfully summarized file: ${path}`);
      console.log(`[scheduledSummarizeFile] Successfully summarized ${path}`);
      
    } catch(e) {
      console.error(`[scheduledSummarizeFile] Error summarizing ${path}:`, e);
      this.addAgentObservation(`Error summarizing file ${path}: ${e.message}`);
    }
  }
  
  /**
   * Starts continuous agent execution
   */
  async startContinuousRun() {
    this.updateState({
      isContinuousRunActive: true,
      observations: [...(this.state.observations || []), "Starting continuous agent execution"]
    });
    
    // Immediately start the first execution
    await this.continueInfer({ reason: 'initial start' });
    
    return { success: true, message: "Continuous run started successfully" };
  }
  
  /**
   * Stops continuous agent execution
   */
  async stopContinuousRun() {
    this.updateState({
      isContinuousRunActive: false,
      observations: [...(this.state.observations || []), "Stopping continuous agent execution"]
    });
    
    // Cancel any pending continueInfer schedules
    try {
      const schedules = this.getSchedules();
      for (const schedule of schedules) {
        if (schedule.callback === 'continueInfer') {
          await this.cancelSchedule(schedule.id);
          console.log(`[stopContinuousRun] Cancelled schedule ${schedule.id} for continueInfer`);
        }
      }
    } catch (error) {
      console.error("[stopContinuousRun] Error cancelling continueInfer schedules:", error);
    }
    
    return { success: true, message: "Continuous run stopped successfully" };
  }

  onMessage(connection: Connection, message: WSMessage) {
    try {
      const parsedMessage = JSON.parse(message as string);
      console.log("ON MESSAGE RECEIVED:", parsedMessage);

      // --- Add Command Handling ---
      if (parsedMessage.type === 'command' && parsedMessage.command) {
        console.log(`Processing command: ${parsedMessage.command}`);
        switch (parsedMessage.command) {
          case 'startContinuousRun':
            // Don't await here, let it run in the background
            this.startContinuousRun().catch(e => console.error("Error starting continuous run from command:", e));
            break;
          case 'stopContinuousRun':
            // Don't await here
            this.stopContinuousRun().catch(e => console.error("Error stopping continuous run from command:", e));
            break;
          // Add other commands here if needed in the future
          default:
            console.warn(`Received unknown command: ${parsedMessage.command}`);
        }
        // Don't call infer() after a command, let the continuous run handle it
        return; // Exit after processing command
      }
      // --- End Command Handling ---

      // --- Existing GitHub Token Logic ---
      // Check if it's a message containing the token
      if (parsedMessage.githubToken) {
        console.log("Processing githubToken update...");
        const githubToken = parsedMessage.githubToken;
        this.updateState({
          githubToken
        });
        // Call infer after updating token
        this.infer();
        return; // Exit after processing token
      }

      // If message is none of the above, log a warning
      console.warn("Received unhandled message structure via send():", parsedMessage);

    } catch (error) {
      console.error("Error processing received message:", error);
      console.error("Raw message data:", message);
    }
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
  addAgentTask(description: string, scheduleId?: string, payload?: Record<string, any>, callbackMethodName?: string) {
    const newTask: Task = {
      id: generateId(),
      description,
      status: 'pending',
      created: new Date(),
      scheduleId,
      payload,
      callbackMethodName
    };

    this.updateState({
      tasks: [...(this.state.tasks || []), newTask],
      observations: [...(this.state.observations || []), `New task added: ${description}`]
    });

    return newTask.id;
  }
  
  /**
   * Cancels a task associated with a specific schedule ID
   */
  cancelTaskByScheduleId(scheduleId: string) {
    if (!this.state.tasks) return false;
    
    const updatedTasks = this.state.tasks.map(task => {
      if (task.scheduleId === scheduleId) {
        return {
          ...task,
          status: 'cancelled' as const,
          updated: new Date(),
        };
      }
      return task;
    });
    
    this.updateState({
      tasks: updatedTasks,
      observations: [...(this.state.observations || []), `Task with schedule ID ${scheduleId} was cancelled`]
    });
    
    return true;
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
        model: smallModel, // Use smaller model for structured generation
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
        model: smallModel, // Use smaller model for structured generation
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
          model: smallModel, // Use smaller model for structured generation
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

        // MODIFIED: Check for intent in user messages BEFORE task generation
        const lastMessage = messages.length > 0 ? messages[messages.length - 1] : null;
        if (lastMessage && lastMessage.role === 'user') {
          const lastUserMessageContent = lastMessage.content || '';
          console.log(`[Intent Check] Checking user message: "${lastUserMessageContent.substring(0, 50)}..."`);
          
          // NEW: Check for start/stop commands FIRST
          if (lastUserMessageContent.toLowerCase().includes('start a continuous run') || 
              lastUserMessageContent.toLowerCase().includes('start continuous run')) {
            console.log("[Intent Check] User message requests start continuous run. Calling startContinuousRun().");
            this.startContinuousRun().catch(e => console.error("Error auto-starting continuous run:", e));
            // The startContinuousRun will already update state and trigger continueInfer
            // So we don't need to generate tasks
            return {}; // Return early to skip task generation and rest of infer method
          } 
          else if (lastUserMessageContent.toLowerCase().includes('stop continuous run')) {
            console.log("[Intent Check] User message requests stop continuous run. Calling stopContinuousRun().");
            this.stopContinuousRun().catch(e => console.error("Error auto-stopping continuous run:", e));
            // Don't return - still allow the rest of the infer method to run
          }
          // NEW: Check for set repository context - we'll guide the user to use the tool
          else if (lastUserMessageContent.toLowerCase().includes('set repo context') ||
                  lastUserMessageContent.toLowerCase().includes('set repository context')) {
            console.log("[Intent Check] User message requests setting repository context. Suggesting tool usage.");
            // Instead of trying to parse the message, we'll let the LLM respond with guidance
            // to use the setRepositoryContext tool
          }
          // ONLY check for task generation if it wasn't a special command
          else {
            console.log(`[Task Gen] Checking if message suggests a task: "${lastUserMessageContent.substring(0, 30)}..."`);
            
            const taskIndicators = [
              'implement', 'create', 'build', 'fix', 'add', 'refactor', 'optimize',
              'update', 'develop', 'design', 'setup', 'write'
            ];

            // Check if the user message suggests a coding task
            if (taskIndicators.some(indicator =>
              lastUserMessageContent.toLowerCase().includes(indicator)) &&
              (lastUserMessageContent.includes('code') || lastUserMessageContent.includes('function') ||
                lastUserMessageContent.includes('class') || lastUserMessageContent.includes('file') ||
                lastUserMessageContent.includes('component'))) {

              console.log("[Task Gen] Last user message suggests a task, calling generateAndAddTask.");
              // Create a task based on the user's request
              await this.generateAndAddTask(lastUserMessageContent);
            } else {
              console.log("[Task Gen] Last user message does not match task criteria.");
            }
          }
        } else {
          console.log("[Intent/Task Gen] No user message found as the last message.");
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
      const toolInfoMap = new Map<string, { call?: any; result?: any }>();

      // --- Refactored Tool Consolidation Logic ---
      if (result.steps && result.steps.length > 0) {
        console.log("[Consolidate] Processing steps to gather tool info...");
        for (const step of result.steps) {
          console.log(`[Consolidate] Processing step type: ${step.stepType}`);
          
          // Collect tool calls
          if (step.toolCalls && step.toolCalls.length > 0) {
            for (const toolCall of step.toolCalls) {
              // @ts-ignore
              const toolCallId = toolCall.toolCallId;
              if (toolCallId) {
                const info = toolInfoMap.get(toolCallId) || {};
                info.call = toolCall; // Store or overwrite call
                toolInfoMap.set(toolCallId, info);
                console.log(`[Consolidate] Stored/Updated call for ${toolCallId} (${toolCall.toolName})`);
              }
            }
          }
          
          // Collect tool results
          if (step.toolResults && step.toolResults.length > 0) {
            for (const toolResult of step.toolResults) {
              // @ts-ignore
              const toolCallId = toolResult.toolCallId;
              if (toolCallId) {
                const info = toolInfoMap.get(toolCallId) || {};
                info.result = toolResult; // Store or overwrite result
                toolInfoMap.set(toolCallId, info);
                console.log(`[Consolidate] Added/Updated result for ${toolCallId}`);
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
              const info = toolInfoMap.get(toolCallId) || {};
              info.call = toolCall;
              toolInfoMap.set(toolCallId, info);

              // Try to find matching result in top-level results
              if (result.toolResults && result.toolResults.length > 0) {
                // @ts-ignore
                const toolResult = result.toolResults.find(r => r.toolCallId === toolCallId);
                if (toolResult) {
                  info.result = toolResult;
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
              
              // Check if this is a directory listing (array from GitHub API)
              const isDirectoryListing = Array.isArray(toolResult.result);
              if (isDirectoryListing) {
                console.log(`[Process Tools] Result appears to be a directory listing for ${args.path}`);
                if (args.path) {
                  // Update state for directory
                  this.addAgentObservation(`Listed directory: ${args.path}`);
                  this.updateCodebaseStructure(args.path, null, 'directory');
                  console.log(`[Process Tools] Updated codebase structure for directory: ${args.path}`);
                }
              } else {
                // Process as a file
                // Extract the content from the GitHub API response
                let fileContentDecoded: string | null = null;

                // First check if toolResult.result is already a string (direct content)
                if (typeof toolResult.result === 'string') {
                  console.log(`[Process Tools] Tool result is already a string, using directly`);
                  fileContentDecoded = toolResult.result;
                }
                // Check if it's a GitHub API response object with content field
                else if (toolResult.result && typeof toolResult.result === 'object') {
                  console.log(`[Process Tools] Tool result is an object, looking for content field`);

                  const resultObj = toolResult.result as any;

                  // Check if the object has a content property (typical GitHub API response)
                  if (resultObj.content && typeof resultObj.content === 'string') {
                    console.log(`[Process Tools] Found content field (length: ${resultObj.content.length})`);

                    // Check if it's base64 encoded (GitHub API typically encodes content)
                    if (resultObj.encoding === 'base64') {
                      console.log(`[Process Tools] Content is base64 encoded, attempting to decode`);

                      try {
                        // Most robust approach - try TextDecoder
                        const cleanBase64 = resultObj.content.replace(/\s/g, '');
                        const binaryStr = Buffer.from(cleanBase64, 'base64');
                        fileContentDecoded = new TextDecoder().decode(binaryStr);
                        console.log(`[Process Tools] Successfully decoded content with TextDecoder (length: ${fileContentDecoded.length})`);
                      } catch (e) {
                        console.error(`[Process Tools] Error decoding with TextDecoder:`, e);

                        // Fallback to simpler method
                        try {
                          console.log(`[Process Tools] Trying alternative decode method...`);
                          const cleanBase64 = resultObj.content.replace(/[\s\r\n]+/g, '');
                          fileContentDecoded = Buffer.from(cleanBase64, 'base64').toString('utf8');
                          console.log(`[Process Tools] Alternative decode succeeded (length: ${fileContentDecoded.length})`);
                        } catch (fallbackError) {
                          console.error(`[Process Tools] All decode methods failed:`, fallbackError);
                        }
                      }
                    } else {
                      // Content is not base64 encoded, use directly
                      console.log(`[Process Tools] Content is not base64 encoded, using directly`);
                      fileContentDecoded = resultObj.content;
                    }
                  } else {
                    // No content field - might still be a directory listing in a different format
                    console.log(`[Process Tools] No content field found, checking if this is a directory listing`);
                    if (args.path) {
                      // Could be a directory in a different format - add an observation but don't try to summarize
                      this.addAgentObservation(`Accessed path: ${args.path}`);
                      this.updateCodebaseStructure(args.path, null, 'directory');
                      console.log(`[Process Tools] Updated codebase as directory without summary: ${args.path}`);
                    }
                    return; // Skip further processing
                  }
                } else {
                  console.log(`[Process Tools] Tool result is not a string or object, cannot extract content`);
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
