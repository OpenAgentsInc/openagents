/** Explicit sustained editor qualification; invoke through openagents browser run. */
const mode = process.env.AF_PERFORMANCE_MODE || "qualification";
if (!["qualification", "diagnostic"].includes(mode))
  throw new Error("Choose qualification or diagnostic mode.");
process.env.AF_PERFORMANCE_MODE = mode;
process.env.AF_RUNTIME_WINDOW_MS ??=
  mode === "qualification" ? "30000" : "10000";
process.env.AF_RUNTIME_WARMUP_MS ??= "5000";
process.env.AF_PERFORMANCE_REPEATS ??= "2";
await import("./benchmark-runtime.mjs");
