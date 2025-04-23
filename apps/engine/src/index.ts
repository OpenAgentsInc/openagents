// Import AllLayers from Program to ensure it's fully evaluated 
import { AllLayers } from "./Program.js"
import { startServer } from "./Server.js"

// Use AllLayers to force evaluation
console.log("DEBUG: Running index.ts with AllLayers from Program.js")
console.log("DEBUG: AllLayers object:", !!AllLayers ? "Defined" : "Undefined")

// Start the server
startServer()
