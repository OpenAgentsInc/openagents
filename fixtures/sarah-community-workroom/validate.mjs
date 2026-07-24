#!/usr/bin/env node
/**
 * SARAH-CW-00 fixture validator.
 * Plain Node. No package imports.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));

const GRANT_SCHEMA = "openagents.sarah.community_work_unit_grant.v1";
const XP_NAMESPACE = "com.openagents.xp";
const HEX64_RE = /^[0-9a-f]{64}$/;

const EXPECTED_SCORING = {
  accepted_work_unit_tier_1: 10,
  accepted_work_unit_tier_2: 20,
  accepted_work_unit_tier_3: 40,
  accepted_independent_verification: 5,
  reproduced_defect: 8,
  accepted_review_of_member_result: 3,
  first_accepted_unit_new_job_type: 5,
};

const AUTHORITY_LAYERS = [
  ["sarah_tick", "openagents_turn_service", "sarah_admitted_profile"],
  ["decomposition", "openagents_turn_service", "sarah_profile_bounded"],
  ["work_unit", "community_agent_own_compute", "work_unit_narrow_grant"],
  ["acceptance", "sarah", "sarah_admitted_profile"],
  ["settlement", "platform_ledger", "neither_sarah_nor_community_agent"],
];

const failures = [];

const fail = (id, message) => {
  failures.push(`${id}: ${message}`);
};

const load = (rel) => JSON.parse(readFileSync(join(root, rel), "utf8"));

const isNonEmptyString = (value) => typeof value === "string" && value.length > 0;

const assertGrantShape = (id, grant, { requireValid = true } = {}) => {
  if (!grant || typeof grant !== "object") {
    fail(id, "grant object required");
    return false;
  }
  if (grant.schema !== GRANT_SCHEMA) {
    if (requireValid) fail(id, `grant.schema must be ${GRANT_SCHEMA}`);
  }
  const missing = [];
  for (const field of [
    "unitRef",
    "groupId",
    "targetRef",
    "allowedActions",
    "budget",
    "expiresAt",
    "idempotencyId",
    "tier",
    "authorityClass",
  ]) {
    if (grant[field] === undefined || grant[field] === null) missing.push(field);
  }
  return { grant, missing };
};

const assertAcceptGroupIdentity = (id, fixture) => {
  const g = fixture.group;
  if (!g) return fail(id, "missing group");
  if (g.protocol !== "nip29") fail(id, "protocol must be nip29");
  if (g.metadataKind !== 39000) fail(id, "metadataKind must be 39000");
  if (g.adminsKind !== 39001) fail(id, "adminsKind must be 39001");
  if (g.membersKind !== 39002) fail(id, "membersKind must be 39002");
  if (g.globalDirectory !== false) fail(id, "globalDirectory must be false");
  if (g.membershipGate !== "invitation_only") fail(id, "membershipGate must be invitation_only");
  if (!Array.isArray(g.forbiddenProtocols) || !g.forbiddenProtocols.includes("nip28")) {
    fail(id, "forbiddenProtocols must include nip28");
  }
  if (!Array.isArray(g.forbiddenProtocols) || !g.forbiddenProtocols.includes("nip72")) {
    fail(id, "forbiddenProtocols must include nip72");
  }
};

const assertAcceptMembership = (id, fixture) => {
  const m = fixture.member;
  if (!m) return fail(id, "missing member");
  if (m.invited !== true) fail(id, "member must be invited");
  if (m.anonymous === true) fail(id, "member must not be anonymous");
  if (!HEX64_RE.test(m.pubkey ?? "")) fail(id, "member pubkey must be 64 hex");
  if (!isNonEmptyString(m.invitationRef)) fail(id, "invitationRef required");
};

const assertAcceptAgent = (id, fixture) => {
  const a = fixture.agent;
  if (!a) return fail(id, "missing agent");
  if (a.anonymous !== false) fail(id, "agent must not be anonymous");
  if (a.attestation !== "nip_oa") fail(id, "attestation must be nip_oa");
  if (a.auth !== "nip_aa_over_nip42") fail(id, "auth must be nip_aa_over_nip42");
  if (!HEX64_RE.test(a.pubkey ?? "")) fail(id, "agent pubkey must be 64 hex");
  if (!HEX64_RE.test(a.operatorPubkey ?? "")) fail(id, "operator pubkey must be 64 hex");
  if (a.pubkey === a.operatorPubkey) fail(id, "agent key must differ from operator key");
  if (a.openagentsHoldsProviderKey !== false) {
    fail(id, "openagents must not hold provider key");
  }
  if (a.openagentsMutatesAgentHome !== false) {
    fail(id, "openagents must not mutate agent home");
  }
};

const assertAcceptGrant = (id, fixture) => {
  const { grant, missing } = assertGrantShape(id, fixture.grant);
  if (!grant) return;
  if (missing.length > 0) fail(id, `grant missing fields: ${missing.join(",")}`);
  if (!Array.isArray(grant.allowedActions) || grant.allowedActions.length < 1) {
    fail(id, "allowedActions must be non-empty");
  }
  if (grant.allowedActions?.includes("*")) fail(id, "wildcard allowedActions forbidden");
  if (grant.authorityClass !== "work_unit_narrow_grant") {
    fail(id, "authorityClass must be work_unit_narrow_grant");
  }
  if (grant.sarahProfileGrant !== false && grant.sarahProfileGrant !== undefined) {
    fail(id, "sarahProfileGrant must be false or absent");
  }
  if (![1, 2, 3].includes(grant.tier)) fail(id, "tier must be 1, 2, or 3");
  if (typeof grant.expiresAt !== "number" || grant.expiresAt <= 0) {
    fail(id, "expiresAt must be positive unix seconds");
  }
  if (!isNonEmptyString(grant.idempotencyId)) fail(id, "idempotencyId required");
};

const assertAcceptLifecycle = (id, fixture) => {
  const steps = fixture.lifecycle;
  if (!Array.isArray(steps) || steps.length < 7) {
    fail(id, "lifecycle must have at least 7 steps");
    return;
  }
  const names = steps.map((s) => s.name);
  for (const required of [
    "publish_request",
    "publish_quote",
    "accept_one_quote",
    "execute_on_operator_compute",
    "publish_result",
    "independent_verify",
    "accept_or_reject",
  ]) {
    if (!names.includes(required)) fail(id, `lifecycle missing step ${required}`);
  }
  const workUnitSteps = steps.filter((s) => s.actor === "community_agent");
  for (const step of workUnitSteps) {
    if (step.authority !== "work_unit_narrow_grant") {
      fail(id, `community agent step ${step.name} must use work_unit_narrow_grant`);
    }
  }
  const relay = fixture.relay_authority;
  if (!relay) return fail(id, "relay_authority required");
  for (const key of [
    "identity",
    "assignment",
    "escrow",
    "acceptance",
    "payment",
    "settlement",
  ]) {
    if (relay[key] !== false) fail(id, `relay_authority.${key} must be false`);
  }
};

const assertAcceptXp = (id, fixture) => {
  const a = fixture.award;
  if (!a) return fail(id, "missing award");
  if (a.kind !== 1985) fail(id, "award kind must be 1985");
  if (a.namespace !== XP_NAMESPACE) fail(id, `namespace must be ${XP_NAMESPACE}`);
  if (a.publisherClass !== "openagents_award_key") {
    fail(id, "publisherClass must be openagents_award_key");
  }
  if (a.selfAuthoredByEarner === true) fail(id, "earner must not self-author award");
  if (a.transferable === true) fail(id, "experience must not transfer");
  if (a.redeemable === true) fail(id, "experience must not redeem");
  if (a.calledEarnings === true) fail(id, "must not call experience earnings");
  if (!HEX64_RE.test(a.workEventId ?? "")) fail(id, "workEventId must be 64 hex");
  if (!isNonEmptyString(a.acceptanceReceiptRef)) {
    fail(id, "acceptanceReceiptRef required");
  }
  if (typeof a.points !== "number" || a.points <= 0) fail(id, "points must be positive");
};

const assertAcceptRank = (id, fixture) => {
  if (!Array.isArray(fixture.awards) || fixture.awards.length < 1) {
    fail(id, "awards required");
    return;
  }
  const sum = fixture.awards.reduce((acc, a) => acc + (a.points ?? 0), 0);
  if (fixture.recomputedTotal !== sum) {
    fail(id, "recomputedTotal must equal award sum");
  }
  const r = fixture.rank;
  if (!r) return fail(id, "missing rank");
  if (r.kind !== 30382) fail(id, "rank kind must be 30382");
  if (r.publisherClass !== "openagents_scorer_key") {
    fail(id, "rank publisher must be openagents_scorer_key");
  }
  if (r.isProjection !== true) fail(id, "rank must be a projection");
  if (r.onDisagreement !== "awards_win") fail(id, "onDisagreement must be awards_win");
  if (r.rankValue !== sum) fail(id, "rankValue must equal award sum");
};

const assertAcceptScoring = (id, fixture) => {
  const table = fixture.table;
  if (!table) return fail(id, "missing table");
  for (const [key, points] of Object.entries(EXPECTED_SCORING)) {
    if (table[key] !== points) fail(id, `scoring ${key} must be ${points}`);
  }
  const rules = fixture.rules;
  if (!rules) return fail(id, "missing rules");
  if (rules.hiddenWeights !== false) fail(id, "hiddenWeights forbidden");
  if (rules.modelInLoop !== false) fail(id, "modelInLoop forbidden");
  if (rules.decayInV1 !== false) fail(id, "decayInV1 must be false");
  if (rules.multipliesPayment !== false) fail(id, "multipliesPayment forbidden");
  if (rules.awardOnVolume !== false) fail(id, "awardOnVolume forbidden");
  if (rules.awardOnAcceptedOutcomesOnly !== true) {
    fail(id, "awardOnAcceptedOutcomesOnly required");
  }
};

const assertAcceptTwoRoom = (id, fixture) => {
  const p = fixture.private_room;
  const c = fixture.community_room;
  const checks = fixture.checks;
  if (!p || !c || !checks) return fail(id, "two-room fields required");
  if (p.conversationTag === c.groupId) {
    fail(id, "group id must not equal private conversation tag");
  }
  if (checks.groupIdEqualsConversationTag !== false) {
    fail(id, "groupIdEqualsConversationTag must be false");
  }
  if (checks.membershipSetsIntersect !== false) {
    fail(id, "membershipSetsIntersect must be false");
  }
  if (checks.sharedHistoryStream !== false) {
    fail(id, "sharedHistoryStream must be false");
  }
  if (checks.privateTurnCarriesCommunityGroupTag !== false) {
    fail(id, "privateTurnCarriesCommunityGroupTag must be false");
  }
  if (checks.communityMembershipReferencesPrivateConversation !== false) {
    fail(id, "communityMembershipReferencesPrivateConversation must be false");
  }
  if (checks.crossRoomPublicationRequiresNewEvent !== true) {
    fail(id, "crossRoomPublicationRequiresNewEvent must be true");
  }
  const privateSet = new Set(p.members ?? []);
  for (const m of c.members ?? []) {
    if (privateSet.has(m)) fail(id, "membership sets must not intersect in oracle");
  }
};

const assertAcceptAuthorityTable = (id, fixture) => {
  const layers = fixture.layers;
  if (!Array.isArray(layers) || layers.length !== AUTHORITY_LAYERS.length) {
    fail(id, "authority layers count mismatch");
    return;
  }
  for (let i = 0; i < AUTHORITY_LAYERS.length; i++) {
    const [layer, who, authority] = AUTHORITY_LAYERS[i];
    if (layers[i]?.layer !== layer) fail(id, `layer[${i}] must be ${layer}`);
    if (layers[i]?.who !== who) fail(id, `layer[${i}].who must be ${who}`);
    if (layers[i]?.authority !== authority) {
      fail(id, `layer[${i}].authority must be ${authority}`);
    }
  }
  const f = fixture.forbidden;
  if (!f) return fail(id, "forbidden map required");
  if (f.communityAgentRunsSarahTick !== false) {
    fail(id, "communityAgentRunsSarahTick must be false");
  }
  if (f.unitCarriesSarahFullProfile !== false) {
    fail(id, "unitCarriesSarahFullProfile must be false");
  }
  if (f.acceptanceSettlesMoney !== false) fail(id, "acceptanceSettlesMoney must be false");
  if (f.relaySettlesMoney !== false) fail(id, "relaySettlesMoney must be false");
};

const assertAcceptSettlement = (id, fixture) => {
  const v1 = fixture.v1;
  if (!v1) return fail(id, "missing v1");
  if (v1.paysMoney !== false) fail(id, "v1.paysMoney must be false");
  if (v1.reward !== "experience_only") fail(id, "v1.reward must be experience_only");
  if (v1.revenueShare !== false) fail(id, "revenueShare forbidden in v1");
  if (v1.bonus !== false) fail(id, "bonus forbidden in v1");
  if (v1.bitcoinPayment !== false) fail(id, "bitcoinPayment forbidden in v1");
  if (v1.copyRequiresNoPayDisclosure !== true) {
    fail(id, "copyRequiresNoPayDisclosure required");
  }
  if (v1.impliesFuturePayment !== false) fail(id, "impliesFuturePayment forbidden");
  if (v1.callsExperienceEarnings !== false) fail(id, "callsExperienceEarnings forbidden");
  const b = fixture.boundaries;
  if (!b) return fail(id, "missing boundaries");
  if (b.platformMoneySettlement !== false) fail(id, "platformMoneySettlement must be false");
  if (b.relaySettlementAuthority !== false) fail(id, "relaySettlementAuthority must be false");
  if (b.zapsAreSettlement !== false) fail(id, "zapsAreSettlement must be false");
};

const assertAccept = (id, fixture) => {
  if (fixture.expect !== "accept") fail(id, "canonical fixture must set expect=accept");
  switch (fixture.contract_rule) {
    case "group_identity_nip29":
      return assertAcceptGroupIdentity(id, fixture);
    case "membership_invitation_only":
      return assertAcceptMembership(id, fixture);
    case "agent_attestation_required":
      return assertAcceptAgent(id, fixture);
    case "work_unit_narrow_grant":
      return assertAcceptGrant(id, fixture);
    case "nip_lbr_lifecycle":
      return assertAcceptLifecycle(id, fixture);
    case "xp_namespace_and_award":
      return assertAcceptXp(id, fixture);
    case "rank_recomputable_from_awards":
      return assertAcceptRank(id, fixture);
    case "scoring_function_v1":
      return assertAcceptScoring(id, fixture);
    case "two_room_rule":
      return assertAcceptTwoRoom(id, fixture);
    case "authority_table":
      return assertAcceptAuthorityTable(id, fixture);
    case "settlement_v1_experience_only":
      return assertAcceptSettlement(id, fixture);
    default:
      fail(id, `unknown contract_rule ${fixture.contract_rule}`);
  }
};

const assertRejectHasDefect = (id, fixture) => {
  if (fixture.expect !== "reject") fail(id, "negative fixture must set expect=reject");
  if (!isNonEmptyString(fixture.reason)) fail(id, "negative fixture must name reason");
  if (!isNonEmptyString(fixture.contract_rule)) {
    fail(id, "negative fixture must name contract_rule");
  }

  const rule = fixture.contract_rule;
  let defectFound = false;

  if (rule === "two_room_rule") {
    if (fixture.checks?.membershipSetsIntersect === true) defectFound = true;
    if (fixture.claim?.privateTurnCarriesCommunityGroupTag === true) defectFound = true;
    if (fixture.claim?.sharedHistoryStream === true) defectFound = true;
    if (
      fixture.private_room &&
      fixture.community_room &&
      Array.isArray(fixture.private_room.members) &&
      Array.isArray(fixture.community_room.members)
    ) {
      const set = new Set(fixture.private_room.members);
      if (fixture.community_room.members.some((m) => set.has(m))) defectFound = true;
    }
  }

  if (rule === "authority_table") {
    if (fixture.claim?.runsSarahTick === true) defectFound = true;
    if (fixture.claim?.authority === "sarah_admitted_profile" && fixture.claim?.actor === "community_agent") {
      defectFound = true;
    }
  }

  if (rule === "work_unit_narrow_grant") {
    const grant = fixture.grant;
    if (grant) {
      if (grant.authorityClass !== "work_unit_narrow_grant") defectFound = true;
      if (grant.sarahProfileGrant === true) defectFound = true;
      if (grant.expiresAt === undefined) defectFound = true;
      if (grant.idempotencyId === undefined) defectFound = true;
      if (
        fixture.claim?.acceptedAfterExpiry === true &&
        typeof grant.expiresAt === "number" &&
        typeof fixture.claim.nowUnix === "number" &&
        grant.expiresAt < fixture.claim.nowUnix
      ) {
        defectFound = true;
      }
    }
  }

  if (rule === "independent_verification") {
    if (fixture.claim?.operatorsDistinct === false) defectFound = true;
    if (
      fixture.claim?.producerOperatorPubkey &&
      fixture.claim.producerOperatorPubkey === fixture.claim.verifierOperatorPubkey
    ) {
      defectFound = true;
    }
  }

  if (rule === "agent_attestation_required") {
    if (fixture.agent?.anonymous === true) defectFound = true;
    if (!fixture.agent?.attestation) defectFound = true;
  }

  if (rule === "membership_invitation_only") {
    if (fixture.member?.invited === false) defectFound = true;
    if (fixture.member?.membershipGate === "open") defectFound = true;
  }

  if (rule === "rank_scorer_keys_only") {
    if (fixture.rank?.publisherClass !== "openagents_scorer_key") defectFound = true;
    if (fixture.rank?.publisherPubkey && fixture.rank.publisherPubkey === fixture.rank.earnerPubkey) {
      defectFound = true;
    }
  }

  if (rule === "xp_namespace_and_award") {
    if (!fixture.award?.workEventId || !fixture.award?.acceptanceReceiptRef) {
      defectFound = true;
    }
  }

  if (rule === "rank_recomputable_from_awards") {
    const sum = (fixture.awards ?? []).reduce((acc, a) => acc + (a.points ?? 0), 0);
    if (fixture.rank?.rankValue !== sum) defectFound = true;
    if (fixture.rank?.onDisagreement === "rank_wins") defectFound = true;
    if (fixture.recomputedTotal !== undefined && fixture.rank?.rankValue !== fixture.recomputedTotal) {
      defectFound = true;
    }
  }

  if (rule === "settlement_v1_experience_only") {
    if (fixture.claim?.paysMoney === true) defectFound = true;
    if (fixture.claim?.calledEarnings === true) defectFound = true;
    if (fixture.claim?.displayLabel === "earnings") defectFound = true;
    if (fixture.claim?.platformMoneySettlement === true) defectFound = true;
  }

  if (rule === "settlement_boundary") {
    if (fixture.claim?.relaySettlementAuthority === true) defectFound = true;
    if (fixture.claim?.settlementCountedPerRelayObservation === true) defectFound = true;
  }

  if (!defectFound) {
    fail(id, "negative fixture does not encode a detectable contract defect");
  }
};

// --- run ---
if (manifest.namespaces?.xp !== XP_NAMESPACE) {
  fail("manifest", `xp namespace must be ${XP_NAMESPACE}`);
}
if (manifest.schemas?.work_unit_grant !== GRANT_SCHEMA) {
  fail("manifest", `work_unit_grant schema mismatch`);
}

for (const rel of manifest.fixtures ?? []) {
  const fixture = load(rel);
  const id = fixture.fixture_id ?? rel;
  assertAccept(id, fixture);
}

for (const rel of manifest.negatives ?? []) {
  const fixture = load(rel);
  const id = fixture.fixture_id ?? rel;
  assertRejectHasDefect(id, fixture);
}

if (failures.length > 0) {
  console.error("SARAH-CW-00 fixture validation FAILED");
  for (const line of failures) console.error(`  - ${line}`);
  process.exit(1);
}

console.log(
  `SARAH-CW-00 fixture validation OK (${manifest.fixtures.length} canonical, ${manifest.negatives.length} negative)`,
);
