import { describe, expect, it } from "vitest";

import {
  backendIds,
  chooseBackend,
  CODER_BACKENDS,
  fetchServedCatalog,
  refuseBackend,
} from "../src/coder-backends.js";

/**
 * The list two surfaces read.
 *
 * `--model` takes its accepted values from it and the status line takes its
 * label from it. These pin the properties those two depend on, so adding a
 * backend stays one entry rather than one entry plus two fixes.
 */
describe("coder backends", () => {
  it("names each backend once", () => {
    expect(backendIds()).toEqual([...new Set(backendIds())]);
    expect(CODER_BACKENDS.length).toBeGreaterThan(1);
  });

  it("gives every backend a label a status line can show", () => {
    for (const backend of CODER_BACKENDS) {
      expect(backend.label).not.toBe("");
      expect(backend.id).not.toBe("");
    }
  });

  it("publishes ids the chat API's own enum lists", () => {
    // These are the values `POST /api/v3/chat/turns` accepts as `model`, so a
    // change here without the matching server change is a refusal at runtime.
    expect(backendIds()).toEqual(["ox-alpha", "gemini-3.7-flash"]);
  });
});

describe("choosing a backend from what the server serves", () => {
  const model = (id: string, available: boolean, isDefault = false) => ({
    id,
    available,
    isDefault,
  });

  it("leads with the preferred backend where the server serves it", () => {
    const chosen = chooseBackend([
      model("gpt-5.6-luna", true, true),
      model("gemini-3.7-flash", true),
    ]);
    expect(chosen?.id).toBe("gemini-3.7-flash");
  });

  it("falls to the server's own default when the preference is not served", () => {
    // The case that sent every session into a 422: no deployment served a model
    // by that id, and the client named it anyway.
    const chosen = chooseBackend([model("gpt-5.6-luna", true, true), model("ox-alpha", false)]);
    expect(chosen?.id).toBe("gpt-5.6-luna");
  });

  it("falls past an unavailable default to something that can answer", () => {
    const chosen = chooseBackend([model("gpt-5.6-luna", false, true), model("ox-alpha", true)]);
    expect(chosen?.id).toBe("ox-alpha");
  });

  it("honours an explicitly named model over the preference", () => {
    const chosen = chooseBackend(
      [model("gpt-5.6-luna", true, true), model("gemini-3.7-flash", true)],
      "gpt-5.6-luna",
    );
    expect(chosen?.id).toBe("gpt-5.6-luna");
  });

  it("chooses nothing when no model has a configured credential", () => {
    expect(chooseBackend([model("gpt-5.6-luna", false, true), model("ox-alpha", false)])).toBe(
      undefined,
    );
  });
});

describe("refusing a named backend", () => {
  const served = [
    { id: "gpt-5.6-luna", available: true, isDefault: true },
    { id: "ox-alpha", available: false, isDefault: false },
  ];

  it("says a model is not served here and names what is", () => {
    const refusal = refuseBackend(served, "gemini-3.7-flash");
    expect(refusal).toContain("gemini-3.7-flash");
    expect(refusal).toContain("gpt-5.6-luna");
  });

  it("separates a missing credential from a missing model", () => {
    const refusal = refuseBackend(served, "ox-alpha");
    expect(refusal).toContain("credential is not configured");
  });

  it("does not refuse a model the server serves and can run", () => {
    expect(refuseBackend(served, "gpt-5.6-luna")).toBe(undefined);
  });

  it("says so plainly when the deployment can run nothing", () => {
    const refusal = refuseBackend([{ id: "ox-alpha", available: false, isDefault: true }], "ox-alpha");
    expect(refusal).toContain("no model with a configured credential");
  });
});

describe("reading the published catalog", () => {
  const withFetch = async (
    handler: (url: string) => Response | Promise<Response>,
    run: () => Promise<unknown>,
  ) => {
    const original = globalThis.fetch;
    globalThis.fetch = ((input: URL | RequestInfo) =>
      Promise.resolve(handler(String(input)))) as typeof fetch;
    try {
      return await run();
    } finally {
      globalThis.fetch = original;
    }
  };

  const api = { origin: "http://localhost:4000", token: "t" };

  it("reads ids and availability from the server's own shape", async () => {
    const served = await withFetch(
      (url) => {
        expect(url).toBe("http://localhost:4000/api/v3/models");
        return new Response(
          JSON.stringify({
            default: "gpt-5.6-luna",
            models: [
              { id: "gpt-5.6-luna", availability: "available", default: true },
              { id: "ox-alpha", availability: "unavailable", default: false },
            ],
          }),
          { status: 200 },
        );
      },
      () => fetchServedCatalog(api),
    );

    expect(served).toEqual([
      { id: "gpt-5.6-luna", available: true, isDefault: true },
      { id: "ox-alpha", available: false, isDefault: false },
    ]);
  });

  it("treats an availability word it has never seen as not available", async () => {
    const served = (await withFetch(
      () =>
        new Response(JSON.stringify({ models: [{ id: "new", availability: "degraded" }] }), {
          status: 200,
        }),
      () => fetchServedCatalog(api),
    )) as readonly { available: boolean }[];

    expect(served[0]?.available).toBe(false);
  });

  it("cannot answer for a server that refuses the route, rather than reporting an empty catalog", async () => {
    // `undefined` is "could not ask", which falls back to the static list. An
    // empty list would mean "serves nothing" and would stop the session.
    for (const status of [401, 404, 500]) {
      const served = await withFetch(
        () => new Response("", { status }),
        () => fetchServedCatalog(api),
      );
      expect(served).toBe(undefined);
    }
  });

  it("cannot answer for an unreachable server", async () => {
    const served = await withFetch(
      () => {
        throw new Error("ECONNREFUSED");
      },
      () => fetchServedCatalog(api),
    );
    expect(served).toBe(undefined);
  });
});
