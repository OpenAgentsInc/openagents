import { describe, expect, it } from "vite-plus/test";

import { generateSarahNostrSigner } from "../nostr-identity/index.ts";
import { verifySignedEvent } from "../nostr-identity/crypto.ts";
import { parseSecretMaterial } from "../nostr-identity/crypto.ts";
import {
  SARAH_ENGRAM_KIND,
  SARAH_READ_STATE_KIND,
  SARAH_REMINDER_KIND,
  advanceReadContexts,
  buildCoreBody,
  buildEngramWriteTemplate,
  buildMemoryBody,
  buildReadStateBlob,
  buildReadStateWriteTemplate,
  buildReminderContent,
  buildReminderWriteTemplate,
  buildTombstoneBody,
  contentDigestOf,
  deriveEngramDTag,
  guardSarahMemoryValue,
  isValidSlug,
  makeNip44MemoryCipher,
  mergeReadContexts,
  parseEngramBody,
  readEngramBody,
  readReadStateBlob,
  readReminderContent,
  sarahConversationContextKey,
  testSarahNostrMemoryCipher,
  validateReadStateBlob,
} from "./index.ts";

/** NIP-AE pinned test conversation key (fixtures only). */
const FIXTURE_KC =
  "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d";

const D_CORE =
  "bdc233238ffe52e272b44cc233c8f33a2bc510b08be04495b225964283be4a90";
const D_MEM_EXAMPLE =
  "72d4f9629106451505d7d341ea85bb3ebad4f654fcfd2aad100d5a35f8a85cba";
const D_MEM_FACT =
  "2a93fa200395921923724765c52989fa4a2b3b81095940f8e9cb4e002664af25";

describe("SARAH-NR-07 engram templates", () => {
  it("derives HMAC-blinded d tags matching NIP-AE vectors", () => {
    expect(deriveEngramDTag(FIXTURE_KC, "core")).toBe(D_CORE);
    expect(deriveEngramDTag(FIXTURE_KC, "mem/example")).toBe(D_MEM_EXAMPLE);
    expect(deriveEngramDTag(FIXTURE_KC, "mem/fact/owner-prefers-codex")).toBe(
      D_MEM_FACT,
    );
  });

  it("round-trips memory body with companion fields through cipher", () => {
    const cipher = testSarahNostrMemoryCipher();
    const signer = generateSarahNostrSigner();
    const ownerPubkey = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    const value = "Owner prefers Codex for coding tasks.";
    const body = buildMemoryBody({
      slug: "mem/fact/owner-prefers-codex",
      value,
      openagents: {
        admission: "admitted",
        entityId: "entity." + "aa".repeat(12),
        sourceEventRefs: [
          {
            eventId: "11".repeat(32),
            role: "turn_record",
          },
        ],
        relations: [
          {
            type: "about",
            targetSlug: "mem/entity/owner",
            direction: "out",
          },
        ],
        derivedFromSlugs: [],
      },
    });
    expect(body.openagents?.contentDigest).toBe(contentDigestOf(value));

    const { template, d, plaintext } = buildEngramWriteTemplate({
      conversation: {
        ownerPubkey,
        sarahPubkey: signer.getPublicKey(),
        conversationKeyHex: FIXTURE_KC,
      },
      body,
      cipher,
      createdAt: 1_753_387_300,
    });

    expect(template.kind).toBe(SARAH_ENGRAM_KIND);
    expect(d).toBe(D_MEM_FACT);
    expect(template.content.startsWith("nip44:v2:test:")).toBe(true);
    expect(template.content).not.toContain(value);
    expect(template.content).not.toContain("openagents.sarah.nip_ae_companion");
    expect(template.tags.some((t) => t[0] === "d" && t[1] === d)).toBe(true);
    expect(
      template.tags.some((t) => t[0] === "p" && t[1] === ownerPubkey),
    ).toBe(true);
    // Slug must not leak in tags
    expect(JSON.stringify(template.tags)).not.toContain("mem/fact");

    const signed = signer.signEvent(template);
    expect(verifySignedEvent(signed)).toBe(true);

    const parsed = parseEngramBody(plaintext);
    expect(parsed).toEqual(body);

    const roundTrip = readEngramBody({ content: template.content, cipher });
    expect(roundTrip).toEqual(body);
  });

  it("round-trips core and tombstone bodies", () => {
    const cipher = testSarahNostrMemoryCipher();
    const signer = generateSarahNostrSigner();
    const ownerPubkey = "01".repeat(32);
    const conversation = {
      ownerPubkey,
      sarahPubkey: signer.getPublicKey(),
      conversationKeyHex: FIXTURE_KC,
    };

    const core = buildEngramWriteTemplate({
      conversation,
      body: buildCoreBody("Sarah agent profile text"),
      cipher,
    });
    expect(core.d).toBe(D_CORE);
    expect(readEngramBody({ content: core.template.content, cipher })).toEqual({
      slug: "core",
      profile: "Sarah agent profile text",
    });

    const tomb = buildEngramWriteTemplate({
      conversation,
      body: buildTombstoneBody("mem/fact/owner-prefers-codex"),
      cipher,
    });
    expect(tomb.d).toBe(D_MEM_FACT);
    expect(readEngramBody({ content: tomb.template.content, cipher })).toEqual({
      slug: "mem/fact/owner-prefers-codex",
      value: null,
    });
  });

  it("rejects invalid slugs and ranking fields", () => {
    expect(isValidSlug("not-a-mem")).toBe(false);
    expect(() =>
      buildMemoryBody({
        slug: "bad",
        value: "x",
        openagents: {
          admission: "candidate",
          entityId: "entity." + "bb".repeat(12),
          sourceEventRefs: [],
          relations: [],
          derivedFromSlugs: [],
        },
      }),
    ).toThrow(/invalid memory slug/);

    const withRanking = JSON.stringify({
      slug: "mem/example",
      value: "ok",
      ranking: 0.9,
    });
    expect(parseEngramBody(withRanking)).toBeNull();
  });

  it("round-trips through real NIP-44 memory cipher", () => {
    // Pinned NIP-AE test keys (fixtures only)
    const sarahSk = parseSecretMaterial("00".repeat(31) + "01");
    const ownerPk =
      "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
    const cipher = makeNip44MemoryCipher({
      sarahSecretKey: sarahSk,
      ownerPubkeyHex: ownerPk,
    });
    expect(cipher.conversationKeyHex).toBe(FIXTURE_KC);

    const body = buildMemoryBody({
      slug: "mem/example",
      value: "hello, agent memory",
      openagents: {
        admission: "candidate",
        entityId: "entity." + "dd".repeat(12),
        sourceEventRefs: [],
        relations: [],
        derivedFromSlugs: [],
      },
    });
    const { template, d } = buildEngramWriteTemplate({
      conversation: {
        ownerPubkey: ownerPk,
        sarahPubkey:
          "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        conversationKeyHex: cipher.conversationKeyHex,
      },
      body,
      cipher,
    });
    expect(d).toBe(D_MEM_EXAMPLE);
    expect(template.content).not.toContain("hello, agent memory");
    expect(readEngramBody({ content: template.content, cipher })).toEqual(body);
  });
});

describe("SARAH-NR-07 read-state merge", () => {
  it("max-register merge is monotonic, associative, and idempotent", () => {
    const a = { "thread:aa": 100, "msg:bb": 50 };
    const b = { "thread:aa": 120, "msg:cc": 80 };
    const c = { "msg:bb": 60 };

    const ab = mergeReadContexts(a, b);
    const abc = mergeReadContexts(ab, c);
    const acb = mergeReadContexts(a, mergeReadContexts(c, b));
    expect(abc).toEqual(acb);
    expect(mergeReadContexts(ab, ab)).toEqual(ab);

    // Monotonic: advancing never lowers
    const advanced = advanceReadContexts(abc, { "thread:aa": 110 });
    expect(advanced["thread:aa"]).toBe(120);
    const advanced2 = advanceReadContexts(abc, { "thread:aa": 200 });
    expect(advanced2["thread:aa"]).toBe(200);
  });

  it("round-trips encrypted read-state blob", () => {
    const cipher = testSarahNostrMemoryCipher();
    const conversation = "sarah." + "ab".repeat(12);
    const blob = buildReadStateBlob({
      clientId: "omega-desktop-1",
      contexts: {
        [sarahConversationContextKey(conversation)]: 1_753_387_300,
      },
    });
    const { template, plaintext } = buildReadStateWriteTemplate({
      slotId: "slot-desktop-01",
      blob,
      cipher,
      createdAt: 1_753_387_300,
    });
    expect(template.kind).toBe(SARAH_READ_STATE_KIND);
    expect(template.content.startsWith("nip44:v2:test:")).toBe(true);
    expect(template.content).not.toContain("client_id");
    expect(
      template.tags.some((t) => t[0] === "t" && t[1] === "read-state"),
    ).toBe(true);
    expect(validateReadStateBlob(plaintext)).toEqual(blob);
    expect(readReadStateBlob({ content: template.content, cipher })).toEqual(
      blob,
    );
  });
});

describe("SARAH-NR-07 reminders", () => {
  it("round-trips pending reminder with not_before and expiration", () => {
    const cipher = testSarahNostrMemoryCipher();
    const content = buildReminderContent({
      status: "pending",
      note: "Follow up on coding fleet status",
      target: {
        a: "44300:" + "aa".repeat(32) + ":turn.1",
        preview: "turn record",
      },
    });
    const notBefore = 1_753_400_000;
    const { template, d } = buildReminderWriteTemplate({
      content,
      cipher,
      notBefore,
      expiration: notBefore + 86_400,
      createdAt: notBefore - 60,
    });
    expect(template.kind).toBe(SARAH_REMINDER_KIND);
    expect(d.length).toBe(32);
    expect(
      template.tags.some(
        (t) => t[0] === "not_before" && t[1] === String(notBefore),
      ),
    ).toBe(true);
    expect(template.content.startsWith("nip44:v2:test:")).toBe(true);
    expect(template.content).not.toContain("Follow up");
    expect(readReminderContent({ content: template.content, cipher })).toEqual(
      content,
    );
  });

  it("omits not_before for terminal statuses", () => {
    const cipher = testSarahNostrMemoryCipher();
    const { template } = buildReminderWriteTemplate({
      content: buildReminderContent({ status: "done" }),
      cipher,
      d: "abc123",
      expiration: 1_753_500_000,
    });
    expect(template.tags.some((t) => t[0] === "not_before")).toBe(false);
    expect(
      template.tags.some((t) => t[0] === "expiration"),
    ).toBe(true);
  });
});

describe("SARAH-NR-07 redaction", () => {
  it("rejects secret-shaped memory values and accepts clean facts", () => {
    const clean = guardSarahMemoryValue("Owner prefers Codex for coding.");
    expect(clean.storable).toBe(true);
    expect(clean.value).toBe("Owner prefers Codex for coding.");

    const nsec = guardSarahMemoryValue(
      "key is nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
    );
    expect(nsec.storable).toBe(false);
    expect(nsec.categories).toContain("private_key");

    const token = guardSarahMemoryValue(
      "OPENAGENTS_AGENT_TOKEN=oa_agent_abcdefghijklmnopqrst",
    );
    expect(token.storable).toBe(false);

    const path = guardSarahMemoryValue("see /Users/christopherdavid/work/secret");
    expect(path.storable).toBe(false);
    expect(path.categories).toContain("home_path");

    expect(() =>
      buildMemoryBody({
        slug: "mem/fact/bad",
        value: "bearer sk-abcdefghijklmnopqrstuvwxyz",
        openagents: {
          admission: "candidate",
          entityId: "entity." + "cc".repeat(12),
          sourceEventRefs: [],
          relations: [],
          derivedFromSlugs: [],
        },
      }),
    ).toThrow(/value rejected/);
  });
});
