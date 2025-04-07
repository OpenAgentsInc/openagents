# Task Management Tools

## Overview

This document describes the task management tools implemented for the Coder agent, which allow for scheduling, listing, and deleting tasks programmatically.

## Tools Implementation

Three main tools have been implemented for task management:

1. **scheduleTask**: Creates new scheduled tasks
2. **listScheduledTasks**: Lists all currently scheduled tasks
3. **deleteScheduledTask**: Deletes a specific scheduled task by ID

## Tool Details

### scheduleTask

This tool allows the agent to schedule tasks for future execution.

```typescript
const scheduleTask = tool({
  description: "A tool to schedule a task to be executed at a later time",
  parameters: unstable_scheduleSchema,
  execute: async ({ when, description }) => {
    // Get the agent context
    const agent = agentContext.getStore();
    if (!agent) {
      throw new Error("No agent found");
    }
    
    // Determine the schedule type and input
    if (when.type === "no-schedule") {
      return "Not a valid schedule input";
    }
    
    const input =
      when.type === "scheduled"
        ? when.date         // Absolute date/time
        : when.type === "delayed"
          ? when.delayInSeconds  // Relative delay in seconds
          : when.type === "cron"
            ? when.cron     // Cron expression for recurring tasks
            : throwError("not a valid schedule input");
            
    // Schedule the task
    try {
      agent.schedule(input!, "executeTask", description);
      return `Task scheduled for type "${when.type}" : ${input}`;
    } catch (error) {
      console.error("error scheduling task", error);
      return `Error scheduling task: ${error}`;
    }
  },
});
```

### listScheduledTasks

This tool allows the agent to list all currently scheduled tasks, including their IDs, descriptions, next run times, and whether they are recurring (cron-based) or one-time tasks.

```typescript
const listScheduledTasks = tool({
  description: "A tool to list all currently scheduled tasks",
  parameters: z.object({}),
  execute: async () => {
    // Get the agent context
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
```

### deleteScheduledTask

This tool allows the agent to delete a scheduled task by its ID. It's designed to be used after first listing the tasks to identify the task to delete.

```typescript
const deleteScheduledTask = tool({
  description: "A tool to delete a previously scheduled task",
  parameters: z.object({
    taskId: z.string().describe("The ID of the task to delete")
  }),
  execute: async ({ taskId }) => {
    // Get the agent context
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
```

## Usage Flow

The typical usage flow for managing tasks is:

1. **Create Task**: Use `scheduleTask` to create a new scheduled task
2. **List Tasks**: Use `listScheduledTasks` to see all scheduled tasks and their IDs
3. **Delete Task**: Use `deleteScheduledTask` with a specific ID to remove a task

## System Prompt Integration

These tools are integrated into the agent's system prompt to ensure the agent knows when to use them:

```
TASK SCHEDULING:
- scheduleTask: Schedule a task to be executed at a later time
- listScheduledTasks: List all currently scheduled tasks with their IDs and details
- deleteScheduledTask: Delete a previously scheduled task by providing its ID

If the user asks to schedule a task, use the scheduleTask tool.
If the user asks to list scheduled tasks, use the listScheduledTasks tool.
If the user asks to delete a scheduled task, use the deleteScheduledTask tool. 
Suggest using listScheduledTasks first to find the task ID.
```

## Implementation Notes

- The tools use the agent context to access the underlying Durable Object methods
- Error handling is implemented at each step to provide clear feedback to users
- The task listing provides formatted output for better readability
- The tools work with the existing scheduling infrastructure in the Coder agent