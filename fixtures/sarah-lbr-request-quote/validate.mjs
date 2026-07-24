#!/usr/bin/env node
/**
 * SARAH-CW-04 LBR request/quote fixture validator.
 * Plain Node. No package imports (CI-safe without workspace install).
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const SCHEMA = "openagents.sarah.lbr_request_quote.v1";
const PACKET = "SARAH-CW-04";
const ISSUE = "OpenAgentsInc/openagents#9228";
const SETTLEMENT = "no_spend";
const PUBLIC_REF =
  /^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/;
const FORBIDDEN_GRANT_PREFIXES = [
  "grant.sarah.",
  "principal.sarah",
  "capability.sarah.",
  "authority.sarah.",
  "role.sarah_orchestrator",
];
const UNSAFE =
  /(ANTHROPIC_API_KEY|OPENAI_API_KEY|SECRET|TOKEN=|-----BEGIN|mnemonic|payment_hash|payment_preimage|preimage|lnbc|file:\/\/|\/Users\/|\/home\/|raw prompt)/iu;

const failures = [];
const fail = (id, message) => {
  failures.push(`${id}: ${message}`);
};

const readJson = (rel) =>
  JSON.parse(readFileSync(join(root, rel), "utf8"));

const isForbiddenGrant = (ref) => {
  const lower = String(ref).toLowerCase();
  return FORBIDDEN_GRANT_PREFIXES.some(
    (p) => lower === p || lower.startsWith(p.toLowerCase()),
  );
};

const assertPublicRef = (id, field, value) => {
  if (typeof value !== "string" || !PUBLIC_REF.test(value)) {
    fail(id, `${field} must be a public-safe ref`);
    return;
  }
  if (UNSAFE.test(value) || isForbiddenGrant(value)) {
    fail(id, `${field} failed public-safe / grant fence`);
  }
};

const assertPublicSafeTree = (id, value, path = "$") => {
  if (value === null || value === undefined) return;
  if (typeof value === "string") {
    if (UNSAFE.test(value) || value.startsWith("nsec1")) {
      fail(id, `unsafe material at ${path}`);
    }
    return;
  }
  if (typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, i) => assertPublicSafeTree(id, item, `${path}[${i}]`));
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    const lower = key.toLowerCase();
    if (
      ["mnemonic", "nsec", "privatekey", "preimage", "bolt11", "invoice"].some(
        (f) => lower.includes(f),
      )
    ) {
      fail(id, `forbidden field ${path}.${key}`);
    }
    assertPublicSafeTree(id, child, `${path}.${key}`);
  }
};

const manifest = readJson("manifest.json");
if (manifest.schema !== SCHEMA) fail("manifest", `schema must be ${SCHEMA}`);
if (manifest.packet !== PACKET) fail("manifest", `packet must be ${PACKET}`);
if (manifest.issue !== ISSUE) fail("manifest", `issue must be ${ISSUE}`);
if (manifest.settlement_mode_v1 !== SETTLEMENT) {
  fail("manifest", `settlement_mode_v1 must be ${SETTLEMENT}`);
}
if (manifest.kinds?.request !== 5934 || manifest.kinds?.quote_feedback !== 7000) {
  fail("manifest", "kinds must pin request 5934 and quote feedback 7000");
}

for (const rel of manifest.fixtures ?? []) {
  const fixture = readJson(rel);
  const id = rel;
  if (fixture.schema !== SCHEMA) fail(id, `schema must be ${SCHEMA}`);
  if (fixture.settlementMode !== SETTLEMENT) {
    fail(id, `settlementMode must be ${SETTLEMENT}`);
  }
  if (fixture.content !== "") fail(id, "content must be empty");
  assertPublicSafeTree(id, fixture);

  if (fixture.role === "work_request") {
    if (fixture.kind !== 5934) fail(id, "work_request kind must be 5934");
    if (fixture.jobType !== "code_task") fail(id, "jobType must be code_task");
    assertPublicRef(id, "objectiveRef", fixture.objectiveRef);
    assertPublicRef(id, "workUnit.grantRef", fixture.workUnit?.grantRef);
    assertPublicRef(id, "workUnit.workUnitRef", fixture.workUnit?.workUnitRef);
    if (!Array.isArray(fixture.workUnit?.repositoryRefs) || fixture.workUnit.repositoryRefs.length < 1) {
      fail(id, "repositoryRefs required");
    }
    if (!(fixture.workUnit?.budgetMsats > 0)) fail(id, "budgetMsats must be positive");
    if (!(fixture.workUnit?.expiresAtUnix > 0)) fail(id, "expiresAtUnix required");
    if (isForbiddenGrant(fixture.workUnit?.grantRef)) {
      fail(id, "canonical grant must not be a Sarah grant");
    }
  }

  if (fixture.role === "quote") {
    if (fixture.kind !== 7000) fail(id, "quote kind must be 7000");
    if (!/^[0-9a-f]{64}$/.test(fixture.requestId ?? "")) {
      fail(id, "requestId must be 64 hex");
    }
    if (!(fixture.amountMsats > 0)) fail(id, "amountMsats must be positive");
    assertPublicRef(id, "workUnitRef", fixture.workUnitRef);
    assertPublicRef(id, "providerRef", fixture.providerRef);
    assertPublicRef(id, "quoteRef", fixture.quoteRef);
    if (fixture.status !== "payment-required" || fixture.statusExtra !== "labor_quote") {
      fail(id, "quote status must be payment-required labor_quote");
    }
  }
}

const expectedNegativeCodes = new Set([
  "sarah_grant_forbidden",
  "unsafe_material",
  "settlement_forbidden",
  "over_budget",
  "work_unit_mismatch",
  "grant_expired",
  "unsafe_content",
]);

for (const rel of manifest.negatives ?? []) {
  const fixture = readJson(rel);
  const id = rel;
  if (fixture.schema !== SCHEMA) fail(id, `schema must be ${SCHEMA}`);
  if (fixture.role !== "negative") fail(id, "role must be negative");
  if (!expectedNegativeCodes.has(fixture.code)) {
    fail(id, `unknown negative code ${fixture.code}`);
  }
  if (!fixture.reason || typeof fixture.reason !== "string") {
    fail(id, "reason required");
  }

  if (fixture.code === "sarah_grant_forbidden") {
    if (!isForbiddenGrant(fixture.workUnit?.grantRef)) {
      fail(id, "expected Sarah grant in workUnit.grantRef");
    }
  }
  if (fixture.code === "unsafe_material") {
    if (!UNSAFE.test(String(fixture.objectiveRef ?? ""))) {
      fail(id, "expected unsafe objectiveRef");
    }
  }
  if (fixture.code === "settlement_forbidden") {
    if (fixture.settlementMode === SETTLEMENT) {
      fail(id, "expected non-no_spend settlementMode");
    }
  }
  if (fixture.code === "over_budget") {
    if (!(fixture.quoteAmountMsats > fixture.requestBudgetMsats)) {
      fail(id, "expected quoteAmountMsats > requestBudgetMsats");
    }
  }
  if (fixture.code === "work_unit_mismatch") {
    if (fixture.requestWorkUnitRef === fixture.quoteWorkUnitRef) {
      fail(id, "expected mismatched work unit refs");
    }
  }
  if (fixture.code === "grant_expired") {
    if (!(fixture.nowUnix > fixture.expiresAtUnix)) {
      fail(id, "expected nowUnix > expiresAtUnix");
    }
  }
  if (fixture.code === "unsafe_content") {
    if (!fixture.content || !UNSAFE.test(fixture.content)) {
      fail(id, "expected unsafe content body");
    }
  }
}

if (failures.length > 0) {
  console.error("sarah-lbr-request-quote fixture validation FAILED:");
  for (const line of failures) console.error(`  - ${line}`);
  process.exit(1);
}

console.log(
  `sarah-lbr-request-quote fixtures OK (${manifest.fixtures.length} canonical, ${manifest.negatives.length} negative)`,
);
