import { mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";
import {
  checkManifestFreshness,
  compileFastFollowManifest,
  inventoryArtifactSource,
  inventoryPublicGitSource,
  inventoryTarget,
  parseFastFollow,
  type CompileFastFollowManifestInput,
  type AuthorityEntry,
  type SourceInventory,
} from "../src/index.ts";

const packageRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = realpathSync(resolve(packageRoot, "../.."));
const source = readFileSync(join(packageRoot, "fixtures/conformance/0.1/valid/minimal.md"), "utf8");
const parsed = parseFastFollow(source);
if (!parsed.valid) throw new Error("valid fixture did not parse");
const hash = (character: string) => `sha256:${character.repeat(64)}` as const;
const gitObject = (character: string) => character.repeat(40);

const target = {
  repository: "owner/repo",
  commit: gitObject("a"),
  tree: gitObject("b"),
  authorities: [
    {
      path: "AGENTS.md",
      byte_size: 10,
      content_digest: hash("1"),
      authority_kinds: ["agent_instructions" as const],
    },
    {
      path: "INVARIANTS.md",
      byte_size: 10,
      content_digest: hash("2"),
      authority_kinds: ["invariants" as const],
    },
  ],
};
const sourceInventory: SourceInventory = {
  source_kind: "public_git",
  source_id: "source",
  canonical_ref: "https://example.com/source",
  commit: gitObject("c"),
  tree: gitObject("d"),
  visibility: "public",
  evidence_confidence: "verified_bytes",
  provenance: { origin: "https://example.com/source", license: "unknown" },
  selected_corpus: [{ path: "README.md", byte_size: 20, content_digest: hash("3") }],
};
const fixture = (): CompileFastFollowManifestInput => ({
  document: parsed.document,
  spec_path: "FASTFOLLOW.md",
  compiler_content_digest: hash("f"),
  target,
  sources: [sourceInventory],
});

describe("FastFollowManifest compiler", () => {
  it("produces byte-identical immutable manifests from identical exact inputs", () => {
    const first = compileFastFollowManifest(fixture());
    const second = compileFastFollowManifest(fixture());
    expect(first.bytes).toBe(second.bytes);
    expect(first.digest).toBe(second.digest);
    expect(first.manifest.do_not_edit).toBe(true);
    expect(first.manifest.manifest_content_digest).toBe(first.digest);
    expect(first.bytes).not.toContain(repositoryRoot);
    expect(first.manifest.classification).toEqual({
      manifest_visibility: "public",
      external_instructions: "untrusted_study_data",
      grants_runtime_authority: false,
    });
  });

  it("emits evidence-only work and blocks copying when provenance is unknown", () => {
    const result = compileFastFollowManifest(fixture());
    expect(result.manifest.work_units).toHaveLength(1);
    expect(result.manifest.work_units[0]).toMatchObject({
      stage: "research",
      authority: "evidence_only",
      source_code_copying: "denied_license_unknown_or_restricted",
    });
    expect(result.manifest.sources[0]?.copying_allowed).toBe(false);
  });

  it("rejects mutable labels and missing or unexpected inventories", () => {
    expect(() =>
      compileFastFollowManifest({ ...fixture(), target: { ...target, commit: "main" } }),
    ).toThrowError(expect.objectContaining({ code: "target_identity_not_exact" }));
    expect(() => compileFastFollowManifest({ ...fixture(), sources: [] })).toThrowError(
      expect.objectContaining({ code: "source_inventory_missing" }),
    );
    const extra = { ...sourceInventory, source_id: "not-authored" };
    expect(() =>
      compileFastFollowManifest({ ...fixture(), sources: [sourceInventory, extra] }),
    ).toThrowError(expect.objectContaining({ code: "source_inventory_unknown" }));
  });

  it("reports drift as typed stale reasons without rebinding the manifest", () => {
    const result = compileFastFollowManifest(fixture());
    expect(checkManifestFreshness(result.manifest, { target, sources: [sourceInventory] })).toEqual(
      {
        state: "fresh",
        manifest_digest: result.digest,
      },
    );
    const stale = checkManifestFreshness(result.manifest, {
      target: {
        ...target,
        tree: gitObject("e"),
        authorities: [
          { ...target.authorities[0]!, content_digest: hash("4") },
          target.authorities[1]!,
        ],
      },
      sources: [{ ...sourceInventory, commit: gitObject("9") }],
    });
    expect(stale.state).toBe("stale");
    if (stale.state === "stale")
      expect(stale.reasons.map((reason) => reason.code)).toEqual([
        "target_tree_changed",
        "target_authority_changed",
        "source_identity_changed",
      ]);
    expect(result.manifest.target.tree).toBe(gitObject("b"));
  });

  it("compiles the root seed graph including overlapping directory authorities", () => {
    const rootResult = parseFastFollow(readFileSync(join(repositoryRoot, "FASTFOLLOW.md"), "utf8"));
    if (!rootResult.valid) throw new Error("root FastFollowSpec is invalid");
    const targetBlock = rootResult.document.blocks.target as Record<string, unknown>;
    const authorityKinds = new Map<string, Array<AuthorityEntry["authority_kinds"][number]>>();
    const add = (field: string, kind: AuthorityEntry["authority_kinds"][number]) => {
      for (const path of targetBlock[field] as string[])
        authorityKinds.set(path, [...(authorityKinds.get(path) ?? []), kind]);
    };
    add("agent_instructions", "agent_instructions");
    add("invariants", "invariants");
    add("product_specs", "product_spec");
    add("assurance_specs", "assurance_spec");
    add("roadmap_authorities", "roadmap");
    const rootTarget = {
      repository: "OpenAgentsInc/openagents",
      commit: gitObject("1"),
      tree: gitObject("2"),
      authorities: [...authorityKinds].map(([path, kinds], index) => ({
        path,
        authority_kinds: kinds,
        byte_size: index + 1,
        content_digest: hash(((index % 9) + 1).toString()),
      })),
    };
    const authoredSources = rootResult.document.blocks.sources as Array<Record<string, unknown>>;
    const rootSources: SourceInventory[] = authoredSources.map((authored, index) =>
      authored.access === "public_source"
        ? {
            source_kind: "public_git",
            source_id: String(authored.id),
            canonical_ref: String(authored.canonical_ref),
            commit: gitObject(((index % 6) + 3).toString()),
            tree: gitObject(((index % 3) + 6).toString()),
            visibility: "public",
            evidence_confidence: "verified_bytes",
            provenance: { origin: String(authored.canonical_ref), license: "unknown" },
            selected_corpus: [],
          }
        : {
            source_kind: "artifact",
            source_id: String(authored.id),
            canonical_ref: String(authored.canonical_ref),
            artifact_digest: hash(((index % 9) + 1).toString()),
            release_identity: `${String(authored.id)}@fixture`,
            visibility: "public",
            evidence_confidence: "inferred_bundle",
            provenance: { origin: String(authored.canonical_ref), license: "unknown" },
            selected_corpus: [],
          },
    );
    const compiled = compileFastFollowManifest({
      document: rootResult.document,
      spec_path: "FASTFOLLOW.md",
      compiler_content_digest: hash("f"),
      target: rootTarget,
      sources: rootSources,
    });
    expect(compiled.manifest.directive_graph.length).toBeGreaterThan(5);
    expect(compiled.manifest.work_units.every((unit) => unit.authority === "evidence_only")).toBe(
      true,
    );
  });

  it("inventories the exact current target and selected public Git bytes through Effect", async () => {
    const head = spawnSync("git", ["rev-parse", "HEAD"], {
      cwd: repositoryRoot,
      encoding: "utf8",
    }).stdout.trim();
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests -- Vite Plus does not expose an Effect test API; this executes the adapter boundary.
    const targetResult = await Effect.runPromise(
      inventoryTarget({
        root: repositoryRoot,
        repository: "OpenAgentsInc/openagents",
        authorities: [
          { path: "AGENTS.md", authority_kinds: ["agent_instructions"] },
          { path: "packages/fast-follow-spec", authority_kinds: ["product_spec"] },
        ],
      }),
    );
    expect(targetResult.commit).toBe(head);
    expect(targetResult.tree).toMatch(/^[a-f0-9]{40}$/);
    expect(targetResult.authorities[0]?.content_digest).toMatch(/^sha256:[a-f0-9]{64}$/);
    expect(targetResult.authorities[1]).toMatchObject({
      path: "packages/fast-follow-spec",
      authority_kinds: ["product_spec"],
    });

    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests -- Vite Plus does not expose an Effect test API; this executes the adapter boundary.
    const publicSource = await Effect.runPromise(
      inventoryPublicGitSource({
        root: repositoryRoot,
        source_id: "openagents",
        canonical_ref: "https://github.com/OpenAgentsInc/openagents",
        selected_paths: ["AGENTS.md"],
        visibility: "public",
        provenance: {
          origin: "https://github.com/OpenAgentsInc/openagents",
          license: "known_permissive",
          license_ref: "LICENSE",
        },
      }),
    );
    expect(publicSource.commit).toBe(head);
    expect(publicSource.evidence_confidence).toBe("verified_bytes");
    expect(publicSource.selected_corpus[0]?.path).toBe("AGENTS.md");
  });

  it("binds closed artifacts to exact bytes, release identity, and explicit confidence", async () => {
    const directory = mkdtempSync(join(tmpdir(), "fast-follow-artifact-"));
    const artifact = join(directory, "closed-product.bin");
    writeFileSync(artifact, "observed bundle bytes");
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests -- Vite Plus does not expose an Effect test API; this executes the adapter boundary.
    const result = await Effect.runPromise(
      inventoryArtifactSource({
        artifact_path: artifact,
        source_id: "closed.product",
        canonical_ref: "installed-artifact://Closed.app",
        release_identity: "Closed.app@1.2.3+build.9",
        visibility: "public",
        evidence_confidence: "inferred_bundle",
        provenance: { origin: "installed bundle", license: "unknown" },
      }),
    );
    expect(result.artifact_digest).toMatch(/^sha256:[a-f0-9]{64}$/);
    expect(result.release_identity).toBe("Closed.app@1.2.3+build.9");
    expect(result.evidence_confidence).toBe("inferred_bundle");
  });
});
