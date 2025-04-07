/**
 * Tool definitions for the AI chat agent
 * Tools can either require human confirmation or execute automatically
 */
import { tool } from "ai";
import { z } from "zod";
import type { ToolExecutionOptions } from "ai";

import { agentContext } from "./server";
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

const scheduleTask = tool({
  description: "A tool to schedule a task to be executed at a later time",
  parameters: unstable_scheduleSchema,
  execute: async ({ when, description }) => {
    // we can now read the agent context from the ALS store
    const agent = agentContext.getStore();
    if (!agent) {
      throw new Error("No agent found");
    }
    function throwError(msg: string): string {
      throw new Error(msg);
    }
    if (when.type === "no-schedule") {
      return "Not a valid schedule input";
    }
    const input =
      when.type === "scheduled"
        ? when.date // scheduled
        : when.type === "delayed"
          ? when.delayInSeconds // delayed
          : when.type === "cron"
            ? when.cron // cron
            : throwError("not a valid schedule input");
    try {
      agent.schedule(input!, "executeTask", description);
    } catch (error) {
      console.error("error scheduling task", error);
      return `Error scheduling task: ${error}`;
    }
    return `Task scheduled for type "${when.type}" : ${input}`;
  },
});

const listScheduledTasks = tool({
  description: "A tool to list all currently scheduled tasks",
  parameters: z.object({}),
  execute: async () => {
    // Get the agent from context
    const agent = agentContext.getStore();
    if (!agent) {
      throw new Error("No agent found");
    }
    
    try {
      // Get all scheduled tasks
      const tasks = await agent.listScheduled();
      
      if (!tasks || tasks.length === 0) {
        return "No scheduled tasks found.";
      }
      
      // Format the tasks for display
      const formattedTasks = tasks.map(task => {
        const nextRun = new Date(task.next_run).toLocaleString();
        return `ID: ${task.id}\nDescription: ${task.tag}\nNext Run: ${nextRun}\nCron: ${task.cron || 'One-time'}\n`;
      }).join("\n");
      
      return `Scheduled Tasks:\n\n${formattedTasks}`;
    } catch (error) {
      console.error("Error listing scheduled tasks:", error);
      return `Error listing scheduled tasks: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

const deleteScheduledTask = tool({
  description: "A tool to delete a previously scheduled task",
  parameters: z.object({
    taskId: z.string().describe("The ID of the task to delete")
  }),
  execute: async ({ taskId }) => {
    // Get the agent from context
    const agent = agentContext.getStore();
    if (!agent) {
      throw new Error("No agent found");
    }
    
    try {
      // Call the deleteScheduled method on the agent
      const deleted = await agent.deleteScheduled(taskId);
      
      if (deleted) {
        return `Successfully deleted scheduled task with ID: ${taskId}`;
      } else {
        return `No task found with ID: ${taskId}`;
      }
    } catch (error) {
      console.error("Error deleting scheduled task:", error);
      return `Error deleting scheduled task: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});
/**
 * Export all available tools
 * These will be provided to the AI model to describe available capabilities
 */
export const tools = {
  getWeatherInformation,
  getLocalTime,
  scheduleTask,
  listScheduledTasks,
  deleteScheduledTask,
};

/**
 * Implementation of confirmation-required tools
 * This object contains the actual logic for tools that need human approval
 * Each function here corresponds to a tool above that doesn't have an execute function
 */
export const executions = {
  getWeatherInformation: async (args: unknown, context: ToolExecutionOptions) => {
    const { city } = args as { city: string };
    console.log(`Getting weather information for ${city}`);
    return `The weather in ${city} is sunny`;
  },
};
