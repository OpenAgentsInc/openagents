import { getSandbox, type Sandbox, type SandboxOptions } from '@cloudflare/sandbox';
import type { OpenClawEnv } from '../types';

const DEFAULT_SANDBOX_ID = 'openclaw-runtime';

export function getInstanceId(env: OpenClawEnv): string {
  const raw = env.OPENCLAW_INSTANCE_ID?.trim();
  return raw && raw.length > 0 ? raw : DEFAULT_SANDBOX_ID;
}

export function getOpenClawSandbox(env: OpenClawEnv, options?: SandboxOptions): Sandbox {
  return getSandbox(env.Sandbox, getInstanceId(env), {
    normalizeId: true,
    ...options,
  });
}
