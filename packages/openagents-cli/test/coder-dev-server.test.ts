import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

import { devServerReady, findSiteCheckout, startDevServer } from "../src/coder-dev-server.js";

/** A directory tree with a Phoenix `mix.exs` at its root. */
const siteCheckout = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-site-"));
  writeFileSync(
    join(root, "mix.exs"),
    'def project do\n  [app: :openagents, version: "0.1.0"]\nend\n',
  );
  mkdirSync(join(root, "lib", "openagents_web"), { recursive: true });
  return root;
};

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("finding the checkout to start a server from", () => {
  it("takes the checkout the session is already inside, from any depth", () => {
    const root = siteCheckout();
    expect(findSiteCheckout(join(root, "lib", "openagents_web"), {})).toBe(root);
  });

  it("prefers an explicitly named checkout over the search", () => {
    const named = siteCheckout();
    const inside = siteCheckout();
    expect(findSiteCheckout(inside, { OPENAGENTS_COM_PATH: named })).toBe(named);
  });

  it("ignores a named path that is not the application", () => {
    const root = siteCheckout();
    // Falls through to the search rather than starting `mix` somewhere that
    // will fail a minute later and less clearly.
    expect(
      findSiteCheckout(root, { OPENAGENTS_COM_PATH: mkdtempSync(join(tmpdir(), "oa-not-")) }),
    ).toBe(root);
  });

  it("does not take a Mix project that is some other application", () => {
    const other = mkdtempSync(join(tmpdir(), "oa-other-"));
    writeFileSync(join(other, "mix.exs"), "def project do\n  [app: :something_else]\nend\n");
    // Not this one: the fallback may still find the real checkout on the
    // machine, but it must not have chosen the wrong Mix project.
    expect(findSiteCheckout(other, {})).not.toBe(other);
  });
});

describe("readiness", () => {
  const stub = (answer: () => Response | Promise<Response>) =>
    vi.stubGlobal("fetch", vi.fn(answer));

  it("counts a serving health endpoint as ready", async () => {
    stub(() => new Response(JSON.stringify({ status: "ok", revision: "image" }), { status: 200 }));
    expect(await devServerReady("http://localhost:4000")).toBe(true);
  });

  it("does not count a server whose database is behind as ready", async () => {
    // Phoenix answers this as its own debug page, so it is a live server that
    // cannot serve yet — neither ready nor absent.
    stub(
      () =>
        new Response("<html>Phoenix.Ecto.PendingMigrationError at GET /healthz</html>", {
          status: 500,
        }),
    );
    expect(await devServerReady("http://localhost:4000")).toBe(false);
  });

  it("does not count an unreachable server as ready", async () => {
    stub(() => {
      throw new Error("ECONNREFUSED");
    });
    expect(await devServerReady("http://localhost:4000")).toBe(false);
  });
});

describe("starting one", () => {
  it("starts nothing when a server is already serving", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(() => new Response(JSON.stringify({ status: "ok" }), { status: 200 })),
    );
    const said: string[] = [];

    const result = await startDevServer("http://localhost:4000", {
      notice: (message) => said.push(message),
    });

    expect(result).toEqual({ started: false });
    // Nothing announced, because nothing happened: the warm path is the common
    // one and should be silent.
    expect(said).toEqual([]);
  });
});
