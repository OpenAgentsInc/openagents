import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

const baseUrl = (
  process.env.SARAH_OPENAGENTS_BASE_URL ?? "https://openagents.com"
).replace(/\/+$/, "");
const requiredTools = ["crm.contact.upsert", "crm.activity.append"];
const existingToken = process.env.SARAH_OPENAGENTS_CRM_MCP_TOKEN?.trim();
const adminBearer =
  process.env.SARAH_S7_CRM_ADMIN_BEARER?.trim() ??
  process.env.OPENAGENTS_ADMIN_API_TOKEN?.trim() ??
  process.env.OPENAGENTS_COM_CRM_TOKEN?.trim();
const requireReady = process.env.SARAH_S7_REQUIRE_CRM_TOOLS === "1";
const outPath = join(
  process.cwd(),
  "docs",
  "evidence",
  "2026-07-08-s7-mcp-catalog-gate.json",
);

async function finish(evidence, exitCode = 0) {
  await mkdir(dirname(outPath), { recursive: true });
  await writeFile(outPath, `${JSON.stringify(evidence, null, 2)}\n`);
  console.log(JSON.stringify(evidence, null, 2));
  process.exitCode = exitCode;
}

async function jsonFetch(path, init) {
  const response = await fetch(`${baseUrl}${path}`, init);
  const text = await response.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = null;
  }
  return { json, response, text };
}

async function mintTemporaryGrant() {
  if (!adminBearer) return null;
  const { json, response, text } = await jsonFetch("/api/operator/crm/mcp-grants", {
    method: "POST",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${adminBearer}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      authorities: ["operator_read", "workspace_write"],
      label: `sarah-s7-catalog-gate-${new Date().toISOString()}`,
      tenant: "tenant.openagents",
    }),
  });

  if (!response.ok || typeof json?.token !== "string") {
    throw new Error(
      `Could not mint temporary CRM MCP grant: ${response.status} ${
        json?.error ?? json?.reason ?? text.slice(0, 200)
      }`,
    );
  }

  return {
    grant: json.grant,
    token: json.token,
  };
}

async function revokeTemporaryGrant(grantRef) {
  if (!adminBearer || !grantRef) return null;
  const { json, response } = await jsonFetch(
    `/api/operator/crm/mcp-grants/${encodeURIComponent(grantRef)}?tenant=tenant.openagents`,
    {
      method: "DELETE",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${adminBearer}`,
      },
    },
  );
  return {
    httpStatus: response.status,
    revoked: Boolean(json?.revoked),
  };
}

async function listTools(token) {
  const { json, response, text } = await jsonFetch("/api/mcp", {
    method: "POST",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      id: crypto.randomUUID(),
      jsonrpc: "2.0",
      method: "tools/list",
    }),
  });

  if (!response.ok || json?.error) {
    throw new Error(
      `MCP tools/list failed: ${response.status} ${
        json?.error?.message ?? text.slice(0, 200)
      }`,
    );
  }

  return Array.isArray(json?.result?.tools)
    ? json.result.tools.map((tool) => String(tool.name)).sort()
    : [];
}

let minted = null;
let revoke = null;

try {
  minted = existingToken ? null : await mintTemporaryGrant();
  const token = existingToken ?? minted?.token ?? null;

  if (!token) {
    const evidence = {
      schema: "sarah.s7_mcp_catalog_gate.v1",
      generatedAt: new Date().toISOString(),
      baseUrl,
      status: "blocked",
      reason: "missing_crm_mcp_token_or_admin_bearer",
      requiredEnv: [
        "SARAH_OPENAGENTS_CRM_MCP_TOKEN=<scoped oa_mcp_... token>",
        "or SARAH_S7_CRM_ADMIN_BEARER / OPENAGENTS_ADMIN_API_TOKEN=<admin bearer used only to mint a temporary scoped grant>",
      ],
    };
    await finish(evidence, requireReady ? 2 : 0);
  } else {
    const tools = await listTools(token);
    const missingTools = requiredTools.filter((tool) => !tools.includes(tool));
    const evidence = {
      schema: "sarah.s7_mcp_catalog_gate.v1",
      generatedAt: new Date().toISOString(),
      baseUrl,
      status: missingTools.length === 0 ? "passed" : "blocked",
      credentialMode: existingToken ? "provided_scoped_token" : "temporary_admin_minted_grant",
      temporaryGrant: minted
        ? {
            grantRef: minted.grant?.grantRef ?? null,
            authorities: minted.grant?.authorities ?? null,
            status: minted.grant?.status ?? null,
            tenantRef: minted.grant?.tenantRef ?? null,
          }
        : null,
      toolCount: tools.length,
      requiredTools: Object.fromEntries(
        requiredTools.map((tool) => [tool, tools.includes(tool)]),
      ),
      availableWriteLikeTools: tools.filter(
        (tool) =>
          tool.includes("upsert") ||
          tool.includes("append") ||
          tool.includes("send") ||
          tool.includes("import") ||
          tool.includes("batch"),
      ),
      missingTools,
      remainingExitGate:
        missingTools.length === 0
          ? "Run SARAH_OPENAGENTS_LIVE_WRITES=1 SARAH_OPENAGENTS_CRM_MCP_TOKEN=<scoped grant> pnpm test:s7-live-crm to prove contact upsert, activity append, and returning CRM context."
          : "Deploy the OpenAgents MCP catalog containing crm.contact.upsert and crm.activity.append, then rerun the S-7 live CRM smoke.",
    };

    if (minted?.grant?.grantRef) {
      revoke = await revokeTemporaryGrant(minted.grant.grantRef);
      evidence.temporaryGrant.revocation = revoke;
    }

    await finish(evidence, missingTools.length > 0 && requireReady ? 2 : 0);
  }
} catch (error) {
  if (minted?.grant?.grantRef) {
    revoke = await revokeTemporaryGrant(minted.grant.grantRef);
  }
  await finish(
    {
      schema: "sarah.s7_mcp_catalog_gate.v1",
      generatedAt: new Date().toISOString(),
      baseUrl,
      status: "blocked",
      reason: "catalog_gate_failed",
      error: error instanceof Error ? error.message : String(error),
      temporaryGrant: minted
        ? {
            grantRef: minted.grant?.grantRef ?? null,
            revokedAfterFailure: revoke,
          }
        : null,
      remainingExitGate:
        "Provide a current scoped Sarah CRM MCP token or a current production admin bearer that can mint a temporary scoped grant, then rerun this catalog gate before the live CRM smoke.",
    },
    requireReady ? 2 : 0,
  );
}
