import { type RouteConfig, index, route } from "@react-router/dev/routes";

export default [
  index("routes/home.tsx"),
  route("spawn", "routes/spawn.tsx"),
  route("login", "routes/login.tsx"),
  route("signup", "routes/signup.tsx"),
  route("agent/:agentId", "routes/agent/$agentId.tsx"),
  route("api/auth/*", "routes/api.auth.$.tsx")
] satisfies RouteConfig;
