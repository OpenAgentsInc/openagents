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
  // IMPORTANT DEBUG LOG - Remove after testing
  console.log("✅✅✅ ACTUAL ERROR HANDLER CALLED WITH:", error);
  
  // Extract the error message from different possible formats
  let errorMessage: string;
  
  // Special case for AI_TypeValidationError - return the full error message
  if (error instanceof Error && error.message.includes('AI_TypeValidationError')) {
    console.log("FOUND AI_TypeValidationError:", error.message);
    
    // First, try to extract the quoted part of the validation error which contains the actual error
    const quotedErrorMatch = error.message.match(/Type validation failed: Value: "([^"]+)"/);
    if (quotedErrorMatch && quotedErrorMatch[1]) {
      console.log("EXTRACTED ERROR MESSAGE FROM QUOTES:", quotedErrorMatch[1]);
      // Check if this contains a context overflow message
      if (quotedErrorMatch[1].includes('context the overflows') || 
          quotedErrorMatch[1].includes('context length of only')) {
        return quotedErrorMatch[1];
      }
    }
    
    // Check if this is a context overflow TypeValidationError
    if (error.message.includes('context the overflows') || 
        error.message.includes('context length of only')) {
      
      // First, try to extract the complete context overflow line from the error message
      const fullContextOverflowMatch = error.message.match(/(Trying to keep the first \d+ tokens when context the overflows\. However, the model is loaded with context length of only \d+ tokens[^.]*\.)/);
      if (fullContextOverflowMatch && fullContextOverflowMatch[1]) {
        console.log("EXTRACTED CONTEXT OVERFLOW WITH PATTERN 1:", fullContextOverflowMatch[1]);
        return fullContextOverflowMatch[1];
      }
      
      // As a fallback, try the more specific regex
      const contextErrorMatch = error.message.match(/Trying to keep the first (\d+) tokens when context the overflows\. However, the model is loaded with context length of only (\d+) tokens/);
      if (contextErrorMatch) {
        // Return the actual error message (first line) for better user experience
        const errorLines = error.message.split('\n');
        // Find the line with the context overflow message
        for (const line of errorLines) {
          if (line.includes('context the overflows')) {
            console.log("EXTRACTED CONTEXT OVERFLOW FROM LINE:", line);
            return line;
          }
        }
        console.log("EXTRACTED CONTEXT OVERFLOW FALLBACK:", errorLines[0]);
        return errorLines[0]; // Fallback to first line
      }
    }
    
    // Extract the useful part of the AI_TypeValidationError
    const typeErrorMatch = error.message.match(/AI_TypeValidationError: Type validation failed: Value: "(.*?)"/);
    if (typeErrorMatch && typeErrorMatch[1]) {
      // Return the raw error value without processing
      console.log("EXTRACTED TYPE ERROR MATCH:", typeErrorMatch[1]);
      return typeErrorMatch[1];
    }
    
    // If we couldn't extract the specific part, return the full message
    console.log("RETURNING FULL ERROR MESSAGE:", error.message);
    return error.message;
  }
  
  if (error instanceof Error) {
    errorMessage = error.message;
    
    // Check for nested error information
    if (error.cause) {
      try {
        // If there's a more specific error cause, use that
        const cause = typeof error.cause === 'string' 
          ? error.cause 
          : JSON.stringify(error.cause);
        
        if (cause && cause.length > 10) { // Only use if it has meaningful content
          errorMessage = `${errorMessage} - ${cause}`;
        }
      } catch (e) {
        // If JSON parsing fails, just use the original message
      }
    }
  } else if (typeof error === 'object' && error !== null) {
    try {
      // Try to extract the most specific error message
      const errorObj = error as any;
      if (errorObj.message) {
        errorMessage = errorObj.message;
      } else if (errorObj.error?.message) {
        errorMessage = errorObj.error.message;
      } else if (errorObj.details) {
        errorMessage = errorObj.details;
      } else {
        errorMessage = JSON.stringify(error);
      }
    } catch (e) {
      errorMessage = "Unknown error object";
    }
  } else {
    errorMessage = String(error);
  }
  
  // Generic error message without helpful details
  if (errorMessage === "An error occurred.") {
    // Check if we're running in browser context and look for console logs
    if (typeof window !== 'undefined' && window.console) {
      return "An error occurred while processing your request. Please check your model settings or try again.";
    }
  }
  
  // Extract useful information from error message if possible
  
  // Handle LMStudio-specific errors - commented out as LMStudio is disabled
  /*
  if (errorMessage.includes("LMStudio") || errorMessage.includes("lmstudio")) {
    if (errorMessage.includes("not running") || errorMessage.includes("unavailable")) {
      return "LMStudio server is not running or not responding. Please start LMStudio and enable the Local Server.";
    }
    
    if (errorMessage.includes("not found") || errorMessage.includes("No Loaded models")) {
      return "No models loaded in LMStudio. Please load a model in LMStudio first.";
    }
  }
  */
  
  // Context window errors
  if (errorMessage.includes("context length of only") || 
      errorMessage.includes("maximum context length") ||
      errorMessage.includes("context window") ||
      errorMessage.includes("token limit") ||
      errorMessage.includes("history is too long") ||
      errorMessage.includes("context the overflows") ||
      errorMessage.includes("context overflow")) {
    
    // For context overflow errors from LMStudio, show the full message
    if (errorMessage.includes("context the overflows")) {
      // Extract the specific error about context overflow
      const contextOverflowMatch = errorMessage.match(/Trying to keep the first (\d+) tokens when context the overflows\. However, the model is loaded with context length of only (\d+) tokens/);
      if (contextOverflowMatch && contextOverflowMatch[1] && contextOverflowMatch[2]) {
        return errorMessage.split("\n")[0]; // Return the first line of the error message which contains the context overflow details
      }
    }
    
    // Try to extract the exact context length from the error message
    const contextLengthMatch = errorMessage.match(/context length of only (\d+) tokens/);
    if (contextLengthMatch && contextLengthMatch[1]) {
      const contextLength = contextLengthMatch[1];
      return `This conversation is too long for the model's ${contextLength} token limit. Try starting a new chat or using a model with a larger context size.`;
    }
    
    return "This conversation is too long for the model's context window. Try starting a new chat or using a model with a larger context size.";
  } 
  
  if (errorMessage.includes("rate limit") || 
      errorMessage.includes("too many requests") ||
      errorMessage.includes("exceeds the limit")) {
    return "Rate limit exceeded: The API service is limiting requests. Please wait a minute and try again.";
  } 
  
  if (errorMessage.includes("invalid_request_error") || 
      errorMessage.includes("bad request")) {
    return "Invalid request. The model may not support this input format.";
  }
  
  if ((errorMessage.includes("Model") || errorMessage.includes("model")) && 
      (errorMessage.includes("not found") || errorMessage.includes("unavailable"))) {
    return "The selected model is not available. Please select a different model.";
  }
  
  if (errorMessage.includes("authentication") || 
      errorMessage.includes("API key") || 
      errorMessage.includes("auth")) {
    return "Authentication error. Please check your API key in Settings.";
  }
  
  if (errorMessage.includes("fetch") || 
      errorMessage.includes("network") || 
      errorMessage.includes("connection")) {
    return "Network error. Please check your internet connection and try again.";
  }
  
  // Try to extract the most specific part of a complex error message
  const errorLines = errorMessage.split(/[\n\r]+/);
  if (errorLines.length > 1) {
    // Find the most meaningful line - often it's the first non-empty line
    const meaningfulLine = errorLines.find(line => 
      line.trim().length > 15 && !line.includes("Error:") && !line.includes("at ")
    );
    
    if (meaningfulLine) {
      return meaningfulLine.trim();
    }
  }
  
  // Return original message if no specific error was matched
  return errorMessage;
}