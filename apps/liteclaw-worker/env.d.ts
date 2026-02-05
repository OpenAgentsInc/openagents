/// <reference types="@cloudflare/workers-types" />

export {};

declare global {
  interface Env {
    Chat: DurableObjectNamespace;
    AI: Ai;
    LITECLAW_SKY_MODE?: string;
    LITECLAW_TOOL_POLICY?: string;
    LITECLAW_TOOL_MAX_CALLS?: string;
    LITECLAW_TOOL_MAX_OUTBOUND_BYTES?: string;
    LITECLAW_HTTP_ALLOWLIST?: string;
    LITECLAW_HTTP_MAX_BYTES?: string;
    LITECLAW_HTTP_TIMEOUT_MS?: string;
  }
}
