import { Hono } from 'hono';
import type { ContentfulStatusCode } from 'hono/utils/http-status';
import type { OpenClawEnv } from '../types';
import { ok, err } from '../response';
import { getOpenClawSandbox } from '../sandbox/sandboxDo';
import { backupToR2 } from '../sandbox/backup';
import { getLastBackup } from '../sandbox/r2';
import {
  approveDevice,
  approvePairingRequest,
  ensureGateway,
  getClawdbotVersion,
  getGatewayStatus,
  invokeGatewayTool,
  listDevices,
  listPairingRequests,
  restartGateway,
  stopGateway,
  streamGatewayResponses,
} from '../sandbox/process';

const v1 = new Hono<{ Bindings: OpenClawEnv }>();

function parsePositiveInt(value: string | null): number | undefined {
  if (!value) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) return undefined;
  return parsed;
}

function parseNonNegativeInt(value: string | null): number | undefined {
  if (!value) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < 0) return undefined;
  return parsed;
}

function parseBoolean(value: string | null): boolean | undefined {
  if (!value) return undefined;
  const normalized = value.trim().toLowerCase();
  if (['1', 'true', 'yes', 'on'].includes(normalized)) return true;
  if (['0', 'false', 'no', 'off'].includes(normalized)) return false;
  return undefined;
}

function parseKinds(searchParams: URLSearchParams): string[] | undefined {
  const raw = searchParams.getAll('kinds');
  const kinds: string[] = [];
  for (const entry of raw) {
    for (const part of entry.split(',')) {
      const value = part.trim();
      if (value) kinds.push(value);
    }
  }
  return kinds.length > 0 ? kinds : undefined;
}

function toStatusCode(status: number | null | undefined, fallback: ContentfulStatusCode): ContentfulStatusCode {
  if (typeof status === 'number' && status >= 400 && status <= 599) {
    return status as ContentfulStatusCode;
  }
  return fallback;
}

function extractGatewayResponseHeaders(headers: Headers): Record<string, string> {
  const allowlist = new Set([
    'x-openclaw-session-key',
    'x-openclaw-agent-id',
    'x-openclaw-message-channel',
    'x-openclaw-account-id',
  ]);
  const result: Record<string, string> = {};
  for (const [key, value] of headers.entries()) {
    const normalized = key.toLowerCase();
    if (!allowlist.has(normalized)) continue;
    const trimmed = value.trim();
    if (!trimmed) continue;
    result[normalized] = trimmed;
  }
  return result;
}

v1.get('/status', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  let gatewayStatus = await getGatewayStatus(sandbox);

  if (gatewayStatus === 'stopped') {
    try {
      await ensureGateway(sandbox, c.env);
      gatewayStatus = 'running';
    } catch {
      gatewayStatus = 'error';
    }
  }

  const lastBackup = await getLastBackup(c.env);
  const version = await getClawdbotVersion(sandbox, c.env);
  const instanceType = c.env.OPENCLAW_INSTANCE_TYPE ?? 'standard-4';

  return c.json(
    ok({
      gateway: { status: gatewayStatus },
      lastBackup,
      container: { instanceType },
      version: { clawdbot: version ?? 'unknown' },
    })
  );
});

v1.post('/gateway/restart', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  try {
    await restartGateway(sandbox, c.env);
    return c.json(ok({ message: 'restarting' }));
  } catch (error) {
    return c.json(err('internal_error', 'failed to restart gateway', { message: String(error) }), 500);
  }
});

v1.post('/gateway/stop', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  try {
    await stopGateway(sandbox);
    return c.json(ok({ stopped: true }));
  } catch (error) {
    return c.json(err('internal_error', 'failed to stop gateway', { message: String(error) }), 500);
  }
});

v1.post('/storage/backup', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  try {
    const lastBackup = await backupToR2(sandbox, c.env);
    return c.json(ok({ lastBackup }));
  } catch (error) {
    return c.json(err('internal_error', 'failed to backup', { message: String(error) }), 500);
  }
});

v1.get('/devices', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  try {
    const devices = await listDevices(sandbox, c.env);
    return c.json(ok(devices));
  } catch (error) {
    return c.json(err('internal_error', 'failed to list devices', { message: String(error) }), 500);
  }
});

v1.post('/devices/:requestId/approve', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const requestId = c.req.param('requestId');

  if (!requestId) {
    return c.json(err('invalid_request', 'requestId is required'), 400);
  }

  try {
    const result = await approveDevice(sandbox, c.env, requestId);
    return c.json(ok({ approved: result.approved, requestId: result.requestId }));
  } catch (error) {
    return c.json(err('internal_error', 'failed to approve device', { message: String(error) }), 500);
  }
});

v1.get('/pairing/:channel', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const channel = c.req.param('channel');

  if (!channel) {
    return c.json(err('invalid_request', 'channel is required'), 400);
  }

  try {
    const requests = await listPairingRequests(sandbox, c.env, channel);
    return c.json(ok(requests));
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to list pairing requests', { message: String(error) }),
      500,
    );
  }
});

v1.post('/pairing/:channel/approve', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const channel = c.req.param('channel');

  if (!channel) {
    return c.json(err('invalid_request', 'channel is required'), 400);
  }

  const body = (await c.req.json().catch(() => null)) as Record<string, unknown> | null;
  const code = typeof body?.code === 'string' ? body.code.trim() : '';
  if (!code) {
    return c.json(err('invalid_request', 'code is required'), 400);
  }
  const notify = typeof body?.notify === 'boolean' ? body.notify : undefined;

  try {
    const result = await approvePairingRequest(sandbox, c.env, channel, code, notify);
    return c.json(ok(result));
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to approve pairing request', { message: String(error) }),
      500,
    );
  }
});

v1.post('/tools/invoke', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const body = await c.req.json().catch(() => null) as Record<string, unknown> | null;
  if (!body || typeof body !== 'object') {
    return c.json(err('invalid_request', 'invalid json body'), 400);
  }

  const tool = typeof body.tool === 'string' ? body.tool.trim() : '';
  if (!tool) {
    return c.json(err('invalid_request', 'tool is required'), 400);
  }

  const action = typeof body.action === 'string' ? body.action : undefined;
  const sessionKey = typeof body.sessionKey === 'string' ? body.sessionKey : undefined;
  const args = body.args && typeof body.args === 'object' && !Array.isArray(body.args)
    ? (body.args as Record<string, unknown>)
    : undefined;
  if (body.args && !args) {
    return c.json(err('invalid_request', 'args must be an object'), 400);
  }

  const headers = body.headers && typeof body.headers === 'object' && !Array.isArray(body.headers)
    ? (body.headers as Record<string, unknown>)
    : undefined;
  if (body.headers && !headers) {
    return c.json(err('invalid_request', 'headers must be an object'), 400);
  }

  const dryRun = typeof body.dryRun === 'boolean' ? body.dryRun : undefined;

  try {
    const { response, status } = await invokeGatewayTool(sandbox, c.env, {
      tool,
      action,
      args,
      sessionKey,
      headers,
      dryRun,
    });
    if (!response.ok) {
      const message = response.error?.message ?? 'tool invocation failed';
      const code = response.error?.type ?? 'gateway_error';
      const httpStatus = toStatusCode(status, 400);
      return c.json(err(code, message, { error: response.error ?? null }), httpStatus);
    }
    return c.json(ok(response.result));
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to invoke tool', { message: String(error) }),
      500,
    );
  }
});

v1.get('/sessions', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const url = new URL(c.req.url);
  const limit = parsePositiveInt(url.searchParams.get('limit'));
  const activeMinutes = parsePositiveInt(url.searchParams.get('activeMinutes'));
  const messageLimit = parseNonNegativeInt(url.searchParams.get('messageLimit'));
  const kinds = parseKinds(url.searchParams);

  const args: Record<string, unknown> = {};
  if (typeof limit === 'number') args.limit = limit;
  if (typeof activeMinutes === 'number') args.activeMinutes = activeMinutes;
  if (typeof messageLimit === 'number') args.messageLimit = messageLimit;
  if (kinds) args.kinds = kinds;

  try {
    const { response, status } = await invokeGatewayTool(sandbox, c.env, {
      tool: 'sessions_list',
      args: Object.keys(args).length > 0 ? args : undefined,
    });
    if (!response.ok) {
      const message = response.error?.message ?? 'failed to list sessions';
      const code = response.error?.type ?? 'gateway_error';
      const httpStatus = toStatusCode(status, 400);
      return c.json(err(code, message, { error: response.error ?? null }), httpStatus);
    }
    return c.json(ok(response.result));
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to list sessions', { message: String(error) }),
      500,
    );
  }
});

v1.get('/sessions/:sessionKey/history', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const sessionKey = c.req.param('sessionKey');
  if (!sessionKey) {
    return c.json(err('invalid_request', 'sessionKey is required'), 400);
  }

  const url = new URL(c.req.url);
  const limit = parsePositiveInt(url.searchParams.get('limit'));
  const includeTools = parseBoolean(url.searchParams.get('includeTools'));

  const args: Record<string, unknown> = { sessionKey };
  if (typeof limit === 'number') args.limit = limit;
  if (typeof includeTools === 'boolean') args.includeTools = includeTools;

  try {
    const { response, status } = await invokeGatewayTool(sandbox, c.env, {
      tool: 'sessions_history',
      args,
      sessionKey,
    });
    if (!response.ok) {
      const message = response.error?.message ?? 'failed to fetch session history';
      const code = response.error?.type ?? 'gateway_error';
      const httpStatus = toStatusCode(status, 400);
      return c.json(err(code, message, { error: response.error ?? null }), httpStatus);
    }
    return c.json(ok(response.result));
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to fetch session history', { message: String(error) }),
      500,
    );
  }
});

v1.post('/sessions/:sessionKey/send', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  const sessionKey = c.req.param('sessionKey');
  if (!sessionKey) {
    return c.json(err('invalid_request', 'sessionKey is required'), 400);
  }

  const body = await c.req.json().catch(() => null) as Record<string, unknown> | null;
  if (!body || typeof body !== 'object') {
    return c.json(err('invalid_request', 'invalid json body'), 400);
  }

  const message = typeof body.message === 'string' ? body.message.trim() : '';
  if (!message) {
    return c.json(err('invalid_request', 'message is required'), 400);
  }

  const timeoutSecondsRaw = typeof body.timeoutSeconds === 'number' ? body.timeoutSeconds : undefined;
  if (typeof timeoutSecondsRaw === 'number' && (!Number.isFinite(timeoutSecondsRaw) || timeoutSecondsRaw < 0)) {
    return c.json(err('invalid_request', 'timeoutSeconds must be a non-negative number'), 400);
  }

  const effectiveTimeout = typeof timeoutSecondsRaw === 'number' ? Math.floor(timeoutSecondsRaw) : 30;
  const timeoutMs = Math.min(Math.max((effectiveTimeout + 10) * 1000, 20_000), 120_000);

  const args: Record<string, unknown> = { sessionKey, message };
  if (typeof timeoutSecondsRaw === 'number') {
    args.timeoutSeconds = Math.floor(timeoutSecondsRaw);
  }

  try {
    const { response, status } = await invokeGatewayTool(sandbox, c.env, {
      tool: 'sessions_send',
      args,
      sessionKey,
      timeoutMs,
    });
    if (!response.ok) {
      const messageText = response.error?.message ?? 'failed to send session message';
      const code = response.error?.type ?? 'gateway_error';
      const httpStatus = toStatusCode(status, 400);
      return c.json(err(code, messageText, { error: response.error ?? null }), httpStatus);
    }
    return c.json(ok(response.result));
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to send session message', { message: String(error) }),
      500,
    );
  }
});

v1.post('/responses', async (c) => {
  const sandbox = getOpenClawSandbox(c.env);
  let bodyText = '';
  try {
    bodyText = await c.req.text();
  } catch {
    return c.json(err('invalid_request', 'invalid request body'), 400);
  }

  if (!bodyText.trim()) {
    return c.json(err('invalid_request', 'request body is required'), 400);
  }

  const headers = extractGatewayResponseHeaders(c.req.raw.headers);

  try {
    const stream = await streamGatewayResponses(sandbox, c.env, {
      body: bodyText,
      headers,
      signal: c.req.raw.signal,
    });
    return new Response(stream, {
      status: 200,
      headers: {
        'content-type': 'text/event-stream; charset=utf-8',
        'cache-control': 'no-cache',
        connection: 'keep-alive',
      },
    });
  } catch (error) {
    return c.json(
      err('internal_error', 'failed to proxy responses', { message: String(error) }),
      500,
    );
  }
});

export default v1;
