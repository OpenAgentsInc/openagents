import { httpAction } from "./_generated/server";
import { internal } from "./_generated/api";

function unauthorized(): Response {
  return new Response("Unauthorized", { status: 401 });
}

export const ingest = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const secret = process.env.NOSTR_INGEST_KEY;
  if (secret) {
    const header = request.headers.get("x-oa-ingest-key");
    if (!header || header !== secret) return unauthorized();
  }

  let payload: { events?: unknown; relay?: string } = {};
  try {
    payload = (await request.json()) as { events?: unknown; relay?: string };
  } catch {
    return new Response("Invalid JSON", { status: 400 });
  }

  if (!Array.isArray(payload.events)) {
    return new Response("Missing events array", { status: 400 });
  }

  const result = await ctx.runMutation(internal.nostr.ingestEvents, {
    events: payload.events,
    relay: payload.relay,
  });
  return new Response(JSON.stringify(result), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});
