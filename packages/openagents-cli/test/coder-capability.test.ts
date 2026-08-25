import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  capabilityTool,
  defaultCapabilityGapRecorder,
  discoverPluginCatalog,
  type CapabilityGap,
  type PluginCatalogEntry,
} from "../src/coder-capability.js";
import {
  isRefusal,
  loadPluginFromManifest,
  PluginApproval,
  type LoadedPlugin,
  type PluginApprovalRequest,
} from "../src/coder-plugins.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/word-stats/manifest.json", import.meta.url),
);

const loadedFixture = ((): LoadedPlugin => {
  const outcome = loadPluginFromManifest(MANIFEST);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
})();

const baseCatalog: ReadonlyArray<PluginCatalogEntry> = [
  {
    name: loadedFixture.manifest.name,
    version: loadedFixture.manifest.version,
    description: loadedFixture.manifest.description,
    manifestPath: MANIFEST,
    artifact: loadedFixture.manifest.artifact,
    capabilities: loadedFixture.manifest.capabilities,
  },
];

describe("capabilityTool", () => {
  it("declares one standing tool that names the catalog, capped rather than unbounded", () => {
    // The description deliberately carries the installed names and first
    // sentences: a model that has never heard what is installed answered
    // "read that conversation back" with an improvised shell script while
    // the capability sat unused. The growth is capped, not zero — past the
    // cap, the rest ride behind `query`.
    const bigCatalog = Array.from({ length: 20 }, (_unused, index) => ({
      ...baseCatalog[0],
      name: `demo_${String(index)}`,
    }));
    const tool = capabilityTool({
      catalog: bigCatalog,
      approval: new PluginApproval(),
      recordGap: () => {},
      onSelect: () => {},
    });
    expect(tool.name).toBe("capability");
    const properties = tool.parameters["properties"] as Record<string, unknown>;
    expect(Object.keys(properties)).toContain("query");
    expect(Object.keys(properties)).toContain("name");
    expect(tool.description).toMatch(/No semantic embedding is available/);
    expect(tool.description).toContain("demo_0");
    expect(tool.description).toContain("demo_11");
    expect(tool.description).not.toContain("demo_12");
    expect(tool.description).toContain("and 8 more");
    expect(tool.parameters["additionalProperties"]).toBe(false);
  });

  it("returns the full catalog on a query and does not use keyword selection", async () => {
    const catalog: ReadonlyArray<PluginCatalogEntry> = [
      baseCatalog[0],
      { ...baseCatalog[0], name: "git_lost_work", description: "Scan a git repository." },
    ];
    const tool = capabilityTool({
      catalog,
      approval: new PluginApproval(),
      recordGap: () => {},
      onSelect: () => {},
    });
    const output = await tool.run({ query: "word" }, new AbortController().signal);
    expect(output).toContain("word_stats");
    expect(output).toContain("git_lost_work");
    expect(output).toMatch(/No semantic embedding is available/);
  });

  it("loads and selects a capability by exact catalog name", async () => {
    const selected: Array<{ readonly plugin: LoadedPlugin; readonly manifestPath: string }> = [];
    const tool = capabilityTool({
      catalog: baseCatalog,
      approval: new PluginApproval(),
      recordGap: () => {},
      onSelect: (plugin, manifestPath) => selected.push({ plugin, manifestPath }),
      load: loadPluginFromManifest,
    });
    const output = await tool.run({ name: "word_stats" }, new AbortController().signal);
    expect(output).toMatch(/Loaded plugin `word_stats`/);
    expect(selected).toHaveLength(1);
    expect(selected[0].plugin.manifest.name).toBe("word_stats");
  });

  it("records a capability gap when an exact name is not in the catalog", async () => {
    const gaps: CapabilityGap[] = [];
    const tool = capabilityTool({
      catalog: baseCatalog,
      approval: new PluginApproval(),
      recordGap: (gap) => {
        gaps.push(gap);
      },
      onSelect: () => {
        throw new Error("onSelect should not be called for a missing capability");
      },
    });
    const output = await tool.run({ name: "not_installed" }, new AbortController().signal);
    expect(gaps).toHaveLength(1);
    expect(gaps[0].requestedName).toBe("not_installed");
    expect(output).toMatch(/No capability named `not_installed`/);
  });

  it("records a gap when the catalog is empty on a query", async () => {
    const gaps: CapabilityGap[] = [];
    const tool = capabilityTool({
      catalog: [],
      approval: new PluginApproval(),
      recordGap: (gap) => {
        gaps.push(gap);
      },
      onSelect: () => {},
    });
    const output = await tool.run({ query: "something" }, new AbortController().signal);
    expect(gaps).toHaveLength(1);
    expect(gaps[0].query).toBe("something");
    expect(output).toMatch(/The local catalog is empty/);
  });
});

describe("PluginApproval", () => {
  it("auto-approves capabilities with no mounts and no hosts", async () => {
    const asked: PluginApprovalRequest[] = [];
    const approval = new PluginApproval({
      ask: (request) => {
        asked.push(request);
        return "allow";
      },
    });
    const result = await approval.check({
      name: "pure",
      digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      capabilities: { mounts: [], hosts: [] },
    });
    expect(result).toBe("approved");
    expect(asked).toHaveLength(0);
  });

  it("asks once for read-only mounts and caches by digest and capabilities", async () => {
    const asked: PluginApprovalRequest[] = [];
    const approval = new PluginApproval({
      ask: (request) => {
        asked.push(request);
        return "allow";
      },
    });
    const request = {
      name: "reader",
      digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      capabilities: { mounts: [{ path: "data", readonly: true as boolean }], hosts: [] },
    };
    expect(await approval.check(request)).toBe("approved");
    expect(await approval.check(request)).toBe("approved");
    expect(asked).toHaveLength(1);
  });

  it("asks every time for declared hosts", async () => {
    const asked: PluginApprovalRequest[] = [];
    const approval = new PluginApproval({
      ask: (request) => {
        asked.push(request);
        return "allow";
      },
    });
    const request = {
      name: "net",
      digest: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
      capabilities: { mounts: [], hosts: ["api.example.com"] },
    };
    await approval.check(request);
    await approval.check(request);
    expect(asked).toHaveLength(2);
  });

  it("asks every time for writable mounts", async () => {
    const asked: PluginApprovalRequest[] = [];
    const approval = new PluginApproval({
      ask: (request) => {
        asked.push(request);
        return "allow";
      },
    });
    const request = {
      name: "writer",
      digest: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
      capabilities: { mounts: [{ path: "data", readonly: false }], hosts: [] },
    };
    await approval.check(request);
    await approval.check(request);
    expect(asked).toHaveLength(2);
  });

  it("refuses non-pure capabilities when no approver is configured", async () => {
    const approval = new PluginApproval();
    const result = await approval.check({
      name: "reader",
      digest: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      capabilities: { mounts: [{ path: "data", readonly: true }], hosts: [] },
    });
    expect(isRefusal(result) && result.code).toBe("approval_unavailable");
  });
});

describe("discoverPluginCatalog", () => {
  it("discovers the checked-in demo plugins from this repository", () => {
    const catalog = discoverPluginCatalog(fileURLToPath(import.meta.url));
    const names = catalog.map((entry) => entry.name).sort();
    expect(names).toContain("word_stats");
    expect(names).toContain("dir_stats");
    expect(names).toContain("file_stats");
    expect(names).toContain("git_lost_work");
    expect(names).toContain("foreign_sessions");
    expect(catalog.every((entry) => entry.artifact.digest.startsWith("sha256:"))).toBe(true);
  });
});

describe("defaultCapabilityGapRecorder", () => {
  it("produces an async writer for the default gap file", () => {
    const recorder = defaultCapabilityGapRecorder();
    expect(typeof recorder).toBe("function");
  });
});
