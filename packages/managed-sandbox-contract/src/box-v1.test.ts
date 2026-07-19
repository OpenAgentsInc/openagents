import { BoxApi, Configuration, ResponseError } from "@asciidev/box-sdk";
import { describe, expect, it } from "vite-plus/test";

import {
  BOX_CAPABILITY_NOT_IMPLEMENTED_CODE,
  BOX_V1_PHASE1_OPERATIONS,
  BOX_V1_UNSUPPORTED_SDK_METHODS,
  capabilityNotImplemented,
} from "./box-v1.ts";
import {
  BOX_CONFORMANCE_LOCKFILE_SHA256,
  BOX_OPENAPI_PROVENANCE,
  BOX_SDK_PROVENANCE,
} from "./provenance.ts";

type RequestOptionsApi = Record<
  string,
  (...args: ReadonlyArray<unknown>) => Promise<{
    path: string;
    method: string;
    query?: Record<string, unknown>;
  }>
>;

const params: Record<string, ReadonlyArray<unknown>> = {
  me: [],
  limits: [],
  boxes: [{}],
  create: [{}],
  get: [{ boxId: "box_test" }],
  update: [{ boxId: "box_test", updateBoxRequest: {} }],
  remove: [{ boxId: "box_test" }],
  stop: [{ boxId: "box_test" }],
  resume: [{ boxId: "box_test" }],
  prompt: [
    {
      boxId: "box_test",
      promptRequest: { provider: "openai", model: "test", prompt: "test" },
    },
  ],
  promptRunStatus: [{ boxId: "box_test", promptId: "prompt_test" }],
  events: [{ boxId: "box_test" }],
  interrupt: [{ boxId: "box_test" }],
  readFile: [{ boxId: "box_test", path: "/workspace/README.md" }],
  writeFile: [
    {
      boxId: "box_test",
      fileWriteRequest: { path: "/workspace/README.md", content: "test" },
    },
  ],
  command: [{ boxId: "box_test", commandRequest: { command: "pwd" } }],
  artifact: [{ boxId: "box_test", path: "/tmp/result.txt" }],
};

describe("Box-v1 compatibility freeze", () => {
  it("pins an auditable SDK and OpenAPI provenance record", () => {
    expect(BOX_SDK_PROVENANCE).toEqual({
      package: "@asciidev/box-sdk",
      version: "0.0.24",
      license: "MIT",
      integrity:
        "sha512-w77vTWA+yrJ5O+FmchCkurjux1UZkQ5yeurnzX/FJTlQulEtj1xp0g/2cSh/GZWLXrgCV0exU99E+NyiilBeHA==",
      shasum: "eb55554ffb5b231888a70e51857f8de336735ac1",
      tarballSha256: "51ac532981c4791ab8662d800cd70b6f18d9a8a01abbd097c627bae3ae45aeb0",
      tarballBytes: 104618,
      licenseSha256: "b7d51a8c93c3b34b607bdb4e15b547e4c7618cf21321c608689b44634f3e3183",
    });
    expect(BOX_OPENAPI_PROVENANCE.sha256).toHaveLength(64);
    expect(BOX_CONFORMANCE_LOCKFILE_SHA256).toHaveLength(64);
  });

  it("matches every admitted route to the unmodified generated SDK request options", async () => {
    const api = new BoxApi(
      new Configuration({ basePath: "https://facade.test/v1" }),
    ) as unknown as RequestOptionsApi;

    await Promise.all(
      BOX_V1_PHASE1_OPERATIONS.map(async (operation) => {
        const requestOptions = await api[`${operation.sdkMethod}RequestOpts`](
          ...(params[operation.sdkMethod] ?? []),
        );
        const expectedPath = operation.path
          .replace("/v1", "")
          .replaceAll("{id}", "box_test")
          .replace("{promptId}", "prompt_test")
          .split("?")[0];
        expect(requestOptions.method).toBe(operation.method);
        expect(requestOptions.path).toBe(expectedPath);
      }),
    );
  });

  it("uses the configured facade base path and bearer token without SDK modification", async () => {
    const requests: Array<{ url: string; authorization: string | null }> = [];
    const fakeFetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const url = String(input);
      const headers = new Headers(init?.headers);
      requests.push({ url, authorization: headers.get("authorization") });
      return new Response(
        JSON.stringify({
          ok: true,
          type: "user.info",
          user: { login: "openagents-conformance", email: null },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    };
    const api = new BoxApi(
      new Configuration({
        basePath: "https://facade.test/v1",
        accessToken: "test-token",
        fetchApi: fakeFetch as typeof fetch,
      }),
    );

    const response = await api.me();
    expect(response.user.login).toBe("openagents-conformance");
    expect(requests).toEqual([
      {
        url: "https://facade.test/v1/me",
        authorization: "Bearer test-token",
      },
    ]);
  });

  it("makes all non-profile SDK methods explicit and returns structured 501", async () => {
    const sdkMethods = Object.getOwnPropertyNames(BoxApi.prototype).filter(
      (name) => name !== "constructor" && !name.endsWith("Raw") && !name.endsWith("RequestOpts"),
    );
    expect(
      new Set([
        ...BOX_V1_PHASE1_OPERATIONS.map((operation) => operation.sdkMethod),
        ...BOX_V1_UNSUPPORTED_SDK_METHODS,
      ]),
    ).toEqual(new Set(sdkMethods));

    for (const method of BOX_V1_UNSUPPORTED_SDK_METHODS) {
      const error = capabilityNotImplemented(method);
      expect(error.status).toBe(501);
      expect(error.code).toBe(BOX_CAPABILITY_NOT_IMPLEMENTED_CODE);
      expect(error.error.details).toEqual(expect.objectContaining({ sdkMethod: method }));
    }

    const api = new BoxApi(
      new Configuration({
        basePath: "https://facade.test/v1",
        fetchApi: (async () =>
          new Response(JSON.stringify(capabilityNotImplemented("fork")), {
            status: 501,
            headers: { "content-type": "application/json" },
          })) as typeof fetch,
      }),
    );
    const caught = await api
      .fork({ boxId: "box_test" })
      .then(() => undefined)
      .catch((error: unknown) => error);
    expect(caught).toBeInstanceOf(ResponseError);
    expect((caught as ResponseError).response.status).toBe(501);
    expect(await (caught as ResponseError).response.json()).toEqual(
      expect.objectContaining({ code: BOX_CAPABILITY_NOT_IMPLEMENTED_CODE }),
    );
  });
});
