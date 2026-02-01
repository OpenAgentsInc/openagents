import { httpRouter } from "convex/server";
import { authComponent, createAuth } from "./auth";
import { ingest as nostrIngest } from "./nostr_http";
import {
  register as controlRegister,
  createProject as controlCreateProject,
  listProjects as controlListProjects,
} from "./control_http";

const http = httpRouter();

authComponent.registerRoutes(http, createAuth, { cors: true });

http.route({
  path: "/nostr/ingest",
  method: "POST",
  handler: nostrIngest,
});

http.route({
  path: "/control/register",
  method: "POST",
  handler: controlRegister,
});

http.route({
  path: "/control/projects",
  method: "POST",
  handler: controlCreateProject,
});

http.route({
  path: "/control/projects",
  method: "GET",
  handler: controlListProjects,
});

export default http;
