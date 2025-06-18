#!/usr/bin/env bun
// Simple test server for testing orchestrator
import { Elysia } from "elysia"

const app = new Elysia()
  .get("/", () => "Hello from test server!")
  .get("/about", () => "About page")
  .get("/error", () => {
    throw new Error("Test error")
  })
  .listen(3333)

console.log("Test server is running at http://localhost:3333")
console.log("Server ready")