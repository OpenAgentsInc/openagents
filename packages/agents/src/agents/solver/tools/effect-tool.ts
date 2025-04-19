import { tool } from "ai";
import { Effect, Cause } from "effect";
import { z } from "zod";

type ZodSchema = z.ZodType<any, any, any>;

/**
 * Factory function for creating Effect-based tools that are compatible
 * with Vercel AI SDK's tool function.
 * 
 * This function wraps an Effect-returning execute function to return a Promise
 * that the Vercel AI SDK expects, while marking the tool as Effect-based.
 */
export function effectTool<P extends ZodSchema, R>(
  definition: {
    description: string;
    parameters: P;
    execute: (params: z.infer<P>, options: {}) => Effect.Effect<R, any, never>;
  }
) {
  // Create a Promise-returning execute function that wraps the Effect
  const promiseExecute = (params: any, options: {}) => {
    const effect = definition.execute(params, options);
    return Effect.runPromise(effect).catch((fiberFailure) => {
      // Safely extract the cause from the FiberFailure
      const cause = fiberFailure && typeof fiberFailure === 'object' ? (fiberFailure as any).cause : undefined;
      
      // Log the fiber failure for debugging
      console.log("[effectTool] Caught FiberFailure:", {
        hasFiberFailure: !!fiberFailure,
        hasCause: !!cause,
        causeType: cause && typeof cause === 'object' && '_tag' in cause ? cause._tag : 'unknown'
      });
      let errorMessage = "Tool execution failed.";
      let errorDetails: Record<string, any> = {};
      
      // Safely check that cause exists before analyzing
      if (cause && Cause.isFailType(cause)) {
        const error = cause.error;
        if (error && typeof error === 'object' && '_tag' in error) {
          // Tagged errors (from Data.TaggedError) include rich type information
          const taggedError = error as { _tag: string, message?: string, [key: string]: any };
          
          // Create a user-friendly error message
          errorMessage = `${taggedError._tag}: ${
            taggedError.message || JSON.stringify(error)
          }`;
          
          // Include all properties from the tagged error for context
          errorDetails = { ...error };
        } else {
          errorMessage = String(error);
        }
      } else if (cause && Cause.isDieType(cause)) {
        // Handle defects - indicating programming errors
        console.error("Tool defected:", cause.defect);
        errorMessage = "Internal error in tool execution.";
        errorDetails = { defect: String(cause.defect) };
      } else if (cause && Cause.isInterruptType(cause)) {
        errorMessage = "Tool execution was interrupted.";
        errorDetails = { interrupted: true };
      } else {
        // Handle case where cause is undefined or doesn't match known types
        errorMessage = `Unknown error in tool execution: ${String(fiberFailure)}`;
        errorDetails = { 
          unknownError: true,
          fiberFailureString: String(fiberFailure),
          fiberFailureType: typeof fiberFailure
        };
        console.error("[effectTool] Unrecognized failure:", fiberFailure);
      }
      
      // Create a custom error object with details from the Cause analysis
      const enrichedError = new Error(errorMessage);
      (enrichedError as any).effectError = errorDetails;
      
      // Safely access cause._tag with proper null checks
      (enrichedError as any).causeType = 
        cause && typeof cause === 'object' && '_tag' in cause ? 
        cause._tag : 'unknown';
      
      // Add the original error for troubleshooting
      (enrichedError as any).originalError = fiberFailure;
      
      console.log("[effectTool] Created enriched error:", {
        message: errorMessage,
        errorDetails,
        causeType: (enrichedError as any).causeType
      });
      
      // Throw the enriched error that will be caught by executeToolEffect
      throw enrichedError;
    });
  };

  // Create the tool using Vercel AI's tool function with type assertion for the parameters
  const baseTool = tool({
    description: definition.description,
    parameters: definition.parameters,
    execute: promiseExecute
  });

  // Add the effectBased marker using type assertion
  return Object.assign(baseTool, { effectBased: true } as const);
}