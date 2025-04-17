import { type RouteConfig, index, route, layout } from "@react-router/dev/routes";

export default [
  layout("components/layouts/with-header.tsx", [
    index("routes/home.tsx"),
    route("login", "routes/login.tsx"),
    route("signup", "routes/signup.tsx"),
  ]),
  // Routes without header
  route("spawn", "routes/spawn.tsx"),
  route("agent/:agentId", "routes/agent/$agentId.tsx"),
  route("api/auth/*", "routes/api.auth.$.tsx"),
  route("projects", "routes/projects.tsx"),
  route("projects/:id", "routes/projects/$id.tsx"),
  route("teams", "routes/teams.tsx"),
  route("members", "routes/members.tsx"),
  route("issues", "routes/issues.tsx"),
  route("issues/:id", "routes/issues/$id.tsx"),
] satisfies RouteConfig;
