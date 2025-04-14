import { tools } from "./tools";
import { type CoderState } from "./types";

/**
 * Options for generating the system prompt
 */
interface SystemPromptOptions {
  state: CoderState; // The agent's current state
  model?: any; // Model information
  temperature?: number; // Temperature setting for generation
}

/**
 * Generates a system prompt for the Coder agent based on its current state
 * 
 * @param options Configuration options including the agent state
 * @returns A string containing the complete system prompt
 */
export function getSystemPrompt(options: SystemPromptOptions): string {
  const { 
    state,
    model,
    temperature = 0.7,
  } = options;

  // Extract values from state
  const { 
    currentRepoOwner, 
    currentRepoName, 
    currentBranch,
    scratchpad,
    codebase,
    tasks,
    observations,
    workingFilePath
  } = state;

  // Get available tools
  const availableTools = Object.keys(tools);

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
  // Add any custom scratchpad content if it exists
  if (scratchpad) {
    systemPrompt += `\n\nSCRATCHPAD (for your internal planning - not visible to user):
${scratchpad}`;
  }

  // Add codebase understanding if it exists
  if (codebase && Object.keys(codebase).length > 0) {
    systemPrompt += `\n\nCODEBASE UNDERSTANDING:`;
    
    // Add modules information if available
    if (codebase.modules && Object.keys(codebase.modules).length > 0) {
      systemPrompt += `\nKey modules:`;
      for (const [name, mod] of Object.entries(codebase.modules)) {
        systemPrompt += `\n- ${name}: ${mod.purpose}`;
      }
    }
    
    // Add file structure information if available
    if (codebase.structure && Object.keys(codebase.structure).length > 0) {
      systemPrompt += `\n\nAnalyzed files:`;
      
      // Get the most recently analyzed files (up to 5)
      const analyzedFiles = Object.values(codebase.structure)
        .filter(file => file.type === 'file' && file.description)
        .sort((a, b) => {
          const aDate = a.metadata?.lastAnalyzed ? new Date(a.metadata.lastAnalyzed).getTime() : 0;
          const bDate = b.metadata?.lastAnalyzed ? new Date(b.metadata.lastAnalyzed).getTime() : 0;
          return bDate - aDate; // Sort descending (most recent first)
        })
        .slice(0, 5);
      
      for (const file of analyzedFiles) {
        systemPrompt += `\n- ${file.path}: ${file.description}`;
        
        if (file.tags && file.tags.length > 0) {
          systemPrompt += ` [${file.tags.join(', ')}]`;
        }
        
        // Add exports if available
        if (file.metadata?.exports && file.metadata.exports.length > 0) {
          systemPrompt += `\n  Exports: ${file.metadata.exports.join(', ')}`;
        }
        
        // Add dependencies if available
        if (file.metadata?.dependencies && file.metadata.dependencies.length > 0) {
          systemPrompt += `\n  Dependencies: ${file.metadata.dependencies.join(', ')}`;
        }
      }
    }
  }

  // Add pending tasks if they exist
  if (tasks && tasks.length > 0) {
    const pendingTasks = tasks.filter(t => t.status === 'pending' || t.status === 'in-progress');
    if (pendingTasks.length > 0) {
      systemPrompt += `\n\nPENDING TASKS:`;
      pendingTasks.forEach(task => {
        systemPrompt += `\n- ${task.description} (${task.status})`;
      });
    }
  }

  // Add recent observations if they exist
  if (observations && observations.length > 0) {
    const recentObservations = observations.slice(-3); // Last 3 observations
    if (recentObservations.length > 0) {
      systemPrompt += `\n\nRECENT OBSERVATIONS:`;
      recentObservations.forEach(obs => {
        systemPrompt += `\n- ${obs}`;
      });
    }
  }

  // Add working file context if it exists
  if (workingFilePath) {
    systemPrompt += `\n\nCURRENTLY FOCUSED ON FILE: ${workingFilePath}`;
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
