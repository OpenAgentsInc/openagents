/**
 * A real Omega host, a real daemon reading, the deployed relay, and this
 * device's own Workroom rows (omega#97).
 *
 * ## What this covers that nothing else does
 *
 * `issue31-full-auto-delivery.test.ts` proves the wire-up over a real relay,
 * but the host it proves it against is this repository's pinned `host.v1`
 * fixture with its identity rebound. That is honest for a contract proof and
 * dishonest as a *device journey*: omega#49's exit forbids a fixture standing in
 * as host authority for one, and omega#97's host half went to the trouble of
 * making the substitution unexpressible on the host side rather than merely
 * discouraged.
 *
 * So this drives the real thing. It runs the host half's own live proof —
 * `a_running_daemon_supplies_the_reading_a_paired_device_reads_on_a_live_relay`
 * in `full_auto_ui` — which spawns the **packaged** `omega-effectd` from
 * `/Applications/Omega.app` under a data root of its own, reads it through the
 * five read methods, and publishes what it measured to the deployed relay. The
 * only thing this test hands that process is a public key: its own. The secret
 * never leaves this process, so the gift wraps on the relay can be opened by
 * exactly one reader, and the host is not it.
 *
 * Then it opens them with the shipped mobile client and pushes the result
 * through `projectIssue31Workroom` — the exact function `home-screen.tsx` calls
 * — into `renderContentView`.
 *
 * ## What it cannot cover
 *
 * The daemon holds **zero runs**, because starting one is starting Full Auto
 * authority, and no model-initiated path may do that. So the run rows are
 * proven empty-but-connected: the three capability rows carry the host's own
 * reading rather than `source_not_connected`, and the Full Auto section renders
 * `ready` against a host that reports no runs. A populated run row needs an
 * owner's own gesture; see the issue thread.
 *
 * ```sh
 * OMEGA_REPO=/path/to/omega \
 * MOBILE_LIVE_RELAY_URL=wss://relay.openagents.com \
 * OMEGA_EFFECTD_BIN=/Applications/Omega.app/Contents/Resources/omega-effectd/bin/omega-effectd \
 *   pnpm --dir apps/openagents-mobile exec vp test --run \
 *   --root ../.. apps/openagents-mobile/tests/issue31-live-host-device.test.ts
 * ```
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { Effect, Stream } from "@effect-native/core/effect";
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import {
  decodeIssue31PairingRecord,
  type Issue31PrivateRecord,
} from "@openagentsinc/sarah/issue31-nostr";

import { buildHomeProgram, renderContentView } from "../src/screens/home-core.ts";
import {
  createIssue31NostrClient,
  type Issue31ConfirmedEvent,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";
import { projectIssue31Workroom } from "../src/workroom/issue31-workroom-projection.ts";

const RELAY_URL = process.env["MOBILE_LIVE_RELAY_URL"]?.trim();
const OMEGA_REPO = process.env["OMEGA_REPO"]?.trim();
const EFFECTD_BIN = process.env["OMEGA_EFFECTD_BIN"]?.trim();
const LIVE = [RELAY_URL, OMEGA_REPO, EFFECTD_BIN].every(
  (value) => value !== undefined && value !== "",
);

/**
 * See the note in `issue31-live-relay.test.ts`: adapt, never widen.
 *
 * A class, not a factory function: the client constructs its sockets with
 * `new`, so a plain arrow function is not a substitute for one.
 */
const LiveSocket = class implements Issue31WebSocketLike {
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: { code?: number; reason?: string }) => void) | null = null;
  readonly #socket: WebSocket;
  constructor(url: string) {
    this.#socket = new WebSocket(url);
    this.#socket.onopen = (event) => this.onopen?.(event);
    this.#socket.onmessage = (event: MessageEvent) => this.onmessage?.({ data: event.data });
    this.#socket.onerror = (event) => this.onerror?.(event);
    this.#socket.onclose = (event: CloseEvent) =>
      this.onclose?.({ code: event.code, reason: event.reason });
  }
  send(data: string): void {
    this.#socket.send(data);
  }
  close(code?: number, reason?: string): void {
    this.#socket.close(code, reason);
  }
} as unknown as new (url: string) => Issue31WebSocketLike;

const memoryCursorStore = (): Issue31RelayCursorStore => {
  const rows = new Map<string, Issue31RelayCursor>();
  const key = (relayUrl: string, room: string) => `${relayUrl}::${room}`;
  return {
    load: async (relayUrl, room) => rows.get(key(relayUrl, room)) ?? null,
    save: async (relayUrl, room, cursor) => {
      rows.set(key(relayUrl, room), cursor);
    },
  };
};

interface PairingHandoff {
  readonly relayUrl: string;
  readonly hostPublicKeyHex: string;
  readonly sarahPublicKeyHex: string;
  readonly devicePublicKeyHex: string;
  readonly grantRef: string;
  readonly pairingRecords: ReadonlyArray<{
    readonly canonicalRecordId: string;
    readonly record: unknown;
  }>;
}

/**
 * The device-authored and host-authored pairing records, as confirmed events.
 *
 * None of these crossed the relay: `record_emitted_pairing` files a record with
 * the host, it does not publish one, and the device's own half is sealed to the
 * host so a relay could never serve it back to its author. The mobile runtime
 * reloads its own half from local storage at launch; this is that half plus the
 * host's, handed over out of band. Everything that matters here — the `host.v1`
 * snapshot and the `fullauto.v1` detail — came off the relay.
 */
const deviceLocalPairing = (
  handoff: PairingHandoff,
): ReadonlyArray<Issue31ConfirmedEvent> =>
  handoff.pairingRecords.map(({ canonicalRecordId, record }, index) => ({
    relayUrl: "device://local",
    room: "owner_private" as const,
    canonicalRecordId,
    privateRumorId: canonicalRecordId,
    privateRecord: decodeIssue31PairingRecord(record) as Issue31PrivateRecord,
    hostAnnouncement: null,
    event: {
      id: canonicalRecordId,
      pubkey: index % 2 === 0 ? handoff.devicePublicKeyHex : handoff.hostPublicKeyHex,
      created_at: Math.floor(Date.now() / 1000) - 10 + index,
      kind: 1_059,
      tags: [["p", handoff.devicePublicKeyHex]],
      content: "device-local",
      sig: "0".repeat(128),
    },
  }));

const waitFor = async (
  predicate: () => boolean,
  label: () => string,
  timeoutMs = 45_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  // The relay's own state is in the message, because "timed out" on its own
  // cannot tell a refused subscription from an empty one.
  throw new Error(`timed out waiting for ${label()}`);
};

const settle = Effect.gen(function* () {
  yield* Effect.yieldNow;
  yield* Effect.yieldNow;
});

describe.skipIf(!LIVE)("a running Omega host, the deployed relay, and this device", () => {
  test("the phone's Workroom rows carry what a real daemon measured", async () => {
    const scratch = mkdtempSync(join(tmpdir(), "omega97-device-"));
    const handoffPath = join(scratch, "pairing.json");
    const deviceSecret = generateSecretKey();
    const devicePublicKey = getPublicKey(deviceSecret);
    const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);

    try {
      // The host half's own live proof, unchanged except for being told which
      // device to address. It spawns the packaged omega-effectd, measures it,
      // and publishes to the deployed relay.
      const host = spawnSync(
        "cargo",
        [
          "test",
          "-p",
          "full_auto_ui",
          "--lib",
          "a_running_daemon_supplies_the_reading_a_paired_device_reads_on_a_live_relay",
          "--",
          "--ignored",
          "--nocapture",
        ],
        {
          cwd: OMEGA_REPO as string,
          encoding: "utf8",
          stdio: ["ignore", "pipe", "pipe"],
          timeout: 900_000,
          env: {
            ...process.env,
            OMEGA_LIVE_RELAY_URL: RELAY_URL as string,
            OMEGA_EFFECTD_BIN: EFFECTD_BIN as string,
            OMEGA_LIVE_DEVICE_PUBKEY: devicePublicKey,
            OMEGA_LIVE_PAIRING_OUT: handoffPath,
          },
        },
      );
      // The daemon's own log lands on stderr, so both streams are the record.
      const hostLog = `${host.stdout ?? ""}${host.stderr ?? ""}`;
      if (host.status !== 0) throw new Error(`the host proof failed:\n${hostLog}`);
      // The daemon answered, and the reading is the daemon's, not the test's.
      expect(hostLog).toContain("answered initialize at generation");
      const measured = /measured reading · generation \d+ · (\d+) run\(s\) · (\d+) lane\(s\)/.exec(
        hostLog,
      );
      if (measured === null) throw new Error(`no measured reading in host log:\n${hostLog}`);
      const measuredRuns = Number(measured[1]);
      const measuredLanes = Number(measured[2]);
      expect(measuredLanes).toBeGreaterThan(0);

      const handoff = JSON.parse(readFileSync(handoffPath, "utf8")) as PairingHandoff;
      expect(handoff.devicePublicKeyHex).toBe(devicePublicKey);

      let latest: Issue31NostrClientSnapshot | null = null;
      const client = createIssue31NostrClient({
        relayUrls: [RELAY_URL as string],
        signer: deviceSigner,
        webSocket: LiveSocket,
        admittedHostPublicKeys: [handoff.hostPublicKeyHex],
        selectedHostPublicKeys: [handoff.hostPublicKeyHex],
        ownerAuthors: [handoff.sarahPublicKeyHex],
        ownerRecipientPublicKeys: [devicePublicKey],
        cursorStore: memoryCursorStore(),
        onSnapshot: (next) => {
          latest = next;
        },
      });
      await client.start();
      try {
        const schemas = (snapshot: Issue31NostrClientSnapshot, schema: string) =>
          snapshot.confirmedEvents.filter((row) => row.privateRecord?.schema === schema);
        await waitFor(
          () =>
            latest !== null &&
            schemas(latest, "openagents.omega.issue31.host.v1").length >= 1 &&
            schemas(latest, "openagents.omega.issue31.fullauto.v1").length >= 1,
          () =>
            `the deployed relay to serve this device the host snapshot and detail (relays: ${JSON.stringify(
              latest === null
                ? null
                : (latest as Issue31NostrClientSnapshot).relays.map((row) => ({
                    state: row.state,
                    gapReason: row.gapReason,
                    rejected: row.rejectedEventCount,
                  })),
            )}, confirmed: ${
              latest === null ? 0 : (latest as Issue31NostrClientSnapshot).confirmedEvents.length
            })`,
        );
        const wire = client.snapshot();
        const snapshot: Issue31NostrClientSnapshot = {
          ...wire,
          confirmedEvents: [...deviceLocalPairing(handoff), ...wire.confirmedEvents],
        };

        const nowUnixSeconds = Math.floor(Date.now() / 1000);
        const projection = projectIssue31Workroom(snapshot, nowUnixSeconds);
        expect(projection.hostBinding.state).toBe("bound");

        // The gap this issue is about, closed against a real host: not one of
        // the three still reports that nothing is connected.
        const hostRows = projection.workroom.rows.filter(
          (row) => row.expectedAuthority === "omega_host_adjunct",
        );
        expect(hostRows).toHaveLength(3);
        for (const row of hostRows) {
          expect(row.source.reasonRef ?? "").not.toContain("source_not_connected");
          expect(row.source.authority).toBe("omega_host_adjunct");
          expect(row.hostObservation).not.toBeNull();
        }

        if (projection.fullAuto.state !== "ready") {
          throw new Error(`expected ready, got ${projection.fullAuto.reason}`);
        }
        // What the daemon actually held. Zero runs is a real observation and a
        // different document from a host that never looked — the section says
        // "reports no Full Auto runs", never "not paired".
        expect(projection.fullAuto.runs).toHaveLength(measuredRuns);
        expect(projection.fullAuto.hostRef).toBe(
          projection.hostBinding.state === "bound" ? projection.hostBinding.host.hostRef : "",
        );

        const program = buildHomeProgram();
        program.workroom.open();
        program.workroom.setReadModel(projection.workroom);
        program.workroom.setFullAutoReadModel(projection.fullAuto);
        await Effect.runPromise(settle);
        const state = await Effect.runPromise(
          Effect.map(Stream.runHead(program.stateChanges), (option) => {
            if (option._tag !== "Some") throw new Error("expected state");
            return option.value;
          }),
        );
        const view = JSON.stringify(renderContentView(state));
        expect(view).toContain(`Host ${projection.fullAuto.hostRef}`);
        expect(view).toContain(`snapshot ${projection.fullAuto.snapshotRef}`);
        expect(view).not.toContain("This device is not paired to an Omega host yet.");
        expect(view).not.toContain(
          "This device is paired, but your Omega host has not published a reading yet.",
        );
        if (measuredRuns === 0) {
          // Stated rather than implied: an empty registry renders as an empty
          // registry. A run to populate these rows has to come from the owner.
          expect(view).toContain("Your Omega host reports no Full Auto runs.");
          expect(view).toContain(
            "No provider accounts were reported. A capacity lane is not an account.",
          );
        }
        // eslint-disable-next-line no-console
        console.log(
          `omega#97 device: ${RELAY_URL} · host ${projection.fullAuto.hostRef} · snapshot ${projection.fullAuto.snapshotRef} · ${measuredRuns} run(s) · ${measuredLanes} lane(s) measured · ${hostRows.length} host rows bound`,
        );
      } finally {
        client.close();
      }
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  }, 900_000);
});
