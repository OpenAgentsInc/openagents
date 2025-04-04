/**
 * Response formatting utilities for streaming
 */

/**
 * Format a tool call for streaming response
 */
export function formatToolCall(toolCall: any): any {
  // Ensure type field exists
  if (!toolCall.type) {
    toolCall.type = "function";
  }
  
  // Validate arguments JSON
  if (toolCall.function?.arguments && typeof toolCall.function.arguments === 'string') {
    try {
      // Try to parse the arguments to verify it's valid JSON
      JSON.parse(toolCall.function.arguments);
    } catch (e) {
      // If parsing fails, set to empty object
      toolCall.function.arguments = "{}";
    }
  }
  
  return toolCall;
}

/**
 * Format plain text errors for legacy clients
 */
export function formatLegacyErrorForStream(errorMessage: string): string {
  // Use the old format for backwards compatibility
  return `data: 3:${JSON.stringify(errorMessage)}\n\n`;
}

/**
 * Create a stream completion message
 */
export function createStreamCompletionMessage(): string {
  return "data: [DONE]\n\n";
}

/**
 * Create an error message in the standard SSE format
 */
export function createErrorDataMessage(errorMessage: string): string {
  const errorData = {
    id: `error-${Date.now()}`,
    role: "assistant",
    content: "",
    choices: [
      {
        delta: {
          content: errorMessage
        }
      }
    ],
    created: Date.now()
  };
  
  return `data: ${JSON.stringify(errorData)}\n\n`;
}