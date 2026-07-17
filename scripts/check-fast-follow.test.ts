import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const specPath = path.join(repoRoot, "FASTFOLLOW.md");
const source = readFileSync(specPath, "utf8");

const readFrontmatterString = (key: string): string => {
  const frontmatter = /^---\n([\s\S]*?)\n---/.exec(source)?.[1];
  if (frontmatter === undefined) throw new Error("FASTFOLLOW.md has no frontmatter");
  const match = new RegExp(`^${key}:\\s+["']?([^"'\\n]+)["']?$`, "m").exec(frontmatter);
  if (match?.[1] === undefined) throw new Error(`FASTFOLLOW.md frontmatter is missing ${key}`);
  return match[1].trim();
};

const readFrontmatterInteger = (key: string): number => {
  const value = Number(readFrontmatterString(key));
  if (!Number.isSafeInteger(value)) throw new Error(`${key} is not an integer`);
  return value;
};

const readBlock = <A>(label: string): A => {
  const match = new RegExp("```" + label + "\\n([\\s\\S]*?)\\n```", "g").exec(source);
  if (match?.[1] === undefined) throw new Error(`FASTFOLLOW.md is missing ${label}`);
  return JSON.parse(match[1]) as A;
};

type Lesson = Readonly<{
  id: string;
  kind: string;
  summary: string;
  stance: string;
}>;

type FastFollowSource = Readonly<{
  id: string;
  title: string;
  role: string;
  access: string;
  canonical_ref: string;
  tracking_policy: string;
  teardown_refs: ReadonlyArray<string>;
  lessons: ReadonlyArray<Lesson>;
}>;

type Directive = Readonly<{
  id: string;
  source_refs: ReadonlyArray<string>;
  target_scopes: ReadonlyArray<string>;
}>;

type Capacity = Readonly<{
  delivery: number;
  research: number;
  implementation: number;
}>;

type WorkGeneration = Readonly<{
  activation: string;
  allowed_stages: ReadonlyArray<string>;
  selection_policy: Readonly<{
    higher_authority_precedence: boolean;
    one_concrete_unit_per_turn: boolean;
    dedupe_key_fields: ReadonlyArray<string>;
    no_material_delta: boolean;
  }>;
  capacity_profiles: Readonly<{
    backlog_available: Capacity;
    backlog_empty: Capacity;
  }>;
  implementation_requirements: ReadonlyArray<string>;
}>;

type Reuse = Readonly<{
  shareable_visibility: string;
  study_packet_key_fields: ReadonlyArray<string>;
  freshness_days: number;
  private_target_analysis: string;
  cross_tenant_private_cache: boolean;
  cache_hit_means: string;
}>;

type Guardrails = Readonly<{
  must_preserve: ReadonlyArray<string>;
  must_reject: ReadonlyArray<string>;
}>;

type Authority = Readonly<{
  allowed: ReadonlyArray<string>;
  denied: ReadonlyArray<string>;
  research_write_paths: ReadonlyArray<string>;
  implementation_requirements: ReadonlyArray<string>;
}>;

const target = readBlock<Record<string, unknown>>("fastfollow-target");
const sources = readBlock<ReadonlyArray<FastFollowSource>>("fastfollow-sources");
const directives = readBlock<ReadonlyArray<Directive>>("fastfollow-directives");
const workGeneration = readBlock<WorkGeneration>("fastfollow-work-generation");
const reuse = readBlock<Reuse>("fastfollow-reuse");
const guardrails = readBlock<Guardrails>("fastfollow-guardrails");
const authority = readBlock<Authority>("fastfollow-authority");

const unique = (values: ReadonlyArray<string>): boolean => new Set(values).size === values.length;
const capacityTotal = (capacity: Capacity): number =>
  capacity.delivery + capacity.research + capacity.implementation;

describe("OpenAgents FastFollowSpec 0.1 seed", () => {
  test("matches the canonical projection schema envelope", () => {
    const schema = JSON.parse(
      readFileSync(path.join(repoRoot, "docs/fastfollow/fast-follow.schema.json"), "utf8"),
    ) as Readonly<{ $id?: string; required?: ReadonlyArray<string> }>;

    const projection = {
      format_version: readFrontmatterString("fast_follow_spec_format_version"),
      spec_id: readFrontmatterString("fast_follow_spec_id"),
      revision: readFrontmatterInteger("fast_follow_revision"),
      title: readFrontmatterString("title"),
      lifecycle_state: readFrontmatterString("lifecycle_state"),
      target,
      sources,
      directives,
      work_generation: workGeneration,
      reuse,
      guardrails,
      authority,
    };

    expect(schema.$id).toBe("https://openagents.com/schemas/fast-follow-spec-projection-0.1.json");
    expect(projection.format_version).toBe("0.1");
    expect(projection.revision).toBe(1);
    expect(projection.lifecycle_state).toBe("admitted");
    expect(Object.keys(projection).toSorted()).toEqual([...(schema.required ?? [])].toSorted());
  });

  test("has stable unique source, lesson, and directive identities with no dangling refs", () => {
    expect(unique(sources.map((item) => item.id))).toBe(true);
    expect(unique(directives.map((item) => item.id))).toBe(true);

    const lessonRefs = new Set<string>();
    for (const item of sources) {
      expect(item.lessons.length).toBeGreaterThan(0);
      expect(unique(item.lessons.map((lesson) => lesson.id))).toBe(true);
      for (const lesson of item.lessons) lessonRefs.add(`${item.id}#${lesson.id}`);
    }

    for (const directive of directives) {
      expect(directive.source_refs.length).toBeGreaterThan(0);
      expect(unique(directive.source_refs)).toBe(true);
      expect(directive.target_scopes.length).toBeGreaterThan(0);
      for (const ref of directive.source_refs) expect(lessonRefs.has(ref), ref).toBe(true);
    }
  });

  test("covers every teardown document and every referenced repository path exists", () => {
    const teardownRoot = path.join(repoRoot, "docs/teardowns");
    const expected = readdirSync(teardownRoot)
      .filter((name) => name.endsWith(".md") && name !== "README.md")
      .map((name) => `docs/teardowns/${name}`)
      .toSorted();
    const referenced = [...new Set(sources.flatMap((item) => item.teardown_refs))].toSorted();

    expect(referenced).toEqual(expected);
    for (const relativePath of referenced) {
      expect(relativePath.startsWith("docs/teardowns/")).toBe(true);
      expect(relativePath.includes(".."), relativePath).toBe(false);
      expect(existsSync(path.join(repoRoot, relativePath)), relativePath).toBe(true);
    }
  });

  test("pins the owner's five-slot profiles without granting dispatch authority", () => {
    expect(workGeneration.activation).toBe("continuous");
    expect(workGeneration.selection_policy).toMatchObject({
      higher_authority_precedence: true,
      one_concrete_unit_per_turn: true,
      no_material_delta: true,
    });
    expect(workGeneration.capacity_profiles.backlog_available).toEqual({
      delivery: 3,
      research: 1,
      implementation: 1,
    });
    expect(workGeneration.capacity_profiles.backlog_empty).toEqual({
      delivery: 0,
      research: 2,
      implementation: 3,
    });
    expect(capacityTotal(workGeneration.capacity_profiles.backlog_available)).toBe(5);
    expect(capacityTotal(workGeneration.capacity_profiles.backlog_empty)).toBe(5);
    expect(workGeneration.implementation_requirements).toEqual([
      "admitted_issue_or_work_packet",
      "current_target_authority_reconciliation",
      "isolated_mutation_claim",
      "target_local_verification",
    ]);
  });

  test("keeps reuse public-only and research inside its explicit write boundary", () => {
    expect(reuse).toMatchObject({
      shareable_visibility: "public_only",
      private_target_analysis: "target_private_by_default",
      cross_tenant_private_cache: false,
      cache_hit_means: "reusable_evidence_not_adoption",
    });
    expect(reuse.study_packet_key_fields.length).toBeGreaterThanOrEqual(7);
    expect(authority.research_write_paths.length).toBeGreaterThan(0);
    for (const relativePath of authority.research_write_paths) {
      expect(relativePath.startsWith("docs/"), relativePath).toBe(true);
      expect(relativePath.includes(".."), relativePath).toBe(false);
    }

    const rejected = guardrails.must_reject.join(" ").toLowerCase();
    const denied = authority.denied.join(" ").toLowerCase();
    expect(rejected).toContain("self-promotion");
    expect(denied).toContain("grant repository");
    expect(denied).toContain("private target analysis");
  });
});
