/**
 * Environment type for Cloudflare Workers
 */
export interface Env {
  // Bindings for the Durable Objects
  CODER: DurableObjectNamespace;
  SOLVER: DurableObjectNamespace;
  
  // Any other environment variables
  [key: string]: any;
}