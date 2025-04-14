/**
 * Tool definitions for the AI chat agent
 * Tools can either require human confirmation or execute automatically
 */
import { tool } from "ai";
import { z } from "zod";
import type { ToolExecutionOptions } from "ai";

import { Coder, agentContext } from "./server";
import {
  unstable_getSchedulePrompt,
  unstable_scheduleSchema,
} from "agents/schedule";
import { GitHubContentSchema } from "../../../apps/mcp-github-server/src/common/types";

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
  parameters: z.object({
    when: unstable_scheduleSchema,
    description: z.string(),
    callbackMethodName: z.enum(['executeTask', 'continueInfer', 'scheduledListFiles', 'scheduledSummarizeFile'])
      .optional()
      .default('executeTask')
      .describe('The specific agent method to call when the schedule fires. Defaults to executeTask.'),
    payload: z.record(z.any())
      .optional()
      .describe('Optional structured data (JSON object) to pass to the callback method.')
  }),
  execute: async ({ when, description, callbackMethodName = 'executeTask', payload }) => {
    const agent = agentContext.getStore();

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

    const scheduleInput =
      when.type === "scheduled"
        ? when.date // scheduled
        : when.type === "delayed"
          ? when.delayInSeconds // delayed
          : when.type === "cron"
            ? when.cron // cron
            : throwError("not a valid schedule input");
    try {
      // Use payload or create a default one with the description
      const scheduleResult = await agent.schedule(
        scheduleInput!, 
        callbackMethodName, 
        payload || { description }
      );
      
      // Get the schedule ID
      const scheduleId = scheduleResult.id;
      
      // Add to agent tasks
      agent.addAgentTask(description, scheduleId, payload, callbackMethodName);
      
      return `Task scheduled with ID: ${scheduleId}. Method: ${callbackMethodName}. Details: ${description}`;
    } catch (error) {
      console.error("error scheduling task", error);
      return `Error scheduling task: ${error}`;
    }
  },
});

const listSystemSchedules = tool({
  description: "A tool to list all currently scheduled tasks at the system level",
  parameters: z.object({}),
  execute: async (_) => {
    const agent = agentContext.getStore();
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
          scheduledTime = new Date(task.time * 1000).toLocaleString();
        } else if (task.type === "delayed") {
          scheduledTime = `${task.delayInSeconds} seconds from creation`;
        } else if (task.type === "cron") {
          scheduledTime = `CRON: ${task.cron}, next run: ${new Date(task.time * 1000).toLocaleString()}`;
        }

        return `ID: ${task.id}\nCallback: ${task.callback}\nType: ${task.type}\nScheduled Time: ${scheduledTime}\nPayload: ${JSON.stringify(task.payload)}`;
      }).join("\n\n");

      return `System Scheduled Tasks:\n\n${formattedTasks}`;
    } catch (error) {
      console.error("Error listing scheduled tasks:", error);
      return `Error listing scheduled tasks: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});

const listAgentTasks = tool({
  description: "Lists the tasks currently tracked in the agent's internal state",
  parameters: z.object({
    status: z.enum(['pending', 'in-progress', 'completed', 'failed', 'cancelled', 'all'])
      .optional()
      .default('pending')
      .describe('Filter tasks by status.')
  }),
  execute: async ({ status = 'pending' }) => {
    const agent = agentContext.getStore();
    // Get agent from context parameter
    if (!agent || !(agent instanceof Coder)) {
      throw new Error("No agent found or agent is not a Coder instance");
    }

    try {
      // Get the agent tasks from state
      const tasks = agent.state.tasks || [];
      
      // Filter tasks based on status
      const filteredTasks = status === 'all' 
        ? tasks 
        : tasks.filter(task => task.status === status);

      if (filteredTasks.length === 0) {
        return `No ${status === 'all' ? '' : status + ' '}tasks found.`;
      }

      // Format the tasks for display
      const formattedTasks = filteredTasks.map(task => {
        return `ID: ${task.id}\nDescription: ${task.description}\nStatus: ${task.status}\nSchedule ID: ${task.scheduleId || 'none'}\nCallback Method: ${task.callbackMethodName || 'none'}\nCreated: ${task.created.toLocaleString()}\n${task.notes && task.notes.length > 0 ? `Notes:\n${task.notes.map(note => `- ${note}`).join('\n')}` : ''}`;
      }).join("\n\n");

      return `Agent Tasks (${status === 'all' ? 'all' : status}):\n\n${formattedTasks}`;
    } catch (error) {
      console.error("Error listing agent tasks:", error);
      return `Error listing agent tasks: ${error instanceof Error ? error.message : String(error)}`;
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

const deleteSystemSchedule = tool({
  description: "A tool to delete a previously scheduled task at the system level",
  parameters: z.object({
    scheduleId: z.string().describe("The ID of the schedule to delete")
  }),
  execute: async ({ scheduleId }) => {
    const agent = agentContext.getStore();
    // Get agent from context parameter
    if (!agent || !(agent instanceof Coder)) {
      throw new Error("No agent found or agent is not a Coder instance");
    }

    try {
      // First check if the schedule exists
      const schedules = agent.getSchedules({ id: scheduleId });

      if (!schedules || schedules.length === 0) {
        return `No schedule found with ID: ${scheduleId}`;
      }

      console.log(`Found schedule to delete:`, schedules[0]);

      // Delete the schedule
      const deleted = await agent.cancelSchedule(scheduleId);

      if (deleted) {
        // Also update any related task in agent's state
        agent.cancelTaskByScheduleId(scheduleId);
        
        return `Successfully deleted schedule with ID: ${scheduleId}`;
      } else {
        return `Failed to delete schedule with ID: ${scheduleId}`;
      }
    } catch (error) {
      console.error("Error deleting system schedule:", error);
      return `Error deleting system schedule: ${error instanceof Error ? error.message : String(error)}`;
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
  listSystemSchedules,
  listAgentTasks,
  deleteSystemSchedule,
  // fetchGitHubFileContent
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

    // Use agentContext to get the agent instance
    const agent = agentContext.getStore();

    if (agent && agent instanceof Coder) {
      console.log("Agent found via agentContext and is a Coder instance");
      // Access state via agent.state if needed
      console.log(`GitHub token available: ${!!agent.state.githubToken}`);
    } else {
      console.log("No agent found via agentContext or agent is not a Coder instance");
    }

    return `The weather in ${city} is sunny`;
  },
};
