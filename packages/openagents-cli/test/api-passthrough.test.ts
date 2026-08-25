import { Effect, Option } from "effect";
import { describe, expect, it } from "vitest";

import {
  API_BASE_PATH,
  decodeRequestBody,
  parseRequestFields,
  parseRequestHeaders,
  resolveApiPath,
  resolveRequestMethod,
  successfulStatus,
} from "../src/api-passthrough.js";

const origin = "http://localhost:4000";

const failureText = async <A, E>(effect: Effect.Effect<A, E>): Promise<string> => {
  const exit = await Effect.runPromiseExit(effect);
  expect(exit._tag).toBe("Failure");
  return exit._tag === "Failure" ? String(exit.cause) : "";
};

describe("passthrough path resolution", () => {
  it("resolves a relative path under the API base", async () => {
    await expect(
      Effect.runPromise(resolveApiPath(origin, "repos/octavia/project/issues")),
    ).resolves.toBe(`${API_BASE_PATH}repos/octavia/project/issues`);
  });

  it("keeps an absolute API path unchanged", async () => {
    await expect(
      Effect.runPromise(resolveApiPath(origin, "/api/v1/repos/octavia/project/issues")),
    ).resolves.toBe("/api/v1/repos/octavia/project/issues");
  });

  it("preserves a query string in both forms", async () => {
    await expect(
      Effect.runPromise(resolveApiPath(origin, "repos/octavia/project/issues?state=closed")),
    ).resolves.toBe(`${API_BASE_PATH}repos/octavia/project/issues?state=closed`);
    await expect(
      Effect.runPromise(resolveApiPath(origin, "/api/v1/labels?per_page=5")),
    ).resolves.toBe("/api/v1/labels?per_page=5");
  });

  it("accepts a complete URL on the configured origin", async () => {
    await expect(Effect.runPromise(resolveApiPath(origin, `${origin}/api/v1/user`))).resolves.toBe(
      "/api/v1/user",
    );
  });

  it.each([
    "https://openagents.com/api/v1/user",
    "//openagents.com/api/v1/user",
    "http://127.0.0.1:5000/api/v1/user",
  ])("refuses %s because it leaves the configured origin", async (path) => {
    expect(await failureText(resolveApiPath(origin, path))).toContain("leaves the configured");
  });

  it("refuses an absolute path outside /api/ and names the relative form", async () => {
    const text = await failureText(resolveApiPath(origin, "/repos/octavia/project/issues"));
    expect(text).toContain("must start with /api/");
    expect(text).toContain("repos/octavia/project/issues");
  });

  it("refuses a relative path that climbs out of the API base", async () => {
    expect(await failureText(resolveApiPath(origin, "../../admin"))).toContain(
      `resolves outside ${API_BASE_PATH}`,
    );
  });

  it("refuses an empty path and a non-HTTP scheme", async () => {
    expect(await failureText(resolveApiPath(origin, "   "))).toContain("cannot be empty");
    expect(await failureText(resolveApiPath(origin, "file:///etc/passwd"))).toContain(
      "must use http or https",
    );
  });
});

describe("passthrough body fields", () => {
  it("collects fields as JSON strings", async () => {
    await expect(
      Effect.runPromise(parseRequestFields(["title=Fix the bug", "state=closed"])),
    ).resolves.toEqual({ title: "Fix the bug", state: "closed" });
  });

  it("keeps every character after the first equals sign", async () => {
    await expect(Effect.runPromise(parseRequestFields(["body=a=b=c"]))).resolves.toEqual({
      body: "a=b=c",
    });
  });

  it("returns an empty object for no fields", async () => {
    await expect(Effect.runPromise(parseRequestFields([]))).resolves.toEqual({});
  });

  it.each(["title", "=value"])("refuses the malformed field %s", async (field) => {
    expect(await failureText(parseRequestFields([field]))).toContain("--field key=value");
  });

  it("refuses a repeated field rather than dropping one silently", async () => {
    expect(await failureText(parseRequestFields(["title=a", "title=b"]))).toContain(
      "set more than once",
    );
  });
});

describe("passthrough headers", () => {
  it("parses and lowercases header names", async () => {
    await expect(
      Effect.runPromise(parseRequestHeaders(["X-Trace-Id: abc", "Accept:  text/plain "])),
    ).resolves.toEqual({ "x-trace-id": "abc", accept: "text/plain" });
  });

  it.each(["authorization: Bearer x", "Authorization: Bearer x", "AUTHORIZATION: Bearer x"])(
    "refuses %s so the session token cannot be replaced",
    async (header) => {
      expect(await failureText(parseRequestHeaders([header]))).toContain(
        "authorization header from your OpenAgents session",
      );
    },
  );

  it("refuses a header without a colon or with an invalid name", async () => {
    expect(await failureText(parseRequestHeaders(["X-Trace-Id abc"]))).toContain(
      "--header 'Name: value'",
    );
    expect(await failureText(parseRequestHeaders(["Bad Name: value"]))).toContain(
      "Invalid header name",
    );
  });
});

describe("passthrough method and body decoding", () => {
  it("defaults to GET without a body and POST with one", () => {
    expect(resolveRequestMethod(Option.none(), false)).toBe("GET");
    expect(resolveRequestMethod(Option.none(), true)).toBe("POST");
  });

  it("always honors an explicit method", () => {
    expect(resolveRequestMethod(Option.some("DELETE"), false)).toBe("DELETE");
    expect(resolveRequestMethod(Option.some("GET"), true)).toBe("GET");
  });

  it("decodes JSON and reports the source of an invalid body", async () => {
    await expect(
      Effect.runPromise(decodeRequestBody('{"labels":["bug"]}', "Standard input")),
    ).resolves.toEqual({ labels: ["bug"] });
    expect(await failureText(decodeRequestBody("not json", "Standard input"))).toContain(
      "Standard input did not contain valid JSON",
    );
    expect(await failureText(decodeRequestBody("  ", "The file body.json"))).toContain(
      "The file body.json contained no JSON body",
    );
  });

  it("treats only 2xx as success", () => {
    expect(successfulStatus(200)).toBe(true);
    expect(successfulStatus(204)).toBe(true);
    expect(successfulStatus(301)).toBe(false);
    expect(successfulStatus(404)).toBe(false);
  });
});
