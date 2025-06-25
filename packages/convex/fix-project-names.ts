#!/usr/bin/env bun

import { ConvexHttpClient } from "convex/browser"
import { api } from "./convex/_generated/api.js"

// Script to update project names for sessions
async function fixProjectNames() {
  console.log("Fixing project names for sessions...")
  
  const client = new ConvexHttpClient("https://proficient-panther-764.convex.cloud")
  
  try {
    // Get all recent sessions
    const sessions = await client.query(api.sessions.listRecent, { limit: 100 })
    console.log(`Found ${sessions?.length || 0} sessions to potentially update`)
    
    // For now, let's just analyze the data
    const projectPaths = new Set<string>()
    sessions?.forEach(session => {
      projectPaths.add(session.project_path)
    })
    
    console.log("\nUnique project paths found:", Array.from(projectPaths))
    
    // Count sessions by project path
    const pathCounts = new Map<string, number>()
    sessions?.forEach(session => {
      const count = pathCounts.get(session.project_path) || 0
      pathCounts.set(session.project_path, count + 1)
    })
    
    console.log("\nSessions per project path:")
    pathCounts.forEach((count, path) => {
      console.log(`  ${path}: ${count} sessions`)
    })
    
    // Show a few sessions with their message counts
    console.log("\nSample sessions with message counts:")
    sessions?.slice(0, 5).forEach(session => {
      console.log(`  Session ${session.id}: ${session.message_count} messages, project: ${session.project_name}`)
    })
    
  } catch (error) {
    console.error("Error:", error)
  }
}

// Run the script
fixProjectNames()