import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  STATIC_BLUEPRINT_CONTRACT_EXPORT,
  STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
  loadBlueprintSignatureRegistry,
  makeBlueprintSignatureRegistryClient,
} from "../src";

describe("Blueprint signature registry client", () => {
  test("loads the static fixture as a normalized registry view", async () => {
    const view = await Effect.runPromise(loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }));

    expect(view.sourceKind).toBe("staticFixture");
    expect(view.registryVersionRef).toBe(STATIC_BLUEPRINT_REGISTRY_VERSION_REF);
    expect(view.safeProjectionPolicyRef).toBe(STATIC_BLUEPRINT_PROGRAM_REGISTRY.policyRef);
    expect(view.contractExportVersionRef).toBe(STATIC_BLUEPRINT_CONTRACT_EXPORT.versionRef);
    expect(view.registry.entries.length).toBeGreaterThan(0);
  });

  test("loads an assignment-inline registry slice without bypassing validation", async () => {
    const inlineRegistry = {
      ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
      entries: [STATIC_BLUEPRINT_PROGRAM_REGISTRY.entries[0]],
      programSignatures: [STATIC_BLUEPRINT_PROGRAM_REGISTRY.programSignatures[0]],
      programTypes: [STATIC_BLUEPRINT_PROGRAM_REGISTRY.programTypes[0]],
    };
    const client = makeBlueprintSignatureRegistryClient();
    const view = await Effect.runPromise(
      client.loadRegistry({
        assignment: {
          blueprintContractExport: STATIC_BLUEPRINT_CONTRACT_EXPORT,
          blueprintRegistry: inlineRegistry,
          blueprintRegistryVersionRef: "blueprint_registry.assignment_inline.test.v1",
        },
        sourceKind: "assignmentInline",
      }),
    );

    expect(view.sourceKind).toBe("assignmentInline");
    expect(view.registryVersionRef).toBe("blueprint_registry.assignment_inline.test.v1");
    expect(view.registry.entries).toHaveLength(1);
    expect(view.safeProjectionPolicyRef).toBe(STATIC_BLUEPRINT_PROGRAM_REGISTRY.policyRef);
  });

  test("rejects a missing assignment-inline registry", async () => {
    await expect(
      Effect.runPromise(
        loadBlueprintSignatureRegistry({
          assignment: {},
          sourceKind: "assignmentInline",
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintRegistryClientError",
      sourceKind: "assignmentInline",
    });
  });

  test("calls future Omega Blueprint HTTP routes and validates responses", async () => {
    const seen: string[] = [];
    const view = await Effect.runPromise(
      loadBlueprintSignatureRegistry({
        baseUrl: "https://omega.test",
        fetch: async (input) => {
          const url = new URL(String(input));
          seen.push(url.pathname);

          if (url.pathname === "/api/blueprint/program-registry") {
            return Response.json(STATIC_BLUEPRINT_PROGRAM_REGISTRY, {
              headers: {
                "x-blueprint-registry-version-ref": "blueprint_registry.omega_http.test.v1",
              },
            });
          }

          if (url.pathname === "/api/blueprint/contracts") {
            return Response.json(STATIC_BLUEPRINT_CONTRACT_EXPORT);
          }

          return new Response("not found", { status: 404 });
        },
        sourceKind: "omegaHttp",
      }),
    );

    expect(seen).toEqual(["/api/blueprint/program-registry", "/api/blueprint/contracts"]);
    expect(view.sourceKind).toBe("omegaHttp");
    expect(view.registryVersionRef).toBe("blueprint_registry.omega_http.test.v1");
    expect(view.contractExportVersionRef).toBe(STATIC_BLUEPRINT_CONTRACT_EXPORT.versionRef);
  });

  test("returns a typed error for malformed Omega registry responses", async () => {
    await expect(
      Effect.runPromise(
        loadBlueprintSignatureRegistry({
          baseUrl: "https://omega.test",
          fetch: async (input) => {
            const url = new URL(String(input));
            return url.pathname === "/api/blueprint/program-registry"
              ? Response.json({ safeProjection: true })
              : Response.json(STATIC_BLUEPRINT_CONTRACT_EXPORT);
          },
          sourceKind: "omegaHttp",
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintRegistryClientError",
      sourceKind: "omegaHttp",
    });
  });

  test("returns a typed error for unsafe Omega registry projections", async () => {
    await expect(
      Effect.runPromise(
        loadBlueprintSignatureRegistry({
          baseUrl: "https://omega.test",
          fetch: async (input) => {
            const url = new URL(String(input));
            return url.pathname === "/api/blueprint/program-registry"
              ? Response.json({
                  ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
                  safeProjection: false,
                })
              : Response.json(STATIC_BLUEPRINT_CONTRACT_EXPORT);
          },
          sourceKind: "omegaHttp",
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintProjectionUnsafe",
    });
  });

  test("returns a typed error when future Omega routes are unavailable", async () => {
    await expect(
      Effect.runPromise(
        loadBlueprintSignatureRegistry({
          baseUrl: "https://omega.test",
          fetch: async () => new Response("not found", { status: 404 }),
          sourceKind: "omegaHttp",
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintRegistryClientError",
      sourceKind: "omegaHttp",
      statusCode: 404,
    });
  });
});
