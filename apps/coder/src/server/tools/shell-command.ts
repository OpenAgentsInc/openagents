/**
 * Shell command execution tool
 */

import { z } from 'zod';
import { tool } from 'ai';
import { promisify } from 'util';
import { exec as execCallback } from 'child_process';
import { 
  ToolExecutionError, 
  ToolTimeoutError,
  ToolArgumentError
} from '@openagents/core/src/chat/errors';

// Promisify exec
const exec = promisify(execCallback);

/**
 * Create a shell command execution tool
 */
export function createShellCommandTool() {
  return tool({
    parameters: z.object({
      command: z.string().describe("The shell command to execute")
    }),
    description: "Execute shell commands on the local system",
    execute: async (args) => {
      console.log("Running command:", args.command);

      try {
        // Validate the command
        if (!args.command || typeof args.command !== 'string') {
          throw new ToolArgumentError({
            toolName: 'shell_command',
            message: 'Invalid command argument',
            userMessage: 'The shell command must be a non-empty string'
          });
        }

        // Set maxBuffer to 5MB and add timeout
        const result = await exec(args.command, {
          maxBuffer: 5 * 1024 * 1024, // 5MB buffer
          timeout: 30000 // 30 second timeout
        });

        // Truncate output if too long (limit to ~100KB to ensure it fits in context)
        const maxOutputLength = 100 * 1024; // 100KB
        let output = result.stdout;
        if (output.length > maxOutputLength) {
          output = output.slice(0, maxOutputLength) + "\n... [Output truncated due to length]";
        }

        return "Executed command: " + args.command + "\n\n" + output;
      } catch (error: any) {
        // Handle specific error types
        if (error?.code === 'ENOBUFS' || error?.message?.includes('maxBuffer')) {
          return "Command output was too large. Please modify the command to produce less output.";
        }
        
        if (error?.code === 'ETIMEDOUT' || error?.signal === 'SIGTERM') {
          throw new ToolTimeoutError({
            toolName: 'shell_command',
            message: `Command execution timed out after 30 seconds: ${args.command}`,
            userMessage: 'Command execution timed out after 30 seconds. Please try a simpler command.',
            timeoutMs: 30000
          });
        }
        
        // Generic error
        throw new ToolExecutionError({
          toolName: 'shell_command',
          message: `Error executing command: ${error.message}`,
          userMessage: `Error executing command: ${error.message}`,
          originalError: error
        });
      }
    }
  });
}