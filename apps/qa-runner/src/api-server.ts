// The QA control HTTP daemon (#6196): drive the full autonomous-QA / eval flow
// over HTTP, not just the CLI.
//
// It runs the existing runner/evals engine IN-PROCESS (QaControl), async, with
// an in-memory job store, on a machine that HAS Chrome (the runner drives a real
// Playwright browser, which cannot run in a Cloudflare Worker). The Worker side
// (/pro) only dereferences the public-safe artifacts this daemon produces.
//
// Endpoints (all require `Authorization: Bearer <khala-agent-token>`):
//   POST /runs               -> submit a run; returns { id, status, ... }
//   GET  /runs/:id           -> job status
//   GET  /runs/:id/artifacts -> video/url, committed test ref, result.json
//                               (incl. additive verify verdict + receipt if
//                               present), and the /pro/runs/:id link
//   POST /evals              -> submit a variant comparison (>= 2 variants)
//   GET  /evals/:id          -> the comparison + /pro/evals/:id link
//   POST /swarm-runs         -> compose qa-runner fanout into a QA Swarm
//                               projection + /qa/{runRef} share URL
//   GET  /swarm-runs/:id     -> swarm run projection/status
//   GET  /healthz            -> liveness (no auth)
//
// OpenAI-compatible shapes where they fit: a 401 returns an OpenAI-style
// `{ error: { message, type, code } }` body; submit responses carry a stable
// `object` discriminator. The inference itself is already OpenAI-compatible
// (khala-openrouter); this surface mirrors that envelope for errors so existing
// OpenAI clients' error handling works.
//
// This file is the transport only — it OWNS no schema and no engine logic; it
// maps HTTP <-> QaControl (control.ts) and enforces auth (control-auth.ts).

import {
  BadRequestError,
  NotArmedError,
  NotFoundError,
  QaControl,
  type ControlOptions,
  type SubmitEvalInput,
  type SubmitRunInput,
  type SubmitSwarmRunInput,
} from "./control";
import {
  allowlistFromEnv,
  bearerFrom,
  makeTokenVerifier,
  type TokenVerifier,
} from "./control-auth";

export interface ApiServerOptions {
  readonly control: QaControl;
  readonly verifier: TokenVerifier;
}

// ── JSON helpers (OpenAI-compatible error envelope) ─────────────────────────

const json = (status: number, body: unknown): Response =>
  new Response(`${JSON.stringify(body, null, 2)}\n`, {
    status,
    headers: { "content-type": "application/json" },
  });

const oaError = (
  status: number,
  message: string,
  type: string,
  code: string,
): Response => json(status, { error: { message, type, code } });

const unauthorized = (reason: string): Response =>
  oaError(401, reason, "invalid_request_error", "invalid_api_key");

// ── the fetch handler (testable without binding a port) ─────────────────────

export function makeFetchHandler(options: ApiServerOptions): (req: Request) => Promise<Response> {
  const { control, verifier } = options;

  return async (req: Request): Promise<Response> => {
    const url = new URL(req.url);
    const path = url.pathname.replace(/\/+$/, "") || "/";
    const method = req.method.toUpperCase();

    // Liveness needs no auth.
    if (path === "/healthz" && method === "GET") {
      return json(200, { object: "qa_control.health", status: "ok" });
    }

    // Auth gate: every other route requires a valid Khala agent bearer token.
    const auth = verifier.verify(bearerFrom(req.headers.get("authorization")));
    if (!auth.ok) return unauthorized(auth.reason ?? "unauthorized");

    try {
      // POST /runs
      if (path === "/runs" && method === "POST") {
        const input = (await readJson(req)) as SubmitRunInput;
        const job = control.submitRun(input ?? {});
        return json(202, { object: "qa_control.run", ...job });
      }

      // GET /runs/:id/artifacts
      const artMatch = /^\/runs\/([^/]+)\/artifacts$/.exec(path);
      if (artMatch && method === "GET") {
        const res = control.runArtifacts(decodeURIComponent(artMatch[1]!));
        return json(200, { object: "qa_control.run_artifacts", ...res });
      }

      // GET /runs/:id
      const runMatch = /^\/runs\/([^/]+)$/.exec(path);
      if (runMatch && method === "GET") {
        const job = control.status(decodeURIComponent(runMatch[1]!));
        return json(200, { object: "qa_control.run", ...job });
      }

      // POST /evals
      if (path === "/evals" && method === "POST") {
        const input = (await readJson(req)) as SubmitEvalInput;
        const job = control.submitEval(input ?? {});
        return json(202, { object: "qa_control.eval", ...job });
      }

      // POST /swarm-runs
      if (path === "/swarm-runs" && method === "POST") {
        const input = (await readJson(req)) as SubmitSwarmRunInput;
        const job = control.submitSwarmRun(input ?? {});
        return json(202, { object: "qa_control.swarm_run", ...job });
      }

      // GET /swarm-runs/:id
      const swarmMatch = /^\/swarm-runs\/([^/]+)$/.exec(path);
      if (swarmMatch && method === "GET") {
        const res = control.swarmRunArtifacts(decodeURIComponent(swarmMatch[1]!));
        return json(200, { object: "qa_control.swarm_run_artifacts", ...res });
      }

      // GET /evals/:id
      const evalMatch = /^\/evals\/([^/]+)$/.exec(path);
      if (evalMatch && method === "GET") {
        const res = control.evalComparison(decodeURIComponent(evalMatch[1]!));
        return json(200, { object: "qa_control.eval_comparison", ...res });
      }

      return oaError(404, `no route ${method} ${path}`, "invalid_request_error", "not_found");
    } catch (error) {
      return mapError(error);
    }
  };
}

async function readJson(req: Request): Promise<unknown> {
  const text = await req.text();
  if (!text.trim()) return {};
  try {
    return JSON.parse(text);
  } catch {
    throw new BadRequestError("request body is not valid JSON");
  }
}

function mapError(error: unknown): Response {
  if (error instanceof BadRequestError) {
    return oaError(400, error.message, "invalid_request_error", "bad_request");
  }
  if (error instanceof NotArmedError) {
    // 403: the request is well-formed but a real run is not armed on this daemon.
    return oaError(403, error.message, "invalid_request_error", "not_armed");
  }
  if (error instanceof NotFoundError) {
    return oaError(404, error.message, "invalid_request_error", "not_found");
  }
  const message = error instanceof Error ? error.message : String(error);
  return oaError(500, message, "api_error", "internal_error");
}

// ── bind a port (the real daemon entrypoint helper) ─────────────────────────

export interface ServeOptions {
  readonly port?: number;
  readonly hostname?: string;
  readonly controlOptions?: Partial<ControlOptions>;
  readonly verifier?: TokenVerifier;
}

/**
 * Build a QaControl + verifier from env and start a Bun HTTP server. Returns the
 * Bun server handle. The daemon (daemon.ts) calls this in API mode.
 *
 * Env:
 *   QA_CONTROL_PORT       (default 8787)
 *   QA_CONTROL_STORE_DIR  (default ./runs/control)
 *   QA_CONTROL_PRO_BASE_URL (default https://openagents.com)
 *   QA_CONTROL_ARM_REAL=1 to allow real (network/spend) runs (default mock-only)
 *   QA_CONTROL_TOKEN_BUDGET default per-run token cap for real runs
 *   QA_CONTROL_TOKENS     comma-separated agent:token allowlist (fail closed if empty)
 */
export function serveControlApi(opts: ServeOptions = {}): ReturnType<typeof Bun.serve> {
  const env = process.env;
  const port = opts.port ?? Number(env.QA_CONTROL_PORT ?? 8787);
  const hostname = opts.hostname ?? env.QA_CONTROL_HOSTNAME ?? "127.0.0.1";
  const control = new QaControl({
    storeDir: env.QA_CONTROL_STORE_DIR ?? "./runs/control",
    proBaseUrl: env.QA_CONTROL_PRO_BASE_URL ?? "https://openagents.com",
    allowReal: env.QA_CONTROL_ARM_REAL === "1",
    defaultTokenBudget: Number(env.QA_CONTROL_TOKEN_BUDGET ?? 0),
    ...opts.controlOptions,
  });
  const verifier = opts.verifier ?? makeTokenVerifier(allowlistFromEnv(env));
  const handler = makeFetchHandler({ control, verifier });

  return Bun.serve({ port, hostname, fetch: handler });
}
