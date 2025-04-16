import { z } from "zod";

// Zod schema for planning thoughts/scratchpad updates
export const PlanningSchema = z.object({
  thought: z.string().describe("A concise thought or step in a plan related to the current task."),
  nextAction: z.string().optional().describe("A potential next immediate action or tool use."),
  questions: z.array(z.string()).optional().describe("Questions to ask the user or resolve internally."),
});

// Zod schema for summarizing file content for the codebase map
export const FileSummarySchema = z.object({
  summary: z.string().describe("A brief summary of the file's purpose and key contents (1-3 sentences)."),
  tags: z.array(z.string()).describe("Keywords or tags describing the file's functionality (e.g., 'auth', 'api-route', 'database', 'component', 'utility')."),
  exports: z.array(z.string()).optional().describe("Key functions, classes, or variables exported by the file."),
  dependencies: z.array(z.string()).optional().describe("Important libraries, modules, or files this file depends on."),
  complexity: z.enum(["low", "medium", "high"]).optional().describe("Assessment of the file's complexity."),
});

// Zod schema for defining a new task based on user request or analysis
export const NewTaskSchema = z.object({
  description: z.string().describe("A clear, actionable description of the coding task."),
  priority: z.enum(["high", "medium", "low"]).optional().describe("Priority of the task."),
  subTasks: z.array(z.string()).optional().describe("Breakdown into smaller steps if applicable."),
});