#!/usr/bin/env bun
// Simple test server for testing orchestrator
import { Elysia } from "elysia"

const app = new Elysia()
  .get("/", () => "Hello from test server!")
  .get("/about", () => "About page")
  .get("/error", () => {
    throw new Error("Test error")
  })

const port = parseInt(process.env.PORT || "3333")
const server = await app.listen(port)

console.log(`Test server is running at http://localhost:${port}`)
console.log("Server ready")

// Keep the process alive
process.on('SIGINT', () => {
  console.log("Shutting down...")
  process.exit(0)
})

process.on('SIGTERM', () => {
  console.log("Shutting down...")
  process.exit(0)
})