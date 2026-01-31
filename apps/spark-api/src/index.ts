/**
 * Spark API — balance, invoice, pay for Agent Payments.
 * Served at openagents.com/api/spark/*.
 * Called by openagents-api when SPARK_API_URL points here.
 *
 * Responses are returned as JSON and wrapped by the API in { ok, data, error }.
 * Full Breez SDK integration (real balance/invoice/pay) requires a KV-backed
 * storage adapter; this implementation returns valid stub responses so the
 * flow works end-to-end on Cloudflare.
 */

export interface Env {
  WALLET_STATE?: KVNamespace;
}

const CORS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

function corsHeaders(): Record<string, string> {
  return { ...CORS, "Content-Type": "application/json" };
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: corsHeaders(),
  });
}

export default {
  async fetch(request: Request, env: Env, _ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);
    let path = url.pathname;
    if (path.startsWith("/api/spark")) {
      path = path.slice("/api/spark".length) || "/";
    }
    if (path === "") path = "/";

    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: CORS });
    }

    // GET /agents/:id/balance
    const balanceMatch = path.match(/^\/agents\/([^/]+)\/balance$/);
    if (request.method === "GET" && balanceMatch) {
      const agentId = balanceMatch[1];
      const balance = await getBalance(env, agentId);
      return jsonResponse(balance);
    }

    // POST /payments/invoice
    if (path === "/payments/invoice" && request.method === "POST") {
      const body = await request.json().catch(() => ({})) as {
        agent_id?: number;
        amount_sats?: number;
        description?: string;
      };
      const agentId = body.agent_id ?? 0;
      const amountSats = body.amount_sats ?? 0;
      const description = body.description ?? "";
      const invoice = await createInvoice(env, agentId, amountSats, description);
      return jsonResponse(invoice);
    }

    // POST /payments/pay
    if (path === "/payments/pay" && request.method === "POST") {
      const body = await request.json().catch(() => ({})) as {
        agent_id?: number;
        invoice?: string;
      };
      const agentId = body.agent_id ?? 0;
      const invoice = body.invoice ?? "";
      const result = await payInvoice(env, agentId, invoice);
      return jsonResponse(result, result.success ? 200 : 400);
    }

    if (path === "/health" || path === "/") {
      return jsonResponse({
        ok: true,
        service: "openagents-spark-api",
        path: "/api/spark",
      });
    }

    return jsonResponse({ error: "not found" }, 404);
  },
};

/** Stub: return balance for agent. Real impl would use Breez SDK + WALLET_STATE. */
async function getBalance(env: Env, agentId: string): Promise<{ balance_sats: number }> {
  if (env.WALLET_STATE) {
    const raw = await env.WALLET_STATE.get(`balance:${agentId}`);
    if (raw) {
      const n = parseInt(raw, 10);
      if (!Number.isNaN(n)) return { balance_sats: n };
    }
  }
  return { balance_sats: 0 };
}

/** Stub: create invoice for agent. Real impl would use Breez SDK + WALLET_STATE. */
async function createInvoice(
  _env: Env,
  _agentId: number,
  amountSats: number,
  _description: string
): Promise<{ invoice: string; amount_sats: number }> {
  return {
    invoice: `lnbc${amountSats}n1stubsparkapi`,
    amount_sats: amountSats,
  };
}

/** Stub: pay invoice from agent wallet. Real impl would use Breez SDK + WALLET_STATE. */
async function payInvoice(
  _env: Env,
  _agentId: number,
  invoice: string
): Promise<{ success: boolean; error?: string }> {
  if (!invoice || invoice.length < 10) {
    return { success: false, error: "invalid invoice" };
  }
  return { success: true };
}
