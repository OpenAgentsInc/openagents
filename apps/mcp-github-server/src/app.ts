import { Hono } from "hono";

export type Bindings = {
  // Add any environment bindings here if needed
};

const app = new Hono<{
  Bindings: Bindings;
}>();

// Render a basic homepage placeholder to make sure the app is up
app.get("/", (c) => {
  return c.html("<h1>MCP Demo</h1>");
});

export default app;
