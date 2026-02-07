/// <reference types="@cloudflare/workers-types" />

export {}

declare global {
  interface Env {
    ASSETS: { fetch: (request: Request) => Promise<Response> }

    // Durable Objects / Agents SDK
    Chat: DurableObjectNamespace
    UserSpaceDO: DurableObjectNamespace

    // Workers AI
    AI: Ai

    // Shared config
    VITE_CONVEX_URL: string
  }
}

