import {
  type RouteConfig,
  index,
  route,
  layout,
} from "@react-router/dev/routes";

/**
 * Pages that SHOULD show the global header live inside the `with-header` layout.
 * Anything that needs a bare (no‑header) canvas is declared beneath that block.
 */
export default [
  layout("components/layouts/with-header.tsx", [
    // --- Public or marketing pages -----------------------------
    index("routes/home.tsx"),
    route("login", "routes/login.tsx"),
    route("signup", "routes/signup.tsx"),

    // --- Auth‑protected app pages -------------------------------
    route("projects", "routes/projects.tsx"),
    route("projects/:id", "routes/projects/$id.tsx"),
    route("teams", "routes/teams.tsx"),
    route("members", "routes/members.tsx"),
    route("issues", "routes/issues.tsx"),
    route("issues/:id", "routes/issues/$id.tsx"),
  ]),

  // ---------------- Routes WITHOUT the global header ------------
  route("spawn", "routes/spawn.tsx"),
  route("agent/:agentId", "routes/agent/$agentId.tsx"),
  route("api/auth/*", "routes/api.auth.$.tsx"),
] satisfies RouteConfig;
