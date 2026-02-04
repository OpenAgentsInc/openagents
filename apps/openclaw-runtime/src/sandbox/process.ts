import type { Sandbox, Process, ExecResult } from '@cloudflare/sandbox';
import type { OpenClawEnv } from '../types';
import {
  CLI_TIMEOUT_MS,
  GATEWAY_HTTP_TIMEOUT_MS,
  GATEWAY_HTTP_URL,
  GATEWAY_PORT,
  GATEWAY_WS_URL,
  STARTUP_TIMEOUT_MS,
} from '../config';
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
  return sandbox.exec(command, { timeout: CLI_TIMEOUT_MS });
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

type GatewayInvokeOk = { ok: true; result: unknown };
type GatewayInvokeErr = { ok: false; error?: { type?: string; message?: string } };
export type GatewayInvokeResponse = GatewayInvokeOk | GatewayInvokeErr;

const GATEWAY_STATUS_MARKER = '__OPENCLAW_GATEWAY_STATUS__';
const GATEWAY_HEADER_ALLOWLIST = new Set(['x-openclaw-message-channel', 'x-openclaw-account-id']);

function escapeShellArg(value: string): string {
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

function normalizeGatewayHeaders(
  headers: Record<string, unknown> | undefined,
): Record<string, string> {
  if (!headers) return {};
  const normalized: Record<string, string> = {};
  for (const [key, value] of Object.entries(headers)) {
    const headerKey = key.trim().toLowerCase();
    if (!GATEWAY_HEADER_ALLOWLIST.has(headerKey)) continue;
    if (typeof value !== 'string') continue;
    const headerValue = value.trim();
    if (!headerValue) continue;
    normalized[headerKey] = headerValue;
  }
  return normalized;
}

function splitGatewayOutput(output: string): { body: string; status: number | null } {
  const index = output.lastIndexOf(GATEWAY_STATUS_MARKER);
  if (index === -1) {
    return { body: output.trim(), status: null };
  }
  const body = output.slice(0, index).trimEnd();
  const statusRaw = output.slice(index + GATEWAY_STATUS_MARKER.length).trim();
  const status = Number.parseInt(statusRaw, 10);
  return { body, status: Number.isFinite(status) ? status : null };
}

export async function invokeGatewayTool(
  sandbox: Sandbox,
  env: OpenClawEnv,
  opts: {
    tool: string;
    action?: string;
    args?: Record<string, unknown>;
    sessionKey?: string;
    headers?: Record<string, unknown>;
    dryRun?: boolean;
    timeoutMs?: number;
  },
): Promise<{ response: GatewayInvokeResponse; status: number | null }> {
  await ensureGateway(sandbox, env);

  const payload: Record<string, unknown> = { tool: opts.tool };
  if (typeof opts.action === 'string' && opts.action.trim()) {
    payload.action = opts.action.trim();
  }
  if (opts.args && Object.keys(opts.args).length > 0) {
    payload.args = opts.args;
  }
  if (typeof opts.sessionKey === 'string' && opts.sessionKey.trim()) {
    payload.sessionKey = opts.sessionKey.trim();
  }
  if (typeof opts.dryRun === 'boolean') {
    payload.dryRun = opts.dryRun;
  }

  const headers = normalizeGatewayHeaders(opts.headers);
  const headerArgs: string[] = [
    '-H',
    escapeShellArg('content-type: application/json'),
    '-H',
    escapeShellArg('accept: application/json'),
  ];

  if (env.OPENCLAW_GATEWAY_TOKEN && env.OPENCLAW_GATEWAY_TOKEN.trim()) {
    headerArgs.push('-H', escapeShellArg(`authorization: Bearer ${env.OPENCLAW_GATEWAY_TOKEN.trim()}`));
  }

  for (const [key, value] of Object.entries(headers)) {
    headerArgs.push('-H', escapeShellArg(`${key}: ${value}`));
  }

  const bodyText = JSON.stringify(payload);
  const format = `\\n${GATEWAY_STATUS_MARKER}%{http_code}`;
  const cmd = [
    'curl',
    '-sS',
    '-X',
    'POST',
    `${GATEWAY_HTTP_URL}/tools/invoke`,
    ...headerArgs,
    '--data',
    escapeShellArg(bodyText),
    '-w',
    escapeShellArg(format),
  ].join(' ');

  const result = await sandbox.exec(cmd, { timeout: opts.timeoutMs ?? GATEWAY_HTTP_TIMEOUT_MS });
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  if (!stdout.trim() && result.exitCode !== 0) {
    throw new Error(`gateway invoke failed: ${stderr || 'unknown error'}`);
  }

  const { body, status } = splitGatewayOutput(stdout);
  if (!body.trim()) {
    throw new Error(`gateway invoke returned empty response: ${stderr || 'no body'}`);
  }

  let parsed: GatewayInvokeResponse;
  try {
    parsed = JSON.parse(body) as GatewayInvokeResponse;
  } catch (error) {
    throw new Error(`gateway invoke returned invalid json: ${error instanceof Error ? error.message : 'parse error'}`);
  }

  return { response: parsed, status };
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
