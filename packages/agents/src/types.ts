import { Coder } from './agents/coder';
import { Solver } from './agents/solver';
import { DurableObjectNamespace } from '@cloudflare/workers-types';

/**
 * Environment variables and bindings for the worker
 */
export interface Env {
  // API Keys
  OPENROUTER_API_KEY: string;
  
  // Durable Object bindings
  Coder: DurableObjectNamespace<typeof Coder>;
  Solver: DurableObjectNamespace<typeof Solver>;
  
  // AI binding from wrangler.jsonc
  AI: {
    generateText: (options: any) => Promise<any>;
  };
}