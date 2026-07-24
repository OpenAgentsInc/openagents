#!/usr/bin/env node
/**
 * SARAH-NR-00 fixture validator.
 * Plain Node. No package imports.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));

const TURN_KIND = 44300;
const RECEIPT_KIND = 44301;
const EPHEMERAL_KIND = 24200;
const ENTRY_VALUES = new Set([
  "turn.started",
  "tool.call",
  "tool.result",
  "tool.error",
  "turn.finished",
  "turn.interrupted",
]);
const TURN_ALT = "OpenAgents Sarah turn record (encrypted)";
const RECEIPT_ALT = "OpenAgents Sarah authority receipt (encrypted)";
const CONVERSATION_RE = /^sarah\.[0-9a-f]{24}$/;
const HEX64_RE = /^[0-9a-f]{64}$/;

const failures = [];

const fail = (id, message) => {
  failures.push(`${id}: ${message}`);
};

const tagValues = (tags, name) =>
  (tags ?? []).filter((tag) => Array.isArray(tag) && tag[0] === name).map((tag) => tag[1]);

const eTags = (tags) =>
  (tags ?? []).filter((tag) => Array.isArray(tag) && tag[0] === "e");

const looksEncrypted = (content) =>
  typeof content === "string" &&
  content.length > 0 &&
  !content.trimStart().startsWith("{") &&
  !content.trimStart().startsWith("[");

const assertAcceptTurnOrReceipt = (id, fixture) => {
  const event = fixture.wire_event;
  if (!event) {
    fail(id, "missing wire_event");
    return;
  }
  if (event.kind !== TURN_KIND && event.kind !== RECEIPT_KIND) {
    fail(id, `kind must be ${TURN_KIND} or ${RECEIPT_KIND}`);
  }
  if (event.kind >= 20000 && event.kind <= 29999) {
    fail(id, "durable fixture must not use ephemeral kind range");
  }
  const conversation = tagValues(event.tags, "conversation");
  if (conversation.length !== 1 || !CONVERSATION_RE.test(conversation[0] ?? "")) {
    fail(id, "exactly one valid conversation tag required");
  }
  const alt = tagValues(event.tags, "alt");
  if (alt.length !== 1) {
    fail(id, "exactly one alt tag required");
  } else if (event.kind === TURN_KIND && alt[0] !== TURN_ALT) {
    fail(id, "turn-record alt text mismatch");
  } else if (event.kind === RECEIPT_KIND && alt[0] !== RECEIPT_ALT) {
    fail(id, "authority-receipt alt text mismatch");
  }
  if (tagValues(event.tags, "d").length > 0) {
    fail(id, "d tag forbidden on append-only kinds");
  }
  if (!looksEncrypted(event.content)) {
    fail(id, "content must look like ciphertext, not JSON plaintext");
  }
  if (eTags(event.tags).length < 1) {
    fail(id, "at least one causal e tag required");
  }
  const agent = tagValues(event.tags, "agent");
  if (agent.length !== 1 || agent[0] !== event.pubkey) {
    fail(id, "agent tag must equal sarah pubkey");
  }
  if (event.kind === TURN_KIND) {
    const entry = tagValues(event.tags, "entry");
    if (entry.length !== 1 || !ENTRY_VALUES.has(entry[0] ?? "")) {
      fail(id, "valid entry tag required");
    }
    const turn = tagValues(event.tags, "turn");
    if (turn.length !== 1 || !turn[0]) {
      fail(id, "turn tag required");
    }
    const payload = fixture.decrypted_payload;
    if (!payload || payload.schema !== "openagents.sarah.turn_record.v1") {
      fail(id, "turn payload schema mismatch");
    }
    if (!Array.isArray(payload?.parents) || payload.parents.length < 1) {
      fail(id, "turn payload parents required");
    }
  }
  if (event.kind === RECEIPT_KIND) {
    const receipt = tagValues(event.tags, "receipt");
    if (receipt.length !== 1 || !receipt[0]) {
      fail(id, "receipt tag required");
    }
    const payload = fixture.decrypted_payload;
    if (!payload || payload.schema !== "openagents.authority_decision_receipt.v1") {
      fail(id, "receipt payload schema mismatch");
    }
    for (const field of [
      "receiptRef",
      "profileRef",
      "profileRevision",
      "programRef",
      "actorRef",
      "actorRole",
      "action",
      "targetRef",
      "triggerRef",
      "conditionResults",
      "startedAt",
      "settledAt",
      "outcome",
      "evidenceRefs",
    ]) {
      if (payload?.[field] === undefined) {
        fail(id, `receipt payload missing ${field}`);
      }
    }
    if (!["succeeded", "refused"].includes(payload?.outcome)) {
      fail(id, "receipt outcome must be succeeded or refused");
    }
  }
};

const assertRejectReason = (id, fixture, expectedChecks) => {
  if (fixture.expect !== "reject") {
    fail(id, "negative fixture must set expect=reject");
  }
  if (!fixture.reason) {
    fail(id, "negative fixture must name reason");
  }
  const event = fixture.wire_event;
  if (fixture.claim) {
    if (fixture.claim.treat_relay_ok_as_admission !== true) {
      fail(id, "admission negative must claim treat_relay_ok_as_admission");
    }
    if (fixture.contract_rule !== "relay_acceptance_is_not_openagents_admission") {
      fail(id, "admission negative must cite contract rule");
    }
    return;
  }
  if (!event) {
    fail(id, "missing wire_event");
    return;
  }
  let matched = false;
  for (const check of expectedChecks) {
    if (check(event)) matched = true;
  }
  if (!matched) {
    fail(id, `wire_event does not exhibit expected defect (${fixture.reason})`);
  }
};

const load = (relativePath) =>
  JSON.parse(readFileSync(join(root, relativePath), "utf8"));

for (const relativePath of manifest.fixtures) {
  const fixture = load(relativePath);
  const id = fixture.fixture_id ?? relativePath;
  if (fixture.expect !== "accept") {
    fail(id, "canonical fixture must set expect=accept");
    continue;
  }
  if (relativePath.endsWith("conversation-mapping.json")) {
    if (fixture.legacy_thread_ref !== `thread.${fixture.conversation_tag_value}`) {
      fail(id, "legacy mapping must round-trip with thread. prefix");
    }
    if (!CONVERSATION_RE.test(fixture.conversation_tag_value ?? "")) {
      fail(id, "conversation tag value shape invalid");
    }
    continue;
  }
  assertAcceptTurnOrReceipt(id, fixture);
}

const negativeChecks = {
  "kind_24200_is_ephemeral_not_durable_turn_record": [
    (event) => event.kind === EPHEMERAL_KIND,
  ],
  conversation_tag_required: [(event) => tagValues(event.tags, "conversation").length === 0],
  causal_parent_required: [(event) => eTags(event.tags).length === 0],
  content_must_be_nip44_ciphertext: [(event) => !looksEncrypted(event.content)],
  nip31_alt_required: [(event) => tagValues(event.tags, "alt").length === 0],
  sarah_must_author_kind_44300: [
    (event) => {
      const agent = tagValues(event.tags, "agent")[0];
      return event.kind === TURN_KIND && agent !== undefined && event.pubkey !== agent;
    },
  ],
  replacement_forbidden_no_d_tag: [(event) => tagValues(event.tags, "d").length > 0],
  receipt_is_record_not_admission: [() => true],
};

for (const relativePath of manifest.negatives) {
  const fixture = load(relativePath);
  const id = fixture.fixture_id ?? relativePath;
  const checks = negativeChecks[fixture.reason] ?? [() => false];
  assertRejectReason(id, fixture, checks);
}

if (manifest.kinds.turn_record !== TURN_KIND || manifest.kinds.authority_receipt !== RECEIPT_KIND) {
  fail("manifest", "kind numbers drifted from contract");
}

if (failures.length > 0) {
  console.error("SARAH-NR-00 fixture validation failed:");
  for (const item of failures) console.error(`- ${item}`);
  process.exit(1);
}

console.log(
  `SARAH-NR-00 fixtures ok: ${manifest.fixtures.length} canonical, ${manifest.negatives.length} negative`,
);
