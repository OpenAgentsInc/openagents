import { httpRouter } from "convex/server";
import { authComponent, createAuth } from "./auth";
import { ingest as nostrIngest } from "./nostr_http";
import {
  register as controlRegister,
  createProject as controlCreateProject,
  listProjects as controlListProjects,
  createOrganization as controlCreateOrganization,
  listOrganizations as controlListOrganizations,
  createIssue as controlCreateIssue,
  listIssues as controlListIssues,
  updateIssue as controlUpdateIssue,
  deleteIssue as controlDeleteIssue,
  listRepos as controlListRepos,
  connectRepo as controlConnectRepo,
  disconnectRepo as controlDisconnectRepo,
  listTokens as controlListTokens,
  createToken as controlCreateToken,
  revokeToken as controlRevokeToken,
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

http.route({
  path: "/control/organizations",
  method: "POST",
  handler: controlCreateOrganization,
});

http.route({
  path: "/control/organizations",
  method: "GET",
  handler: controlListOrganizations,
});

http.route({
  path: "/control/issues",
  method: "POST",
  handler: controlCreateIssue,
});

http.route({
  path: "/control/issues",
  method: "GET",
  handler: controlListIssues,
});

http.route({
  path: "/control/issues",
  method: "PATCH",
  handler: controlUpdateIssue,
});

http.route({
  path: "/control/issues",
  method: "DELETE",
  handler: controlDeleteIssue,
});

http.route({
  path: "/control/repos",
  method: "GET",
  handler: controlListRepos,
});

http.route({
  path: "/control/repos",
  method: "POST",
  handler: controlConnectRepo,
});

http.route({
  path: "/control/repos",
  method: "DELETE",
  handler: controlDisconnectRepo,
});

http.route({
  path: "/control/tokens",
  method: "GET",
  handler: controlListTokens,
});

http.route({
  path: "/control/tokens",
  method: "POST",
  handler: controlCreateToken,
});

http.route({
  path: "/control/tokens",
  method: "DELETE",
  handler: controlRevokeToken,
});

export default http;
