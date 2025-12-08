/**
 * Static Planners for TerminalBench Tasks
 *
 * Each planner decomposes a TB task into micro-steps.
 * Using static planners initially (not FM-based) for stability.
 */

import type { MicroPlan, MicroStep } from "./micro-task-types.js";
import { createMicroStep } from "./micro-task-types.js";

export type Planner = (taskId: string, description: string) => MicroPlan;

// --- Path Tracing Planner ---

export const pathTracingPlanner: Planner = (taskId, description) => {
  const steps: MicroStep[] = [
    createMicroStep(1, "READ_FILE_RANGE", "Read PPM header", {
      path: "/app/image.ppm",
      start: 1,
      end: 5,
    }),
    createMicroStep(2, "WRITE_FILE", "Write image.c with read_ppm", {
      path: "image.c",
    }),
    createMicroStep(3, "COMPILE", "Compile image.c", {
      command: "gcc -static -o image image.c -lm",
    }),
    createMicroStep(4, "RUN_COMMAND", "Run and check output", {
      command: "./image > output.ppm && head -5 output.ppm",
    }),
  ];

  return { taskId, steps };
};

// --- Generic File Task Planner ---

export const genericFilePlanner: Planner = (taskId, description) => {
  const steps: MicroStep[] = [];
  let stepId = 1;

  const fileMatches = description.match(/[\/\w.-]+\.(c|py|js|ts|txt|json|md|sh)/g);
  const outputFiles = fileMatches?.filter((f) =>
    description.includes(`write`) ||
    description.includes(`create`) ||
    description.includes(`output`)
  ) ?? [];

  if (outputFiles.length > 0) {
    for (const file of outputFiles) {
      steps.push(createMicroStep(stepId++, "WRITE_FILE", `Create ${file}`, { path: file }));
    }
  } else {
    steps.push(createMicroStep(stepId++, "WRITE_FILE", "Create output file", { path: "output.txt" }));
  }

  if (description.includes("compile") || description.includes("gcc")) {
    steps.push(createMicroStep(stepId++, "COMPILE", "Compile code", { command: "gcc -o main main.c" }));
    steps.push(createMicroStep(stepId++, "RUN_COMMAND", "Run program", { command: "./main" }));
  } else if (description.includes("run") || description.includes("execute")) {
    steps.push(createMicroStep(stepId++, "RUN_COMMAND", "Execute task", { command: "echo 'done'" }));
  }

  if (steps.length === 0) {
    steps.push(createMicroStep(1, "WRITE_FILE", "Create solution", { path: "solution.txt" }));
  }

  return { taskId, steps };
};

// --- Regex Log Planner ---

export const regexLogPlanner: Planner = (taskId, description) => {
  const steps: MicroStep[] = [
    createMicroStep(1, "READ_FILE_RANGE", "Read log sample", {
      path: "/app/input.log",
      start: 1,
      end: 20,
    }),
    createMicroStep(2, "WRITE_FILE", "Create regex script", {
      path: "extract.sh",
    }),
    createMicroStep(3, "RUN_COMMAND", "Run extraction", {
      command: "chmod +x extract.sh && ./extract.sh",
    }),
  ];

  return { taskId, steps };
};

// --- Code Edit Planner ---

export const codeEditPlanner: Planner = (taskId, description) => {
  const steps: MicroStep[] = [];
  let stepId = 1;

  const fileMatches = description.match(/[\/\w.-]+\.(c|py|js|ts|java|go|rs)/g);
  const targetFile = fileMatches?.[0] ?? "main.c";

  steps.push(createMicroStep(stepId++, "READ_FILE_RANGE", `Read ${targetFile}`, {
    path: targetFile,
    start: 1,
    end: 50,
  }));

  steps.push(createMicroStep(stepId++, "EDIT_FILE", `Edit ${targetFile}`, {
    path: targetFile,
  }));

  if (targetFile.endsWith(".c")) {
    steps.push(createMicroStep(stepId++, "COMPILE", "Compile code", {
      command: `gcc -o main ${targetFile}`,
    }));
    steps.push(createMicroStep(stepId++, "RUN_COMMAND", "Test output", {
      command: "./main",
    }));
  }

  return { taskId, steps };
};

// --- Planner Registry ---

const plannerRegistry: Record<string, Planner> = {
  "path-tracing": pathTracingPlanner,
  "regex-log": regexLogPlanner,
};

export function getPlannerForTask(taskId: string, description: string): Planner {
  const lowerId = taskId.toLowerCase();

  for (const [key, planner] of Object.entries(plannerRegistry)) {
    if (lowerId.includes(key)) {
      return planner;
    }
  }

  if (description.includes("edit") || description.includes("modify")) {
    return codeEditPlanner;
  }

  return genericFilePlanner;
}

export function createPlan(taskId: string, description: string): MicroPlan {
  const planner = getPlannerForTask(taskId, description);
  return planner(taskId, description);
}
