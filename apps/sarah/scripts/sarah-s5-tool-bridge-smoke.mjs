const baseUrl = (process.env.SARAH_S5_SMOKE_BASE_URL ?? "http://localhost:3000").replace(
  /\/+$/,
  "",
);
const prospectRef = `s5-smoke-${crypto.randomUUID()}`;
const toolCallId = `s5-smoke-${crypto.randomUUID()}`;

async function postToolCall() {
  const response = await fetch(`${baseUrl}/api/eve/tool-call`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      origin: baseUrl,
      cookie: `sarah_prospect_ref=${encodeURIComponent(prospectRef)}`,
    },
    body: JSON.stringify({
      toolCallId,
      toolName: "demo_sales_context",
      args: {
        topic: "S-5 realtime tool bridge smoke",
      },
    }),
  });
  const json = await response.json();
  if (!response.ok || !json.ok) {
    throw new Error(`Tool bridge failed: ${JSON.stringify(json)}`);
  }
  return json;
}

async function readReceipt() {
  const response = await fetch(`${baseUrl}/api/operator/ops`, {
    headers: { accept: "application/json" },
  });
  const json = await response.json();
  if (!response.ok || !json.ok) {
    throw new Error(`Operator ops read failed: ${JSON.stringify(json)}`);
  }

  return (json.receipts ?? []).find(
    (receipt) =>
      receipt.prospectRef === prospectRef &&
      receipt.toolsUsed?.some((tool) => tool.toolCallId === toolCallId),
  );
}

const result = await postToolCall();
let receipt = null;
for (let attempt = 0; attempt < 10; attempt += 1) {
  receipt = await readReceipt();
  if (receipt) break;
  await new Promise((resolve) => setTimeout(resolve, 250));
}

if (!receipt) {
  throw new Error(
    `S-5 tool receipt was not visible in operator ops for ${toolCallId}`,
  );
}

console.log(
  JSON.stringify(
    {
      schema: "sarah.s5_tool_bridge_smoke.v1",
      generatedAt: new Date().toISOString(),
      baseUrl,
      prospectRef,
      toolCallId,
      result: {
        ok: result.ok,
        toolName: result.toolName,
        receipt: result.receipt,
      },
      receipt: {
        sessionId: receipt.sessionId,
        threadId: receipt.threadId,
        transcriptTurns: receipt.transcriptTurns,
        toolsUsed: receipt.toolsUsed,
      },
    },
    null,
    2,
  ),
);
