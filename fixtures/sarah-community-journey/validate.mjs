#!/usr/bin/env node
/**
 * SARAH-CW-09 community journey receipt fixture validator.
 * Plain Node. No package imports (CI-safe without workspace install).
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const SCHEMA = "openagents.sarah.community_journey_receipt.v1";
const PACKET = "SARAH-CW-09";
const ISSUE = "OpenAgentsInc/openagents#9231";

const FORBIDDEN_KEYS = [
  "mnemonic",
  "nsec",
  "privateKey",
  "privateKeyHex",
  "privateKeyBytes",
  "seckey",
  "secretKey",
  "secretKeyHex",
  "seed",
  "seedHex",
  "rawKey",
  "SARAH_NOSTR_IDENTITY_SECRET",
];

const REQUIRED_STEP_IDS = [
  "J01_invite_outside_developer",
  "J02_developer_joins_room",
  "J03_attach_own_agent",
  "J04_relay_admits_attested_agent",
  "J05_unit_published_and_quoted",
  "J06_accept_exactly_one_quote",
  "J07_local_execute_with_evidence",
  "J08_independent_verifier",
  "J09_accept_award_and_rank",
  "J10_no_payment_room_copy",
  "J11_rejected_result_typed_appeal",
  "J12_revoked_member_loses_access",
  "J13_refuse_replay_self_verify_expired",
  "J14_credentials_home_unchanged",
  "J15_abuse_sybil_rate_limit",
  "J16_abuse_awards_accepted_only",
  "J17_abuse_prompt_injection_quoted",
  "J18_abuse_public_safe_unit_payload",
  "J19_abuse_scorer_only_rank",
  "J20_open_community_room_pane",
  "J21_developer_confirms_own_words",
];

const failures = [];
const fail = (id, message) => {
  failures.push(`${id}: ${message}`);
};

const isForbiddenName = (name) => {
  const lower = name.toLowerCase();
  return FORBIDDEN_KEYS.some(
    (f) => f.toLowerCase() === lower || lower.includes(f.toLowerCase()),
  );
};

const assertPublicSafe = (value, path = "$") => {
  if (value === null || value === undefined) return;
  if (typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((item, i) => assertPublicSafe(item, `${path}[${i}]`));
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    if (isForbiddenName(key)) {
      fail("redaction", `forbidden field ${path}.${key}`);
    }
    if (typeof child === "string" && child.startsWith("nsec1")) {
      fail("redaction", `nsec leaf at ${path}.${key}`);
    }
    assertPublicSafe(child, `${path}.${key}`);
  }
};

const validateReceipt = (id, receipt) => {
  if (receipt.schema !== SCHEMA) fail(id, `schema must be ${SCHEMA}`);
  if (receipt.packet !== PACKET) fail(id, `packet must be ${PACKET}`);
  if (receipt.issue !== ISSUE) fail(id, `issue must be ${ISSUE}`);
  if (receipt.mode !== "simulated" && receipt.mode !== "live") {
    fail(id, "mode must be simulated or live");
  }
  if (!receipt.generatedAt) fail(id, "generatedAt required");
  if (
    !receipt.candidate ||
    !["mock", "outside_developer_live"].includes(receipt.candidate.kind)
  ) {
    fail(id, "candidate.kind must be mock or outside_developer_live");
  }
  if (receipt.mode === "simulated" && receipt.candidate.kind !== "mock") {
    fail(id, "simulated mode requires candidate.kind=mock");
  }
  if (
    receipt.mode === "live" &&
    receipt.candidate.kind !== "outside_developer_live"
  ) {
    fail(id, "live mode requires candidate.kind=outside_developer_live");
  }
  if (
    !Array.isArray(receipt.steps) ||
    receipt.steps.length !== REQUIRED_STEP_IDS.length
  ) {
    fail(id, `steps must have length ${REQUIRED_STEP_IDS.length}`);
  } else {
    for (let i = 0; i < REQUIRED_STEP_IDS.length; i += 1) {
      const step = receipt.steps[i];
      if (step?.id !== REQUIRED_STEP_IDS[i]) {
        fail(id, `step[${i}] id must be ${REQUIRED_STEP_IDS[i]}`);
      }
      if (
        !["passed", "failed", "skipped_human", "not_run"].includes(step.status)
      ) {
        fail(id, `step ${step.id} has invalid status`);
      }
      if (
        step.class === "human" &&
        receipt.mode === "simulated" &&
        step.status === "passed"
      ) {
        fail(id, `human step ${step.id} must not pass in simulated mode`);
      }
    }
  }
  if (!receipt.redaction?.ok || !receipt.redaction?.forbiddenFieldsScanned) {
    fail(id, "redaction.ok and forbiddenFieldsScanned required");
  }
  if (!receipt.independentReviewer?.checklist?.length) {
    fail(id, "independentReviewer.checklist required");
  }
  if (!receipt.summary || typeof receipt.summary.automatedPassed !== "number") {
    fail(id, "summary.automatedPassed required");
  }
  if (
    receipt.mode === "simulated" &&
    receipt.summary.automatedFailed === 0 &&
    receipt.summary.overall !== "simulated_green"
  ) {
    fail(id, "simulated green receipt must report overall=simulated_green");
  }
  assertPublicSafe(receipt, id);
};

const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));
for (const entry of manifest.receipts) {
  const receipt = JSON.parse(readFileSync(join(root, entry.path), "utf8"));
  validateReceipt(entry.id, receipt);
}

// Negative: secret field must be rejected by local redaction walk
const negative = JSON.parse(
  readFileSync(join(root, "negative/secret-field.json"), "utf8"),
);
const before = failures.length;
assertPublicSafe(negative, "negative.secret-field");
if (failures.length === before) {
  fail("negative.secret-field", "expected forbidden field detection");
} else {
  // expected failures for the negative vector — drop them as success signal
  while (failures.length > before) failures.pop();
}

if (failures.length > 0) {
  console.error("SARAH-CW-09 journey fixture validation FAILED:");
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}

console.log(
  `SARAH-CW-09 journey fixtures OK (${manifest.receipts.length} receipt(s), negative redaction checked)`,
);
