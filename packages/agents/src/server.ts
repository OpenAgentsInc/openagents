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
  
  private ctx: any; // DurableObjectState type
  
  constructor(ctx: any, env: Env) {
    super(ctx, env);
    this.ctx = ctx; // Store ctx for direct storage access
    console.log("[Constructor] Coder instance created.");
  }

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
    console.log('[updateState] Updated in-memory state via this.setState.');
  }

  async executeTask(payload: Record<string, any>, task: Schedule<Record<string, any>>) {
    const description = payload.description || "Scheduled task executed";
    const newMessage = {
      id: generateId(),
      role: "user" as const,
      content: `This is a scheduled notice for you to now execute the following task: ${ description } `,
      createdAt: new Date(),
      parts: [
        {
          type: "text" as const,
          text: `This is a scheduled notice for you to now execute the following task: ${ description } `
        }
      ],
    };

    await this.updateState({
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
    console.log(`[continueInfer] Cycle start. Payload: ${JSON.stringify(payload)}`);

    // --- Explicit Context Read ---
    let owner: string | undefined;
    let repo: string | undefined;
    let branch: string | undefined;
    let isContinuousRunActive = false;
    
    try {
      console.log("[continueInfer] Explicitly reading repoContextData...");
      // Read minimal context
      const storedContext = await this.ctx.storage.get('repoContextData');
      if (storedContext) {
        owner = storedContext.currentRepoOwner;
        repo = storedContext.currentRepoName;
        branch = storedContext.currentBranch;
        console.log(`[continueInfer] Read context Owner: ${owner}, Repo: ${repo}`);
      } else {
        console.log("[continueInfer] No repoContextData found.");
      }

      // Read the active flag from in-memory state
      isContinuousRunActive = this.state.isContinuousRunActive;
      console.log(`[continueInfer] Active flag from this.state: ${isContinuousRunActive}`);

      // If context was read successfully but not in memory state, update memory state
      if (owner && repo && (this.state.currentRepoOwner !== owner || this.state.currentRepoName !== repo)) {
        console.log("[continueInfer] Updating in-memory state with repository context from storage");
        this.updateState({
          currentRepoOwner: owner,
          currentRepoName: repo,
          currentBranch: branch
        });
      }
    } catch(e) {
      console.error("[continueInfer] Error reading state:", e);
    }
    // --- End Explicit Context Read ---

    if (!isContinuousRunActive) {
      console.log(`[continueInfer] Run inactive. Stopping.`);
      return;
    }

    // Wrap the operation in blockConcurrencyWhile to ensure it completes
    await this.ctx.blockConcurrencyWhile(async () => {
      try {
        // --- Decide NEXT SINGLE Action ---
        // Pass the READ owner/repo to the planner
        const nextAction = await this.planNextExplorationStep(owner, repo, branch);

        if (nextAction) {
          console.log(`[continueInfer] Planning next action: ${nextAction.type} - ${nextAction.path || nextAction.description}`);

          // Schedule the specific action (payload should include owner/repo/branch)
          await this.schedule(5, // Short delay to execute the action soon
            nextAction.type === 'listFiles' ? 'scheduledListFiles' : 'scheduledSummarizeFile',
            nextAction.payload
          );
          await this.addAgentObservation(`Scheduled next action: ${nextAction.type} for ${nextAction.path || 'N/A'}`);

          // Only reschedule the planner if an action was successfully planned and the run is still active
          if (this.state.isContinuousRunActive) {
            const planningIntervalSeconds = 120; // Longer interval (2 minutes) for planning the next step
            console.log(`[continueInfer] Action scheduled. Rescheduling planning cycle in ${planningIntervalSeconds} seconds.`);
            await this.schedule(planningIntervalSeconds, 'continueInfer', { reason: 'next planning cycle' });
          } else {
            console.log(`[continueInfer] Run was stopped during planning/scheduling. Not rescheduling planning cycle.`);
          }
        } else {
          console.log("[continueInfer] No further exploration steps planned. Run potentially stopped or finished.");
          await this.addAgentObservation("No specific exploration step found. Stopping or waiting for manual restart/context.");
          
          // Check if run is still active before stopping
          if (this.state.isContinuousRunActive) {
            console.log("[continueInfer] Planner returned null, stopping continuous run.");
            await this.stopContinuousRun();
          }
        }

      } catch (error) {
        console.error("[continueInfer] Error during planning or scheduling:", error);
        // Reschedule self even on error
        if (this.state.isContinuousRunActive) {
          const errorDelaySeconds = 300;
          console.log(`[continueInfer] Rescheduling planning cycle after error in ${errorDelaySeconds} seconds.`);
          await this.schedule(errorDelaySeconds, 'continueInfer', { reason: 'error recovery planning' });
        }
      }
    }); // End of blockConcurrencyWhile
  }

  /**
   * Method to determine the next exploration step.
   */
  private async planNextExplorationStep(providedOwner?: string, providedRepo?: string, providedBranch?: string): Promise<{ type: 'listFiles' | 'summarizeFile'; path?: string; description?: string; payload: any } | null> {
    // If context isn't provided, try to get it from storage directly
    let owner = providedOwner;
    let repo = providedRepo;
    let branch = providedBranch;
    
    // If not provided as parameters, try to read directly
    if (!owner || !repo) {
      try {
        console.log("[planNextExplorationStep] Explicitly reading repoContextData...");
        const storedContext = await this.ctx.storage.get('repoContextData');
        if (storedContext) {
          console.log("[planNextExplorationStep] Successfully read context from storage:", JSON.stringify(storedContext));
          owner = storedContext.currentRepoOwner;
          repo = storedContext.currentRepoName;
          branch = storedContext.currentBranch;
        } else {
          console.log("[planNextExplorationStep] No repoContextData found in storage.");
        }
      } catch(e) {
        console.error("[planNextExplorationStep] Error reading repoContextData:", e);
      }
    }
    
    console.log(`[planNextExplorationStep ENTRY] Read Owner: ${owner}, Read Repo: ${repo}`);

    // CHECK FOR REPOSITORY CONTEXT
    if (!owner || !repo) {
      console.warn("[planNextExplorationStep] Repository context (owner/name) not found in storage or parameters. Cannot plan file/dir actions.");
      // Add an observation asking the user to set context
      await this.addAgentObservation("Please set the repository context using 'setRepositoryContext' before starting exploration.");
      // Since we can't proceed without repo context, consider stopping the continuous run
      await this.stopContinuousRun().catch(e => console.error("Error stopping continuous run after missing repo context:", e));
      return null; // Cannot plan without context
    }

    // Get current state of exploration
    const codebaseStructure = this.state.codebase?.structure || {};
    const allPaths = Object.keys(codebaseStructure);
    const filesExplored = Object.values(codebaseStructure).filter(file => file.type === 'file');
    const directoriesExplored = Object.values(codebaseStructure).filter(file => file.type === 'directory');

    console.log(`[planNextExplorationStep] Files explored: ${filesExplored.length}, Directories explored: ${directoriesExplored.length}`);

    // --- HIERARCHICAL PLANNING LOGIC USING CHILDREN ARRAY ---

    // 0. If root directory hasn't been listed yet, list it (initial case)
    if (!codebaseStructure['/'] || codebaseStructure['/'].contentsListed !== true) {
      console.log("[planNextExplorationStep] Planning: List root directory");
      return {
        type: 'listFiles',
        path: '/',
        description: 'List repository root directory',
        payload: {
          path: '/',
          owner: owner,
          repo: repo,
          branch: branch || 'main'
        }
      };
    }

    // 1. Find a directory that HAS been listed (contentsListed=true) but contains child directories THAT HAVE NOT been listed
    const listedDirsWithUnlistedChildren = [];
    
    for (const parentDir of directoriesExplored) {
      // Only consider directories that have been successfully listed and have children
      if (parentDir.contentsListed === true && parentDir.children && parentDir.children.length > 0) {
        // Find child directories that haven't been listed yet
        const unlistedChildDirs = parentDir.children
          .filter(child => child.type === 'directory')
          .filter(childDir => {
            const childNode = codebaseStructure[childDir.path];
            return !childNode || childNode.contentsListed !== true;
          });
        
        if (unlistedChildDirs.length > 0) {
          listedDirsWithUnlistedChildren.push({
            parent: parentDir,
            children: unlistedChildDirs
          });
        }
      }
    }

    console.log(`[planNextExplorationStep] Found ${listedDirsWithUnlistedChildren.length} dirs with unlisted children`);    

    if (listedDirsWithUnlistedChildren.length > 0) {
      // Prioritize paths that match common important directory names
      const importantDirNames = ['src', 'app', 'packages', 'apps', 'lib', 'core', 'components'];
      
      // First, look for important child directories
      for (const { children } of listedDirsWithUnlistedChildren) {
        const importantChild = children.find(child => {
          const childName = child.path.split('/').pop() || ''; // Get the last part of the path
          return importantDirNames.includes(childName);
        });
        
        if (importantChild) {
          console.log(`[planNextExplorationStep] Planning: List important child directory '${importantChild.path}'`);  
          return {
            type: 'listFiles',
            path: importantChild.path,
            description: `List child directory '${importantChild.path}'`,
            payload: { path: importantChild.path, owner, repo, branch: branch || 'main' }
          };
        }
      }
      
      // If no important ones found, just pick the first one
      const firstUnlistedChild = listedDirsWithUnlistedChildren[0].children[0];
      console.log(`[planNextExplorationStep] Planning: List child directory '${firstUnlistedChild.path}'`);  
      return {
        type: 'listFiles',
        path: firstUnlistedChild.path,
        description: `List child directory '${firstUnlistedChild.path}'`,
        payload: { path: firstUnlistedChild.path, owner, repo, branch: branch || 'main' }
      };
    }

    // 2. Look for any directory that hasn't been listed yet
    const unlistedDirectory = directoriesExplored.find(dir => dir.contentsListed !== true);
    if (unlistedDirectory) {
      console.log(`[planNextExplorationStep] Planning: List directory '${unlistedDirectory.path}' (contents not yet listed)`);  
      return {
        type: 'listFiles',
        path: unlistedDirectory.path,
        description: `List directory '${unlistedDirectory.path}'`,
        payload: { path: unlistedDirectory.path, owner, repo, branch: branch || 'main' }
      };
    }

    // 3. If all directories have been listed, find a file to summarize within a listed directory
    // Focus on files that we know exist (seen in directory listings) but haven't been summarized
    const filesToSummarize = filesExplored.filter(file => {
      // Look for files with placeholder descriptions that we know exist
      const needsSummary = !file.description || 
                          file.description.startsWith('Accessed at') || 
                          file.description.startsWith('Seen in');
      
      // Skip certain file types
      const ignoredExtensions = ['.gitignore', 'LICENSE', 'yarn.lock', 'yarn-error.log', 'package-lock.json'];
      const ignoredPaths = ['.vscode/', '.cursor/'];
      
      const shouldIgnore = ignoredExtensions.some(ext => file.path.endsWith(ext)) || 
                         ignoredPaths.some(path => file.path.includes(path));
                         
      return needsSummary && !shouldIgnore;
    });

    if (filesToSummarize.length > 0) {
      // Prioritize files with extensions suggesting source code
      const sourceCodeExtensions = ['.ts', '.tsx', '.js', '.jsx', '.py', '.java', '.cpp', '.go', '.rs', '.html', '.css'];
      const sourceCodeFiles = filesToSummarize.filter(file => {
        const extension = file.path.split('.').pop()?.toLowerCase() || '';
        return sourceCodeExtensions.includes(`.${extension}`);
      });
      
      const fileToSummarize = sourceCodeFiles.length > 0 ? sourceCodeFiles[0] : filesToSummarize[0];
      console.log(`[planNextExplorationStep] Planning: Summarize file '${fileToSummarize.path}'`);  
      return {
        type: 'summarizeFile',
        path: fileToSummarize.path,
        description: `Summarize file '${fileToSummarize.path}'`,
        payload: { path: fileToSummarize.path, owner, repo, branch: branch || 'main' }
      };
    }

    // --- END HIERARCHICAL PLANNING LOGIC ---

    console.log("[planNextExplorationStep] No specific next step found (exploration might be complete).");
    return null; // No specific action decided for now
  }

  /**
   * Fetches directory contents from GitHub API
   * @private Helper method to fetch directory listing
   */
  private async fetchDirectoryContents(path: string, owner: string, repo: string, branch: string = 'main'): Promise < any[] | null > {
  console.log(`[fetchDirectoryContents] Fetching directory: ${path} from ${owner}/${repo}:${branch}`);

  if(!this.state.githubToken) {
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
  private async fetchFileContent(path: string, owner: string, repo: string, branch: string = 'main'): Promise < string | null > {
  console.log(`[fetchFileContent] Fetching file: ${path} from ${owner}/${repo}:${branch}`);

  if(!this.state.githubToken) {
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
  // Explicitly read repository context directly from storage
  let owner: string | undefined;
  let repo: string | undefined;
  let branch: string | undefined;
  
  try {
    console.log("[scheduledListFiles] Explicitly reading repoContextData from storage...");
    const storedContext = await this.ctx.storage.get('repoContextData');
    if (storedContext) {
      console.log("[scheduledListFiles] Successfully read context from storage");
      owner = storedContext.currentRepoOwner;
      repo = storedContext.currentRepoName;
      branch = storedContext.currentBranch;
    } else {
      console.log("[scheduledListFiles] No repoContextData found in storage.");
    }
  } catch(e) {
    console.error("[scheduledListFiles] Error reading repoContextData:", e);
  }
  
  // Wrap the key functionality in blockConcurrencyWhile to ensure state updates complete
  await this.ctx.blockConcurrencyWhile(async () => {
    // Create a safe copy of the payload without any potential tokens
    const safePath = payload.path;
    console.log(`[scheduledListFiles] Executing for path: ${safePath}`);
    const { path, payloadOwner, payloadRepo, payloadBranch } = { 
      path: payload.path, 
      payloadOwner: payload.owner, 
      payloadRepo: payload.repo, 
      payloadBranch: payload.branch 
    };
    
    console.log(`[scheduledListFiles ENTRY] From payload - Owner: ${payloadOwner}, Repo: ${payloadRepo}, From storage - Owner: ${owner}, Repo: ${repo}`);

    // Check if path is valid
    if (!path) {
      console.error("[scheduledListFiles] Missing path in payload.");
      return;
    }

    // Prioritize payload values, then storage values
    // This gives explicit payload values highest precedence
    const effectiveOwner = payloadOwner || owner;
    const effectiveRepo = payloadRepo || repo;
    const effectiveBranch = payloadBranch || branch || 'main';

    if (!effectiveOwner || !effectiveRepo) {
      console.error("[scheduledListFiles] Missing owner or repo in both payload and storage.");
      await this.addAgentObservation("Cannot list files: Repository owner/name not available. Please set repository context first.");
      return;
    }

    try {
      // Add an observation about listing files
      await this.addAgentObservation(`Listing files for: ${path}`);

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

      // --- Prepare children data ---
      const childrenData = listing.map(item => ({
        name: item.name,
        type: item.type === 'dir' ? 'directory' : 'file',
        path: path.endsWith('/') ? `${path}${item.name}` : `${path}/${item.name}`
      }));
      // --- End children data preparation ---

      // Update the parent directory node, marking listed and adding children
      await this.updateCodebaseStructure(path, null, 'directory', true, childrenData);
      console.log(`[scheduledListFiles] Marked directory ${path} as contentsListed=true with ${childrenData.length} children`);

      // No need to add entries for individual items - updateCodebaseStructure will handle this
      // based on the childrenData we're passing

      // Add an observation with the results
      await this.addAgentObservation(`Listed ${listing.length} items in directory ${path}`);
      console.log(`[scheduledListFiles] Successfully processed directory ${path} with ${listing.length} items`);

    } catch (e) {
      console.error(`[scheduledListFiles] Error listing ${path}:`, e);
      await this.addAgentObservation(`Error listing files for ${path}: ${e.message}`);
    }
  }); // End of blockConcurrencyWhile
}

  /**
   * Method specifically scheduled to summarize a file.
   * Only performs the file fetching and summarization, without calling infer().
   */
  public async scheduledSummarizeFile(payload: { path: string, owner?: string, repo?: string, branch?: string }) {
  // Explicitly read repository context directly from storage
  let storedOwner: string | undefined;
  let storedRepo: string | undefined;
  let storedBranch: string | undefined;
  
  try {
    console.log("[scheduledSummarizeFile] Explicitly reading repoContextData from storage...");
    const storedContext = await this.ctx.storage.get('repoContextData');
    if (storedContext) {
      console.log("[scheduledSummarizeFile] Successfully read repoContextData:", JSON.stringify(storedContext));
      storedOwner = storedContext.currentRepoOwner;
      storedRepo = storedContext.currentRepoName;
      storedBranch = storedContext.currentBranch;
    } else {
      console.log("[scheduledSummarizeFile] No repoContextData found in storage.");
    }
  } catch(e) {
    console.error("[scheduledSummarizeFile] Error reading repoContextData:", e);
  }
  
  // Wrap the key functionality in blockConcurrencyWhile to ensure state updates complete
  await this.ctx.blockConcurrencyWhile(async () => {
    console.log(`[scheduledSummarizeFile ENTRY] From payload - Owner: ${payload.owner}, Repo: ${payload.repo}, From storage - Owner: ${storedOwner}, Repo: ${storedRepo}`);

    // Create a safe copy of the payload without any potential tokens
    const safePath = payload.path;
    console.log(`[scheduledSummarizeFile] Executing for path: ${safePath}`);
    const { path, owner, repo, branch } = payload;

    // Check if payload has all required fields
    if (!path) {
      console.error("[scheduledSummarizeFile] Missing path in payload.");
      return;
    }

    // Prioritize payload values, then storage values, then state values
    // This gives explicit payload values highest precedence
    const effectiveOwner = owner || storedOwner || this.state.currentRepoOwner;
    const effectiveRepo = repo || storedRepo || this.state.currentRepoName;
    const effectiveBranch = branch || storedBranch || this.state.currentBranch || 'main';

    if (!effectiveOwner || !effectiveRepo) {
      console.error("[scheduledSummarizeFile] Missing owner or repo in payload, storage, and state.");
      await this.addAgentObservation("Cannot summarize file: Repository owner/name not available. Please set repository context first.");
      return;
    }

    try {
      // Add an observation about summarizing the file
      await this.addAgentObservation(`Summarizing file: ${path}`);

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
      await this.setCurrentFile(path);

      // Add success observation
      await this.addAgentObservation(`Successfully summarized file: ${path}`);
      console.log(`[scheduledSummarizeFile] Successfully summarized ${path}`);

    } catch (e) {
      console.error(`[scheduledSummarizeFile] Error summarizing ${path}:`, e);
      await this.addAgentObservation(`Error summarizing file ${path}: ${e.message}`);
    }
  }); // End of blockConcurrencyWhile
}

  /**
   * Starts continuous agent execution
   */
  async startContinuousRun() {
  // Explicitly read repository context directly from storage to verify it's available
  try {
    console.log("[startContinuousRun] Explicitly reading repoContextData from storage...");
    const storedContext = await this.ctx.storage.get('repoContextData');
    if (storedContext) {
      console.log("[startContinuousRun] Successfully read context:", JSON.stringify(storedContext));
      
      // Update in-memory state if needed
      if (this.state.currentRepoOwner !== storedContext.currentRepoOwner || 
          this.state.currentRepoName !== storedContext.currentRepoName) {
        this.updateState({
          currentRepoOwner: storedContext.currentRepoOwner,
          currentRepoName: storedContext.currentRepoName,
          currentBranch: storedContext.currentBranch
        });
      }
    } else {
      console.log("[startContinuousRun] Warning: No repoContextData found in storage. Continuous run may fail.");
    }
  } catch(e) {
    console.error("[startContinuousRun] Error reading repoContextData:", e);
  }

  // Wrap in blockConcurrencyWhile to ensure state updates complete
  return await this.ctx.blockConcurrencyWhile(async () => {
    console.log(`[startContinuousRun ENTRY] Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}, Active: ${this.state.isContinuousRunActive}`);

    this.updateState({
      isContinuousRunActive: true,
      observations: [...(this.state.observations || []), "Starting continuous agent execution"]
    });

    // Immediately start the first execution
    await this.continueInfer({ reason: 'initial start' });

    return { success: true, message: "Continuous run started successfully" };
  });
}

  /**
   * Stops continuous agent execution
   */
  async stopContinuousRun() {
  // Wrap in blockConcurrencyWhile to ensure state updates complete
  return await this.ctx.blockConcurrencyWhile(async () => {
    console.log(`[stopContinuousRun ENTRY] Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}, Active: ${this.state.isContinuousRunActive}`);

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
  });
}

async onMessage(connection: Connection, message: WSMessage) {
  try {
    const parsedMessage = JSON.parse(message as string);

    // Create a safe copy for logging that redacts sensitive information
    const safeMessageForLogging = { ...parsedMessage };
    if (safeMessageForLogging.githubToken) {
      safeMessageForLogging.githubToken = "[REDACTED]";
    }

    // If there's a user message, include it but don't log full content
    if (safeMessageForLogging.userMessage) {
      const userMsg = safeMessageForLogging.userMessage;
      safeMessageForLogging.userMessage = {
        ...userMsg,
        content: userMsg.content ?
          (userMsg.content.length > 50 ? userMsg.content.substring(0, 50) + '...' : userMsg.content)
          : '[no content]'
      };
    }

    console.log("ON MESSAGE RECEIVED:", safeMessageForLogging);

    // Flag to decide whether to call infer
    let callInfer = false;

    // --- Command Handling ---
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

    // --- GitHub Token Logic ---
    // Check if it's a message containing the token
    if (parsedMessage.githubToken) {
      console.log("Processing githubToken update...");
      const githubToken = parsedMessage.githubToken;
      await this.updateState({
        githubToken
      });

      // Only call infer if there's also a user message present
      if (parsedMessage.userMessage && parsedMessage.userMessage.content) {
        console.log("User message present with token, will call infer.");
        callInfer = true;
      } else {
        console.log("Token update only, not calling infer.");
        return; // Exit if only token was updated
      }
    }

    // --- User Message Handling ---
    // Check if there's a user message that needs inference
    else if (parsedMessage.userMessage && parsedMessage.userMessage.content) {
      console.log("User message present, will call infer.");
      callInfer = true;
    }

    // --- Unhandled Message Structure ---
    else {
      console.warn("Received unhandled message structure via send():", safeMessageForLogging);
      return; // Exit for unhandled message types
    }

    // Call infer only if flagged to do so
    if (callInfer) {
      console.log("Calling infer() based on message contents...");
      // No longer trying to force state hydration - the new context loading approach should work better
      this.infer();
    }

  } catch (error) {
    console.error("Error processing received message:", error);
    // Don't log raw message data as it might contain tokens
    console.error("Error parsing message - message is not logged for security");
  }
}

  /**
   * Sets the current repository context
   */
  public async setRepositoryContext(owner: string, repo: string, branch: string = 'main') {
    console.log(`[setRepositoryContext ENTRY] Current state - Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}`);
    console.log(`Setting repository context to ${owner}/${repo} on branch ${branch}`);
    
    const contextData = {
      currentRepoOwner: owner,
      currentRepoName: repo,
      currentBranch: branch
    };
    
    try {
      // Explicitly write ONLY context data to specific key
      await this.ctx.storage.put('repoContextData', contextData);
      console.log(`[setRepositoryContext] Explicitly persisted minimal context to 'repoContextData'`);

      // Update in-memory state using base method AFTER successful persistence
      this.updateState(contextData);
      
    } catch (e) {
      console.error("[setRepositoryContext] FAILED to persist context data:", e);
      // Add observation about failure
      await this.addAgentObservation(`Error setting repository context: ${e.message}`);
      throw e; // Re-throw error
    }
    
    console.log(`[setRepositoryContext EXIT] Updated state - Owner: ${this.state.currentRepoOwner}, Repo: ${this.state.currentRepoName}`);
    return { success: true, message: `Context set to ${owner}/${repo}:${branch}` };
}

/**
 * Adds a task to the agent's state
 */
async addAgentTask(description: string, scheduleId ?: string, payload ?: Record<string, any>, callbackMethodName ?: string) {
  const newTask: Task = {
    id: generateId(),
    description,
    status: 'pending',
    created: new Date(),
    scheduleId,
    payload,
    callbackMethodName
  };

  await this.updateState({
    tasks: [...(this.state.tasks || []), newTask],
    observations: [...(this.state.observations || []), `New task added: ${description}`]
  });

  return newTask.id;
}

/**
 * Cancels a task associated with a specific schedule ID
 */
async cancelTaskByScheduleId(scheduleId: string) {
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

  await this.updateState({
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

    await this.updateState({
      tasks: [...(this.state.tasks || []), newTask],
      observations: [...(this.state.observations || []), `New task generated: ${newTask.description}`]
    });
    return newTask.id;

  } catch (error) {
    console.error("Error generating task object:", error);
    await this.addAgentObservation(`Failed to generate structured task for prompt: ${prompt}`);
    return null; // Indicate failure
  }
}

  /**
   * Updates a task's status
   */
  private async updateTaskStatus(taskId: string, status: Task['status'], notes ?: string) {
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

  await this.updateState({
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
  private async addAgentObservation(observation: string) {
  await this.updateState({
    observations: [...(this.state.observations || []), observation]
  });
}

  /**
   * Updates information about a file or directory in the codebase structure with AI-generated summary
   * @param path The path of the file or directory
   * @param content The file content (null for directories)
   * @param nodeType 'file' or 'directory'
   * @param contentsJustListed Whether this directory's contents were just successfully listed
   * @param childrenData Optional array of child objects for directories
   */
  private async updateCodebaseStructure(
    path: string, 
    content: string | null, 
    nodeType: 'file' | 'directory' = 'file', 
    contentsJustListed: boolean = false,
    childrenData?: { name: string; type: 'file' | 'directory'; path: string }[]
  ) {
  console.log(`[updateCodebaseStructure] Starting for path: ${path}, nodeType: ${nodeType}, contentsListed: ${contentsJustListed}`);
  console.log(`[updateCodebaseStructure] Content length: ${content ? content.length : 'null'}, Children: ${childrenData ? childrenData.length : 'none'}`);

  // Safely log state keys without exposing sensitive data
  const safeStateKeys = Object.keys(this.state || {})
    .filter(key => key !== 'githubToken' && key !== 'token' && !key.toLowerCase().includes('token'));
  console.log(`[updateCodebaseStructure] Current state keys: ${safeStateKeys.join(', ')}`);
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
    // Set contentsListed flag for directories
    // If it's a directory, either use the provided flag or keep existing value
    contentsListed: nodeType === 'directory' ? (contentsJustListed || existingNode.contentsListed || false) : undefined,
    // If childrenData is provided (meaning a directory was just listed), store it.
    // Otherwise, keep existing children (if any). Only relevant for directories.
    children: nodeType === 'directory' ? (childrenData || existingNode.children || undefined) : undefined,
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

  // Prepare updates for child nodes ONLY if childrenData was provided
  const childStructureUpdates: Record<string, FileNode> = {};
  if (nodeType === 'directory' && childrenData) {
    for (const child of childrenData) {
      // Add/update child node only if it doesn't exist or lacks details
      if (!structure[child.path] || !structure[child.path].description || structure[child.path].description.startsWith('Accessed at')) {
        childStructureUpdates[child.path] = {
          ...(structure[child.path] || {}), // Keep existing data if present
          type: child.type,
          path: child.path,
          description: structure[child.path]?.description || `Seen in ${path} listing`, // Basic description
          tags: structure[child.path]?.tags || [],
          metadata: structure[child.path]?.metadata || {},
          contentsListed: structure[child.path]?.contentsListed || false // Default subdirs to not listed
        };
      }
    }
  }

  // Update the codebase structure in the state, including parent and potentially new children
  await this.updateState({
    codebase: {
      ...(this.state.codebase || { structure: {} }),
      structure: {
        ...structure,
        [path]: updatedNode, // Update the parent node
        ...childStructureUpdates // Add/update child nodes
      }
    }
  });

  // Also add an observation
  await this.addAgentObservation(summary
    ? `Analyzed file: ${path}. Purpose: ${summary.summary.substring(0, 30)}...`
    : `Accessed ${nodeType}: ${path}${contentsJustListed ? " and listed its contents" : ""}`
  );
}

  /**
   * Sets the file currently being worked on
   */
  private async setCurrentFile(filePath: string) {
  await this.updateState({
    workingFilePath: filePath
  });
}

@unstable_callable({
  description: "Generate an AI response based on the current messages",
  streaming: true
})
async infer(githubToken ?: string) {
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

      await this.addAgentObservation(`Generated response: ${snippet}`);

      // MODIFIED: Check for intent in user messages BEFORE task generation
      const lastMessage = messages.length > 0 ? messages[messages.length - 1] : null;
      if (lastMessage && lastMessage.role === 'user') {
        const lastUserMessageContent = lastMessage.content || '';
        console.log(`[Intent Check] Checking user message: "${lastUserMessageContent.substring(0, 50)}..."`);

        // Track if we detected a command intent
        let commandIntentDetected = false;

        // Check for start/stop commands FIRST
        if (lastUserMessageContent.toLowerCase().includes('start a continuous run') ||
          lastUserMessageContent.toLowerCase().includes('start continuous run')) {
          commandIntentDetected = true;
          console.log("[Intent Check] User message requests start continuous run. Calling startContinuousRun().");
          this.startContinuousRun().catch(e => console.error("Error auto-starting continuous run:", e));
          await this.addAgentObservation("Continuous run initiated by user message.");
          // RESTORE early return to prevent redundant actions - user gets confirmation via state update
          return {}; // Return early to prevent duplicating exploration steps
        }
        else if (lastUserMessageContent.toLowerCase().includes('stop continuous run')) {
          commandIntentDetected = true;
          console.log("[Intent Check] User message requests stop continuous run. Calling stopContinuousRun().");
          this.stopContinuousRun().catch(e => console.error("Error auto-stopping continuous run:", e));
          await this.addAgentObservation("Continuous run stopped by user message.");
          // Continue with the infer method to generate a confirmation message
        }
        // Modified: Check for set repository context and directly call the method when possible
        else if (lastUserMessageContent.toLowerCase().includes('set repo context') ||
          lastUserMessageContent.toLowerCase().includes('set repository context')) {
          commandIntentDetected = true;
          console.log("[Intent Check] User message requests setting repository context.");

          // --- BEGIN DIRECT TOOL EXECUTION LOGIC ---
          // Attempt to parse owner/repo/branch directly from the message (heuristic)
          // This is brittle, but necessary if the LLM won't use the tool.
          const match = lastUserMessageContent.match(/set.*?context\s+to\s+([\w-]+)\/([\w-]+)(?:\s+(\S+))?/i);
          if (match) {
            const owner = match[1];
            const repo = match[2];
            const branch = match[3] || 'main'; // Default to main if not specified
            console.log(`[Intent Check] Parsed context: ${owner}/${repo}:${branch}. Calling setRepositoryContext directly.`);
            try {
              // Directly call the instance method, don't wait for LLM tool call
              await this.setRepositoryContext(owner, repo, branch);
              await this.addAgentObservation(`Repository context set via direct intent parsing: ${owner}/${repo}:${branch}`);
              // No longer returning early - allow generateText to create a confirmation message
            } catch (e) {
              console.error("Error directly calling setRepositoryContext:", e);
              await this.addAgentObservation(`Error setting context: ${e.message}`);
              // Allow infer to continue to generate an error message
            }
          } else {
            console.warn("[Intent Check] Could not parse owner/repo/branch from message. Letting LLM handle it (might suggest tool).");
            // Let the LLM generate a response, hopefully suggesting the tool.
          }
          // --- END DIRECT TOOL EXECUTION LOGIC ---
        }
        // ONLY check for task generation if no command intent was detected
        if (!commandIntentDetected) {
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
        } else {
          console.log("[Task Gen] Skipping task generation as command intent was detected.");
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
        await this.addAgentObservation(`Used tool: ${toolCall.toolName} with args: ${JSON.stringify(toolCall.args)}`);

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
          await this.addAgentObservation(`Tool result from ${toolCall.toolName}: ${resultSnippet}`);

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
                await this.addAgentObservation(`Listed directory: ${args.path}`);
                await this.updateCodebaseStructure(args.path, null, 'directory');
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
                    await this.addAgentObservation(`Accessed path: ${args.path}`);
                    await this.updateCodebaseStructure(args.path, null, 'directory');
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
                  await this.setCurrentFile(args.path);
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
    await this.updateState({
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
