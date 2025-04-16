import { tools } from "../../common/tools";
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
    currentProblem,
    scratchpad,
    steps,
    observations
  } = state;

  // Get available tools
  const availableTools = Object.keys(tools);

  // Base system prompt
  let systemPrompt = `You are an autonomous problem-solving agent specializing in mathematical, logical, and analytical challenges.

PRIMARY FUNCTIONS:
1. Break down complex problems into manageable steps
2. Apply mathematical and logical reasoning
3. Show detailed work and explanations
4. Verify solutions with multiple approaches
5. Handle numerical calculations accurately
6. Explain problem-solving strategies

GUIDELINES:
1. ANALYZE problems thoroughly before solving
2. SHOW all work and intermediate steps
3. VERIFY solutions with different methods
4. EXPLAIN reasoning clearly and concisely
5. USE appropriate mathematical notation
6. CHECK edge cases and assumptions
7. MAINTAIN solution accuracy`;

  // Add state information
  
  // Add current problem context if available
  if (currentProblem) {
    systemPrompt += `\n\nCURRENT PROBLEM:
Description: ${currentProblem.description}
Type: ${currentProblem.type}
Status: ${currentProblem.status}`;

    if (currentProblem.constraints && currentProblem.constraints.length > 0) {
      systemPrompt += `\nConstraints:${currentProblem.constraints.map(constraint => `\n- ${constraint}`).join('')}`;
    }
  }

  // Add solution steps if available
  if (steps && steps.length > 0) {
    systemPrompt += `\n\nSOLUTION STEPS:`;
    steps.forEach(step => {
      systemPrompt += `\n- ${step.description}: ${step.content}${step.verified ? ' (verified)' : ' (unverified)'}`;
    });
  }

  // Add any custom scratchpad content if it exists
  if (scratchpad) {
    systemPrompt += `\n\nSCRATCHPAD (for your internal planning - not visible to user):
${scratchpad}`;
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

  // Add temperature hint
  if (temperature < 0.3) {
    systemPrompt += `\n\nYou are running at a low temperature setting (${temperature}). Focus on precision, correctness, and mathematical accuracy.`;
  } else if (temperature > 0.7) {
    systemPrompt += `\n\nYou are running at a high temperature setting (${temperature}). Feel free to be more creative with solution approaches while maintaining mathematical rigor.`;
  }

  return systemPrompt;
}