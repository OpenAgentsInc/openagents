import { httpRouter } from 'convex/server';
import { ingest as nostrIngest } from './nostr_http';
import { resolveToken, registerAgent } from './control_auth';
import {
  handleInstanceGet,
  handleInstancePost,
  handleInstanceStatusPost,
  handleInstanceSecretPost,
  handleInstanceSecretGet,
  handleBillingSummaryGet,
} from './openclaw_control_http';

const http = httpRouter();

http.route({
  path: '/nostr/ingest',
  method: 'POST',
  handler: nostrIngest,
});

http.route({
  path: '/control/auth/resolve-token',
  method: 'POST',
  handler: resolveToken,
});

http.route({
  path: '/control/auth/agent/register',
  method: 'POST',
  handler: registerAgent,
});

// OpenClaw control API (used by API worker)
http.route({
  path: '/control/openclaw/instance',
  method: 'GET',
  handler: handleInstanceGet,
});
http.route({
  path: '/control/openclaw/instance',
  method: 'POST',
  handler: handleInstancePost,
});
http.route({
  path: '/control/openclaw/instance/status',
  method: 'POST',
  handler: handleInstanceStatusPost,
});
http.route({
  path: '/control/openclaw/instance/secret',
  method: 'POST',
  handler: handleInstanceSecretPost,
});
http.route({
  path: '/control/openclaw/instance/secret',
  method: 'GET',
  handler: handleInstanceSecretGet,
});
http.route({
  path: '/control/openclaw/billing/summary',
  method: 'GET',
  handler: handleBillingSummaryGet,
});

export default http;
