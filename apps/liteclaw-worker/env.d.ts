/// <reference types="@cloudflare/workers-types" />

export {};

declare global {
  interface Env {
    Chat: DurableObjectNamespace;
    AI: Ai;
    LITECLAW_SKY_MODE?: string;
  }
}
