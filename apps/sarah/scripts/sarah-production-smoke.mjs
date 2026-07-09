const baseUrl = (
  process.env.SARAH_PRODUCTION_BASE_URL ?? "https://openagents.com/sarah"
).replace(/\/+$/, "");
const mintRealtimeToken =
  process.env.SARAH_PRODUCTION_SMOKE_MINT_TOKEN === "1";
const timeoutMs = Number(process.env.SARAH_PRODUCTION_SMOKE_TIMEOUT_MS ?? 15_000);

function timeoutSignal() {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  return { signal: controller.signal, clear: () => clearTimeout(timeout) };
}

async function request(path, init = {}) {
  const timeout = timeoutSignal();
  try {
    const response = await fetch(`${baseUrl}${path}`, {
      ...init,
      signal: timeout.signal,
    });
    const text = await response.text();
    return {
      ok: response.ok,
      status: response.status,
      text,
      json: text && response.headers.get("content-type")?.includes("json")
        ? JSON.parse(text)
        : null,
    };
  } finally {
    timeout.clear();
  }
}

function assert(condition, message, evidence) {
  if (!condition) {
    const error = new Error(message);
    error.evidence = evidence;
    throw error;
  }
}

async function main() {
  const page = await request("/", {
    headers: { accept: "text/html" },
  });
  assert(page.ok, "Public Sarah page did not return 2xx.", page);
  assert(
    page.text.includes("Sarah") &&
      page.text.includes("OpenAgents") &&
      page.text.includes("AI"),
    "Public Sarah page did not include the pre-connect disclosure signals.",
    { status: page.status },
  );

  const sessionConfig = await request("/api/realtime/session-config", {
    headers: { accept: "application/json" },
  });
  assert(sessionConfig.ok, "Realtime session config did not return 2xx.", {
    status: sessionConfig.status,
    text: sessionConfig.text.slice(0, 500),
  });
  assert(
    sessionConfig.json?.voice === "shimmer",
    "Production Sarah voice is not pinned to the feminine shimmer voice.",
    { voice: sessionConfig.json?.voice },
  );
  assert(
    sessionConfig.json?.instructions?.includes("You are Sarah, OpenAgents' AI sales employee."),
    "Session config did not include Sarah identity instructions.",
    { instructionsPrefix: sessionConfig.json?.instructions?.slice(0, 200) },
  );

  let token = {
    skipped: true,
    reason:
      "Set SARAH_PRODUCTION_SMOKE_MINT_TOKEN=1 to mint a realtime token and verify Gateway wiring.",
  };
  if (mintRealtimeToken) {
    token = await request("/api/realtime/token", {
      method: "POST",
      headers: {
        accept: "application/json",
        origin: baseUrl,
        referer: `${baseUrl}/`,
      },
    });
    assert(token.ok, "Realtime token mint failed.", {
      status: token.status,
      text: token.text.slice(0, 500),
    });
    assert(
      token.json?.url && Array.isArray(token.json?.tools),
      "Realtime token response did not include websocket URL and tool definitions.",
      token.json,
    );
  }

  const evidence = {
    schema: "sarah.production_smoke.v1",
    generatedAt: new Date().toISOString(),
    baseUrl,
    mintRealtimeToken,
    checks: {
      publicPage: { status: page.status, ok: true },
      sessionConfig: {
        status: sessionConfig.status,
        ok: true,
        voice: sessionConfig.json?.voice,
      },
      token: mintRealtimeToken
        ? {
            status: token.status,
            ok: true,
            toolCount: token.json?.tools?.length ?? 0,
          }
        : token,
    },
  };
  console.log(JSON.stringify(evidence, null, 2));
}

await main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        schema: "sarah.production_smoke.v1",
        generatedAt: new Date().toISOString(),
        baseUrl,
        ok: false,
        error: error instanceof Error ? error.message : String(error),
        evidence: error?.evidence ?? null,
      },
      null,
      2,
    ),
  );
  process.exitCode = 1;
});
