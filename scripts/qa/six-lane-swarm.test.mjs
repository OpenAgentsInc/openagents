import assert from "node:assert/strict";
import test from "node:test";

import { LANE_IDS, evaluateProbe, validateRegistry } from "./six-lane-swarm.mjs";

const lane = (id) => ({ id, surface: id, command: "true", probes: [] });

test("registry requires every QA lane exactly once", () => {
  const registry = { schema: "openagents.qa.six-lane-registry.v1", lanes: LANE_IDS.map(lane) };
  assert.equal(validateRegistry(registry), registry);
  assert.throws(
    () => validateRegistry({ ...registry, lanes: registry.lanes.slice(1) }),
    /exactly once/,
  );
  assert.throws(
    () => validateRegistry({ ...registry, lanes: [...registry.lanes, lane(LANE_IDS[0])] }),
    /exactly once/,
  );
});

test("probe evaluation reports status, media, body, and JSON contract drift", () => {
  const failures = evaluateProbe(
    {
      status: 200,
      contentType: "application/json",
      bodyIncludes: "ready",
      jsonAssertions: [
        { path: "openapi", equals: "3.1.0" },
        { path: "paths./api/public/product-promises.get", present: true },
      ],
    },
    {
      status: 404,
      contentType: "text/plain",
      body: JSON.stringify({ openapi: "3.0.0", paths: {} }),
    },
  );
  assert.deepEqual(failures, [
    "expected status 200, got 404",
    "expected content-type containing application/json, got text/plain",
    'body did not include "ready"',
    'openapi expected "3.1.0", got "3.0.0"',
    "paths./api/public/product-promises.get was absent",
  ]);
});

test("probe evaluation accepts the advertised OpenAPI path shape", () => {
  const body = JSON.stringify({
    openapi: "3.1.0",
    paths: { "/api/public/product-promises": { get: {} } },
  });
  assert.deepEqual(
    evaluateProbe(
      {
        status: 200,
        contentType: "application/json",
        jsonAssertions: [{ path: "paths./api/public/product-promises.get", present: true }],
      },
      { status: 200, contentType: "application/json; charset=utf-8", body },
    ),
    [],
  );
});
