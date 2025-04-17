/**
 * Common tools that can be used by multiple agent types
 */
import { tool } from "ai";
import { z } from "zod";
import {
  unstable_getSchedulePrompt,
  unstable_scheduleSchema,
} from "agents/schedule";

/**
 * Weather information tool that requires human confirmation
 * When invoked, this will present a confirmation dialog to the user
 * The actual implementation is in the executions object below
 */
const getWeatherInformation = tool({
  description: "show the weather in a given city to the user",
  parameters: z.object({ city: z.string() }),
  execute: async ({ city }) => {
    console.log(`Getting weather information for ${city}`);
    return `The weather in ${city} is sunny`;
  },
  // Omitting execute function makes this tool require human confirmation
});

/**
 * Local time tool that executes automatically
 * Since it includes an execute function, it will run without user confirmation
 * This is suitable for low-risk operations that don't need oversight
 */
const getLocalTime = tool({
  description: "get the local time for a specified location",
  parameters: z.object({ location: z.string() }),
  execute: async ({ location }) => {
    console.log(`Getting local time for ${location}`);
    return "10am";
  },
});

/**
 * Schedule a task to be executed at a later time
 */
const scheduleTask = tool({
  description: "A tool to schedule a task to be executed at a later time",
  parameters: z.object({
    when: unstable_scheduleSchema,
    description: z.string(),
    callbackMethodName: z.string()
      .optional()
      .describe('The specific agent method to call when the schedule fires. Defaults to executeTask.'),
    payload: z.record(z.any())
      .optional()
      .describe('Optional structured data (JSON object) to pass to the callback method.')
  }),
  execute: async ({ when, description, callbackMethodName = 'executeTask', payload }) => {
    // The execute function will be implemented by the specific agent
    // This is just a placeholder declaration for common schema
    throw new Error("scheduleTask must be implemented by the specific agent");
  },
});

/**
 * List all currently scheduled tasks at the system level
 */
const listSystemSchedules = tool({
  description: "A tool to list all currently scheduled tasks at the system level",
  parameters: z.object({}),
  execute: async (_) => {
    // The execute function will be implemented by the specific agent
    // This is just a placeholder declaration for common schema
    throw new Error("listSystemSchedules must be implemented by the specific agent");
  },
});

/**
 * Delete a previously scheduled task at the system level
 */
const deleteSystemSchedule = tool({
  description: "A tool to delete a previously scheduled task at the system level",
  parameters: z.object({
    scheduleId: z.string().describe("The ID of the schedule to delete")
  }),
  execute: async ({ scheduleId }) => {
    // The execute function will be implemented by the specific agent
    // This is just a placeholder declaration for common schema
    throw new Error("deleteSystemSchedule must be implemented by the specific agent");
  },
});

/**
 * Set the repository context for GitHub operations
 */
const setRepositoryContext = tool({
  description: "Set the repository context (owner, name, branch) for GitHub operations",
  parameters: z.object({
    owner: z.string().describe("The GitHub username or organization name that owns the repository"),
    repo: z.string().describe("The name of the repository"),
    branch: z.string().optional().default("main").describe("The branch to use, defaults to 'main'")
  }),
  execute: async ({ owner, repo, branch = 'main' }) => {
    // The execute function will be implemented by the specific agent
    // This is just a placeholder declaration for common schema
    throw new Error("setRepositoryContext must be implemented by the specific agent");
  }
});

/**
 * Export all common tools
 * These will be provided to the AI model to describe available capabilities
 */
export const tools = {
  getWeatherInformation,
  getLocalTime,
  scheduleTask,
  listSystemSchedules,
  deleteSystemSchedule,
  cancelSchedule: deleteSystemSchedule,
  setRepositoryContext,
};

/**
 * Implementation of confirmation-required tools
 * This object contains the actual logic for tools that need human approval
 * Each function here corresponds to a tool above that doesn't have an execute function
 */
export const executions = {
  getWeatherInformation: async (args: unknown) => {
    const { city } = args as { city: string };
    console.log(`Getting weather information for ${city}`);
    return `The weather in ${city} is sunny`;
  },
};