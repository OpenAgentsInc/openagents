import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";
import { Schema } from "effect";
import * as BundledMeta from "./_generated/bundled-0.144.1/meta.gen.ts";
import * as BundledSchema from "./_generated/bundled-0.144.1/schema.gen.ts";
import * as CurrentMeta from "./_generated/current-source/meta.gen.ts";
import { bundledCodex01441ProtocolManifest, currentSourceProtocolManifest } from "./parity.ts";
import { evaluateCodexBinaryCompatibility } from "./compatibility.ts";
import { renderProtocolDiff } from "./drift.ts";

const count = (value: object) => Object.keys(value).length;

describe("Codex app-server protocol authority", () => {
  it("keeps the current-source 126/1/11/72 inventory and stable partition", () => {
    expect(count(CurrentMeta.CLIENT_REQUEST_METHODS)).toBe(126);
    expect(count(CurrentMeta.CLIENT_NOTIFICATION_METHODS)).toBe(1);
    expect(count(CurrentMeta.SERVER_REQUEST_METHODS)).toBe(11);
    expect(count(CurrentMeta.SERVER_NOTIFICATION_METHODS)).toBe(72);
    expect(currentSourceProtocolManifest.requestPartition).toEqual({
      generatedStable: 87,
      deprecatedCompatibility: 3,
      experimentalGated: 36,
    });
  });

  it("keeps the bundled denominator independent", () => {
    expect(count(BundledMeta.CLIENT_REQUEST_METHODS)).toBe(125);
    expect(count(BundledMeta.CLIENT_NOTIFICATION_METHODS)).toBe(1);
    expect(count(BundledMeta.SERVER_REQUEST_METHODS)).toBe(11);
    expect(count(BundledMeta.SERVER_NOTIFICATION_METHODS)).toBe(69);
    expect(bundledCodex01441ProtocolManifest.requestPartition.experimentalGated).toBe(35);
  });

  it("runtime-decodes generated params through Effect Schema", () => {
    const decoded = Schema.decodeUnknownSync(BundledSchema.V1InitializeParams)({
      clientInfo: { name: "openagents", title: "OpenAgents", version: "0.1.0" },
      capabilities: null,
    });
    expect(decoded.clientInfo.name).toBe("openagents");
  });

  it("fails closed for any unreviewed binary tuple", () => {
    expect(
      evaluateCodexBinaryCompatibility({
        version: "0.144.1",
        target: "aarch64-apple-darwin",
        sha256: "wrong",
      }),
    ).toEqual({ _tag: "Incompatible", reason: "unverified-hash" });
  });

  it("renders a human-readable drift falsifier", () => {
    const changed = {
      ...structuredClone(currentSourceProtocolManifest),
      members: currentSourceProtocolManifest.members.slice(1),
    };
    expect(renderProtocolDiff(currentSourceProtocolManifest, changed)).toContain("-");
  });

  it("binds manifests to the committed generated schema", () => {
    for (const [lane, manifest] of [
      ["current-source", currentSourceProtocolManifest],
      ["bundled-0.144.1", bundledCodex01441ProtocolManifest],
    ] as const) {
      const path = resolve(import.meta.dirname, "_generated", lane, "schema.gen.ts");
      const digest = createHash("sha256").update(readFileSync(path)).digest("hex");
      expect(digest).toBe(manifest.generatedSchemaSha256);
    }
  });
});
