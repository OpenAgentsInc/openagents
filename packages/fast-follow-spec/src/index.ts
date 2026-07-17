import { createHash } from "node:crypto";
import { existsSync, lstatSync, realpathSync, readFileSync } from "node:fs";
import { dirname, join, parse, relative, resolve, sep } from "node:path";
import { Schema } from "effect";
import {
  AuthoritySchema,
  BLOCKS,
  DirectivesSchema,
  FORMAT_VERSION,
  FrontmatterSchema,
  GuardrailsSchema,
  ProjectionSchema,
  ReuseSchema,
  SourcesSchema,
  TargetSchema,
  WorkGenerationSchema,
} from "./schema.ts";

export * from "./schema.ts";
export * from "./manifest.ts";
export * from "./inventory.ts";

const decodeFrontmatter = Schema.decodeUnknownSync(FrontmatterSchema);
const decodeProjection = Schema.decodeUnknownSync(ProjectionSchema);
const decodeTarget = Schema.decodeUnknownSync(TargetSchema);
const decodeSources = Schema.decodeUnknownSync(SourcesSchema);
const decodeDirectives = Schema.decodeUnknownSync(DirectivesSchema);
const decodeWorkGeneration = Schema.decodeUnknownSync(WorkGenerationSchema);
const decodeReuse = Schema.decodeUnknownSync(ReuseSchema);
const decodeGuardrails = Schema.decodeUnknownSync(GuardrailsSchema);
const decodeAuthority = Schema.decodeUnknownSync(AuthoritySchema);
const blockDecoders = new Map<string, (value: unknown) => unknown>([
  ["fastfollow-target", decodeTarget],
  ["fastfollow-sources", decodeSources],
  ["fastfollow-directives", decodeDirectives],
  ["fastfollow-work-generation", decodeWorkGeneration],
  ["fastfollow-reuse", decodeReuse],
  ["fastfollow-guardrails", decodeGuardrails],
  ["fastfollow-authority", decodeAuthority],
]);

export type DiagnosticCode =
  | "missing_frontmatter"
  | "invalid_frontmatter"
  | "unsupported_version"
  | "missing_block"
  | "duplicate_block"
  | "block_order"
  | "unknown_block"
  | "invalid_json"
  | "schema_error"
  | "path_escape"
  | "duplicate_id"
  | "dangling_reference"
  | "discovery_missing"
  | "discovery_escape";

export interface FastFollowDiagnostic {
  readonly code: DiagnosticCode;
  readonly message: string;
  readonly path?: string;
}
export interface FastFollowDocument {
  readonly frontmatter: Record<string, unknown>;
  readonly objective: string;
  readonly projection: Record<string, unknown>;
  readonly blocks: Readonly<Record<string, unknown>>;
  readonly source: string;
}
export type ParseResult =
  | {
      readonly valid: true;
      readonly document: FastFollowDocument;
      readonly diagnostics: readonly [];
    }
  | { readonly valid: false; readonly diagnostics: readonly FastFollowDiagnostic[] };

const diagnostic = (code: DiagnosticCode, message: string, path?: string): FastFollowDiagnostic =>
  path === undefined ? { code, message } : { code, message, path };

const parseScalar = (input: string): unknown => {
  const value = input.trim();
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  )
    return value.slice(1, -1);
  if (/^-?\d+$/.test(value)) return Number(value);
  if (value === "true" || value === "false") return value === "true";
  return value;
};

const parseFrontmatter = (
  source: string,
): { value?: Record<string, unknown>; end?: number; error?: FastFollowDiagnostic } => {
  const match = /^(?:\uFEFF)?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/.exec(source);
  if (!match)
    return {
      error: diagnostic("missing_frontmatter", "document must begin with YAML frontmatter"),
    };
  const value: Record<string, unknown> = {};
  for (const [index, line] of match[1]!.split(/\r?\n/).entries()) {
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    const field = /^([a-zA-Z0-9_]+):\s*(.*?)\s*(?:#.*)?$/.exec(line);
    if (!field || field[2] === "")
      return {
        error: diagnostic(
          "invalid_frontmatter",
          `unsupported frontmatter syntax on line ${index + 2}`,
        ),
      };
    if (Object.hasOwn(value, field[1]!))
      return {
        error: diagnostic("invalid_frontmatter", `duplicate frontmatter field: ${field[1]}`),
      };
    value[field[1]!] = parseScalar(field[2]!);
  }
  return { value, end: match[0].length };
};

const isRelativePath = (value: string): boolean => {
  if (!value || value.startsWith("/") || value.startsWith("\\") || /^[A-Za-z]:[\\/]/.test(value))
    return false;
  const normalized = value.replaceAll("\\", "/");
  return !normalized.split("/").includes("..") && !normalized.includes("\0");
};

const pathEntries = (blocks: Record<string, unknown>): Array<[string, unknown]> => {
  const target = blocks.target as Record<string, unknown> | undefined;
  const sources = blocks.sources as Array<Record<string, unknown>> | undefined;
  const directives = blocks.directives as Array<Record<string, unknown>> | undefined;
  const work = blocks.work_generation as Record<string, unknown> | undefined;
  const authority = blocks.authority as Record<string, unknown> | undefined;
  const entries: Array<[string, unknown]> = [];
  const add = (path: string, value: unknown) => entries.push([path, value]);
  for (const key of [
    "agent_instructions",
    "invariants",
    "product_specs",
    "assurance_specs",
    "roadmap_authorities",
  ]) {
    for (const [i, value] of ((target?.[key] as unknown[]) ?? []).entries())
      add(`target.${key}[${i}]`, value);
  }
  for (const [key, value] of Object.entries(
    (target?.artifact_paths as Record<string, unknown>) ?? {},
  ))
    add(`target.artifact_paths.${key}`, value);
  for (const [i, source] of (sources ?? []).entries())
    for (const [j, value] of ((source.teardown_refs as unknown[]) ?? []).entries())
      add(`sources[${i}].teardown_refs[${j}]`, value);
  for (const [i, directive] of (directives ?? []).entries())
    for (const [j, value] of ((directive.target_scopes as unknown[]) ?? []).entries())
      add(`directives[${i}].target_scopes[${j}]`, value);
  const initial = work?.initial_program as Record<string, unknown> | undefined;
  if (initial) add("work_generation.initial_program.strategy_ref", initial.strategy_ref);
  for (const [i, value] of ((authority?.research_write_paths as unknown[]) ?? []).entries())
    add(`authority.research_write_paths[${i}]`, value);
  return entries;
};

export const parseFastFollow = (source: string): ParseResult => {
  const diagnostics: FastFollowDiagnostic[] = [];
  const frontmatter = parseFrontmatter(source);
  if (frontmatter.error || !frontmatter.value || frontmatter.end === undefined)
    return { valid: false, diagnostics: [frontmatter.error!] };
  if (frontmatter.value.fast_follow_spec_format_version !== FORMAT_VERSION)
    diagnostics.push(diagnostic("unsupported_version", `supported format is ${FORMAT_VERSION}`));
  try {
    decodeFrontmatter(frontmatter.value);
  } catch (error) {
    diagnostics.push(diagnostic("invalid_frontmatter", String(error)));
  }

  const body = source.slice(frontmatter.end);
  const headings = Array.from(body.matchAll(/^## (.+?)\s*$/gm)).map((match) => ({
    label: match[1]!,
    index: match.index!,
  }));
  const seen = new Map<string, Array<{ index: number; json: string }>>();
  const fence = /^```(fastfollow-[a-z-]+)\s*\r?\n([\s\S]*?)^```\s*$/gm;
  for (const match of body.matchAll(fence)) {
    const list = seen.get(match[1]!) ?? [];
    list.push({ index: match.index!, json: match[2]!.trim() });
    seen.set(match[1]!, list);
  }
  const known = new Set(BLOCKS.map((entry) => entry[1]));
  for (const name of seen.keys())
    if (!known.has(name as never))
      diagnostics.push(diagnostic("unknown_block", `unknown normative block: ${name}`));

  const blocks: Record<string, unknown> = {};
  let previous = -1;
  for (const [heading, blockName, , field] of BLOCKS) {
    const matches = seen.get(blockName) ?? [];
    if (matches.length === 0) {
      diagnostics.push(diagnostic("missing_block", `missing ${blockName} block`, field));
      continue;
    }
    if (matches.length > 1)
      diagnostics.push(diagnostic("duplicate_block", `repeated ${blockName} block`, field));
    const current = matches[0]!;
    if (current.index < previous)
      diagnostics.push(diagnostic("block_order", `${blockName} is out of order`, field));
    previous = current.index;
    const nearestHeading = headings.findLast((item) => item.index < current.index);
    if (nearestHeading?.label !== heading)
      diagnostics.push(diagnostic("block_order", `${blockName} must follow ## ${heading}`, field));
    try {
      const value: unknown = JSON.parse(current.json);
      blocks[field] = value;
      try {
        blockDecoders.get(blockName)!(value);
      } catch (error) {
        diagnostics.push(diagnostic("schema_error", String(error), field));
      }
    } catch (error) {
      diagnostics.push(diagnostic("invalid_json", `${blockName}: ${String(error)}`, field));
    }
  }

  for (const [path, value] of pathEntries(blocks))
    if (typeof value !== "string" || !isRelativePath(value))
      diagnostics.push(
        diagnostic("path_escape", "repository paths must be relative and contained", path),
      );

  const sources = Array.isArray(blocks.sources)
    ? (blocks.sources as Array<Record<string, unknown>>)
    : [];
  const directives = Array.isArray(blocks.directives)
    ? (blocks.directives as Array<Record<string, unknown>>)
    : [];
  const unique = (values: unknown[], path: string) => {
    const found = new Set<unknown>();
    for (const value of values) {
      if (found.has(value))
        diagnostics.push(diagnostic("duplicate_id", `duplicate id: ${String(value)}`, path));
      found.add(value);
    }
  };
  unique(
    sources.map((item) => item.id),
    "sources",
  );
  const lessonRefs = new Set<string>();
  for (const sourceItem of sources) {
    const lessons = Array.isArray(sourceItem.lessons)
      ? (sourceItem.lessons as Array<Record<string, unknown>>)
      : [];
    unique(
      lessons.map((item) => item.id),
      `sources.${String(sourceItem.id)}.lessons`,
    );
    for (const lesson of lessons) lessonRefs.add(`${String(sourceItem.id)}#${String(lesson.id)}`);
  }
  unique(
    directives.map((item) => item.id),
    "directives",
  );
  for (const directive of directives)
    for (const ref of Array.isArray(directive.source_refs) ? directive.source_refs : []) {
      if (typeof ref !== "string" || !lessonRefs.has(ref))
        diagnostics.push(
          diagnostic(
            "dangling_reference",
            `unresolved lesson reference: ${String(ref)}`,
            `directives.${String(directive.id)}`,
          ),
        );
    }
  const directiveIds = new Set(directives.map((item) => item.id));
  const initial = (blocks.work_generation as Record<string, unknown> | undefined)
    ?.initial_program as Record<string, unknown> | undefined;
  for (const ref of Array.isArray(initial?.directive_order) ? initial.directive_order : [])
    if (!directiveIds.has(ref))
      diagnostics.push(
        diagnostic(
          "dangling_reference",
          `unresolved directive reference: ${String(ref)}`,
          "work_generation.initial_program",
        ),
      );

  if (diagnostics.length > 0) return { valid: false, diagnostics };
  const projection: Record<string, unknown> = {
    format_version: frontmatter.value.fast_follow_spec_format_version,
    spec_id: frontmatter.value.fast_follow_spec_id,
    revision: frontmatter.value.fast_follow_revision,
    title: frontmatter.value.title,
    lifecycle_state: frontmatter.value.lifecycle_state,
    ...blocks,
  };
  try {
    decodeProjection(projection);
  } catch (error) {
    return { valid: false, diagnostics: [diagnostic("schema_error", String(error), "projection")] };
  }
  const objectiveMatch = /^## Objective\s*\r?\n([\s\S]*?)(?=^## Target\s*$)/m.exec(body);
  return {
    valid: true,
    diagnostics: [],
    document: {
      frontmatter: frontmatter.value,
      objective: objectiveMatch?.[1]?.trim() ?? "",
      projection,
      blocks,
      source,
    },
  };
};

const normalizeJson = (input: unknown): unknown =>
  Array.isArray(input)
    ? input.map(normalizeJson)
    : input && typeof input === "object"
      ? Object.fromEntries(
          Object.entries(input as Record<string, unknown>)
            .toSorted(([a], [b]) => a.localeCompare(b))
            .map(([key, item]) => [key, normalizeJson(item)]),
        )
      : input;

export const stableJson = (value: unknown): string => JSON.stringify(normalizeJson(value));
const sha256 = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");
export const computeDocumentDigest = (source: string | Uint8Array): string => sha256(source);
export const intentProjection = (document: FastFollowDocument): Record<string, unknown> => {
  const { created_at: _created, updated_at: _updated, ...frontmatter } = document.frontmatter;
  return { intent_digest_version: "fast-follow-intent-0.1", frontmatter, ...document.blocks };
};
export const computeIntentDigest = (document: FastFollowDocument): string =>
  sha256(stableJson(intentProjection(document)));

const quoteYaml = (value: unknown): string =>
  typeof value === "number" ? String(value) : JSON.stringify(value);
export const serializeFastFollow = (document: FastFollowDocument): string => {
  const frontmatterOrder = [
    "fast_follow_spec_format_version",
    "fast_follow_spec_id",
    "fast_follow_revision",
    "title",
    "artifact_type",
    "lifecycle_state",
    "author",
    "linked_target_repo",
    "created_at",
    "updated_at",
  ];
  const keys = [
    ...frontmatterOrder,
    ...Object.keys(document.frontmatter)
      .filter((key) => !frontmatterOrder.includes(key))
      .toSorted(),
  ];
  const lines = [
    "---",
    ...keys.map((key) => `${key}: ${quoteYaml(document.frontmatter[key])}`),
    "---",
    "",
    "# " + String(document.frontmatter.title),
    "",
    "## Objective",
    "",
    document.objective,
  ];
  for (const [heading, blockName, , field] of BLOCKS)
    lines.push(
      "",
      `## ${heading}`,
      "",
      `\`\`\`${blockName}`,
      JSON.stringify(document.blocks[field], null, 2),
      "```",
    );
  return lines.join("\n") + "\n";
};

export interface DiscoveryResult {
  readonly valid: boolean;
  readonly path?: string;
  readonly diagnostic?: FastFollowDiagnostic;
}
export const discoverFastFollow = (start: string, repositoryRoot?: string): DiscoveryResult => {
  const root = realpathSync(repositoryRoot ?? findFilesystemRoot(start));
  let startingDirectory =
    existsSync(start) && lstatSync(start).isDirectory() ? start : dirname(start);
  while (!existsSync(startingDirectory) && dirname(startingDirectory) !== startingDirectory)
    startingDirectory = dirname(startingDirectory);
  let current = realpathSync(startingDirectory);
  while (current === root || current.startsWith(root + sep)) {
    const agents = join(current, "AGENTS.md");
    const candidate = join(current, "FASTFOLLOW.md");
    if (existsSync(agents)) {
      if (!existsSync(candidate))
        return {
          valid: false,
          diagnostic: diagnostic(
            "discovery_missing",
            `no FASTFOLLOW.md beside ${agents}`,
            candidate,
          ),
        };
      if (lstatSync(candidate).isSymbolicLink())
        return {
          valid: false,
          diagnostic: diagnostic(
            "discovery_escape",
            "FASTFOLLOW.md must not be a symlink",
            candidate,
          ),
        };
      const actual = realpathSync(candidate);
      if (relative(root, actual).startsWith(".."))
        return {
          valid: false,
          diagnostic: diagnostic("discovery_escape", "FASTFOLLOW.md escapes repository", candidate),
        };
      return { valid: true, path: actual };
    }
    if (current === root) break;
    current = dirname(current);
  }
  return {
    valid: false,
    diagnostic: diagnostic(
      "discovery_missing",
      "no applicable AGENTS.md/FASTFOLLOW.md scope found",
      start,
    ),
  };
};
const findFilesystemRoot = (start: string): string => parse(resolve(start)).root;

export const readFastFollow = (path: string): ParseResult =>
  parseFastFollow(readFileSync(path, "utf8"));

export const starterFastFollow = (title: string, specId: string): string => {
  const now = new Date().toISOString();
  const source = `---\nfast_follow_spec_format_version: "0.1"\nfast_follow_spec_id: "${specId}"\nfast_follow_revision: 1\ntitle: ${JSON.stringify(title)}\nartifact_type: "learning_intent"\nlifecycle_state: "proposed"\nauthor: "owner"\nlinked_target_repo: "owner/repository"\ncreated_at: "${now}"\nupdated_at: "${now}"\n---\n\n# ${title}\n\n## Objective\n\nDescribe the learning objective and explicit non-goal.\n\n`;
  const blocks: Record<string, unknown> = {
    target: {
      id: specId.split(".")[0],
      repository: "owner/repository",
      root: ".",
      agent_instructions: ["AGENTS.md"],
      invariants: ["INVARIANTS.md"],
      product_specs: [],
      assurance_specs: [],
      roadmap_authorities: [],
      artifact_paths: {
        studies: "docs/fastfollow/studies",
        gaps: "docs/fastfollow/gaps",
        candidates: "docs/fastfollow/candidates",
        receipts: "docs/fastfollow/receipts",
      },
    },
    sources: [
      {
        id: "source",
        title: "Source",
        role: "upstream",
        access: "public_source",
        canonical_ref: "https://example.com/source",
        tracking_policy: "pinned_each_run",
        teardown_refs: [],
        lessons: [
          {
            id: "lesson",
            kind: "architecture",
            summary: "Study one bounded lesson.",
            stance: "study",
          },
        ],
      },
    ],
    directives: [
      {
        id: "directive",
        title: "Directive",
        priority: 1,
        source_refs: ["source#lesson"],
        target_scopes: ["src"],
        desired_outcome: "Produce evidence.",
        work_products: ["study_packet"],
        constraints: [],
      },
    ],
    work_generation: {
      activation: "manual",
      allowed_stages: ["research"],
      selection_policy: {
        higher_authority_precedence: true,
        one_concrete_unit_per_turn: true,
        dedupe_key_fields: ["directive"],
        no_material_delta: true,
      },
      capacity_profiles: {
        backlog_available: { delivery: 1, research: 0, implementation: 0 },
        backlog_empty: { delivery: 0, research: 1, implementation: 0 },
      },
      implementation_requirements: ["admitted_issue_or_work_packet"],
    },
    reuse: {
      shareable_visibility: "public_only",
      study_packet_key_fields: ["source_digest"],
      freshness_days: 30,
      private_target_analysis: "target_private_by_default",
      cross_tenant_private_cache: false,
      cache_hit_means: "reusable_evidence_not_adoption",
    },
    guardrails: { must_preserve: ["target authority"], must_reject: ["source self-admission"] },
    authority: {
      allowed: ["research"],
      denied: ["deployment"],
      research_write_paths: ["docs/fastfollow"],
      implementation_requirements: ["separate target authority"],
    },
  };
  let result = source;
  for (const [heading, blockName, , field] of BLOCKS)
    result += `## ${heading}\n\n\`\`\`${blockName}\n${JSON.stringify(blocks[field], null, 2)}\n\`\`\`\n\n`;
  return result;
};
