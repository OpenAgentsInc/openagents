import { httpRouter } from "convex/server";
import { authComponent, createAuth } from "./auth";
import { ingest as nostrIngest } from "./nostr_http";

const http = httpRouter();

authComponent.registerRoutes(http, createAuth, { cors: true });

http.route({
  path: "/nostr/ingest",
  method: "POST",
  handler: nostrIngest,
});

export default http;
