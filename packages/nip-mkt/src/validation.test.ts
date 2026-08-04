import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";
import { finalizeEvent, type Event, type EventTemplate } from "nostr-effect/pure";

import { serializeSignedEvent } from "./transport.js";
import {
  MktValidationError,
  decodePrivateBase,
  decodePrivateWithProfiles,
  decodePublicHead,
  parseJsonRejectingDuplicateMembers,
  validatePublicHead,
  validateRawPrivateRecord,
  validateRawPrivateRecordBase,
} from "./validation.js";

interface PublicTemplate {
  readonly kind: number;
  readonly tags: readonly (readonly string[])[];
  readonly content?: string;
}

interface InvalidMutation {
  readonly name: string;
  readonly expected_code?: string;
  readonly kind?: number;
  readonly remove?: string;
  readonly remove_tag?: string;
  readonly add?: readonly string[];
  readonly set?: readonly string[];
  readonly set_tag?: readonly string[];
  readonly content?: string;
  readonly content_bytes?: number;
}

const fixture = <A>(name: string): A =>
  JSON.parse(
    readFileSync(
      fileURLToPath(new URL(`../contract/fixtures/nipmkt/${name}`, import.meta.url)),
      "utf8",
    ),
  ) as A;

const publicFixture = fixture<{
  readonly valid: readonly PublicTemplate[];
  readonly invalid: readonly InvalidMutation[];
}>("public-heads.json");
const grammarFixture = fixture<{
  readonly base: EventTemplate;
  readonly invalid: readonly InvalidMutation[];
  readonly synthetic_profile: {
    readonly id: string;
    readonly version: number;
    readonly critical_members: readonly string[];
    readonly understood_members: readonly string[];
  };
}>("common-grammar.json");
const closingFixture = fixture<{
  readonly private_base: EventTemplate;
  readonly profile_support: {
    readonly id: string;
    readonly version: number;
    readonly critical_members: readonly string[];
    readonly understood_members: readonly string[];
  };
  readonly validation_cases: readonly {
    readonly id: string;
    readonly input: {
      readonly remove_tag?: string;
      readonly content?: string;
      readonly profile_id?: string;
      readonly profile_version?: number;
    };
    readonly expected: {
      readonly relay_base?: string;
      readonly client?: string;
      readonly code: string;
    };
  }[];
}>("relay-closing.json");

const privateKey = new Uint8Array(32).fill(1);

function unsignedEvent(template: PublicTemplate): Event {
  return {
    id: "0".repeat(64),
    pubkey: "1".repeat(64),
    created_at: 1,
    kind: template.kind,
    tags: template.tags.map((tag) => [...tag]),
    content: template.content ?? "{}",
    sig: "0".repeat(128),
  };
}

function mutateTemplate(template: EventTemplate, mutation: InvalidMutation): EventTemplate {
  let tags = template.tags.map((tag) => [...tag]);
  const remove = mutation.remove ?? mutation.remove_tag;
  if (remove !== undefined) tags = tags.filter((tag) => tag[0] !== remove);
  if (mutation.add !== undefined) tags.push([...mutation.add]);
  const replacement = mutation.set ?? mutation.set_tag;
  if (replacement !== undefined) {
    const index = tags.findIndex((tag) => tag[0] === replacement[0]);
    if (index < 0) throw new Error(`missing replacement tag for ${mutation.name}`);
    tags[index] = [...replacement];
  }
  return {
    ...template,
    tags,
    content:
      mutation.content ??
      (mutation.content_bytes === undefined
        ? template.content
        : "x".repeat(mutation.content_bytes)),
  };
}

function signedRaw(template: EventTemplate): string {
  return serializeSignedEvent(
    finalizeEvent(
      { ...template, created_at: template.created_at ?? 1 },
      privateKey,
      new Uint8Array(32),
    ),
  );
}

function expectCode(run: () => unknown, code: string, label: string): void {
  try {
    run();
    throw new Error(`accepted invalid fixture: ${label}`);
  } catch (cause) {
    expect(cause, label).toBeInstanceOf(MktValidationError);
    expect((cause as MktValidationError).code, label).toBe(code);
  }
}

describe("NIP-MKT fixture validation", () => {
  test("accepts every public-head fixture and rejects every invalid case", () => {
    for (const fixtureCase of publicFixture.valid) {
      expect(validatePublicHead(unsignedEvent(fixtureCase)).kind).toBe(fixtureCase.kind);
    }
    for (const fixtureCase of publicFixture.invalid) {
      const valid = publicFixture.valid.find(({ kind }) => kind === fixtureCase.kind);
      if (valid === undefined) throw new Error(`missing public fixture for ${fixtureCase.kind}`);
      const mutated = mutateTemplate(
        {
          kind: valid.kind,
          tags: valid.tags.map((tag) => Array.from(tag)),
          created_at: 1,
          content: "{}",
        },
        fixtureCase,
      );
      expect(() => validatePublicHead(unsignedEvent(mutated)), fixtureCase.name).toThrow(
        MktValidationError,
      );
    }
  });

  test("accepts public content caps inclusively and rejects one byte beyond", () => {
    for (const [kind, maximum] of [
      [39600, 16_384],
      [39603, 4_096],
    ] as const) {
      const valid = publicFixture.valid.find((candidate) => candidate.kind === kind);
      if (valid === undefined) throw new Error(`missing public fixture for ${kind}`);
      const event = unsignedEvent(valid);
      event.content = `{"x":"${"a".repeat(maximum - 8)}"}`;
      expect(new TextEncoder().encode(event.content)).toHaveLength(maximum);
      expect(validatePublicHead(event).kind).toBe(kind);
      event.content += " ";
      expectCode(() => validatePublicHead(event), "event_too_large", `${kind} cap + 1`);
    }
  });

  test("replays all common grammar cases with exact codes", () => {
    expect(validateRawPrivateRecordBase(signedRaw(grammarFixture.base), false).event.kind).toBe(
      39604,
    );
    for (const fixtureCase of grammarFixture.invalid) {
      const raw = signedRaw(mutateTemplate(grammarFixture.base, fixtureCase));
      expectCode(
        () => validateRawPrivateRecordBase(raw, false),
        fixtureCase.expected_code!,
        fixtureCase.name,
      );
    }
  });

  test("fails closed on unknown critical profile members", () => {
    const profile = grammarFixture.synthetic_profile;
    const raw = signedRaw(grammarFixture.base);
    expect(validateRawPrivateRecordBase(raw, false).event.kind).toBe(39604);
    expectCode(
      () =>
        validateRawPrivateRecord(
          raw,
          [
            {
              id: profile.id,
              version: profile.version,
              criticalMembers: profile.critical_members,
              understoodMembers: profile.understood_members,
            },
          ],
          false,
        ),
      "unsupported_critical_member",
      "unknown critical member",
    );
  });

  test("replays relay-closing base and client profile decisions", () => {
    const support = closingFixture.profile_support;
    for (const fixtureCase of closingFixture.validation_cases) {
      const mutation: InvalidMutation = {
        name: fixtureCase.id,
        ...(fixtureCase.input.remove_tag === undefined
          ? {}
          : { remove_tag: fixtureCase.input.remove_tag }),
        ...(fixtureCase.input.content === undefined ? {} : { content: fixtureCase.input.content }),
      };
      let template = mutateTemplate(closingFixture.private_base, mutation);
      if (fixtureCase.input.profile_id !== undefined) {
        const version = fixtureCase.input.profile_version ?? 1;
        template = mutateTemplate(template, {
          name: fixtureCase.id,
          set_tag: ["profile", fixtureCase.input.profile_id, String(version)],
          content: JSON.stringify({
            schema: "openagents.mkt.v1",
            profile: fixtureCase.input.profile_id,
            profile_version: version,
            session_id: template.tags.find((tag) => tag[0] === "session")?.[1],
          }),
        });
      } else if (fixtureCase.input.profile_version !== undefined) {
        template = mutateTemplate(template, {
          name: fixtureCase.id,
          set_tag: ["profile", support.id, String(fixtureCase.input.profile_version)],
          content: JSON.stringify({
            schema: "openagents.mkt.v1",
            profile: support.id,
            profile_version: fixtureCase.input.profile_version,
            session_id: template.tags.find((tag) => tag[0] === "session")?.[1],
          }),
        });
      }
      const raw = signedRaw(template);
      if (fixtureCase.expected.relay_base === "accept") {
        expect(validateRawPrivateRecordBase(raw, false).event.kind, fixtureCase.id).toBe(39604);
        expectCode(
          () =>
            validateRawPrivateRecord(
              raw,
              [
                {
                  id: support.id,
                  version: support.version,
                  criticalMembers: support.critical_members,
                  understoodMembers: support.understood_members,
                },
              ],
              false,
            ),
          fixtureCase.expected.code,
          fixtureCase.id,
        );
      } else {
        expectCode(
          () => validateRawPrivateRecordBase(raw, false),
          fixtureCase.expected.code,
          fixtureCase.id,
        );
      }
    }
  });

  test("exposes typed Effect decoding boundaries", () =>
    // Vite Plus does not expose the Effect test extension in this workspace.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    Effect.runPromise(
      Effect.gen(function* () {
        const privateRaw = signedRaw(grammarFixture.base);
        const defaultError = yield* Effect.flip(decodePrivateBase(privateRaw));
        expect(defaultError).toBeInstanceOf(MktValidationError);
        expect((yield* decodePrivateBase(privateRaw, false)).event.kind).toBe(39604);
        expect(
          (yield* decodePrivateWithProfiles(privateRaw, [{ id: "conformance", version: 1 }], false))
            .event.kind,
        ).toBe(39604);
        const publicRaw = signedRaw({
          ...publicFixture.valid[0]!,
          tags: publicFixture.valid[0]!.tags.map((tag) => Array.from(tag)),
          created_at: 1,
          content: "{}",
        });
        expect((yield* decodePublicHead(publicRaw)).kind).toBe(39600);
      }),
    ));

  test("classifies malformed causal and address references as invalid_reference", () => {
    const receipt = publicFixture.valid.find(({ kind }) => kind === 39603);
    const offering = publicFixture.valid.find(({ kind }) => kind === 39601);
    if (receipt === undefined || offering === undefined) throw new Error("missing public fixtures");
    expectCode(
      () =>
        validatePublicHead(
          unsignedEvent(
            mutateTemplate(
              {
                ...receipt,
                tags: receipt.tags.map((tag) => [...tag]),
                created_at: 1,
                content: receipt.content ?? "{}",
              },
              { name: "bad close reference", set: ["x", "cc"] },
            ),
          ),
        ),
      "invalid_reference",
      "public receipt close reference",
    );
    expectCode(
      () =>
        validatePublicHead(
          unsignedEvent(
            mutateTemplate(
              {
                ...offering,
                tags: offering.tags.map((tag) => [...tag]),
                created_at: 1,
                content: offering.content ?? "{}",
              },
              {
                name: "bad provider reference identifier",
                set: ["provider", `39600:${"1".repeat(64)}:Provider`],
              },
            ),
          ),
        ),
      "invalid_reference",
      "offering provider reference",
    );
    const invalidOfferingReference = signedRaw({
      ...grammarFixture.base,
      tags: [
        ...grammarFixture.base.tags,
        ["a", `39601:${"c".repeat(64)}:Offering`, "", "offering"],
        ["expiration", "2"],
      ],
    });
    expectCode(
      () => validateRawPrivateRecordBase(invalidOfferingReference, true),
      "invalid_reference",
      "RFQ offering reference",
    );
  });

  test("rejects duplicate escaped members and trailing bytes directly", () => {
    expectCode(
      () => parseJsonRejectingDuplicateMembers('{"x":1,"\\u0078":2}'),
      "duplicate_json_member",
      "escaped duplicate",
    );
    expectCode(
      () => parseJsonRejectingDuplicateMembers("{} true"),
      "invalid_json",
      "trailing JSON",
    );
  });
});
