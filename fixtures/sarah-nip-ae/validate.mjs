#!/usr/bin/env node
/**
 * SARAH-NR-07a fixture validator.
 * Plain Node. No package imports.
 */
import { createHmac } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));

const ENGRAM_KIND = 30174;
const ALT = "encrypted agent memory record";
const D_TAG_DOMAIN = "agent-memory/v1/d-tag";
const COMPANION_SCHEMA = "openagents.sarah.nip_ae_companion.v1";
const MEMORY_SLUG_RE =
  /^mem\/[a-z0-9][a-z0-9_-]{0,63}(\/[a-z0-9][a-z0-9_-]{0,63})*$/;
const HEX64_RE = /^[0-9a-f]{64}$/;
const ENTITY_ID_RE = /^entity\.[0-9a-f]{24}$/;
const DIGEST_RE = /^sha256:[0-9a-f]{64}$/;
const RELATION_TYPE_RE = /^[a-z][a-z0-9_]{0,63}$/;
const ADMISSION = new Set(["admitted", "candidate", "rejected"]);
const SOURCE_ROLES = new Set(["turn_record", "tool_result", "owner_message", "import"]);
const DIRECTIONS = new Set(["out", "in", "both"]);
const FORBIDDEN_RANKING_KEYS = new Set([
  "ranking",
  "feedback_weight",
  "score",
  "weight",
  "rank",
  "embedding",
  "vector",
]);

const failures = [];

const fail = (id, message) => {
  failures.push(`${id}: ${message}`);
};

const tagValues = (tags, name) =>
  (tags ?? [])
    .filter((tag) => Array.isArray(tag) && tag[0] === name)
    .map((tag) => tag[1]);

const looksEncrypted = (content) =>
  typeof content === "string" &&
  content.length > 0 &&
  !content.trimStart().startsWith("{") &&
  !content.trimStart().startsWith("[");

const isValidSlug = (slug) => {
  if (typeof slug !== "string" || slug.length === 0 || slug.length > 255) return false;
  if (slug === "core") return true;
  return MEMORY_SLUG_RE.test(slug);
};

const deriveDTag = (conversationKeyHex, slug) => {
  const key = Buffer.from(conversationKeyHex, "hex");
  const domain = Buffer.from(D_TAG_DOMAIN, "utf8");
  const slugBytes = Buffer.from(slug, "utf8");
  const msg = Buffer.concat([domain, Buffer.from([0]), slugBytes]);
  return createHmac("sha256", key).update(msg).digest("hex");
};

const collectKeys = (value, into = new Set()) => {
  if (value === null || typeof value !== "object") return into;
  if (Array.isArray(value)) {
    for (const item of value) collectKeys(item, into);
    return into;
  }
  for (const [key, child] of Object.entries(value)) {
    into.add(key);
    collectKeys(child, into);
  }
  return into;
};

const hasForbiddenRanking = (payload) => {
  const keys = collectKeys(payload);
  for (const key of FORBIDDEN_RANKING_KEYS) {
    if (keys.has(key)) return true;
  }
  return false;
};

const assertOpenagentsCompanion = (id, body, { allowTombstone = false } = {}) => {
  if (body.slug === "core") {
    if (typeof body.profile !== "string" || body.profile.length === 0) {
      fail(id, "core body requires non-empty profile");
    }
    if (hasForbiddenRanking(body)) {
      fail(id, "ranking-like fields forbidden on core");
    }
    return;
  }

  if (!isValidSlug(body.slug) || body.slug === "core") {
    fail(id, "memory body slug invalid");
  }

  if (body.value === null) {
    if (!allowTombstone) fail(id, "unexpected tombstone");
    return;
  }

  if (typeof body.value !== "string" || body.value.length === 0) {
    fail(id, "memory value must be non-empty string or null");
  }

  const oa = body.openagents;
  if (!oa || typeof oa !== "object") {
    fail(id, "openagents companion object required");
    return;
  }
  if (oa.schema !== COMPANION_SCHEMA) {
    fail(id, "openagents.schema mismatch");
  }
  if (!ADMISSION.has(oa.admission)) {
    fail(id, "openagents.admission invalid");
  }
  if (!ENTITY_ID_RE.test(oa.entityId ?? "")) {
    fail(id, "openagents.entityId shape invalid");
  }
  if (!DIGEST_RE.test(oa.contentDigest ?? "")) {
    fail(id, "openagents.contentDigest shape invalid");
  }
  if (!Array.isArray(oa.sourceEventRefs)) {
    fail(id, "openagents.sourceEventRefs required");
  } else if (oa.admission === "admitted" && oa.sourceEventRefs.length < 1) {
    fail(id, "admitted memories require sourceEventRefs");
  } else {
    for (const ref of oa.sourceEventRefs) {
      if (!HEX64_RE.test(ref?.eventId ?? "")) {
        fail(id, "sourceEventRefs.eventId must be 64 hex");
      }
      if (!SOURCE_ROLES.has(ref?.role)) {
        fail(id, "sourceEventRefs.role invalid");
      }
    }
  }
  if (!Array.isArray(oa.relations)) {
    fail(id, "openagents.relations required");
  } else {
    for (const rel of oa.relations) {
      if (!RELATION_TYPE_RE.test(rel?.type ?? "")) {
        fail(id, "relation type invalid");
      }
      if (!isValidSlug(rel?.targetSlug ?? "")) {
        fail(id, "relation targetSlug invalid");
      }
      if (!DIRECTIONS.has(rel?.direction)) {
        fail(id, "relation direction invalid");
      }
    }
  }
  if (!Array.isArray(oa.derivedFromSlugs)) {
    fail(id, "openagents.derivedFromSlugs required");
  } else {
    for (const slug of oa.derivedFromSlugs) {
      if (!isValidSlug(slug)) fail(id, "derivedFromSlugs entry invalid");
    }
  }
  if (hasForbiddenRanking(body)) {
    fail(id, "ranking-like fields forbidden in durable body");
  }
};

const assertAcceptWire = (id, fixture) => {
  const event = fixture.wire_event;
  if (!event) {
    fail(id, "missing wire_event");
    return;
  }
  if (event.kind !== ENGRAM_KIND) {
    fail(id, `kind must be ${ENGRAM_KIND}`);
  }
  const d = tagValues(event.tags, "d");
  if (d.length !== 1 || !HEX64_RE.test(d[0] ?? "")) {
    fail(id, "exactly one 64-hex d tag required");
  }
  const p = tagValues(event.tags, "p");
  if (p.length !== 1 || !HEX64_RE.test(p[0] ?? "")) {
    fail(id, "exactly one 64-hex p tag required");
  }
  const alt = tagValues(event.tags, "alt");
  if (alt.length !== 1 || alt[0] !== ALT) {
    fail(id, "exactly one correct alt tag required");
  }
  if (!looksEncrypted(event.content)) {
    fail(id, "content must look like ciphertext, not JSON plaintext");
  }
  if (!HEX64_RE.test(event.pubkey ?? "")) {
    fail(id, "pubkey must be 64 hex");
  }
  // Owner must not equal author for Sarah-authored engrams in fixtures.
  if (event.pubkey === p[0]) {
    fail(id, "owner must not author engram (pubkey must differ from p)");
  }

  if (fixture.slug && fixture.conversation_key_hex) {
    const expected = deriveDTag(fixture.conversation_key_hex, fixture.slug);
    if (d[0] !== expected) {
      fail(id, `d tag does not match HMAC for slug ${fixture.slug}`);
    }
  }

  if (fixture.decrypted_payload) {
    assertOpenagentsCompanion(id, fixture.decrypted_payload, {
      allowTombstone: fixture.is_tombstone === true,
    });
    if (fixture.is_tombstone === true && fixture.decrypted_payload.value !== null) {
      fail(id, "tombstone fixture must set value null");
    }
    if (fixture.slug && fixture.decrypted_payload.slug !== fixture.slug) {
      fail(id, "payload slug must match fixture.slug");
    }
  }
};

const assertDTagVectors = (id, fixture) => {
  if (!fixture.conversation_key_hex || !Array.isArray(fixture.vectors)) {
    fail(id, "d-tag vectors fixture incomplete");
    return;
  }
  if (fixture.domain !== D_TAG_DOMAIN) {
    fail(id, "d-tag domain mismatch");
  }
  for (const vector of fixture.vectors) {
    const got = deriveDTag(fixture.conversation_key_hex, vector.slug);
    if (got !== vector.d) {
      fail(id, `vector mismatch for ${vector.slug}: got ${got}`);
    }
  }
};

const assertProjectionRule = (id, fixture) => {
  const rules = fixture.rules;
  if (!rules) {
    fail(id, "projection rules missing");
    return;
  }
  if (rules.kind !== ENGRAM_KIND) fail(id, "projection rule kind mismatch");
  if (rules.projection_is_authority !== false) {
    fail(id, "projection_is_authority must be false");
  }
  if (rules.on_disagreement !== "engrams_win") {
    fail(id, "on_disagreement must be engrams_win");
  }
  if (rules.ranking_location !== "projection_only") {
    fail(id, "ranking_location must be projection_only");
  }
  if (rules.rebuildable_from_engrams !== true) {
    fail(id, "rebuildable_from_engrams must be true");
  }
  if (rules.memory_without_engram !== "defect") {
    fail(id, "memory_without_engram must be defect");
  }
  if (rules.durable_authority !== "nip_ae_engrams") {
    fail(id, "durable_authority must be nip_ae_engrams");
  }
};

const assertRejectReason = (id, fixture, expectedChecks) => {
  if (fixture.expect !== "reject") {
    fail(id, "negative fixture must set expect=reject");
  }
  if (!fixture.reason) {
    fail(id, "negative fixture must name reason");
  }

  if (fixture.claim) {
    let matched = false;
    for (const check of expectedChecks) {
      if (check(fixture)) matched = true;
    }
    if (!matched) {
      fail(id, `claim does not exhibit expected defect (${fixture.reason})`);
    }
    return;
  }

  if (fixture.decrypted_payload && !fixture.wire_event) {
    let matched = false;
    for (const check of expectedChecks) {
      if (check(fixture)) matched = true;
    }
    if (!matched) {
      fail(id, `payload does not exhibit expected defect (${fixture.reason})`);
    }
    return;
  }

  const event = fixture.wire_event;
  if (!event && !fixture.decrypted_payload) {
    fail(id, "missing wire_event or claim");
    return;
  }

  let matched = false;
  for (const check of expectedChecks) {
    if (check(fixture)) matched = true;
  }
  if (!matched) {
    fail(id, `fixture does not exhibit expected defect (${fixture.reason})`);
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
  if (relativePath.endsWith("d-tag-vectors.json")) {
    assertDTagVectors(id, fixture);
    continue;
  }
  if (relativePath.endsWith("projection-rebuild-rule.json")) {
    assertProjectionRule(id, fixture);
    continue;
  }
  assertAcceptWire(id, fixture);
}

const negativeChecks = {
  content_must_be_nip44_ciphertext: [
    (fixture) => !looksEncrypted(fixture.wire_event?.content),
  ],
  kind_must_be_30174: [(fixture) => fixture.wire_event?.kind !== ENGRAM_KIND],
  d_tag_required: [(fixture) => tagValues(fixture.wire_event?.tags, "d").length === 0],
  p_tag_required: [(fixture) => tagValues(fixture.wire_event?.tags, "p").length === 0],
  nip31_alt_required: [
    (fixture) => tagValues(fixture.wire_event?.tags, "alt").length === 0,
  ],
  sarah_must_author_kind_30174: [
    (fixture) => {
      const event = fixture.wire_event;
      if (!event) return false;
      const p = tagValues(event.tags, "p")[0];
      return event.kind === ENGRAM_KIND && p !== undefined && event.pubkey === p;
    },
  ],
  ranking_forbidden_in_durable_body: [
    (fixture) => hasForbiddenRanking(fixture.decrypted_payload),
  ],
  d_tag_must_match_slug_hmac: [
    (fixture) => {
      if (!fixture.slug || !fixture.conversation_key_hex || !fixture.wire_event) {
        return false;
      }
      const d = tagValues(fixture.wire_event.tags, "d")[0];
      const expected = deriveDTag(fixture.conversation_key_hex, fixture.slug);
      return d !== expected;
    },
  ],
  projection_never_outranks_engrams: [
    (fixture) =>
      fixture.claim?.projection_is_authority === true ||
      fixture.claim?.on_disagreement === "projection_wins",
  ],
  wiki_links_are_not_the_graph: [
    (fixture) => fixture.claim?.treat_wiki_links_as_typed_edges === true,
  ],
  admitted_requires_source_event_refs: [
    (fixture) => {
      const oa = fixture.decrypted_payload?.openagents;
      return (
        oa?.admission === "admitted" &&
        Array.isArray(oa.sourceEventRefs) &&
        oa.sourceEventRefs.length === 0
      );
    },
  ],
};

for (const relativePath of manifest.negatives) {
  const fixture = load(relativePath);
  const id = fixture.fixture_id ?? relativePath;
  const checks = negativeChecks[fixture.reason] ?? [() => false];
  assertRejectReason(id, fixture, checks);
}

if (manifest.kinds.engram !== ENGRAM_KIND) {
  fail("manifest", "kind number drifted from contract");
}
if (manifest.nip_ae?.d_tag_domain !== D_TAG_DOMAIN) {
  fail("manifest", "d_tag_domain drifted from contract");
}
if (manifest.nip_ae?.alt !== ALT) {
  fail("manifest", "alt text drifted from contract");
}

if (failures.length > 0) {
  console.error("SARAH-NR-07a fixture validation failed:");
  for (const item of failures) console.error(`- ${item}`);
  process.exit(1);
}

console.log(
  `SARAH-NR-07a fixtures ok: ${manifest.fixtures.length} canonical, ${manifest.negatives.length} negative`,
);
