import { tools } from "../../common/tools";
import { solverTools } from "./tools";
import { type SolverState } from "./types";

/**
 * Options for generating the solver system prompt
 */
interface SystemPromptOptions {
  state: SolverState; // The agent's current state
  model?: any; // Model information
  temperature?: number; // Temperature setting for generation
}

/**
 * Generates a system prompt for the Solver agent based on its current state
 *
 * @param options Configuration options including the agent state
 * @returns A string containing the complete system prompt
 */
export function getSolverSystemPrompt(options: SystemPromptOptions): string {
  const {
    state,
    model,
    temperature = 0.7,
  } = options;

  // Extract values from state
  const {
    currentIssue,
    currentRepoOwner,
    currentRepoName,
    currentBranch,
    implementationSteps,
    observations,
    scratchpad,
    workingFilePath
  } = state;

  // Get available tools
  const allTools = { ...tools, ...solverTools };
  const availableTools = Object.keys(allTools);

  // Base system prompt
  let systemPrompt = `You are an autonomous issue-solving agent designed to analyze, plan, and implement solutions for GitHub and Linear issues. You work methodically to resolve issues in software projects.

PRIMARY FUNCTIONS:
1. Analyze issue descriptions to understand requirements
2. Plan implementation steps for solving issues
3. Research existing code to understand the problem context
4. Implement solutions by modifying code
5. Test changes to ensure they fix the issue
6. Document the solution with clear explanations
7. Update issue status and add comments as you make progress`;

  // Add repository context if available
  if (currentRepoOwner && currentRepoName) {
    systemPrompt += `\n\nCURRENT CONTEXT:
Repository: ${currentRepoOwner}/${currentRepoName}${currentBranch ? `\nBranch: ${currentBranch}` : ''}`;
  }

  // Add current issue context if available
  if (currentIssue) {
    systemPrompt += `\n\nCURRENT ISSUE:
#${currentIssue.number}: ${currentIssue.title}
Status: ${currentIssue.status}
Source: ${currentIssue.source}
${currentIssue.url ? `URL: ${currentIssue.url}` : ''}

Description:
${currentIssue.description}`;

    if (currentIssue.labels && currentIssue.labels.length > 0) {
      systemPrompt += `\nLabels: ${currentIssue.labels.join(', ')}`;
    }
    
    if (currentIssue.assignee) {
      systemPrompt += `\nAssigned to: ${currentIssue.assignee}`;
    }
  }

  // Add implementation steps if available
  if (implementationSteps && implementationSteps.length > 0) {
    systemPrompt += `\n\nIMPLEMENTATION PLAN:`;
    implementationSteps.forEach((step, index) => {
      systemPrompt += `\n${index + 1}. ${step.description} (${step.status}) - ${step.type}`;
      if (step.notes) {
        systemPrompt += `\n   Note: ${step.notes}`;
      }
      if (step.filePaths && step.filePaths.length > 0) {
        systemPrompt += `\n   Files: ${step.filePaths.join(', ')}`;
      }
    });
  }

  // Add scratchpad for internal thoughts
  if (scratchpad) {
    systemPrompt += `\n\nSCRATCHPAD (for your internal planning - not visible to user):
${scratchpad}`;
  }

  // Add recent observations
  if (observations && observations.length > 0) {
    const recentObservations = observations.slice(-3); // Last 3 observations
    if (recentObservations.length > 0) {
      systemPrompt += `\n\nRECENT OBSERVATIONS:`;
      recentObservations.forEach(obs => {
        systemPrompt += `\n- ${obs}`;
      });
    }
  }

  // Add working file context if available
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
      const toolObj = allTools[toolName as keyof typeof allTools];
      if (toolObj) {
        systemPrompt += `\n- \`${toolName}\`: ${toolObj.description || 'No description available'}`;
      }
    });
  }

  // Add usage guidelines
  systemPrompt += `\n\nGUIDELINES:
1. FOLLOW A METHODICAL APPROACH to issue resolution - understand, plan, implement, test
2. USE TOOLS to gather information and interact with GitHub/Linear issues
3. ANALYZE ISSUE CONTEXT before suggesting solutions
4. BREAK DOWN COMPLEX ISSUES into manageable steps
5. FOLLOW CODE STANDARDS and patterns in the existing codebase
6. DOCUMENT YOUR CHANGES and reasoning clearly
7. UPDATE ISSUE STATUS as you make progress
8. PROVIDE CLEAR EXPLANATIONS for your implementation decisions`;

  // Add temperature hint
  if (temperature < 0.3) {
    systemPrompt += `\n\nYou are running at a low temperature setting (${temperature}). Focus on precision, correctness, and careful implementation.`;
  } else if (temperature > 0.7) {
    systemPrompt += `\n\nYou are running at a high temperature setting (${temperature}). Feel free to be more creative and exploratory with solutions while maintaining correctness.`;
  }

  return systemPrompt;
}