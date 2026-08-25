import { describe, expect, it } from "vitest";

import { rememberTool, type RememberTransport } from "../src/coder-remember.js";

interface Sent {
  readonly url: string;
  readonly method: string;
  readonly headers: Readonly<Record<string, string>>;
  readonly body: unknown;
}

/**
 * A faked transport, recording what the tool sent and answering what the
 * server would. Nothing here touches the network or the disk, which is the
 * point: the tool must reach the API and must never write locally.
 */
const transport = (reply: (sent: Sent) => { readonly status: number; readonly body: unknown }) => {
  const sent: Array<Sent> = [];
  const send: RememberTransport = (url, init) => {
    const call: Sent = {
      url: url.toString(),
      method: init.method,
      headers: init.headers,
      body: JSON.parse(init.body) as unknown,
    };
    sent.push(call);
    const answer = reply(call);
    return Promise.resolve({
      status: answer.status,
      json: () => Promise.resolve(answer.body),
    });
  };
  return { sent, send };
};

const signal = new AbortController().signal;

/** A transport that cannot reach the server at all. */
const unreachable: RememberTransport = () => Promise.reject(new Error("ECONNREFUSED"));

describe("the remember tool", () => {
  it("posts to the memories API rather than writing a local ledger", async () => {
    const { send, sent } = transport(() => ({
      status: 201,
      body: {
        memory: {
          id: "mem_42",
          bucket: "user",
          body: "Uses pnpm, not npm.",
          source_ref: "thread-7",
          superseded_by: null,
          created_at: "2026-08-25T12:00:00Z",
        },
      },
    }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      sourceRef: "thread-7",
      fetch: send,
    });
    const answer = await tool.run({ body: "Uses pnpm, not npm." }, signal);

    expect(sent.length).toBe(1);
    expect(sent[0]!.url).toBe("https://openagents.com/api/v1/memories");
    expect(sent[0]!.method).toBe("POST");
    expect(sent[0]!.headers["authorization"]).toBe("Bearer account-token");
    // The bucket is `user` because the reader asked. Nothing this tool writes
    // is inferred, so nothing it writes is `learned`.
    expect(sent[0]!.body).toMatchObject({
      body: "Uses pnpm, not npm.",
      bucket: "user",
      source_ref: "thread-7",
    });
    expect(answer).toContain("Stored");
    expect(answer).toContain("mem_42");
  });

  it("sends a correction as a new memory carrying supersedes", async () => {
    const { send, sent } = transport(() => ({
      status: 201,
      body: { memory: { id: "mem_43", bucket: "user", body: "Uses bun." } },
    }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      fetch: send,
    });
    const answer = await tool.run({ body: "Uses bun.", supersedes: "mem_42" }, signal);

    expect(sent[0]!.body).toMatchObject({ supersedes: "mem_42" });
    expect(answer).toContain("supersedes mem_42");
  });

  it("omits the source reference on a lane that keeps no server record", async () => {
    const { send, sent } = transport(() => ({
      status: 201,
      body: { memory: { id: "mem_44" } },
    }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      fetch: send,
    });
    await tool.run({ body: "Prefers tabs." }, signal);

    expect(Object.keys(sent[0]!.body as Record<string, unknown>)).not.toContain("source_ref");
  });

  // Every failing path has to come back as a refusal the model can repeat. A
  // silent success would tell the reader their preference is stored when it is
  // not, which is the one outcome worse than not storing it.
  it("refuses honestly when the session holds no credential", async () => {
    const { send, sent } = transport(() => ({ status: 201, body: {} }));

    const tool = rememberTool({ origin: "https://openagents.com", fetch: send });
    const answer = await tool.run({ body: "Uses pnpm." }, signal);

    expect(sent.length).toBe(0);
    expect(answer).toContain("Refusal");
    expect(answer).toContain("openagents auth login");
  });

  it("refuses honestly when the server rejects the credential", async () => {
    const { send } = transport(() => ({
      status: 401,
      body: {
        code: "unauthenticated",
        message: "Requires an API token with the scope this route needs",
        errors: {},
      },
    }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "stale-token",
      fetch: send,
    });
    const answer = await tool.run({ body: "Uses pnpm." }, signal);

    expect(answer).toContain("Refusal");
    expect(answer).toContain("nothing was stored");
    expect(answer).toContain("openagents auth login");
  });

  it("surfaces the quota refusal with the reason the server gave", async () => {
    const { send } = transport(() => ({
      status: 429,
      body: {
        code: "memory_quota_reached",
        message:
          "This account already holds 200 memories. Remove one, or supersede one, before writing another.",
        errors: {},
      },
    }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      fetch: send,
    });
    const answer = await tool.run({ body: "One more thing." }, signal);

    expect(answer).toContain("Refusal");
    expect(answer).toContain("already holds 200 memories");
    expect(answer).toContain("supersede one");
  });

  it("names the rejected field on a validation refusal", async () => {
    const { send } = transport(() => ({
      status: 422,
      body: {
        code: "validation_failed",
        message: "The request could not be processed",
        errors: { supersedes: ["names no live memory of this account"] },
      },
    }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      fetch: send,
    });
    const answer = await tool.run({ body: "Something.", supersedes: "mem_absent" }, signal);

    expect(answer).toContain("Refusal");
    expect(answer).toContain("supersedes: names no live memory of this account");
  });

  it("refuses honestly when the API cannot be reached", async () => {
    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      fetch: unreachable,
    });
    const answer = await tool.run({ body: "Uses pnpm." }, signal);

    expect(answer).toContain("Refusal");
    expect(answer).toContain("could not be reached");
  });

  it("refuses an empty body without spending a request", async () => {
    const { send, sent } = transport(() => ({ status: 201, body: {} }));

    const tool = rememberTool({
      origin: "https://openagents.com",
      token: "account-token",
      fetch: send,
    });
    const answer = await tool.run({ body: "   " }, signal);

    expect(sent.length).toBe(0);
    expect(answer).toContain("`body` is required");
  });

  // Recall is server-side inside `POST /api/v1/responses`, so the declaration
  // carries a write and nothing else. A read tool here would be the model
  // spending a round on context the server already attached.
  it("declares one write action and no read", async () => {
    const tool = rememberTool({ origin: "https://openagents.com", token: "t" });

    expect(tool.name).toBe("remember");
    expect(tool.description).toContain("Explicit requests only");
    expect(tool.description).toContain("There is no matching read");
    const properties = (tool.parameters as { properties: Record<string, unknown> }).properties;
    expect(Object.keys(properties)).toEqual(["body", "supersedes"]);
  });
});
