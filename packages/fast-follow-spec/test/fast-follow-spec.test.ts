import {
  mkdtempSync,
  mkdirSync,
  readdirSync,
  realpathSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";
import {
  computeDocumentDigest,
  computeIntentDigest,
  discoverFastFollow,
  parseFastFollow,
  serializeFastFollow,
  stableJson,
  starterFastFollow,
} from "../src/index.ts";

const packageRoot = resolve(import.meta.dirname, "..");
const validPath = join(packageRoot, "fixtures/conformance/0.1/valid/minimal.md");
const valid = readFileSync(validPath, "utf8");

describe("FastFollowSpec 0.1", () => {
  it("parses the frozen fixture and the OpenAgents root seed", () => {
    expect(parseFastFollow(valid).valid).toBe(true);
    expect(
      parseFastFollow(readFileSync(resolve(packageRoot, "../../FASTFOLLOW.md"), "utf8")).valid,
    ).toBe(true);
  });

  it("round trips through a byte-stable serializer", () => {
    const parsed = parseFastFollow(valid);
    if (!parsed.valid) throw new Error(JSON.stringify(parsed.diagnostics));
    const once = serializeFastFollow(parsed.document);
    const again = parseFastFollow(once);
    if (!again.valid) throw new Error(JSON.stringify(again.diagnostics));
    expect(serializeFastFollow(again.document)).toBe(once);
  });

  it("separates exact document and canonical intent identities", () => {
    const parsed = parseFastFollow(valid);
    if (!parsed.valid) throw new Error("fixture invalid");
    const timestampOnly = valid.replace("2026-07-17T00:00:00Z", "2026-07-18T00:00:00Z");
    const timestampParsed = parseFastFollow(timestampOnly);
    if (!timestampParsed.valid) throw new Error("timestamp fixture invalid");
    expect(computeDocumentDigest(timestampOnly)).not.toBe(computeDocumentDigest(valid));
    expect(computeIntentDigest(timestampParsed.document)).toBe(
      computeIntentDigest(parsed.document),
    );
    const unknown = valid.replace(
      'title: "Minimal Fast Follow"',
      'title: "Minimal Fast Follow"\nextension_policy: "bound"',
    );
    const unknownParsed = parseFastFollow(unknown);
    if (!unknownParsed.valid) throw new Error(JSON.stringify(unknownParsed.diagnostics));
    expect(computeIntentDigest(unknownParsed.document)).not.toBe(
      computeIntentDigest(parsed.document),
    );
    expect(stableJson({ b: 1, a: { z: 2, y: 3 } })).toBe('{"a":{"y":3,"z":2},"b":1}');
  });

  it("freezes one invalid fixture for every stable parser diagnostic", () => {
    const directory = join(packageRoot, "fixtures/conformance/0.1/invalid");
    const files = readdirSync(directory)
      .filter((file) => file.endsWith(".json"))
      .toSorted();
    expect(files.length).toBeGreaterThanOrEqual(12);
    for (const file of files) {
      const fixture = JSON.parse(readFileSync(join(directory, file), "utf8")) as {
        replace?: string;
        with?: string;
        append?: string;
        code: string;
        first_only?: boolean;
        special?: string;
      };
      let source = valid;
      if (fixture.special === "duplicate_source") {
        const parsed = parseFastFollow(valid);
        if (!parsed.valid) throw new Error("fixture invalid");
        const marker = "```fastfollow-sources\n";
        const start = source.indexOf(marker) + marker.length;
        const end = source.indexOf("\n```", start);
        const sources = JSON.parse(source.slice(start, end)) as unknown[];
        sources.push(sources[0]);
        source = source.slice(0, start) + JSON.stringify(sources) + source.slice(end);
      } else if (fixture.replace !== undefined) {
        source = fixture.first_only
          ? source.replace(fixture.replace, fixture.with ?? "")
          : source.replaceAll(fixture.replace, fixture.with ?? "");
      }
      if (fixture.append) source += fixture.append;
      const result = parseFastFollow(source);
      expect(result.valid, file).toBe(false);
      if (!result.valid)
        expect(
          result.diagnostics.map((item) => item.code),
          file,
        ).toContain(fixture.code);
    }
  });

  it("resolves only the nearest same-scope spec and rejects symlinks", () => {
    const root = mkdtempSync(join(tmpdir(), "fast-follow-discovery-"));
    const nested = join(root, "apps/demo");
    mkdirSync(nested, { recursive: true });
    writeFileSync(join(root, "AGENTS.md"), "root");
    writeFileSync(join(root, "FASTFOLLOW.md"), valid);
    writeFileSync(join(nested, "AGENTS.md"), "nested");
    const missing = discoverFastFollow(nested, root);
    expect(missing.valid).toBe(false);
    expect(missing.diagnostic?.code).toBe("discovery_missing");
    writeFileSync(join(nested, "FASTFOLLOW.md"), valid);
    expect(discoverFastFollow(join(nested, "src/file.ts"), root).path).toBe(
      realpathSync(join(nested, "FASTFOLLOW.md")),
    );
    const linked = join(root, "linked");
    mkdirSync(linked);
    writeFileSync(join(linked, "AGENTS.md"), "linked");
    symlinkSync(join(root, "FASTFOLLOW.md"), join(linked, "FASTFOLLOW.md"));
    expect(discoverFastFollow(linked, root).diagnostic?.code).toBe("discovery_escape");
  });

  it("generates a valid starter", () => {
    expect(parseFastFollow(starterFastFollow("Example", "example.fast_follow")).valid).toBe(true);
  });
});
