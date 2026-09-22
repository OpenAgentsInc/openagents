#!/usr/bin/env node
// One POST /v1/systemone call, Node's built-in fetch.
//
// Reads OPENAGENTS_BASE_URL and OPENAGENTS_API_KEY from the environment —
// the key stays out of every command line. Run:
//
//   node ask.js "I was charged twice on the March invoice."

async function main() {
  const base = (process.env.OPENAGENTS_BASE_URL || "").replace(/\/$/, "");
  const key = process.env.OPENAGENTS_API_KEY || "";
  if (!base || !key) {
    console.error("set OPENAGENTS_BASE_URL and OPENAGENTS_API_KEY");
    return 2;
  }
  const state = process.argv[2] || "I was charged twice.";

  const response = await fetch(`${base}/v1/systemone`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${key}`,
      "content-type": "application/json",
      "idempotency-key": "example-ask-1",
      "x-attempt": "1",
    },
    signal: AbortSignal.timeout(30_000),
    body: JSON.stringify({
      model: "shared-kev",
      state,
      questions: {
        refund: {
          type: "noul",
          instructions: "Does the customer ask for money back?",
        },
        department: {
          type: "choice",
          instructions: "Which team should handle this request?",
          criteria: {
            billing: "Charges, invoices, and refunds",
            technical: "Bugs and outages",
            none: "No team fits this request",
          },
        },
      },
    }),
  }).catch((error) => ({error}));

  if (response.error) {
    console.error(String(response.error));
    return 4;
  }
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    // Errors are typed: {"error": {"code", "message"}}.
    console.error(JSON.stringify(payload));
    return response.status >= 500 ? 4 : 3;
  }
  for (const [name, answer] of Object.entries(payload.answers)) {
    console.log(`${name}: ${JSON.stringify(answer)}`);
  }
  console.log(`model: ${payload.model}  usage: ${JSON.stringify(payload.usage)}`);
  return 0;
}

process.exit(await main());
