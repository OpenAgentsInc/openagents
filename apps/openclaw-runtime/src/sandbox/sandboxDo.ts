import { getSandbox, type Sandbox, type SandboxOptions } from '@cloudflare/sandbox';
import type { OpenClawEnv } from '../types';

const DEFAULT_SANDBOX_ID = 'openclaw-runtime';

export function sanitizeInstanceId(value: string): string {
  let out = '';
  for (const ch of value) {
    if (ch >= '0' && ch <= '9') {
      out += ch;
      continue;
    }
    const lower = ch.toLowerCase();
    if (lower >= 'a' && lower <= 'z') {
      out += lower;
      continue;
    }
    if (ch === '-' || ch === '_') {
      out += '-';
      continue;
    }
    out += '-';
  }
  return out.replace(/-+/g, '-').replace(/^-+|-+$/g, '');
}

export function getInstanceId(env: OpenClawEnv, override?: string): string {
  const raw = override?.trim() || env.OPENCLAW_INSTANCE_ID?.trim() || DEFAULT_SANDBOX_ID;
  const sanitized = sanitizeInstanceId(raw);
  return sanitized.length > 0 ? sanitized : DEFAULT_SANDBOX_ID;
}

export function getOpenClawSandbox(
  env: OpenClawEnv,
  options?: SandboxOptions,
  instanceIdOverride?: string,
): Sandbox {
  return getSandbox(env.Sandbox, getInstanceId(env, instanceIdOverride), {
    normalizeId: true,
    ...options,
  });
}
