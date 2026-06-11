import { describe, expect, test } from "vitest";

import {
  analyzeInventory,
  buildInventory,
  normalizeAllowlist,
  openApiSchemaDescriptions,
} from "./check-public-projection-freshness.mjs";

const inventory = [
  {
    operationId: "getExample",
    schema: "ExampleProjection",
    sourceKind: "fixture",
    surface: "/api/public/example -> ExampleProjection",
  },
];

describe("public projection freshness check", () => {
  test("fails a public projection schema that lacks the staleness contract", () => {
    const analysis = analyzeInventory({
      allowlist: [],
      inventory,
      schemaSources: new Map([
        [
          "ExampleProjection",
          "export const ExampleProjection = S.Struct({ generatedAt: S.String })",
        ],
      ]),
    });

    expect(analysis.problems).toEqual([
      "/api/public/example -> ExampleProjection is missing maxStalenessSeconds/staleness contract; add fields or allowlist it with an issue ref",
    ]);
    expect(analysis.results[0]).toMatchObject({
      hasTimestamp: true,
      hasStaleness: false,
      missing: ["maxStalenessSeconds/staleness contract"],
    });
  });

  test("passes a compliant projection with generatedAt and maxStalenessSeconds", () => {
    const analysis = analyzeInventory({
      allowlist: [],
      inventory,
      schemaSources: new Map([
        [
          "ExampleProjection",
          [
            "export const ExampleProjection = S.Struct({",
            "  generatedAt: S.String,",
            "  staleness: S.Struct({ maxStalenessSeconds: S.Int }),",
            "})",
          ].join("\n"),
        ],
      ]),
    });

    expect(analysis.problems).toEqual([]);
    expect(analysis.results[0]).toMatchObject({
      allowlisted: false,
      hasTimestamp: true,
      hasStaleness: true,
      missing: [],
    });
  });

  test("allows a grandfathered projection only when the allowlist carries an issue ref", () => {
    const analysis = analyzeInventory({
      allowlist: [
        {
          surface: "/api/public/example -> ExampleProjection",
          issueRef: "OpenAgentsInc/openagents#4751",
          reason:
            "Grandfathered public projection lacks explicit freshness fields.",
        },
      ],
      inventory,
      schemaSources: new Map([
        ["ExampleProjection", "export const ExampleProjection = S.Struct({})"],
      ]),
    });

    expect(analysis.problems).toEqual([]);
    expect(analysis.results[0]).toMatchObject({
      allowlisted: true,
      missing: [
        "generatedAt/lastRebuiltAt",
        "maxStalenessSeconds/staleness contract",
      ],
    });
  });

  test("rejects allowlist entries without issue references", () => {
    expect(
      normalizeAllowlist([
        {
          surface: "/api/public/example -> ExampleProjection",
          issueRef: "projection-staleness",
          reason:
            "Grandfathered public projection lacks explicit freshness fields.",
        },
      ]).problems,
    ).toEqual([
      "allowlist[0] /api/public/example -> ExampleProjection must carry an OpenAgents issue ref",
    ]);
  });

  test("builds inventory from public OpenAPI routes and computed endpoint constants", () => {
    const openApiSource = `
      const paths = () => ({
        '/api/public/plain': {
          get: operation({
            operationId: 'getPlain',
            responses: { '200': okJson('Plain.', '#/components/schemas/PlainProjection') },
          }),
        },
        [PublicLaunchDashboardEndpoint]: {
          get: operation({
            operationId: 'getLaunchDashboard',
            responses: { '200': okJson('Dashboard.', '#/components/schemas/PublicLaunchDashboard') },
          }),
        },
        '/api/forum/work-requests': {
          get: operation({
            operationId: 'listForumWorkRequests',
            responses: { '200': okJson('Work requests.', '#/components/schemas/ForumWorkRequestListResponse') },
          }),
        },
        '/api/forum/moderation/queue': {
          get: operation({
            operationId: 'listForumModerationQueue',
            responses: { '200': okJson('Moderation queue.', '#/components/schemas/ForumModerationQueueResponse') },
          }),
        },
      })
    `;
    const inventory = buildInventory({
      openApiSource,
      sourceRoot: new URL("../workers/api/src", import.meta.url).pathname,
    });

    expect(inventory).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          operationId: "getPlain",
          schema: "PlainProjection",
          surface: "/api/public/plain -> PlainProjection",
        }),
        expect.objectContaining({
          operationId: "getLaunchDashboard",
          schema: "PublicLaunchDashboard",
          surface: "/api/public/launch-dashboard -> PublicLaunchDashboard",
        }),
        expect.objectContaining({
          operationId: "listForumWorkRequests",
          schema: "ForumWorkRequestListResponse",
          surface:
            "/api/forum/work-requests -> ForumWorkRequestListResponse",
        }),
      ]),
    );
    expect(inventory).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ schema: "ForumModerationQueueResponse" }),
      ]),
    );
  });

  test("uses OpenAPI schema descriptions as schema evidence", () => {
    const descriptions = openApiSchemaDescriptions(`
      const schemaComponents = () => ({
        ExampleProjection: objectSummary(
          'Public projection with generatedAt and maxStalenessSeconds.',
        ),
      })
    `);

    const analysis = analyzeInventory({
      allowlist: [],
      inventory,
      schemaSources: descriptions,
    });

    expect(analysis.problems).toEqual([]);
  });
});
