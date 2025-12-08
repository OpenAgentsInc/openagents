/**
 * Dynamic Planner for TerminalBench Tasks
 *
 * Instead of static plans, we let FM figure out what to do.
 * The plan just has a single step "Complete the task" and FM
 * uses the full task description to decide which tool to call.
 *
 * For multi-step tasks, FM will be called multiple times until
 * it signals completion or we hit max turns.
 */

import type { MicroPlan } from "./micro-task-types.js";
import { createMicroStep } from "./micro-task-types.js";

export type Planner = (taskId: string, description: string) => MicroPlan;

/**
 * Simple single-step planner that lets FM decide what to do.
 * FM has the full task description and will pick the right tool.
 */
export const dynamicPlanner: Planner = (taskId, description) => {
  // Create a simple plan with just one step
  // FM will see the full task description and decide what tool to use
  const steps = [
    createMicroStep(1, "RUN_COMMAND", "Complete the task", {}),
  ];

  return { taskId, steps };
};

export function createPlan(taskId: string, description: string): MicroPlan {
  return dynamicPlanner(taskId, description);
}
