import type { Sandbox, Process, ExecResult } from '@cloudflare/sandbox';
import type { OpenClawEnv } from '../types';
import {
  CLI_TIMEOUT_MS,
  GATEWAY_HTTP_TIMEOUT_MS,
  GATEWAY_HTTP_URL,
  GATEWAY_PORT,
  GATEWAY_STREAM_TIMEOUT_MS,
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

type PairingRequest = {
  id?: string;
  code?: string;
  createdAt?: string;
  lastSeenAt?: string;
  meta?: Record<string, string>;
};

type PairingListResult = {
  channel: string;
  requests: PairingRequest[];
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

function parsePairingJson(stdout: string, stderr: string): PairingListResult {
  const jsonMatch = stdout.match(/\{[\s\S]*\}/);
  if (!jsonMatch) {
    return { channel: '', requests: [], raw: stdout, stderr, parseError: 'No JSON payload found' };
  }
  try {
    const parsed = JSON.parse(jsonMatch[0]);
    return {
      channel: typeof parsed.channel === 'string' ? parsed.channel : '',
      requests: Array.isArray(parsed.requests) ? (parsed.requests as PairingRequest[]) : [],
    };
  } catch {
    return { channel: '', requests: [], raw: stdout, stderr, parseError: 'Failed to parse JSON payload' };
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

export async function listPairingRequests(
  sandbox: Sandbox,
  env: OpenClawEnv,
  channel: string,
): Promise<{ channel: string; requests: PairingRequest[] }> {
  const trimmed = channel.trim();
  if (!trimmed) {
    throw new Error('channel is required');
  }
  await ensureGateway(sandbox, env);
  const result = await execCli(sandbox, `openclaw pairing list ${escapeShellArg(trimmed)} --json`);
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const parsed = parsePairingJson(stdout, stderr);
  if (parsed.parseError) {
    throw new Error(parsed.parseError);
  }
  return {
    channel: parsed.channel || trimmed,
    requests: parsed.requests,
  };
}

export async function approvePairingRequest(
  sandbox: Sandbox,
  env: OpenClawEnv,
  channel: string,
  code: string,
  notify?: boolean,
): Promise<{ approved: boolean; channel: string; code: string; stdout?: string; stderr?: string }> {
  const trimmedChannel = channel.trim();
  if (!trimmedChannel) {
    throw new Error('channel is required');
  }
  const trimmedCode = code.trim();
  if (!trimmedCode) {
    throw new Error('code is required');
  }
  await ensureGateway(sandbox, env);
  const notifyFlag = notify ? ' --notify' : '';
  const result = await execCli(
    sandbox,
    `openclaw pairing approve ${escapeShellArg(trimmedChannel)} ${escapeShellArg(trimmedCode)}${notifyFlag}`,
  );
  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  if (result.exitCode !== 0) {
    throw new Error(stderr || stdout || 'pairing approve failed');
  }
  return { approved: true, channel: trimmedChannel, code: trimmedCode, stdout, stderr };
}

type GatewayInvokeOk = { ok: true; result: unknown };
type GatewayInvokeErr = { ok: false; error?: { type?: string; message?: string } };
export type GatewayInvokeResponse = GatewayInvokeOk | GatewayInvokeErr;

const GATEWAY_STATUS_MARKER = '__OPENCLAW_GATEWAY_STATUS__';
const GATEWAY_HEADER_ALLOWLIST = new Set(['x-openclaw-message-channel', 'x-openclaw-account-id']);
const GATEWAY_RESPONSE_HEADER_ALLOWLIST = new Set([
  'x-openclaw-message-channel',
  'x-openclaw-account-id',
  'x-openclaw-session-key',
  'x-openclaw-agent-id',
]);

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

function normalizeGatewayResponseHeaders(
  headers: Record<string, string> | undefined,
): Record<string, string> {
  if (!headers) return {};
  const normalized: Record<string, string> = {};
  for (const [key, value] of Object.entries(headers)) {
    const headerKey = key.trim().toLowerCase();
    if (!GATEWAY_RESPONSE_HEADER_ALLOWLIST.has(headerKey)) continue;
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

export async function streamGatewayResponses(
  sandbox: Sandbox,
  env: OpenClawEnv,
  opts: {
    body: string;
    headers?: Record<string, string>;
    timeoutMs?: number;
    signal?: AbortSignal;
  },
): Promise<ReadableStream<Uint8Array>> {
  await ensureGateway(sandbox, env);

  const headers = normalizeGatewayResponseHeaders(opts.headers);
  const headerArgs: string[] = [
    '-H',
    escapeShellArg('content-type: application/json'),
    '-H',
    escapeShellArg('accept: text/event-stream'),
  ];

  if (env.OPENCLAW_GATEWAY_TOKEN && env.OPENCLAW_GATEWAY_TOKEN.trim()) {
    headerArgs.push('-H', escapeShellArg(`authorization: Bearer ${env.OPENCLAW_GATEWAY_TOKEN.trim()}`));
  }

  for (const [key, value] of Object.entries(headers)) {
    headerArgs.push('-H', escapeShellArg(`${key}: ${value}`));
  }

  const cmd = [
    'curl',
    '-sS',
    '-N',
    '-X',
    'POST',
    `${GATEWAY_HTTP_URL}/v1/responses`,
    ...headerArgs,
    '--data',
    escapeShellArg(opts.body),
  ].join(' ');

  const encoder = new TextEncoder();
  let controller: ReadableStreamDefaultController<Uint8Array> | null = null;
  const abortController = new AbortController();
  if (opts.signal) {
    opts.signal.addEventListener('abort', () => abortController.abort());
  }

  const stream = new ReadableStream<Uint8Array>({
    start(controllerArg) {
      controller = controllerArg;
      const safeEnqueue = (chunk: Uint8Array) => {
        try {
          controller?.enqueue(chunk);
        } catch {
          // ignore
        }
      };

      sandbox
        .exec(cmd, {
          timeout: opts.timeoutMs ?? GATEWAY_STREAM_TIMEOUT_MS,
          stream: true,
          signal: abortController.signal,
          onOutput: (streamName, data) => {
            if (streamName === 'stderr') {
              if (data.trim()) console.log('gateway responses stderr:', data);
              return;
            }
            safeEnqueue(encoder.encode(data));
          },
          onError: (error) => {
            try {
              controller?.error(error);
            } catch {
              // ignore
            }
          },
          onComplete: () => {
            try {
              controller?.close();
            } catch {
              // ignore
            }
          },
        })
        .catch((error) => {
          try {
            controller?.error(error);
          } catch {
            // ignore
          }
        });
    },
    cancel() {
      abortController.abort();
    },
  });

  return stream;
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
