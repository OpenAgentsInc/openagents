/**
 * Tool definitions for the AI chat agent
 * Tools can either require human confirmation or execute automatically
 */
import { tool } from "ai";
import { z } from "zod";
import type { ToolExecutionOptions } from "ai";

import { Coder } from "./server";
import {
  unstable_getSchedulePrompt,
  unstable_scheduleSchema,
} from "agents/schedule";
import { GitHubContentSchema } from "../../../apps/mcp-github-server/src/common/types";
import { agentContext } from "./server";

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

const scheduleTask = tool({
  description: "A tool to schedule a task to be executed at a later time",
  parameters: unstable_scheduleSchema,
  execute: async ({ when, description }, { agent }) => {
    // Get agent from context parameter
    if (!agent || !(agent instanceof Coder)) {
      throw new Error("No agent found or agent is not a Coder instance");
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
  execute: async (_, { agent }) => {
    // Get agent from context parameter
    if (!agent || !(agent instanceof Coder)) {
      throw new Error("No agent found or agent is not a Coder instance");
    }

    try {
      // Get the scheduled tasks using the correct agent method
      const tasks = agent.getSchedules();
      console.log("Retrieved scheduled tasks:", tasks);

      if (!tasks || tasks.length === 0) {
        return "No scheduled tasks found.";
      }

      // Format the tasks for display
      const formattedTasks = tasks.map(task => {
        let scheduledTime = "Unknown";
        if (task.type === "scheduled") {
          scheduledTime = new Date(task.time).toLocaleString();
        } else if (task.type === "delayed") {
          scheduledTime = `${task.delayInSeconds} seconds from creation`;
        } else if (task.type === "cron") {
          scheduledTime = `CRON: ${task.cron}`;
        }

        return `Task info: ${JSON.stringify(task)}`;
      }).join("\n\n");

      return `Scheduled Tasks:\n\n${formattedTasks}`;
    } catch (error) {
      console.error("Error listing scheduled tasks:", error);
      return `Error listing scheduled tasks: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});


// async function githubRequest(url: string, options: { token?: string }) {
//   const headers: Record<string, string> = {
//     'Accept': 'application/vnd.github.v3+json',
//     'User-Agent': 'OpenAgents-GitHub-Client'
//   };

//   if (options.token) {
//     headers['Authorization'] = `Bearer ${options.token}`;
//     console.log("Using GitHub token:", options.token.slice(0, 15));
//   } else {
//     console.log("No GitHub token found");
//   }

//   const response = await fetch(url, { headers });
//   if (!response.ok) {
//     const error = await response.text();
//     console.error(`GitHub API error (${response.status}):`, error);
//     throw new Error(`GitHub API error (${response.status}): ${error}`);
//   }
//   return response.json();
// }

// async function getFileContents(
//   owner: string,
//   repo: string,
//   path: string,
//   branch?: string,
//   token?: string
// ) {
//   let url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;
//   if (branch) {
//     url += `?ref=${branch}`;
//   }

//   const response = await githubRequest(url, { token });
//   const data = GitHubContentSchema.parse(response);

//   // If it's a file, decode the content
//   if (!Array.isArray(data) && data.content) {
//     // Replace newlines and spaces that GitHub adds to base64
//     const cleanContent = data.content.replace(/\n/g, '');
//     data.content = atob(cleanContent);
//   }

//   return data;
// }

const deleteScheduledTask = tool({
  description: "A tool to delete a previously scheduled task",
  parameters: z.object({
    taskId: z.string().describe("The ID of the task to delete")
  }),
  execute: async ({ taskId }, { agent }) => {
    // Get agent from context parameter
    if (!agent || !(agent instanceof Coder)) {
      throw new Error("No agent found or agent is not a Coder instance");
    }

    try {
      // First check if the task exists
      const tasks = agent.getSchedules({ id: taskId });

      if (!tasks || tasks.length === 0) {
        return `No task found with ID: ${taskId}`;
      }

      console.log(`Found task to delete:`, tasks[0]);

      // Delete the task
      const deleted = await agent.cancelSchedule(taskId);

      if (deleted) {
        return `Successfully deleted task with ID: ${taskId}`;
      } else {
        return `Failed to delete task with ID: ${taskId}`;
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
  // fetchGitHubFileContent
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

    // Log if we have an agent and if it's a Coder
    if (context.agent) {
      console.log("Has agent in execution context");
      if (context.agent instanceof Coder) {
        console.log("Agent is a Coder instance");
        console.log(`GitHub token available: ${!!context.agent.githubToken}`);
      }
    } else {
      console.log("No agent in execution context");
    }

    return `The weather in ${city} is sunny`;
  },
};
