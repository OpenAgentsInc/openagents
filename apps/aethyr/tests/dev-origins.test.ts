import { test } from "node:test";
import assert from "node:assert/strict";
import { devOrigins } from "../tools/dev-origins.mjs";

test("overview has its own exact loopback pair; existing deployments keep defaults", () => {
  for (const profile of [undefined, "medieval-town", "haunted-house"]) {
    const result = devOrigins({ VITE_WORLD_PROFILE: profile });
    assert.equal(result.editorPort, 55304);
    assert.equal(result.previewPort, 55305);
  }
  const overview = devOrigins({ VITE_WORLD_PROFILE: "medieval-overview" });
  assert.equal(overview.editorOrigin, "http://127.0.0.1:55306");
  assert.equal(overview.previewOrigin, "http://127.0.0.1:55307");
  assert.equal(
    devOrigins({
      AETHYRFORGE_EDITOR_PORT: "55400",
      AETHYRFORGE_PREVIEW_PORT: "55401",
    }).previewPort,
    55401,
  );
});

test("open ground runs alongside town on an independent loopback pair", () => {
  const ground = devOrigins({ VITE_WORLD_PROFILE: "open-ground" });
  assert.equal(ground.editorOrigin, "http://127.0.0.1:55316");
  assert.equal(ground.previewOrigin, "http://127.0.0.1:55317");
});

test("dev origins reject shared ports, arbitrary hosts and malformed/out-of-range ports", () => {
  for (const value of [
    "80",
    "65536",
    "55306.0",
    "https://untrusted.example",
    "55306/path",
    "",
    " 55306",
    "-55306",
  ]) {
    assert.throws(() => devOrigins({ AETHYRFORGE_EDITOR_PORT: value }));
  }
  assert.throws(() =>
    devOrigins({ VITE_WORLD_PROFILE: "https://untrusted.example/world" }),
  );
  assert.throws(() =>
    devOrigins({
      AETHYRFORGE_EDITOR_PORT: "55307",
      AETHYRFORGE_PREVIEW_PORT: "55307",
    }),
  );
});
