import { httpRouter } from 'convex/server';
import { ingest as nostrIngest } from './nostr_http';

const http = httpRouter();

http.route({
  path: '/nostr/ingest',
  method: 'POST',
  handler: nostrIngest,
});

export default http;
