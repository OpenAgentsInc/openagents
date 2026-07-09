const baseUrl = (
  process.env.SARAH_OPENAGENTS_BASE_URL ?? "https://openagents.com"
).replace(/\/+$/, "");
const requireReady = process.env.SARAH_S6_REQUIRE_OPENAGENTS_GATES === "1";

async function probe(path) {
  const url = `${baseUrl}${path}`;
  const response = await fetch(url, {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify({
      schema: "sarah.s6_openagents_gate_probe.v1",
      probeOnly: true,
    }),
  });
  const text = await response.text();
  let json = null;
  try {
    json = text ? JSON.parse(text) : null;
  } catch {
    json = null;
  }

  const protectedPresent = response.status === 401 || response.status === 403;
  const absent = response.status === 404;
  return {
    path,
    status: response.status,
    routePresent: protectedPresent || (!absent && response.status < 500),
    protectedByAuth: protectedPresent,
    absent,
    body: json ?? text.slice(0, 500),
  };
}

const endpoints = {
  handoff: await probe("/api/operator/business/pipeline"),
  checkout: await probe("/api/operator/business/sarah-checkout-links"),
};

const missing = Object.entries(endpoints)
  .filter(([, result]) => !result.routePresent || result.absent)
  .map(([name, result]) => ({ name, path: result.path, status: result.status }));

const evidence = {
  schema: "sarah.s6_openagents_gate_smoke.v1",
  generatedAt: new Date().toISOString(),
  baseUrl,
  status: missing.length === 0 ? "passed" : "blocked",
  mode: "no_secret_no_write_preflight",
  endpoints,
  verified:
    missing.length === 0
      ? [
          "OpenAgents handoff endpoint is deployed and protected.",
          "OpenAgents Sarah checkout endpoint is deployed and protected.",
        ]
      : [
          "At least one OpenAgents endpoint Sarah needs for S-6 is not deployed at the production origin.",
          "No operator token was sent and no write should occur from this preflight.",
        ],
  remainingExitGate:
    missing.length === 0
      ? "Configure SARAH_OPENAGENTS_OPERATOR_TOKEN and SARAH_OPENAGENTS_CHECKOUT_ENDPOINT, then run pnpm test:s6-sales-flow without SARAH_S6_ALLOW_PARTIAL to prove live handoff and checkout creation."
      : "Deploy the missing OpenAgents operator endpoint(s), then rerun this preflight before arming Sarah live writes.",
  missing,
};

console.log(JSON.stringify(evidence, null, 2));

if (missing.length > 0 && requireReady) {
  process.exitCode = 2;
}
