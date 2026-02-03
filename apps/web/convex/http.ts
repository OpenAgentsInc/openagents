import { httpRouter } from 'convex/server';
import { ingest as nostrIngest } from './nostr_http';
import { resolveToken, registerAgent } from './control_auth';

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

export default http;
