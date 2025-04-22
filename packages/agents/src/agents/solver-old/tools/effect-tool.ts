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
          errorMessage = `${taggedError._tag}: ${taggedError.message || JSON.stringify(error)
            }`;

          // Include all properties from the tagged error for context
          errorDetails = { ...error };
        } else {
          errorMessage = String(error);
        }
      } else if (cause && Cause.isDieType(cause)) {
        // Handle defects - indicating programming errors
        console.log("[effectTool] Cause identified as Die. Defect:",
          cause.defect ? String(cause.defect) : "No defect message");

        // Add detailed defect information and message
        errorMessage = `Internal error in tool execution (Defect): ${cause.defect ? String(cause.defect) : "Unknown defect"
          }`;

        // Simplify for troubleshooting - just pass the defect message directly
        errorDetails = {
          defectType: 'die',
          defectMessage: cause.defect ? String(cause.defect) : "Unknown defect",
          requiresToken: cause.defect && String(cause.defect).includes("GitHub token"),
          defectStack: new Error().stack // Capture call stack
        };

        console.log("[effectTool] Setting enrichedError for Die case:", errorMessage);
      } else if (cause && Cause.isInterruptType(cause)) {
        errorMessage = "Tool execution was interrupted.";
        errorDetails = { interrupted: true };
      } else {
        // Handle case where cause is undefined or doesn't match known types
        console.error("[effectTool] Cause was not Fail, Die, or Interrupt. Details:", {
          causeExists: !!cause,
          causeType: cause ? typeof cause : 'undefined',
          causeIsObject: cause && typeof cause === 'object',
          causeHasTag: cause && typeof cause === 'object' && '_tag' in cause,
          causeTag: cause && typeof cause === 'object' && '_tag' in cause ? cause._tag : 'none',
          fiberFailureType: typeof fiberFailure
        });

        // Try to extract useful information from the fiber failure
        let failureStr = String(fiberFailure);
        let tokenMissing = failureStr.includes("GitHub token");

        errorMessage = tokenMissing
          ? `GitHub token is missing in the agent state. Please ensure the agent has a GitHub token set.`
          : `Unknown error in tool execution: ${failureStr}`;

        errorDetails = {
          unknownError: true,
          fiberFailureString: failureStr,
          fiberFailureType: typeof fiberFailure,
          tokenMissing: tokenMissing,
          causePresent: !!cause,
          stack: new Error().stack
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
