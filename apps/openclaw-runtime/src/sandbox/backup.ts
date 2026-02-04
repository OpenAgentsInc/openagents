import { collectFile, type Sandbox } from '@cloudflare/sandbox';
import type { OpenClawEnv } from '../types';
import {
  BACKUP_ARCHIVE,
  BACKUP_DIR,
  CONFIG_DIR,
  SKILLS_DIR,
} from '../config';
import { getBackupKey, getLastSyncKey } from './r2';

const BACKUP_TIMEOUT_MS = 60_000;

function toBase64(data: Uint8Array): string {
  return Buffer.from(data).toString('base64');
}

export async function backupToR2(sandbox: Sandbox, env: OpenClawEnv): Promise<string> {
  const timestamp = new Date().toISOString();

  await sandbox.exec(`mkdir -p ${BACKUP_DIR} ${CONFIG_DIR} ${SKILLS_DIR}`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.exec(`rsync -a --delete ${CONFIG_DIR}/ ${BACKUP_DIR}/clawdbot/`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.exec(`rsync -a --delete ${SKILLS_DIR}/ ${BACKUP_DIR}/skills/`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.exec(`printf '${timestamp}' > ${BACKUP_DIR}/.last-sync`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.exec(`tar -czf ${BACKUP_ARCHIVE} -C ${BACKUP_DIR} clawdbot skills .last-sync`, { timeout: BACKUP_TIMEOUT_MS });

  const stream = await sandbox.readFileStream(BACKUP_ARCHIVE);
  const { content } = await collectFile(stream);
  const bytes = typeof content === 'string' ? new TextEncoder().encode(content) : content;

  await env.OPENCLAW_BUCKET.put(getBackupKey(env), bytes, {
    httpMetadata: { contentType: 'application/gzip' },
  });
  await env.OPENCLAW_BUCKET.put(getLastSyncKey(env), timestamp, {
    httpMetadata: { contentType: 'text/plain' },
  });

  return timestamp;
}

export async function restoreFromR2(sandbox: Sandbox, env: OpenClawEnv): Promise<boolean> {
  const obj = await env.OPENCLAW_BUCKET.get(getBackupKey(env));
  if (!obj) return false;

  const data = new Uint8Array(await obj.arrayBuffer());
  await sandbox.exec(`mkdir -p ${BACKUP_DIR} ${CONFIG_DIR} ${SKILLS_DIR}`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.writeFile(BACKUP_ARCHIVE, toBase64(data), { encoding: 'base64' });
  await sandbox.exec(`tar -xzf ${BACKUP_ARCHIVE} -C ${BACKUP_DIR}`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.exec(`rsync -a ${BACKUP_DIR}/clawdbot/ ${CONFIG_DIR}/`, { timeout: BACKUP_TIMEOUT_MS });
  await sandbox.exec(`rsync -a ${BACKUP_DIR}/skills/ ${SKILLS_DIR}/`, { timeout: BACKUP_TIMEOUT_MS });

  return true;
}
