import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const profilePath = path.join(repoRoot, "AUTHORITY.md");
const source = readFileSync(profilePath, "utf8");

const readFrontmatterString = (key: string): string => {
  const frontmatter = /^---\n([\s\S]*?)\n---/.exec(source)?.[1];
  if (frontmatter === undefined) throw new Error("AUTHORITY.md has no frontmatter");
  const match = new RegExp(`^${key}:\\s+["']?([^"'\\n]+)["']?$`, "m").exec(frontmatter);
  if (match?.[1] === undefined) throw new Error(`AUTHORITY.md frontmatter is missing ${key}`);
  return match[1].trim();
};

const readFrontmatterInteger = (key: string): number => {
  const value = Number(readFrontmatterString(key));
  if (!Number.isSafeInteger(value)) throw new Error(`${key} is not an integer`);
  return value;
};

const readBlock = <A>(label: string): A => {
  const match = new RegExp("```" + label + "\\n([\\s\\S]*?)\\n```", "g").exec(source);
  if (match?.[1] === undefined) throw new Error(`AUTHORITY.md is missing ${label}`);
  return JSON.parse(match[1]) as A;
};

type Order = Readonly<{
  authority_may_amplify: boolean;
  explicit_deny_wins: boolean;
  composition: string;
  precedence: ReadonlyArray<string>;
}>;

type Program = Readonly<{
  id: string;
  order: number;
  status: string;
  outcome: string;
  authority_refs: ReadonlyArray<string>;
  advance_when: string;
}>;

type Grant = Readonly<{
  id: string;
  roles: ReadonlyArray<string>;
  actions: ReadonlyArray<string>;
  resources: ReadonlyArray<string>;
  program_refs: ReadonlyArray<string>;
  condition_refs: ReadonlyArray<string>;
}>;

type Condition = Readonly<{
  id: string;
  rule: string;
  currency?: string;
  max_incremental_spend_per_day?: number;
  max_new_recurring_spend_per_month?: number;
  max_external_campaign_or_subscription_spend?: number;
  max_transfer?: number;
}>;

type Independence = Readonly<{
  producer_may_verify_own_obligation: boolean;
  producer_may_admit_own_assurance_revision: boolean;
  producer_may_release_from_own_evidence_alone: boolean;
  owner_designated_independent_reviewer_role: string;
  minimum_independent_identity: string;
}>;

type Escalation = Readonly<{
  waiting_is_terminal: boolean;
  steps: ReadonlyArray<string>;
  needs_owner_entry_requires: ReadonlyArray<string>;
}>;

type Reserved = Readonly<{
  id: string;
  category: string;
}>;

type Receipts = Readonly<{
  schema_id: string;
  required_fields: ReadonlyArray<string>;
  outcomes: ReadonlyArray<string>;
  public_safe_only: boolean;
  raw_secrets_forbidden: boolean;
  private_evidence_by_reference_only: boolean;
}>;

const order = readBlock<Order>("authority-delegation-order");
const programs = readBlock<ReadonlyArray<Program>>("authority-delegation-programs");
const grants = readBlock<ReadonlyArray<Grant>>("authority-delegation-grants");
const conditions = readBlock<ReadonlyArray<Condition>>("authority-delegation-conditions");
const independence = readBlock<Independence>("authority-delegation-independence");
const escalation = readBlock<Escalation>("authority-delegation-escalation");
const reserved = readBlock<ReadonlyArray<Reserved>>("authority-delegation-reserved");
const receipts = readBlock<Receipts>("authority-delegation-receipts");

const unique = (values: ReadonlyArray<string>): boolean => new Set(values).size === values.length;

describe("OpenAgents AuthorityDelegationSpec 0.1 root profile", () => {
  test("is an admitted, revisioned, non-amplifying owner profile", () => {
    expect(readFrontmatterString("authority_delegation_format_version")).toBe("0.1");
    expect(readFrontmatterString("authority_profile_id")).toBe(
      "openagents.owner-delegated-autonomy",
    );
    expect(readFrontmatterInteger("authority_revision")).toBe(6);
    expect(readFrontmatterString("lifecycle_state")).toBe("admitted");
    expect(readFrontmatterString("admitted_by")).toBe(
      "current_owner_direction_2026-07-19_managed_sandbox_delivery",
    );
    expect(order).toMatchObject({
      authority_may_amplify: false,
      explicit_deny_wins: true,
      composition: "intersection",
    });
    expect(order.precedence[0]).toBe("system_and_current_owner_instruction");
    expect(order.precedence.at(-1)).toBe("fastfollowspec_and_transcript_evidence");
  });

  test("orders Full Auto, managed sandboxes, root specs, Fast Follow, then growth and revenue", () => {
    expect(programs.map(({ id, order: programOrder }) => [id, programOrder])).toEqual([
      ["program.full_auto_release", 1],
      ["program.managed_agent_sandboxes", 2],
      ["program.root_specs", 3],
      ["program.fast_follow_full_harvest", 4],
      ["program.promise_growth_revenue", 5],
    ]);
    expect(programs[0]?.status).toBe("active");
    expect(unique(programs.map(({ id }) => id))).toBe(true);
    for (const program of programs) {
      expect(program.outcome.length).toBeGreaterThan(40);
      expect(program.authority_refs.length).toBeGreaterThan(0);
      expect(program.advance_when.length).toBeGreaterThan(10);
    }
  });

  test("resolves every grant reference exactly and keeps identities unique", () => {
    expect(unique(grants.map(({ id }) => id))).toBe(true);
    expect(unique(conditions.map(({ id }) => id))).toBe(true);
    expect(unique(reserved.map(({ id }) => id))).toBe(true);

    const programIds = new Set(programs.map(({ id }) => id));
    const conditionIds = new Set(conditions.map(({ id }) => id));
    for (const grant of grants) {
      expect(grant.roles.length, grant.id).toBeGreaterThan(0);
      expect(grant.actions.length, grant.id).toBeGreaterThan(0);
      expect(grant.resources.length, grant.id).toBeGreaterThan(0);
      expect(unique(grant.roles), grant.id).toBe(true);
      expect(unique(grant.actions), grant.id).toBe(true);
      expect(unique(grant.resources), grant.id).toBe(true);
      for (const ref of grant.program_refs)
        expect(programIds.has(ref), `${grant.id}:${ref}`).toBe(true);
      for (const ref of grant.condition_refs) {
        expect(conditionIds.has(ref), `${grant.id}:${ref}`).toBe(true);
      }
    }
  });

  test("uses conservative bootstrap budgets and zero financial movement", () => {
    const byId = new Map(conditions.map((condition) => [condition.id, condition]));
    expect(byId.get("condition.cloud_budget")).toMatchObject({
      currency: "USD",
      max_incremental_spend_per_day: 100,
      max_new_recurring_spend_per_month: 100,
    });
    expect(byId.get("condition.zero_external_spend")).toMatchObject({
      currency: "USD",
      max_external_campaign_or_subscription_spend: 0,
    });
    expect(byId.get("condition.no_financial_movement")).toMatchObject({
      currency: "USD",
      max_transfer: 0,
    });
  });

  test("admits autonomous RC publication and bounded release communication", () => {
    const grant = grants.find(({ id }) => id === "grant.autonomous_rc_release_and_communication");
    expect(grant?.roles).toEqual(["release_operator"]);
    expect(grant?.actions).toEqual(
      expect.arrayContaining([
        "classify_release_impact",
        "publish_verified_github_prerelease",
        "promote_signed_rc_release",
        "request_candidate_test",
        "ingest_release_feedback",
        "publish_release_changelog",
      ]),
    );
    expect(grant?.resources).toEqual(
      expect.arrayContaining([
        "OpenAgentsInc/openagents_rc_releases",
        "github_linked_release_issues",
        "openagents_forum_release_candidates",
      ]),
    );
    expect(grant?.condition_refs).toEqual(
      expect.arrayContaining([
        "condition.autonomous_rc_only",
        "condition.release_impact",
        "condition.release_communication",
        "condition.release_attribution",
      ]),
    );
  });

  test("designates Sarah as a brokered, citation-bound, non-amplifying orchestrator", () => {
    const grant = grants.find(({ id }) => id === "grant.sarah_company_orchestration");
    expect(grant?.roles).toEqual(["sarah_orchestrator"]);
    expect(grant?.actions).toEqual(
      expect.arrayContaining([
        "maintain_owner_contact",
        "read_business_context",
        "prioritize_program_work",
        "delegate_repository_work",
        "inspect_owner_coding_capacity",
        "dispatch_owner_capacity_coding_workers",
        "inspect_existing_full_auto_run",
        "control_existing_full_auto_run",
        "inspect_own_harness",
        "review_own_terminal_history_and_propose_harness",
        "publish_release_candidate",
      ]),
    );
    expect(grant?.condition_refs).toEqual(
      expect.arrayContaining([
        "condition.owner_scope",
        "condition.capability_broker",
        "condition.citations",
        "condition.existing_runtime_gate",
      ]),
    );
    expect(grant?.resources).toEqual(
      expect.arrayContaining([
        "owner_linked_pylon_coding_capacity",
        "owner_full_auto_runs",
        "owner_private_sarah_harness",
      ]),
    );
    expect(grant?.actions).not.toEqual(
      expect.arrayContaining([
        "start_full_auto_run",
        "read_memoharness_private_bank",
        "promote_harness_candidate",
        "increase_own_authority",
      ]),
    );
  });

  test("admits only Sarah's closed managed-sandbox vocabulary behind scope, budget, and runtime gates", () => {
    const grant = grants.find(({ id }) => id === "grant.sarah_managed_sandbox");
    expect(grant).toMatchObject({
      roles: ["sarah_orchestrator"],
      resources: ["authenticated_owner_openagents_managed_sandboxes"],
      program_refs: ["program.managed_agent_sandboxes"],
    });
    expect(grant?.actions).toEqual([
      "create_managed_sandbox",
      "list_managed_sandboxes",
      "inspect_managed_sandbox",
      "dispatch_managed_sandbox_work",
      "interrupt_managed_sandbox_turn",
      "stop_managed_sandbox",
      "resume_managed_sandbox",
      "delete_managed_sandbox",
    ]);
    expect(grant?.condition_refs).toEqual(
      expect.arrayContaining([
        "condition.managed_sandbox_scope",
        "condition.managed_sandbox_budget",
        "condition.managed_sandbox_runtime_admission",
      ]),
    );
    expect(grant?.actions).not.toEqual(
      expect.arrayContaining(["run_gcloud", "operate_generic_container", "start_full_auto_run"]),
    );
  });

  test("requires real reviewer independence", () => {
    expect(independence).toMatchObject({
      producer_may_verify_own_obligation: false,
      producer_may_admit_own_assurance_revision: false,
      producer_may_release_from_own_evidence_alone: false,
      owner_designated_independent_reviewer_role: "independent_reviewer",
    });
    expect(independence.minimum_independent_identity).toContain("distinct_clean_session");
    expect(grants.find(({ id }) => id === "grant.independent_assurance")?.actions).toContain(
      "admit_assurance_revision_when_source_spec_allows_owner_designated_independent_reviewer",
    );
  });

  test("has a non-waiting escalation ladder and exact owner exception fields", () => {
    expect(escalation.waiting_is_terminal).toBe(false);
    expect(escalation.steps).toHaveLength(8);
    expect(escalation.steps[0]).toBe("verify_live_blocker_and_stale_claim_state");
    expect(escalation.steps.at(-1)).toBe(
      "record_one_irreducible_reserved_owner_action_and_continue_other_work",
    );
    expect(escalation.needs_owner_entry_requires).toEqual([
      "reserved_category",
      "exact_target",
      "attempted_steps",
      "smallest_ui_action",
      "work_still_in_motion",
      "closure_receipt",
    ]);
  });

  test("reserves secrets, custody, legal identity, destruction, over-budget work, and false claims", () => {
    const requiredReservedIds = [
      "reserved.secret_export",
      "reserved.financial_custody",
      "reserved.legal_people",
      "reserved.customer_data_destruction",
      "reserved.human_identity",
      "reserved.over_budget",
      "reserved.invariant_weakening",
      "reserved.unsupported_claim",
      "reserved.self_amplification",
      "reserved.stable_release_without_direction",
    ];
    expect(reserved.map(({ id }) => id).toSorted()).toEqual(requiredReservedIds.toSorted());
    for (const item of reserved) expect(item.category.length, item.id).toBeGreaterThan(30);
  });

  test("requires bounded redacted authority receipts", () => {
    expect(receipts).toMatchObject({
      schema_id: "openagents.authority_decision_receipt.v1",
      public_safe_only: true,
      raw_secrets_forbidden: true,
      private_evidence_by_reference_only: true,
    });
    expect(receipts.required_fields).toEqual(
      expect.arrayContaining([
        "profile_revision",
        "program_ref",
        "grant_ref",
        "action",
        "target_ref",
        "condition_results",
        "outcome",
        "evidence_refs",
      ]),
    );
    expect(receipts.outcomes).toContain("needs_owner_reserved_action");
  });

  test("is linked from repository law, product intent, and the accepted plan", () => {
    const agents = readFileSync(path.join(repoRoot, "AGENTS.md"), "utf8");
    const invariants = readFileSync(path.join(repoRoot, "INVARIANTS.md"), "utf8");
    const productSpec = readFileSync(
      path.join(repoRoot, "specs/openagents/authority-delegation.product-spec.md"),
      "utf8",
    );
    const acceptedPlan = readFileSync(
      path.join(repoRoot, "docs/sol/2026-07-18-owner-delegated-autonomy-accepted-plan.md"),
      "utf8",
    );
    const design = readFileSync(
      path.join(repoRoot, "docs/authority/AUTHORITY_DELEGATION_SPEC.md"),
      "utf8",
    );

    for (const document of [agents, invariants, productSpec, acceptedPlan, design]) {
      expect(document).toContain("AUTHORITY.md");
    }
    expect(agents).toContain("## Delegated Authority");
    expect(invariants).toContain("Delegated action authority is explicit");
    expect(productSpec).toContain("AD-AC-20");
  });
});
