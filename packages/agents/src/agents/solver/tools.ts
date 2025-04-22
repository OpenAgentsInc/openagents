// Import AsyncLocalStorage
import { AsyncLocalStorage } from "async_hooks";
import { Solver } from "./index";

// Create an async local storage context for the current solver instance
export const solverContext = new AsyncLocalStorage<Solver | null>();

// Define tools to satisfy the other code
export type SolverToolName = string;

// Create empty tools object
export const solverTools: Record<string, any> = {
  // Empty object - we don't need tools in the minimal version
};