import { type RouteConfig, index, route } from "@react-router/dev/routes";

export default [
  index("routes/home.tsx"),
  route("spawn", "routes/spawn.tsx"),
  route("login", "routes/login.tsx"),
  route("agent/:agentId", "routes/agent/$agentId.tsx")
] satisfies RouteConfig;
