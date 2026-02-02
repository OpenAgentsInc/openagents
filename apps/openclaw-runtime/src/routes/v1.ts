import { Hono } from 'hono';
import type { OpenClawEnv } from '../types';
import { ok, err } from '../response';
import { getOpenClawSandbox } from '../sandbox/sandboxDo';
import { backupToR2 } from '../sandbox/backup';
import { getLastBackup } from '../sandbox/r2';
import {
  approveDevice,
  ensureGateway,
  getClawdbotVersion,
  getGatewayStatus,
  listDevices,
  restartGateway,
} from '../sandbox/process';

const v1 = new Hono<{ Bindings: OpenClawEnv }>();

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

export default v1;
