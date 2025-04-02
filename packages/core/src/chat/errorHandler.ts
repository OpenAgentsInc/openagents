// Error handling utilities for AI chat responses

// Interface for stream error format
export interface StreamErrorData {
  error: boolean;
  message: string;
  details: string;
}

/**
 * Parses custom error format from SSE streams
 */
export function parseStreamError(chunk: string): StreamErrorData | null {
  // Check if it's our special error format
  const errorPrefix = "data: error:";
  if (chunk.startsWith(errorPrefix)) {
    try {
      const errorJsonString = chunk.substring(errorPrefix.length);
      const errorData = JSON.parse(errorJsonString) as StreamErrorData;
      
      // Add default fields if they're missing
      return {
        error: true,
        message: errorData.message || "Unknown error",
        details: errorData.details || ""
      };
    } catch (e) {
      // If parsing fails, create a generic error object
      return {
        error: true,
        message: "Error in AI response",
        details: chunk.substring(errorPrefix.length)
      };
    }
  }
  
  return null;
}

/**
 * Creates a user-friendly error message from various error types
 */
export function createUserFriendlyErrorMessage(error: unknown): string {
  const errorMessage = error instanceof Error 
    ? error.message 
    : typeof error === 'object' && error !== null 
      ? JSON.stringify(error) 
      : String(error);
  
  // Extract useful information from error message if possible
  if (errorMessage.includes("context length of only")) {
    return "This conversation is too long for the model's context window. Try starting a new chat or using a model with a larger context size.";
  } 
  
  if (errorMessage.includes("rate limit")) {
    return "Rate limit exceeded. Please wait a moment before sending another message.";
  } 
  
  if (errorMessage.includes("invalid_request_error")) {
    return "Invalid request. The model may not support this input format.";
  }
  
  if (errorMessage.includes("Model") && errorMessage.includes("not found")) {
    return "The selected model is not available. Please select a different model.";
  }
  
  // Return original message if no specific error was matched
  return errorMessage;
}