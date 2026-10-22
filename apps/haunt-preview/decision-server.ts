import { spawn, type ChildProcess } from "node:child_process";
import { homedir } from "node:os";
import type { Plugin, ViteDevServer } from "vite";

/** Keep the Jev SDK and its key in the native local process. */
export function decisionServer(): Plugin {
  return {
    name: "haunt-decision-server",
    configureServer(server: ViteDevServer) {
      let child: ChildProcess | undefined;
      let starting: Promise<string> | undefined;
      const origin = () =>
        `http://127.0.0.1:${(server.httpServer!.address() as { port: number }).port}`;
      const start = () =>
        (starting ??= new Promise<string>((resolve, reject) => {
          const binary =
            process.env.HAUNT_DIRECTOR_BIN ||
            `${homedir()}/work/openagents-target-1/debug/haunt-director`;
          child = spawn(binary, [], {
            env: {
              ...process.env,
              HAUNT_EDITOR_ORIGIN: origin(),
              HAUNT_DIRECTOR_PORT: "0",
            },
            stdio: ["ignore", "pipe", "pipe"],
          });
          let output = "";
          const timer = setTimeout(() => {
            child?.kill();
            reject(new Error("The local decision service couldn't start."));
          }, 2000);
          child.stdout!.on("data", (chunk) => {
            output += chunk;
            const match = output.match(/^http:\/\/127\.0\.0\.1:\d+/m);
            if (match) {
              clearTimeout(timer);
              resolve(match[0]);
            }
          });
          child.once("error", () => {
            clearTimeout(timer);
            reject(new Error("Build the local scene decision service first."));
          });
          child.once("exit", () => {
            starting = undefined;
            clearTimeout(timer);
            reject(
              new Error(
                "Natural-language edits need the local decision service and its server credential.",
              ),
            );
          });
        }));
      server.httpServer?.once("close", () => child?.kill());
      server.middlewares.use(async (req, res, next) => {
        if (
          req.url !== "/api/haunt/decision" &&
          req.url !== "/api/haunt/capabilities"
        )
          return next();
        const reply = (status: number, body: unknown) => {
          res.statusCode = status;
          res.setHeader("Content-Type", "application/json");
          res.setHeader("Cache-Control", "no-store");
          res.end(JSON.stringify(body));
        };
        if (req.url === "/api/haunt/capabilities") {
          if (req.method !== "GET")
            return reply(405, { error: "Use the scene controls." });
          try {
            await start();
            return reply(200, { available: true });
          } catch {
            return reply(200, { available: false });
          }
        }
        if (
          req.method !== "POST" ||
          req.headers.origin !== origin() ||
          !req.headers["content-type"]?.startsWith("application/json")
        )
          return reply(403, {
            error: "Use the scene controls in this editor.",
          });
        try {
          const chunks: Buffer[] = [];
          let length = 0;
          for await (const chunk of req) {
            length += chunk.length;
            if (length > 4096)
              return reply(413, { error: "Describe one short scene change." });
            chunks.push(chunk);
          }
          const endpoint = await start();
          const upstream = await fetch(`${endpoint}/decide`, {
            method: "POST",
            headers: { "Content-Type": "application/json", Origin: origin() },
            body: Buffer.concat(chunks),
            signal: AbortSignal.timeout(2500),
          });
          reply(upstream.status, await upstream.json());
        } catch {
          return reply(503, {
            error:
              "The decision service is unavailable. Use the scene controls or try again.",
          });
        }
      });
    },
  };
}
