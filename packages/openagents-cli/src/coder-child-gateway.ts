/**
 * A local OpenAI-compatible endpoint that lends the session's thread grant to
 * child coding agents.
 *
 * Delegation used to require the reader to name a child model and to hold a
 * provider credential of their own, which meant the fleet was off by default
 * and `/delegate` in a fresh session did nothing. That was backwards: the
 * session already holds a grant the server minted for it, the grant is already
 * metered against the thread's own budget, and a child is the same kind of
 * spend as a reply. So children run on the session's grant, and delegation
 * needs no flags, no key, and no second account.
 *
 * A harness cannot talk to `POST /api/inference/proxy` directly, for two
 * reasons:
 *
 * - **Path.** An OpenAI-compatible client appends `/chat/completions` to its
 *   base URL, and the proxy is one fixed path that answers nothing else.
 * - **Tool history.** The proxy maps a `tool` message to a
 *   `function_call_output` item and sends it *alone*, which the provider
 *   refuses without the `function_call` that preceded it — and the response id
 *   that would link them is never given to a client. A coding agent calls a
 *   tool on nearly every step, so the second step of every child would fail.
 *   This gateway therefore flattens a tool exchange into plain turns: the
 *   assistant's call becomes assistant text naming the call, and the result
 *   becomes a user turn carrying the output. The harness keeps its own state
 *   machine and only needs the next call from the model, so a flattened history
 *   costs prompt-shape fidelity and nothing else.
 *
 * The grant never reaches the harness: it is held here, the child is told only
 * a loopback URL, and the placeholder key it sends back is ignored. That is why
 * the gateway binds to `127.0.0.1` and to a port the operating system picks —
 * anything on the box could otherwise spend the thread's budget, and a fixed
 * port would collide between two sessions.
 */

import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { Redacted } from "effect";

/** What the child gateway needs in order to spend a thread's grant. */
export interface ChildGrant {
  /** The proxy URL the grant was minted for. */
  readonly proxyUrl: string;
  readonly token: Redacted.Redacted<string>;
  /** The model the grant pins. A request body cannot select another. */
  readonly model: string;
}

/** One chat-completions message as a harness sends it. */
interface ClientMessage {
  readonly role?: unknown;
  readonly content?: unknown;
  readonly tool_calls?: unknown;
  readonly tool_call_id?: unknown;
}

/** A turn the proxy accepts: a role and text, and nothing else. */
interface FlatMessage {
  readonly role: "system" | "user" | "assistant";
  readonly content: string;
}

/**
 * Flatten a harness's message list into turns the proxy accepts.
 *
 * Exported for the tests, which is the only way to check the mapping without
 * standing up a server and a child.
 */
export function flattenForProxy(messages: ReadonlyArray<unknown>): ReadonlyArray<FlatMessage> {
  const flat: FlatMessage[] = [];

  for (const raw of messages) {
    if (typeof raw !== "object" || raw === null) continue;
    const message = raw as ClientMessage;
    const text = textOf(message.content);
    const role = typeof message.role === "string" ? message.role : "user";

    if (role === "tool") {
      const id = typeof message.tool_call_id === "string" ? message.tool_call_id : "";
      // A result is put on a user turn because that is the only role the proxy
      // will carry text on that the model has not already spoken.
      flat.push({ role: "user", content: `[tool result${id === "" ? "" : ` ${id}`}]\n${text}` });
      continue;
    }

    const calls = describeCalls(message.tool_calls);
    if (role === "assistant" && calls !== undefined) {
      flat.push({ role: "assistant", content: `${text}\n[tool call]\n${calls}`.trim() });
      continue;
    }

    // An empty turn is dropped rather than sent: the proxy refuses a request
    // whose input is empty, and a blank assistant turn is what a harness emits
    // around a call it has already described.
    if (text.trim().length === 0) continue;
    flat.push({
      role: role === "system" ? "system" : role === "assistant" ? "assistant" : "user",
      content: text,
    });
  }

  return flat;
}

/** The text of a message whose content may be a string or a part array. */
function textOf(content: unknown): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .map((part) => {
      if (typeof part === "string") return part;
      if (typeof part === "object" && part !== null) {
        const text = (part as { text?: unknown }).text;
        return typeof text === "string" ? text : "";
      }
      return "";
    })
    .join("");
}

/** The calls on an assistant message, as one line each, or nothing. */
function describeCalls(toolCalls: unknown): string | undefined {
  if (!Array.isArray(toolCalls) || toolCalls.length === 0) return undefined;
  const lines: string[] = [];
  for (const raw of toolCalls) {
    if (typeof raw !== "object" || raw === null) continue;
    const fn = (raw as { function?: { name?: unknown; arguments?: unknown } }).function ?? {};
    const id = (raw as { id?: unknown }).id;
    const name = typeof fn.name === "string" ? fn.name : "tool";
    const args = typeof fn.arguments === "string" ? fn.arguments : "";
    const suffix = typeof id === "string" && id.length > 0 ? ` id=${id}` : "";
    lines.push(`${name}(${args})${suffix}`);
  }
  return lines.length === 0 ? undefined : lines.join("\n");
}

/** A running gateway: where children should send their calls, and how to stop. */
export interface ChildGateway {
  /** The base URL a harness config points at, without a trailing slash. */
  readonly baseUrl: string;
  /** The model id, as the harness must name it: `provider/model`. */
  readonly modelId: string;
  close(): Promise<void>;
}

/** The provider name children see. Part of the model id they are given. */
export const CHILD_PROVIDER = "openagents";

/**
 * Start the gateway on a loopback port of the operating system's choosing.
 *
 * Resolves once it is listening, because a child launched against a port that
 * is not up yet fails on its first call and reports it as a provider error.
 */
export async function startChildGateway(grant: ChildGrant): Promise<ChildGateway> {
  const server = createServer((request, response) => {
    const chunks: Buffer[] = [];
    request.on("data", (chunk: Buffer) => chunks.push(chunk));
    request.on("end", () => {
      void forward(Buffer.concat(chunks).toString("utf8"), grant, response);
    });
  });

  // A child that hangs must not hold the console open on the way out.
  server.unref();

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });

  const address = server.address() as AddressInfo;
  return {
    baseUrl: `http://127.0.0.1:${String(address.port)}/v1`,
    modelId: `${CHILD_PROVIDER}/${grant.model}`,
    close: () =>
      new Promise<void>((resolve) => {
        server.close(() => resolve());
      }),
  };
}

/** Spend the grant on one child call and stream the answer back verbatim. */
async function forward(
  body: string,
  grant: ChildGrant,
  response: import("node:http").ServerResponse,
): Promise<void> {
  let payload: Record<string, unknown> = {};
  try {
    const parsed: unknown = JSON.parse(body === "" ? "{}" : body);
    if (typeof parsed === "object" && parsed !== null) payload = parsed as Record<string, unknown>;
  } catch {
    // An unparseable body is treated as an empty one, and the proxy's own
    // refusal is what the child then reports.
  }

  const messages = Array.isArray(payload["messages"]) ? payload["messages"] : [];
  const tools = payload["tools"];

  const upstream = await fetch(grant.proxyUrl, {
    method: "POST",
    headers: {
      authorization: `Bearer ${Redacted.value(grant.token)}`,
      "content-type": "application/json",
      accept: "text/event-stream, application/json",
    },
    body: JSON.stringify({
      // The grant pins the model, so whatever the child named is ignored here
      // rather than passed through and refused.
      model: grant.model,
      stream: true,
      messages: flattenForProxy(messages),
      ...(Array.isArray(tools) && tools.length > 0 ? { tools } : {}),
    }),
  }).catch((cause: unknown) => {
    response.writeHead(502, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        error: { message: `The inference proxy could not be reached: ${String(cause)}` },
      }),
    );
    return undefined;
  });

  if (upstream === undefined) return;

  if (!upstream.ok) {
    // The proxy answers a refusal as `{"error":{"code":"…"}}`, which an
    // OpenAI-compatible client reports as an unexplained server error because
    // it looks for `error.message`. Putting the status and the body in a
    // message is the difference between a child that says
    // `budget_exhausted` and three children that say `exited with code 1`.
    const detail = (await upstream.text().catch(() => "")).slice(0, 400);
    response.writeHead(upstream.status, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        error: {
          type: "openagents_proxy",
          message:
            `The OpenAgents inference proxy refused this call with HTTP ${String(upstream.status)}` +
            `${detail === "" ? "." : `: ${detail}`}`,
        },
      }),
    );
    return;
  }

  response.writeHead(upstream.status, {
    "content-type": upstream.headers.get("content-type") ?? "text/event-stream",
  });

  if (upstream.body === null) {
    response.end();
    return;
  }

  const reader = upstream.body.getReader();
  for (;;) {
    // The body is a stream and each read depends on the one before it.
    // eslint-disable-next-line no-await-in-loop
    const { done, value } = await reader.read();
    if (done) break;
    response.write(Buffer.from(value));
  }
  response.end();
}
