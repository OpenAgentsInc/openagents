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

This tool retrieves and displays information about all scheduled tasks using the agent's `getSchedules()` method.

```typescript
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
          scheduledTime = new Date(task.date).toLocaleString();
        } else if (task.type === "delayed") {
          scheduledTime = `${task.delayInSeconds} seconds from creation`;
        } else if (task.type === "cron") {
          scheduledTime = `CRON: ${task.cron}`;
        }
        
        return `Task ID: ${task.id}
Description: ${task.description || "No description"}
Type: ${task.type}
Scheduled Time: ${scheduledTime}`;
      }).join("\n\n");
      
      return `Scheduled Tasks:\n\n${formattedTasks}`;
    } catch (error) {
      console.error("Error listing scheduled tasks:", error);
      return `Error listing scheduled tasks: ${error instanceof Error ? error.message : String(error)}`;
    }
  },
});
```

### deleteScheduledTask

This tool deletes a specific scheduled task by ID using the agent's `deleteScheduled()` method.

```typescript
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
      // First check if the task exists
      const tasks = agent.getSchedules({ id: taskId });
      
      if (!tasks || tasks.length === 0) {
        return `No task found with ID: ${taskId}`;
      }
      
      console.log(`Found task to delete:`, tasks[0]);
      
      // Delete the task
      const deleted = agent.deleteScheduled(taskId);
      
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
```

## Implementation Details

### Schedule API

The implementation uses the agent's built-in scheduling API:

1. **Task Creation**: The `agent.schedule()` method creates a new scheduled task.

2. **Task Retrieval**: The `agent.getSchedules()` method retrieves all scheduled tasks, with optional filtering criteria.

3. **Task Deletion**: The `agent.deleteScheduled()` method deletes a specific task by ID.

### Task Data Structure

Each task has the following structure:

- `id`: Unique identifier for the task
- `description`: Human-readable description of the task
- `type`: Type of schedule ("scheduled", "delayed", or "cron")
- Additional type-specific properties:
  - For "scheduled" tasks: `date` (ISO date string)
  - For "delayed" tasks: `delayInSeconds` (number)
  - For "cron" tasks: `cron` (cron expression string)

### Error Handling

The tools include comprehensive error handling to manage common issues:

- Missing agent context
- Non-existent tasks for retrieval or deletion
- Storage operation failures
- Invalid schedule formats

## System Prompt Integration

These tools are integrated into the agent's system prompt to ensure the agent knows when to use them:

```
TASK SCHEDULING:
- scheduleTask: Schedule a task to be executed at a later time
- listScheduledTasks: List all currently scheduled tasks with their details
- deleteScheduledTask: Delete a scheduled task by providing its ID

If the user asks to schedule a task, use the scheduleTask tool.
If the user asks to list scheduled tasks, use the listScheduledTasks tool.
If the user asks to delete a scheduled task, use the deleteScheduledTask tool.
```

## Usage Flow

The typical usage flow for managing tasks is:

1. **Create Task**: Use `scheduleTask` to create a new scheduled task
2. **View Tasks**: Use `listScheduledTasks` to see all scheduled tasks and their IDs
3. **Delete Task**: Use `deleteScheduledTask` with a specific ID to remove a task