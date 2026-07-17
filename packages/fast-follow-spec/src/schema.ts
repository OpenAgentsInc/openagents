import { Schema as S } from "effect";

export const FORMAT_VERSION = "0.1" as const;
export const NonEmptyString = S.String.check(S.isMinLength(1));
export const StableId = NonEmptyString;
export const RelativePath = NonEmptyString;
export const StringList = S.Array(NonEmptyString);
export const PathList = S.Array(RelativePath);

export const FrontmatterSchema = S.Struct({
  fast_follow_spec_format_version: S.Literal(FORMAT_VERSION),
  fast_follow_spec_id: StableId,
  fast_follow_revision: S.Number,
  title: NonEmptyString,
  artifact_type: S.Literal("learning_intent"),
  lifecycle_state: S.Literals(["proposed", "admitted", "superseded", "retired"]),
  author: NonEmptyString,
  linked_target_repo: NonEmptyString,
  created_at: NonEmptyString,
  updated_at: NonEmptyString,
});
export type FastFollowFrontmatter = typeof FrontmatterSchema.Type;

export const TargetSchema = S.Struct({
  id: StableId,
  repository: NonEmptyString,
  root: S.Literal("."),
  agent_instructions: PathList,
  invariants: PathList,
  product_specs: PathList,
  assurance_specs: PathList,
  roadmap_authorities: PathList,
  artifact_paths: S.Struct({
    studies: RelativePath,
    gaps: RelativePath,
    candidates: RelativePath,
    receipts: RelativePath,
  }),
});
export type FastFollowTarget = typeof TargetSchema.Type;

export const LessonSchema = S.Struct({
  id: StableId,
  kind: S.Literals([
    "architecture",
    "product_ux",
    "protocol",
    "reliability",
    "security",
    "release",
    "extension",
    "evaluation",
    "economics",
  ]),
  summary: NonEmptyString,
  stance: S.Literals(["study", "adapt", "adapt_with_stronger_boundaries", "reject"]),
});
export const SourceSchema = S.Struct({
  id: StableId,
  title: NonEmptyString,
  role: S.Literals(["upstream", "local_synthesis"]),
  access: S.Literals(["public_source", "public_artifact", "installed_artifact", "mixed"]),
  canonical_ref: NonEmptyString,
  tracking_policy: S.Literals(["pinned_each_run", "manual_snapshot", "release_or_commit"]),
  teardown_refs: PathList,
  lessons: S.Array(LessonSchema),
});
export const SourcesSchema = S.Array(SourceSchema);
export type FastFollowSource = typeof SourceSchema.Type;

export const DirectiveSchema = S.Struct({
  id: StableId,
  title: NonEmptyString,
  priority: S.Number,
  source_refs: StringList,
  target_scopes: PathList,
  desired_outcome: NonEmptyString,
  work_products: S.Array(
    S.Literals([
      "study_packet",
      "gap_assessment",
      "candidate_issue",
      "product_spec_delta",
      "assurance_delta",
      "implementation",
    ]),
  ),
  constraints: StringList,
  acceptance_refs: S.optionalKey(StringList),
});
export const DirectivesSchema = S.Array(DirectiveSchema);
export type FastFollowDirective = typeof DirectiveSchema.Type;

const CapacitySchema = S.Struct({
  delivery: S.Number,
  research: S.Number,
  implementation: S.Number,
});
export const WorkGenerationSchema = S.Struct({
  activation: S.Literals(["manual", "backlog_fallback", "continuous"]),
  initial_program: S.optionalKey(
    S.Struct({
      strategy_ref: RelativePath,
      directive_order: StringList,
      default_stage: S.Literals([
        "research",
        "gap_analysis",
        "candidate_proposal",
        "implementation",
        "verification",
      ]),
      advance_when: S.Literal("current_directive_terminal_or_blocked"),
      on_exhaustion: S.Literals(["stop", "return_to_catalog"]),
      implementation_admission: S.Literal("separate_target_authority_required"),
    }),
  ),
  allowed_stages: S.Array(
    S.Literals([
      "research",
      "gap_analysis",
      "candidate_proposal",
      "implementation",
      "verification",
    ]),
  ),
  selection_policy: S.Struct({
    higher_authority_precedence: S.Boolean,
    one_concrete_unit_per_turn: S.Boolean,
    dedupe_key_fields: StringList,
    no_material_delta: S.Boolean,
  }),
  capacity_profiles: S.Struct({ backlog_available: CapacitySchema, backlog_empty: CapacitySchema }),
  implementation_requirements: StringList,
});
export type FastFollowWorkGeneration = typeof WorkGenerationSchema.Type;

export const ReuseSchema = S.Struct({
  shareable_visibility: S.Literal("public_only"),
  study_packet_key_fields: StringList,
  freshness_days: S.Number,
  private_target_analysis: S.Literal("target_private_by_default"),
  cross_tenant_private_cache: S.Literal(false),
  cache_hit_means: S.Literal("reusable_evidence_not_adoption"),
});
export const GuardrailsSchema = S.Struct({ must_preserve: StringList, must_reject: StringList });
export const AuthoritySchema = S.Struct({
  allowed: StringList,
  denied: StringList,
  research_write_paths: PathList,
  implementation_requirements: StringList,
});

export const ProjectionSchema = S.Struct({
  format_version: S.Literal(FORMAT_VERSION),
  spec_id: StableId,
  revision: S.Number,
  title: NonEmptyString,
  lifecycle_state: S.Literals(["proposed", "admitted", "superseded", "retired"]),
  target: TargetSchema,
  sources: SourcesSchema,
  directives: DirectivesSchema,
  work_generation: WorkGenerationSchema,
  reuse: ReuseSchema,
  guardrails: GuardrailsSchema,
  authority: AuthoritySchema,
});
export type FastFollowProjection = typeof ProjectionSchema.Type;

export const BLOCKS = [
  ["Target", "fastfollow-target", TargetSchema, "target"],
  ["Sources", "fastfollow-sources", SourcesSchema, "sources"],
  ["Learning Directives", "fastfollow-directives", DirectivesSchema, "directives"],
  ["Work Generation", "fastfollow-work-generation", WorkGenerationSchema, "work_generation"],
  ["Reuse and Evidence", "fastfollow-reuse", ReuseSchema, "reuse"],
  ["Guardrails", "fastfollow-guardrails", GuardrailsSchema, "guardrails"],
  ["Authority Boundaries", "fastfollow-authority", AuthoritySchema, "authority"],
] as const;
