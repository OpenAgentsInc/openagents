import type { Sandbox, Process, ExecResult } from '@cloudflare/sandbox';
import type { OpenClawEnv } from '../types';
import { CLI_TIMEOUT_MS, GATEWAY_PORT, GATEWAY_WS_URL, STARTUP_TIMEOUT_MS } from '../config';
import { restoreFromR2 } from './backup';

const GATEWAY_START_COMMAND = '/usr/local/bin/start-openclaw.sh';

const ENV_ALLOWLIST = [
  'OPENAI_API_KEY',
  'OPENROUTER_API_KEY',
  'ANTHROPIC_API_KEY',
  'OPENCLAW_GATEWAY_TOKEN',
  'OPENCLAW_BIND_MODE',
  'OPENCLAW_DEV_MODE',
];

function buildGatewayEnv(env: OpenClawEnv): Record<string, string> {
  const envVars: Record<string, string> = {};
  for (const key of ENV_ALLOWLIST) {
    const value = env[key as keyof OpenClawEnv];
    if (typeof value === 'string' && value.length > 0) {
      envVars[key] = value;
    }
  }
  return envVars;
}

function isGatewayCommand(command: string): boolean {
  const lowered = command.toLowerCase();
  if (lowered.includes('start-openclaw.sh')) return true;
  if (lowered.includes('openclaw gateway')) return true;
  if (lowered.includes('clawdbot gateway')) return true;
  return false;
}

export async function findGatewayProcess(sandbox: Sandbox): Promise<Process | null> {
  const processes = await sandbox.listProcesses();
  for (const proc of processes) {
    if (!isGatewayCommand(proc.command)) continue;
    if (proc.status === 'starting' || proc.status === 'running') {
      return proc;
    }
  }
  return null;
}

export async function getGatewayStatus(sandbox: Sandbox): Promise<'running' | 'starting' | 'stopped' | 'error'> {
  try {
    const proc = await findGatewayProcess(sandbox);
    if (!proc) return 'stopped';
    if (proc.status === 'starting') return 'starting';
    if (proc.status === 'running') return 'running';
    return 'error';
  } catch {
    return 'error';
  }
}

export async function ensureGateway(sandbox: Sandbox, env: OpenClawEnv): Promise<Process> {
  const existing = await findGatewayProcess(sandbox);
  if (existing) {
    await existing.waitForPort(GATEWAY_PORT, { mode: 'tcp', timeout: STARTUP_TIMEOUT_MS });
    return existing;
  }

  try {
    await restoreFromR2(sandbox, env);
  } catch (error) {
    console.log('Restore skipped or failed:', error instanceof Error ? error.message : error);
  }

  const envVars = buildGatewayEnv(env);
  const proc = await sandbox.startProcess(GATEWAY_START_COMMAND, {
    env: Object.keys(envVars).length > 0 ? envVars : undefined,
  });

  await proc.waitForPort(GATEWAY_PORT, { mode: 'tcp', timeout: STARTUP_TIMEOUT_MS });
  return proc;
}

export async function restartGateway(sandbox: Sandbox, env: OpenClawEnv): Promise<void> {
  const existing = await findGatewayProcess(sandbox);
  if (existing) {
    try {
      await existing.kill();
    } catch (error) {
      console.log('Failed to kill gateway process:', error instanceof Error ? error.message : error);
    }
  }
  await ensureGateway(sandbox, env);
}

type DeviceListResult = {
  pending: unknown[];
  paired: unknown[];
  raw?: string;
  stderr?: string;
  parseError?: string;
};

function parseDeviceJson(stdout: string, stderr: string): DeviceListResult {
  const jsonMatch = stdout.match(/\{[\s\S]*\}/);
  if (!jsonMatch) {
    return { pending: [], paired: [], raw: stdout, stderr, parseError: 'No JSON payload found' };
  }
  try {
    const parsed = JSON.parse(jsonMatch[0]);
    return {
      pending: Array.isArray(parsed.pending) ? parsed.pending : [],
      paired: Array.isArray(parsed.paired) ? parsed.paired : [],
    };
  } catch {
    return { pending: [], paired: [], raw: stdout, stderr, parseError: 'Failed to parse JSON payload' };
  }
}

async function execCli(sandbox: Sandbox, command: string): Promise<ExecResult> {
  return sandbox.exec(command, { timeoutMs: CLI_TIMEOUT_MS });
}

export async function listDevices(sandbox: Sandbox, env: OpenClawEnv): Promise<{ pending: unknown[]; paired: unknown[] }> {
  await ensureGateway(sandbox, env);
  const result = await execCli(sandbox, `openclaw devices list --json --url ${GATEWAY_WS_URL}`);
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const parsed = parseDeviceJson(stdout, stderr);
  if (parsed.parseError) {
    throw new Error(parsed.parseError);
  }
  return { pending: parsed.pending, paired: parsed.paired };
}

export async function approveDevice(sandbox: Sandbox, env: OpenClawEnv, requestId: string): Promise<{ approved: boolean; requestId: string; stdout?: string; stderr?: string }> {
  await ensureGateway(sandbox, env);
  const result = await execCli(sandbox, `openclaw devices approve ${requestId} --url ${GATEWAY_WS_URL}`);
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const approved = result.exitCode === 0 || stdout.toLowerCase().includes('approved');
  return { approved, requestId, stdout, stderr };
}

let cachedVersion: string | null = null;

export async function getClawdbotVersion(sandbox: Sandbox, env: OpenClawEnv): Promise<string | null> {
  if (env.OPENCLAW_VERSION) return env.OPENCLAW_VERSION;
  if (cachedVersion) return cachedVersion;
  try {
    const result = await execCli(sandbox, 'openclaw --version');
    const stdout = result.stdout?.trim() ?? '';
    const match = stdout.match(/(\d+\.\d+\.\d+(?:-[\w.-]+)?)/);
    cachedVersion = match ? match[1] : stdout || null;
    return cachedVersion;
  } catch {
    return null;
  }
}
