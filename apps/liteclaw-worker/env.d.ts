/// <reference types="@cloudflare/workers-types" />

export {};

declare global {
  interface Env {
    Chat: DurableObjectNamespace;
    Sandbox?: DurableObjectNamespace;
    AI: Ai;
    LITECLAW_SKY_MODE?: string;
    LITECLAW_TOOL_POLICY?: string;
    LITECLAW_TOOL_CHOICE?: string;
    LITECLAW_TOOL_MAX_CALLS?: string;
    LITECLAW_TOOL_MAX_OUTBOUND_BYTES?: string;
    LITECLAW_TOOL_ADMIN_SECRET?: string;
    LITECLAW_EXECUTOR_KIND?: string;
    LITECLAW_TUNNEL_URL?: string;
    LITECLAW_TUNNEL_TOKEN?: string;
    LITECLAW_TUNNEL_TIMEOUT_MS?: string;
    LITECLAW_TUNNEL_ACCESS_CLIENT_ID?: string;
    LITECLAW_TUNNEL_ACCESS_CLIENT_SECRET?: string;
    CF_ACCESS_CLIENT_ID?: string;
    CF_ACCESS_CLIENT_SECRET?: string;
    LITECLAW_HTTP_ALLOWLIST?: string;
    LITECLAW_HTTP_MAX_BYTES?: string;
    LITECLAW_HTTP_TIMEOUT_MS?: string;
    LITECLAW_EXTENSION_ALLOWLIST?: string;
    LITECLAW_EXTENSION_DEFAULTS?: string;
    LITECLAW_EXTENSION_CATALOG_URL?: string;
    LITECLAW_EXTENSION_CATALOG_JSON?: string;
    LITECLAW_EXTENSION_CATALOG_KEY?: string;
    LITECLAW_EXTENSION_ADMIN_SECRET?: string;
    LITECLAW_EXTENSION_KV?: KVNamespace;
    LITECLAW_EXTENSION_BUCKET?: R2Bucket;
  }
}
