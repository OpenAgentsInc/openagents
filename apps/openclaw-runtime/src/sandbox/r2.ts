import type { OpenClawEnv } from '../types';
import { getInstanceId } from './sandboxDo';

export function getBackupKey(env: OpenClawEnv, instanceId?: string): string {
  return `openclaw/${getInstanceId(env, instanceId)}/backup.tar.gz`;
}

export function getLastSyncKey(env: OpenClawEnv, instanceId?: string): string {
  return `openclaw/${getInstanceId(env, instanceId)}/last-sync.txt`;
}

export async function getLastBackup(env: OpenClawEnv, instanceId?: string): Promise<string | null> {
  const obj = await env.OPENCLAW_BUCKET.get(getLastSyncKey(env, instanceId));
  if (!obj) return null;
  const text = await obj.text();
  return text.trim() || null;
}
