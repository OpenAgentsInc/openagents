import { startTBRun } from "./src/desktop/handlers.js";

console.log("=== Testing TB Run with Real Task ===");
console.log("Task: regex-log (simple task)");
console.log("process.execPath:", process.execPath);
console.log("");

try {
  const result = await startTBRun({
    suitePath: "./tasks/terminal-bench-2.json",
    taskIds: ["regex-log"],
    timeout: 600, // 10 minutes
    maxTurns: 100,
  });
  
  console.log("✅ TB run started successfully!");
  console.log("Run ID:", result.runId);
  console.log("");
  console.log("⏳ Waiting 60 seconds for task to execute...");
  console.log("   (Agent should authenticate and work on the task)");
  console.log("");
  
  // Wait for run to have time to execute
  await new Promise(resolve => setTimeout(resolve, 60000));
  
  console.log("✅ Test complete - check results in:", `results/${result.runId}`);
} catch (error) {
  console.error("❌ TB run failed:", error);
  console.error("Stack:", error.stack);
  process.exit(1);
}
