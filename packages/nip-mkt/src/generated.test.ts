import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

import { describe, expect, test } from "vite-plus/test";
import { Schema } from "effect";

import {
  CONTRACT_SHA256,
  CONTRACT_SOURCE_COMMIT,
  FIXTURE_MANIFEST_SHA256,
  MKT_KINDS,
  MKT_TAG_REQUIREMENTS_BY_KIND,
  MktEventSchema,
  PRIVATE_MKT_KINDS,
  PUBLIC_MKT_KINDS,
} from "./generated.js";

const packageRoot = new URL("../", import.meta.url);
const sha256 = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const hex = "1".repeat(64);
const decodeMktEvent = Schema.decodeUnknownSync(MktEventSchema);

const tagsByKind: Readonly<Record<number, readonly (readonly string[])[]>> = {
  39600: [
    ["d", "provider"],
    ["status", "active"],
    ["published_at", "1"],
    ["profile", "conformance", "1"],
  ],
  39601: [
    ["d", "offering"],
    ["profile", "conformance", "1"],
    ["status", "active"],
    ["provider", `39600:${hex}:provider`],
    ["published_at", "1"],
  ],
  39602: [
    ["d", "conformance"],
    ["version", "1"],
    ["x", hex],
    ["r", "https://example.test/profile"],
    ["status", "active"],
  ],
  39603: [
    ["d", hex],
    ["profile", "conformance", "1"],
    ["outcome", "completed"],
    ["x", hex],
    ["role", "provider"],
  ],
  39604: [
    ["d", hex],
    ["session", hex],
    ["profile", "conformance", "1"],
    ["p", hex, "", "provider"],
    ["alt", "RFQ"],
    ["a", `39601:${hex}:offering`, "", "offering"],
    ["expiration", "2"],
  ],
  39605: [
    ["d", hex],
    ["session", hex],
    ["profile", "conformance", "1"],
    ["p", hex, "", "requester"],
    ["alt", "Quote"],
    ["e", hex, "", "rfq"],
    ["expiration", "2"],
    ["quote", "firm"],
    ["reservation", "hard"],
  ],
  39606: [
    ["d", hex],
    ["session", hex],
    ["profile", "conformance", "1"],
    ["p", hex, "", "provider"],
    ["alt", "Order"],
    ["e", hex, "", "quote"],
  ],
  39607: [
    ["d", hex],
    ["session", hex],
    ["profile", "conformance", "1"],
    ["p", hex, "", "provider"],
    ["alt", "Status"],
    ["e", hex, "", "order"],
    ["seq", "0"],
    ["state", "accepted"],
  ],
  39608: [
    ["d", hex],
    ["session", hex],
    ["profile", "conformance", "1"],
    ["p", hex, "", "provider"],
    ["alt", "Cancel"],
    ["e", hex, "", "order"],
    ["action", "request"],
    ["reason", "operator_request"],
  ],
  39609: [
    ["d", hex],
    ["session", hex],
    ["profile", "conformance", "1"],
    ["p", hex, "", "provider"],
    ["alt", "Close"],
    ["e", hex, "", "order"],
    ["outcome", "completed"],
    ["terminal_at", "2"],
  ],
};

const invalidTagByKind: Readonly<Record<number, readonly string[]>> = {
  39600: ["status", "unknown"],
  39601: ["provider", "39600:bad:provider"],
  39602: ["version", "0"],
  39603: ["x", "bad"],
  39604: ["a", "39601:bad:offering", "", "offering"],
  39605: ["reservation", "unknown"],
  39606: ["e", "bad", "", "quote"],
  39607: ["state", "unknown"],
  39608: ["action", "unknown"],
  39609: ["terminal_at", "-1"],
};

function contentFor(kind: number, profile = "conformance"): string {
  return kind < 39604
    ? "{}"
    : JSON.stringify({
        schema: "openagents.mkt.v1",
        profile,
        profile_version: 1,
        session_id: hex,
      });
}

function eventFor(kind: number, tags = tagsByKind[kind]) {
  if (tags === undefined) throw new Error(`missing generated-schema fixture for ${kind}`);
  return {
    id: hex,
    pubkey: hex,
    created_at: 1,
    kind,
    tags,
    content: contentFor(kind),
    sig: "2".repeat(128),
  };
}

describe("pinned Immortal contract", () => {
  test("records the source and generated kind partition", () => {
    expect(CONTRACT_SOURCE_COMMIT).toBe("15e77e0c9958b2334a8471c250cf7476f4c28598");
    expect(MKT_KINDS).toEqual([
      39600, 39601, 39602, 39603, 39604, 39605, 39606, 39607, 39608, 39609,
    ]);
    expect(PUBLIC_MKT_KINDS).toEqual([39600, 39601, 39602, 39603]);
    expect(PRIVATE_MKT_KINDS).toEqual([39604, 39605, 39606, 39607, 39608, 39609]);
  });

  test("keeps contract and manifest bytes pinned", async () => {
    const [sourceBytes, contract, manifest] = await Promise.all([
      readFile(new URL("contract/SOURCE.json", packageRoot)),
      readFile(new URL("contract/immortal-contract.json", packageRoot)),
      readFile(new URL("contract/immortal-fixtures.json", packageRoot)),
    ]);
    const source = JSON.parse(sourceBytes.toString("utf8")) as {
      commit: string;
      contract_sha256: string;
      fixture_manifest_sha256: string;
    };
    expect(CONTRACT_SOURCE_COMMIT).toBe(source.commit);
    expect(sha256(contract)).toBe(CONTRACT_SHA256);
    expect(sha256(manifest)).toBe(FIXTURE_MANIFEST_SHA256);
    expect(CONTRACT_SHA256).toBe(source.contract_sha256);
    expect(FIXTURE_MANIFEST_SHA256).toBe(source.fixture_manifest_sha256);
  });

  test("generates a kind-discriminated schema with required tag grammar", () => {
    for (const kind of MKT_KINDS) {
      const tags = tagsByKind[kind];
      if (tags === undefined) throw new Error(`missing generated-schema fixture for ${kind}`);
      const event = eventFor(kind, tags);
      expect(decodeMktEvent(event).kind).toBe(kind);
      expect(MKT_TAG_REQUIREMENTS_BY_KIND[kind]).toContain("d");
      expect(() =>
        decodeMktEvent({ ...event, tags: tags.filter((tag) => tag[0] !== "d") }),
      ).toThrow();
      expect(() =>
        decodeMktEvent({ ...event, tags: [...tags, tags.find((tag) => tag[0] === "d")!] }),
      ).toThrow();

      const invalidTag = invalidTagByKind[kind];
      if (invalidTag === undefined)
        throw new Error(`missing negative generated-schema fixture for ${kind}`);
      const invalidTags = tags.map((tag) =>
        tag[0] === invalidTag[0] &&
        ((invalidTag[0] !== "e" && invalidTag[0] !== "a") || tag[3] === invalidTag[3])
          ? invalidTag
          : tag,
      );
      expect(() => decodeMktEvent({ ...event, tags: invalidTags })).toThrow();
    }
  });

  test("checks private content envelopes and conditional status references", () => {
    const rfq = eventFor(39604);
    expect(() => decodeMktEvent({ ...rfq, content: "not-json" })).toThrow();
    expect(() => decodeMktEvent({ ...rfq, content: contentFor(39604, "other-profile") })).toThrow();

    const status = eventFor(39607);
    const sequenceOne = status.tags.map((tag) => (tag[0] === "seq" ? ["seq", "1"] : tag));
    expect(() => decodeMktEvent({ ...status, tags: sequenceOne })).toThrow();
    expect(
      decodeMktEvent({
        ...status,
        tags: [...sequenceOne, ["e", hex, "", "previous"]],
      }).kind,
    ).toBe(39607);
  });

  test("replays every vendored MKT fixture byte-for-byte against the manifest", async () => {
    const manifestBytes = await readFile(new URL("contract/immortal-fixtures.json", packageRoot));
    const manifest = JSON.parse(manifestBytes.toString("utf8")) as {
      fixtures: ReadonlyArray<{ path: string; sha256: string; bytes: number }>;
    };
    const selected = manifest.fixtures.filter(
      ({ path }) =>
        path.startsWith("tests/fixtures/nipmkt/") ||
        path === "tests/fixtures/nip44/market-client.json",
    );
    expect(selected).toHaveLength(8);
    const replayed = await Promise.all(
      selected.map(async (fixture) => {
        const relativePath = fixture.path.replace(/^tests\//, "contract/");
        return { bytes: await readFile(new URL(relativePath, packageRoot)), fixture };
      }),
    );
    for (const { bytes, fixture } of replayed) {
      expect(bytes.byteLength, fixture.path).toBe(fixture.bytes);
      expect(sha256(bytes), fixture.path).toBe(fixture.sha256);
    }
  });
});
