import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createReadStream, readFileSync } from "node:fs";
import { stat, realpath } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { homedir } from "node:os";
import { devOrigins } from "./tools/dev-origins.mjs";

const localOrigins = devOrigins();
const allowedPrivateOrigins = new Set([
  localOrigins.editorOrigin,
  localOrigins.previewOrigin,
]);

const privateRoot = resolve(
  process.env.AETHYRFORGE_PRIVATE_ASSETS ??
    resolve(homedir(), ".openagents/verse/private/aethyrforge"),
);
const streamRoot = process.env.AETHYRFORGE_STREAM_ROOT
  ? resolve(process.env.AETHYRFORGE_STREAM_ROOT)
  : undefined;
const contentTypes: Record<string, string> = {
  ".json": "application/json",
  ".bin": "application/octet-stream",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".wasm": "application/wasm",
};

export default defineConfig({
  plugins: [
    react(),
    {
      name: "aethyrforge-host-player-compatibility",
      transformIndexHtml: {
        order: "pre",
        handler(html, context) {
          if (
            context.filename.endsWith("/player.html") &&
            process.env.VITE_OPENAGENTS_HOST_ORIGIN
          )
            return readFileSync(resolve("apps/player/host-shell.html"), "utf8");
          return html;
        },
      },
    },
    {
      name: "aethyrforge-private-local-assets",
      configureServer(server) {
        if (streamRoot)
          server.middlewares.use("/player-artifact.json", async (req, res) => {
            const origin = req.headers.origin;
            if (
              (origin && !allowedPrivateOrigins.has(origin)) ||
              req.headers["sec-fetch-site"] === "cross-site"
            ) {
              res.statusCode = 403;
              res.end();
              return;
            }
            if (req.method !== "GET" && req.method !== "HEAD") {
              res.statusCode = 405;
              res.end();
              return;
            }
            try {
              const file = resolve(streamRoot, "player-artifact.json");
              const rootPath = await realpath(streamRoot),
                candidate = await realpath(file);
              if (!candidate.startsWith(rootPath + sep))
                throw new Error("Unavailable artifact");
              const info = await stat(candidate);
              if (!info.isFile() || info.size > 16 * 1024 * 1024)
                throw new Error("Unavailable artifact");
              res.setHeader("Content-Type", "application/json");
              res.setHeader("Content-Length", info.size);
              res.setHeader("Cache-Control", "no-store");
              if (req.method === "HEAD") res.end();
              else createReadStream(candidate).pipe(res);
            } catch {
              res.statusCode = 404;
              res.end();
            }
          });
        for (const prefix of [
          "private-assets",
          ...(streamRoot ? ["starter"] : []),
        ])
          server.middlewares.use("/" + prefix, async (req, res, next) => {
            try {
              const origin = req.headers.origin;
              if (
                (origin && !allowedPrivateOrigins.has(origin)) ||
                req.headers["sec-fetch-site"] === "cross-site"
              ) {
                res.statusCode = 403;
                res.end("Private project assets");
                return;
              }
              if (req.method !== "GET" && req.method !== "HEAD") {
                res.statusCode = 405;
                res.end();
                return;
              }
              const relative = decodeURIComponent(
                (req.url ?? "/").split("?")[0],
              );
              if (relative.includes("..") || !contentTypes[extname(relative)]) {
                res.statusCode = 404;
                res.end();
                return;
              }
              const roots = [
                ...(streamRoot ? [resolve(streamRoot, prefix)] : []),
                ...(prefix === "private-assets" ? [privateRoot] : []),
              ];
              let realFile: string | undefined;
              let size = 0;
              for (const root of roots) {
                try {
                  const file = resolve(root, "." + relative);
                  const rootPath = await realpath(root);
                  const candidate = await realpath(file);
                  if (!candidate.startsWith(rootPath + sep)) continue;
                  const info = await stat(candidate);
                  if (!info.isFile()) continue;
                  realFile = candidate;
                  size = info.size;
                  break;
                } catch {
                  /* A staged overlay does not contain every source asset. */
                }
              }
              if (!realFile) {
                if (prefix === "starter") {
                  next();
                  return;
                }
                res.statusCode = 404;
                res.end();
                return;
              }
              res.setHeader("Content-Type", contentTypes[extname(realFile)]);
              res.setHeader("Content-Length", size);
              res.setHeader("Cache-Control", "no-cache");
              res.setHeader("Cross-Origin-Resource-Policy", "same-site");
              if (req.method === "HEAD") res.end();
              else createReadStream(realFile).pipe(res);
            } catch {
              res.statusCode = 404;
              res.end("Prepare the private Medieval Town assets first.");
            }
          });
      },
    },
  ],
  server: {
    host: "127.0.0.1",
    allowedHosts: ["127.0.0.1", "localhost"],
    strictPort: true,
  },
  build: {
    rollupOptions: {
      input: {
        boids: resolve("boids.html"),
        editor: resolve("index.html"),
        player: resolve("player.html"),
        preview: resolve("preview.html"),
        universe: resolve("universe.html"),
        "universe-preview": resolve("universe-preview.html"),
      },
    },
  },
});
