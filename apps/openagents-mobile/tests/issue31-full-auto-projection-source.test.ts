/**
 * omega#49: the Full Auto section must describe *this* device's host.
 *
 * The contract reader was already honest; nothing was feeding it. Every device,
 * paired or not, rendered `no_host_projection` — a wiring gap presented to the
 * owner as a pairing fact, the same shape of lie as showing a discovery error
 * for an authentication failure.
 *
 * These tests hold the three refusals apart. An unpaired device says it is not
 * paired. A paired device holding a malformed detail says the detail is
 * unreadable, and never quietly reports itself as unpaired. A detail bound to
 * another host or another snapshot is withheld rather than drawn as current.
 */
import { Effect, Stream } from "@effect-native/core/effect";
import { readFileSync } from "node:fs";
import {
  ISSUE31_PAIRING_SCHEMA,
  decodeIssue31PairingRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import { describe, expect, test } from "vite-plus/test";

import {
  buildHomeProgram,
  renderContentView,
} from "../src/screens/home-core";
import {
  activeIssue31GrantForDevice,
  issue31FullAutoProjectionFromSnapshot,
  type Issue31FullAutoSourceEvent,
  type Issue31FullAutoSourceSnapshot,
} from "../src/workroom/issue31-full-auto-projection-source";

const FIXTURE_ROOT = "../../../packages/sarah/fixtures/issue31-workroom";

const fixture = (name: string): Record<string, unknown> =>
  JSON.parse(
    readFileSync(new URL(`${FIXTURE_ROOT}/${name}`, import.meta.url), "utf8"),
  ) as Record<string, unknown>;

const hostAdjunct = (): Record<string, unknown> =>
  fixture("openagents.omega.issue31.host.v1.canonical.json");
const fullAutoAdjunct = (): Record<string, unknown> =>
  fixture("openagents.omega.issue31.fullauto.v1.canonical.json");

/** The host named by both canonical fixtures, and by the grant below. */
const HOST_REF = "host.omega.device-alpha";
const SNAPSHOT_REF = "snapshot.omega.issue31.000042";
const HOST_KEY = "d".repeat(64);
const SARAH_KEY = "3".repeat(64);
const DEVICE_KEY = "b".repeat(64);

const REQUEST_ID = "6".repeat(64);
const CHALLENGE_ID = "5".repeat(64);
const RESPONSE_ID = "4".repeat(64);
const GRANT_ID = "1".repeat(64);
const HOST_ADJUNCT_ID = "7".repeat(64);
const FULL_AUTO_ID = "8".repeat(64);

const sourceEvent = (
  canonicalRecordId: string,
  privateRecord: unknown,
  createdAt = 1_700_000_000,
): Issue31FullAutoSourceEvent => ({
  canonicalRecordId,
  event: { created_at: createdAt },
  privateRecord,
});

/**
 * A complete signed pairing chain: request, challenge, response, scoped grant.
 * `foldIssue31Grant` refuses anything shorter, which is the point — the grant
 * is what entitles this device to read this host at all.
 */
const pairingChain = (
  overrides: Readonly<{
    hostRef?: string;
    hostPublicKeyHex?: string;
    devicePublicKeyHex?: string;
    expiresAt?: number;
  }> = {},
): ReadonlyArray<Issue31FullAutoSourceEvent> => {
  const hostRef = overrides.hostRef ?? HOST_REF;
  const hostPublicKeyHex = overrides.hostPublicKeyHex ?? HOST_KEY;
  const devicePublicKeyHex = overrides.devicePublicKeyHex ?? DEVICE_KEY;
  const expiresAt = overrides.expiresAt ?? 1_700_003_000;
  const identity = { hostRef, hostPublicKeyHex, devicePublicKeyHex };
  return [
    sourceEvent(
      REQUEST_ID,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_request",
        ...identity,
        issuedAt: 1_699_999_700,
        pairingRequestRef: "pairing.request.device_1",
        requestedScopes: ["observe_issue31", "control_full_auto"],
        expiresAt: 1_700_000_500,
      }),
    ),
    sourceEvent(
      CHALLENGE_ID,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_challenge",
        ...identity,
        issuedAt: 1_699_999_800,
        pairingChallengeRef: "pairing.challenge.device_1",
        pairingRequestEventId: REQUEST_ID,
        challenge: "9".repeat(64),
        expiresAt: 1_700_000_500,
      }),
    ),
    sourceEvent(
      RESPONSE_ID,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_response",
        ...identity,
        issuedAt: 1_699_999_900,
        pairingResponseRef: "pairing.response.device_1",
        pairingChallengeEventId: CHALLENGE_ID,
        challenge: "9".repeat(64),
        expiresAt: 1_700_000_500,
      }),
    ),
    sourceEvent(
      GRANT_ID,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "scoped_grant",
        ...identity,
        sarahPublicKeyHex: SARAH_KEY,
        issuedAt: 1_700_000_000,
        pairingResponseEventId: RESPONSE_ID,
        grantRef: "grant.omega.device_1",
        generation: 1,
        scopes: ["observe_issue31", "control_full_auto"],
        expiresAt,
      }),
    ),
  ];
};

const snapshotWith = (
  events: ReadonlyArray<Issue31FullAutoSourceEvent>,
): Issue31FullAutoSourceSnapshot => ({
  devicePublicKeyHex: DEVICE_KEY,
  admittedHostPublicKeys: [HOST_KEY],
  selectedHostPublicKeys: [HOST_KEY],
  confirmedEvents: events,
});

const paired = (
  host: unknown = hostAdjunct(),
  detail: unknown = fullAutoAdjunct(),
): Issue31FullAutoSourceSnapshot =>
  snapshotWith([
    ...pairingChain(),
    sourceEvent(HOST_ADJUNCT_ID, host, 1_700_000_100),
    sourceEvent(FULL_AUTO_ID, detail, 1_700_000_200),
  ]);

const NOW = 1_700_000_500;

describe("issue31FullAutoProjectionFromSnapshot", () => {
  test("an unpaired device says it is not paired, even with host records present", () => {
    // Both adjuncts are on the relay. Without a grant the phone has no standing
    // to read either one, and it must not borrow the payload's own hostRef.
    const snapshot = snapshotWith([
      sourceEvent(HOST_ADJUNCT_ID, hostAdjunct(), 1_700_000_100),
      sourceEvent(FULL_AUTO_ID, fullAutoAdjunct(), 1_700_000_200),
    ]);
    expect(activeIssue31GrantForDevice(snapshot, NOW)).toBeNull();
    expect(issue31FullAutoProjectionFromSnapshot(snapshot, NOW)).toEqual({
      schema: "openagents.mobile.issue31.fullauto.read-model.v1",
      state: "unavailable",
      reason: "no_host_projection",
    });
  });

  test("an expired grant reads as unpaired rather than as a live host", () => {
    const snapshot = snapshotWith([
      ...pairingChain({ expiresAt: 1_700_000_100 }),
      sourceEvent(HOST_ADJUNCT_ID, hostAdjunct(), 1_700_000_100),
      sourceEvent(FULL_AUTO_ID, fullAutoAdjunct(), 1_700_000_200),
    ]);
    expect(issue31FullAutoProjectionFromSnapshot(snapshot, NOW)).toMatchObject({
      state: "unavailable",
      reason: "no_host_projection",
    });
  });

  test("a paired device projects the host's runs, accounts, and evidence", () => {
    const model = issue31FullAutoProjectionFromSnapshot(paired(), NOW);
    if (model.state !== "ready") throw new Error(`expected ready, got ${model.reason}`);
    expect(model.hostRef).toBe(HOST_REF);
    expect(model.snapshotRef).toBe(SNAPSHOT_REF);
    expect(model.runs).toHaveLength(2);
    expect(model.accounts).toHaveLength(3);
    expect(model.runs.map((run) => run.runRef)).toContain("run.full-auto.run-01");
  });

  test("a paired device with a malformed detail says unreadable, not unpaired", () => {
    const model = issue31FullAutoProjectionFromSnapshot(
      paired(hostAdjunct(), fixture("openagents.omega.issue31.fullauto.v1.negative-partial-chain.json")),
      NOW,
    );
    expect(model).toMatchObject({
      state: "unavailable",
      reason: "host_projection_unreadable",
    });
  });

  test("a paired device with a malformed host snapshot says unreadable, not unpaired", () => {
    // The grant proves pairing. Reporting "not paired" here would blame the
    // owner's device for the host's broken record.
    const model = issue31FullAutoProjectionFromSnapshot(
      paired({ ...hostAdjunct(), projections: [] }),
      NOW,
    );
    expect(model).toMatchObject({
      state: "unavailable",
      reason: "host_projection_unreadable",
    });
  });

  test("a detail bound to another snapshot is withheld rather than shown as current", () => {
    const model = issue31FullAutoProjectionFromSnapshot(
      paired(hostAdjunct(), {
        ...fullAutoAdjunct(),
        snapshotRef: "snapshot.omega.issue31.000041",
      }),
      NOW,
    );
    expect(model).toMatchObject({ state: "unavailable", reason: "snapshot_mismatch" });
  });

  test("a detail from a host this device never paired with is refused", () => {
    // Same relay, different machine. The payload names its own host, and that
    // claim must not select it into this device's Workroom.
    const foreignDetail = issue31FullAutoProjectionFromSnapshot(
      paired(hostAdjunct(), { ...fullAutoAdjunct(), hostRef: "host.omega.device-beta" }),
      NOW,
    );
    expect(foreignDetail).toMatchObject({ state: "unavailable", reason: "snapshot_mismatch" });

    // And a host snapshot for another machine is not adopted as the binding at
    // all, so its snapshotRef can never be the one a detail is checked against.
    const foreignHost = issue31FullAutoProjectionFromSnapshot(
      paired(
        { ...hostAdjunct(), hostRef: "host.omega.device-beta" },
        { ...fullAutoAdjunct(), hostRef: "host.omega.device-beta" },
      ),
      NOW,
    );
    // Paired, but nothing bound to this grant — not "never paired".
    expect(foreignHost).toMatchObject({ state: "unavailable", reason: "no_host_snapshot" });
  });

  test("a paired host that has published no Full Auto detail reports absence", () => {
    const snapshot = snapshotWith([
      ...pairingChain(),
      sourceEvent(HOST_ADJUNCT_ID, hostAdjunct(), 1_700_000_100),
    ]);
    // Connected and silent about Full Auto. Reporting this as "not paired"
    // contradicted the grant and the host snapshot the device was holding.
    expect(issue31FullAutoProjectionFromSnapshot(snapshot, NOW)).toMatchObject({
      state: "unavailable",
      reason: "no_full_auto_detail",
    });
  });

  test("a grant for another device on the same relay grants nothing here", () => {
    const snapshot = snapshotWith([
      ...pairingChain({ devicePublicKeyHex: "c".repeat(64) }),
      sourceEvent(HOST_ADJUNCT_ID, hostAdjunct(), 1_700_000_100),
      sourceEvent(FULL_AUTO_ID, fullAutoAdjunct(), 1_700_000_200),
    ]);
    expect(issue31FullAutoProjectionFromSnapshot(snapshot, NOW)).toMatchObject({
      state: "unavailable",
      reason: "no_host_projection",
    });
  });

  test("a host key the build never admitted cannot ground a grant", () => {
    const snapshot: Issue31FullAutoSourceSnapshot = {
      ...snapshotWith([
        ...pairingChain(),
        sourceEvent(HOST_ADJUNCT_ID, hostAdjunct(), 1_700_000_100),
        sourceEvent(FULL_AUTO_ID, fullAutoAdjunct(), 1_700_000_200),
      ]),
      admittedHostPublicKeys: [],
      selectedHostPublicKeys: [],
    };
    expect(issue31FullAutoProjectionFromSnapshot(snapshot, NOW)).toMatchObject({
      state: "unavailable",
      reason: "no_host_projection",
    });
  });
});

const settle = Effect.gen(function* () {
  yield* Effect.yieldNow;
  yield* Effect.yieldNow;
});

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") throw new Error("expected state");
    return option.value;
  });

describe("Workroom Full Auto state", () => {
  test("starts unavailable and reaches the screen once a host projection is set", async () => {
    const program = buildHomeProgram();
    expect(program.initialState.issue31FullAuto).toMatchObject({
      state: "unavailable",
      reason: "no_host_projection",
    });

    program.workroom.open();
    program.workroom.setFullAutoReadModel(
      issue31FullAutoProjectionFromSnapshot(paired(), NOW),
    );
    await Effect.runPromise(settle);
    const state = await Effect.runPromise(lastState(program));
    expect(state.issue31FullAuto.state).toBe("ready");
    const view = JSON.stringify(renderContentView(state));
    expect(view).toContain("Finish the issue 31 mobile workroom");
    expect(view).not.toContain("This device is not paired to an Omega host yet.");
  });
});
