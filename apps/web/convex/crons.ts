import { cronJobs } from "convex/server";

import { internal } from "./_generated/api";

const crons = cronJobs();

// Guardrail: never leave runs stuck in `streaming` forever.
// NOTE: This is intentionally internal-only; the public Worker already tries to finalize in a `finally`,
// but isolates can be evicted and upstream streams can stall.
crons.interval(
  "autopilot.finalizeStaleRuns",
  { minutes: 1 },
  internal.autopilot.messages.finalizeStaleRuns,
  { staleAfterMs: 90_000, limit: 50 },
);

export default crons;

