// Simple Node.js script to directly test the minimal layer
import { Layer, Effect } from "effect";
import { NodeContext } from "@effect/platform-node";
import { PlanManager } from "./src/github/PlanManager.js";
import { PlanManagerLayer } from "./src/github/PlanManager.js";

// Create a minimal effect to test PlanManager service resolution
const minimalTest = Effect.gen(function*() {
  console.log("MINIMAL TEST: Starting minimal test effect");
  console.log("MINIMAL TEST: PlanManager Tag:", PlanManager);
  
  try {
    // Try to access the PlanManager service
    const planManager = yield* PlanManager;
    console.log("MINIMAL TEST: Successfully accessed PlanManager service:", planManager);
    return "Success! PlanManager service was found";
  } catch (error) {
    console.error("MINIMAL TEST ERROR:", error);
    throw error;
  }
});

// Create a minimal layer with just PlanManagerLayer and NodeContext
const minimalLayer = Layer.mergeAll(
  PlanManagerLayer,
  NodeContext.layer
);

console.log("MINIMAL TEST: Created minimal layer");
console.log("MINIMAL TEST: Running minimalTest effect with minimalLayer...");

// Run the effect with the minimal layer
Effect.runPromise(
  Effect.provide(minimalTest, minimalLayer)
)
.then(result => {
  console.log("MINIMAL TEST RESULT:", result);
})
.catch(error => {
  console.error("MINIMAL TEST FAILED:", error);
});