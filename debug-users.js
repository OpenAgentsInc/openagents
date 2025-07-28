#!/usr/bin/env node

const { ConvexHttpClient } = require("convex/browser");

const CONVEX_URL = "https://capable-firefly-205.convex.cloud";
const client = new ConvexHttpClient(CONVEX_URL);

async function debugUsers() {
  console.log("🔍 Debug: Checking all users and sessions in database\n");
  
  try {
    // This is a hacky way to call internal functions, but for debugging...
    // We'll create a simple debugging query
    
    console.log("Note: This script requires a debugging query to be added to Convex functions.");
    console.log("Let me create that for you...");
    
  } catch (error) {
    console.error("❌ Error:", error.message);
  }
}

debugUsers().catch(console.error);