import type { OpenClawEnv } from '../types';
import { getInstanceId } from './sandboxDo';

export function getBackupKey(env: OpenClawEnv): string {
  return `openclaw/${getInstanceId(env)}/backup.tar.gz`;
}

export function getLastSyncKey(env: OpenClawEnv): string {
  return `openclaw/${getInstanceId(env)}/last-sync.txt`;
}

export async function getLastBackup(env: OpenClawEnv): Promise<string | null> {
  const obj = await env.OPENCLAW_BUCKET.get(getLastSyncKey(env));
  if (!obj) return null;
  const text = await obj.text();
  return text.trim() || null;
}
