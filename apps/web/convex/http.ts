import { httpRouter } from 'convex/server';
import { ingest as nostrIngest } from './nostr_http';
import { registerAgent, resolveToken } from './control_auth';
import {
  handleAgentSignup,
  handleAgentByKeyHash,
  handleAgentTouchKey,
} from './agent_control_http';

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

// Agent login (API-key auth; used by API worker)
http.route({
  path: '/control/agent/signup',
  method: 'POST',
  handler: handleAgentSignup,
});
http.route({
  path: '/control/agent/by-key-hash',
  method: 'GET',
  handler: handleAgentByKeyHash,
});
http.route({
  path: '/control/agent/touch',
  method: 'POST',
  handler: handleAgentTouchKey,
});

export default http;
