/**
 * Live relay proof for the mobile Issue 31 Nostr client (OMEGA-MOB-31-01).
 *
 * Opt-in, because it needs the network and a real relay:
 *
 * ```sh
 * MOBILE_LIVE_RELAY_URL=wss://openagents-nostr-relay-ezxz4mgdsq-uc.a.run.app \
 *   pnpm --dir apps/openagents-mobile exec vp test --run tests/issue31-live-relay.test.ts
 * ```
 *
 * Every other test in this suite drives `ScriptedWebSocket`, which proves the
 * state machine but never the wire. The physical-device exit on omega#45
 * cannot be satisfied here, but "the client talks to the real relay" can be,
 * and that is the half a scripted socket can never cover.
 *
 * The relay requires NIP-42 and refuses an auth event whose `relay` tag names
 * a host other than its configured public URL, so this drives the real
 * challenge/response rather than assuming an open relay.
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { openNodeSqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { LocalKeySigner } from "nostr-effect/identity";
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import {
  NIP_29_CREATE_GROUP_KIND,
  NIP_29_GROUP_CHAT_KIND,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  NIP_AP_PERSONA_KIND,
  attachOwnerAttestation,
} from "@openagentsinc/sarah/community";
import type { Issue31SignedNostrEvent } from "@openagentsinc/sarah/issue31-nostr";

import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";
import {
  createIssue31CommunityRecordStore,
  issue31CommunityConfirmedEventsFrom,
  issue31MergeCommunityHistory,
  type Issue31CommunityDatabase,
} from "../src/workroom/issue31-community-record-store.ts";
import { projectIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";

const LIVE_RELAY_URL = process.env.MOBILE_LIVE_RELAY_URL?.trim();

/**
 * Node 24 ships the browser-shaped global `WebSocket`, which is the same
 * runtime shape the client's `webSocket` factory produces on device.
 *
 * It is not *structurally* assignable to `Issue31WebSocketLike`: the interface
 * types its handler arguments as `unknown`, and a `(ev: Event) => void`
 * handler cannot accept `unknown` contravariantly. That is a deliberate
 * property of the interface — it keeps the client from reaching into
 * DOM-specific event fields — so this test adapts rather than widening the
 * client's contract to make an assignment compile.
 */
const openLiveSocket = (url: string): Issue31WebSocketLike => {
  const socket = new WebSocket(url);
  const adapter: Issue31WebSocketLike = {
    onopen: null,
    onmessage: null,
    onerror: null,
    onclose: null,
    send: (data) => socket.send(data),
    close: (code, reason) => socket.close(code, reason),
  };
  socket.onopen = (event) => adapter.onopen?.(event);
  socket.onmessage = (event: MessageEvent) => adapter.onmessage?.({ data: event.data });
  socket.onerror = (event) => adapter.onerror?.(event);
  socket.onclose = (event: CloseEvent) =>
    adapter.onclose?.({ code: event.code, reason: event.reason });
  return adapter;
};

const withSocket = async <A>(
  url: string,
  run: (
    socket: Issue31WebSocketLike,
    next: () => Promise<ReadonlyArray<unknown>>,
  ) => Promise<A>,
): Promise<A> => {
  const socket = openLiveSocket(url);
  const inbox: Array<ReadonlyArray<unknown>> = [];
  let waiting: ((frame: ReadonlyArray<unknown>) => void) | null = null;
  socket.onmessage = ({ data }) => {
    const frame = JSON.parse(String(data)) as ReadonlyArray<unknown>;
    if (waiting) {
      const resolve = waiting;
      waiting = null;
      resolve(frame);
      return;
    }
    inbox.push(frame);
  };
  const next = (): Promise<ReadonlyArray<unknown>> =>
    new Promise((resolve, reject) => {
      const buffered = inbox.shift();
      if (buffered) {
        resolve(buffered);
        return;
      }
      const timer = setTimeout(() => reject(new Error("relay frame timeout")), 15_000);
      waiting = (frame) => {
        clearTimeout(timer);
        resolve(frame);
      };
    });
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("relay connect timeout")), 15_000);
    socket.onopen = () => {
      clearTimeout(timer);
      resolve();
    };
    socket.onerror = (error) => {
      clearTimeout(timer);
      reject(error instanceof Error ? error : new Error("relay socket error"));
    };
  });
  try {
    return await run(socket, next);
  } finally {
    socket.close(1000, "done");
  }
};

describe.skipIf(LIVE_RELAY_URL === undefined || LIVE_RELAY_URL === "")(
  "mobile Issue 31 client against a live relay",
  () => {
    test("authenticates with NIP-42 and round-trips a signed event", async () => {
      const url = LIVE_RELAY_URL as string;
      const device = LocalKeySigner.fromPrivateKey(generateSecretKey());

      await withSocket(url, async (socket, next) => {
        // The relay challenges proactively on open.
        const challengeFrame = await next();
        expect(challengeFrame[0]).toBe("AUTH");
        const challenge = challengeFrame[1] as string;
        expect(typeof challenge).toBe("string");

        const authEvent = await device.signEvent({
          kind: 22242,
          created_at: Math.floor(Date.now() / 1000),
          tags: [
            ["relay", url],
            ["challenge", challenge],
          ],
          content: "",
        });
        socket.send(JSON.stringify(["AUTH", authEvent]));

        const authOk = await next();
        expect(authOk[0]).toBe("OK");
        expect(authOk[2]).toBe(true);

        // A device-signed record the client would later project.
        const marker = `mobile-live-${device.publicKey.slice(0, 12)}`;
        const record = await device.signEvent({
          kind: 1,
          created_at: Math.floor(Date.now() / 1000),
          tags: [["t", "oa-mobile-live"]],
          content: marker,
        });
        socket.send(JSON.stringify(["EVENT", record]));

        const publishOk = await next();
        expect(publishOk[0]).toBe("OK");
        expect(publishOk[2]).toBe(true);

        socket.send(
          JSON.stringify(["REQ", "live", { authors: [device.publicKey], kinds: [1], limit: 5 }]),
        );

        const seen: Array<string> = [];
        for (let frame = await next(); frame[0] !== "EOSE"; frame = await next()) {
          if (frame[0] === "EVENT") {
            seen.push((frame[2] as { content: string }).content);
          }
        }
        expect(seen).toContain(marker);
      });
    }, 40_000);

    /**
     * The community restart property, on the deployed relay (omega#48).
     *
     * The in-process `startTestRelay` proof of this lives in
     * `issue31-community-persistence.test.ts`. Both relays are real; this one is
     * the one the app actually talks to, with NIP-42 in front of it, so the
     * claim "a revocation survives a restart the relay will not replay" is made
     * about the deployment rather than only about a relay we start ourselves.
     *
     * Still not a phone. The omega#49 exits stand.
     */
    test("a revocation survives a restart the deployed relay will not replay", async () => {
      const url = LIVE_RELAY_URL as string;
      const group = `oa.omega.issue31.community.${Math.random().toString(16).slice(2, 10)}`;
      const admin = liveParty();
      const operator = liveParty();
      const agent = liveParty();
      const directory = mkdtempSync(join(tmpdir(), "issue31-live-community-"));
      const databasePath = join(directory, "community.db");
      const cursorStore = makeSharedCursorStore();
      const base = Math.floor(Date.now() / 1_000) - 3_600;

      const attestation = attachOwnerAttestation({
        agentPubkey: agent.pubkey,
        operatorSeckeyHex: operator.secretKeyHex,
      });
      // NIP-29 moderation is scoped to a group that exists. A relay answering
      // `kind 9000` for an id it has never seen with
      // `restricted: group not found` is stating a missing precondition, not a
      // missing capability — omega#49 recorded that refusal as the deployed
      // relay being unable to host a community room, and it is not.
      const created = liveSign(admin.secretKey, {
        kind: NIP_29_CREATE_GROUP_KIND,
        created_at: base - 1,
        tags: [["h", group]],
      });
      const admitted = liveSign(admin.secretKey, {
        kind: NIP_29_PUT_USER_KIND,
        created_at: base,
        tags: [["h", group], ["p", operator.pubkey]],
      });
      const persona = liveSign(agent.secretKey, {
        kind: NIP_AP_PERSONA_KIND,
        created_at: base + 1,
        tags: [["d", "worker"], ["h", group], [...attestation]],
      });
      const revocation = liveSign(admin.secretKey, {
        kind: NIP_29_REMOVE_USER_KIND,
        created_at: base + 2,
        tags: [["h", group], ["p", operator.pubkey]],
      });
      // Traffic after the removal, so the replay cursor advances past it.
      const laterChatter = liveSign(admin.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: base + 600,
        tags: [["h", group]],
        content: "room carries on after the removal",
      });

      const config = {
        groupId: group,
        adminPubkeys: [admin.pubkey],
        scorerPubkeys: [] as ReadonlyArray<string>,
        ownerAppealPubkey: null,
        viewerPubkey: operator.pubkey,
        nowUnixSeconds: Math.floor(Date.now() / 1_000),
      };

      try {
        // The group writes are the admin's, so the session must be the admin's.
        await publishLive(url, [created, admitted, revocation, laterChatter], admin.secretKey);
        await publishLive(url, [persona], agent.secretKey);

        let first: Issue31NostrClientSnapshot | null = null;
        const firstClient = liveClient(url, cursorStore, group, (next) => {
          first = next;
        });
        const firstStore = createIssue31CommunityRecordStore({
          database: openLiveDatabase(databasePath),
          groupId: group,
        });
        try {
          await firstClient.start();
          await liveWaitFor(() => {
            const ids = new Set(
              (((): Issue31NostrClientSnapshot | null => first)()?.confirmedEvents ?? []).map(
                (row) => row.event.id,
              ),
            );
            return [admitted, persona, revocation, laterChatter].every((event) => ids.has(event.id));
          }, "the deployed relay to return the four community records");
          for (const row of (
            ((): Issue31NostrClientSnapshot | null => first)()?.confirmedEvents ?? []
          ).filter((row) => row.room === "community")) {
            firstStore.put(row.event);
          }
          const live = projectIssue31CommunityReadModel(
            ((): Issue31NostrClientSnapshot => first!)(),
            config,
          );
          expect(live.viewerRoleStatus).toBe("revoked");
        } finally {
          firstClient.close();
          firstStore.close();
        }

        const secondStore = createIssue31CommunityRecordStore({
          database: openLiveDatabase(databasePath),
          groupId: group,
        });
        const restored = secondStore.load();
        // The four records this test authored must all survive the restart.
        // The total is deliberately not asserted: now that the group genuinely
        // exists on the relay, the relay also serves its own NIP-29 `39xxx`
        // group-state events, which the fold reads as corroboration and never
        // as authority. Pinning an exact count would make a correct relay look
        // like drift.
        // `created` is the group's precondition, not a community record the
        // client admits, so it is deliberately not in this set.
        for (const authored of [admitted, persona, revocation, laterChatter]) {
          expect(restored.some((row) => row.id === authored.id)).toBe(true);
        }
        expect(restored.length).toBeGreaterThanOrEqual(4);

        const now = Math.floor(Date.now() / 1_000);
        const readmitted = liveSign(admin.secretKey, {
          kind: NIP_29_PUT_USER_KIND,
          created_at: now,
          tags: [["h", group], ["p", operator.pubkey]],
        });
        const personaAgain = liveSign(agent.secretKey, {
          kind: NIP_AP_PERSONA_KIND,
          created_at: now + 1,
          tags: [["d", "worker"], ["h", group], [...attestation]],
        });
        await publishLive(url, [readmitted], admin.secretKey);
        await publishLive(url, [personaAgain], agent.secretKey);

        let second: Issue31NostrClientSnapshot | null = null;
        const secondClient = liveClient(url, cursorStore, group, (next) => {
          second = next;
        });
        try {
          await secondClient.start();
          await liveWaitFor(() => {
            const ids = new Set(
              (((): Issue31NostrClientSnapshot | null => second)()?.confirmedEvents ?? []).map(
                (row) => row.event.id,
              ),
            );
            return ids.has(readmitted.id) && ids.has(personaAgain.id);
          }, "the deployed relay to return the re-invitation");

          const fresh = ((): Issue31NostrClientSnapshot => second!)();
          const freshIds = new Set(fresh.confirmedEvents.map((row) => row.event.id));
          expect(freshIds.has(revocation.id)).toBe(false);

          // The shipped behaviour before this change, on the deployed relay.
          const withoutHistory = projectIssue31CommunityReadModel(fresh, config);
          expect(withoutHistory.viewerRole).toBe("agent_operator");

          const merged = projectIssue31CommunityReadModel(
            issue31MergeCommunityHistory(fresh, issue31CommunityConfirmedEventsFrom(restored)),
            config,
          );
          expect(merged.agents.map((row) => row.agentPubkey)).not.toContain(agent.pubkey);
          expect(merged.viewerRole).toBe("member");
        } finally {
          secondClient.close();
          secondStore.close();
        }
      } finally {
        rmSync(directory, { recursive: true, force: true });
      }
    }, 120_000);
  },
);

const liveParty = () => {
  const secretKey = generateSecretKey();
  return {
    secretKey,
    secretKeyHex: [...secretKey].map((b) => b.toString(16).padStart(2, "0")).join(""),
    pubkey: getPublicKey(secretKey),
  };
};

const liveSign = (
  secretKey: Uint8Array,
  input: Readonly<{
    kind: number;
    created_at: number;
    tags: ReadonlyArray<ReadonlyArray<string>>;
    content?: string;
  }>,
): Issue31SignedNostrEvent =>
  finalizeEvent(
    {
      kind: input.kind,
      created_at: input.created_at,
      tags: input.tags.map((tag) => [...tag]),
      content: input.content ?? "",
    },
    secretKey,
  ) as unknown as Issue31SignedNostrEvent;

/** One cursor store across both launches, because the device has one. */
const makeSharedCursorStore = (): Issue31RelayCursorStore => {
  const rows = new Map<string, Issue31RelayCursor>();
  const key = (relayUrl: string, room: string) => `${relayUrl}::${room}`;
  return {
    load: async (relayUrl, room) => rows.get(key(relayUrl, room)) ?? null,
    save: async (relayUrl, room, cursor) => {
      rows.set(key(relayUrl, room), cursor);
    },
  };
};

const openLiveDatabase = (path: string): Issue31CommunityDatabase => {
  const database = openNodeSqliteDatabase(path);
  return {
    execSync: (sql) => database.exec(sql),
    runSync: (sql, ...params) => database.run(sql, [...params]),
    getAllSync: <Row,>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.all<Row>(sql, [...params]),
    closeSync: () => database.close(),
  };
};

const liveWaitFor = async (
  predicate: () => boolean,
  label: string,
  timeoutMs = 40_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${label}`);
};

/**
 * Publish through one NIP-42-authenticated socket and wait for each OK.
 *
 * `authAs` matters for NIP-29. The relay authorises a group write against the
 * *authenticated* pubkey, not the event author, so publishing an admin's
 * moderation event over a session authenticated as a throwaway key is refused
 * with `auth-required: NIP-29 group write`. Records outside a group are
 * unaffected, which is why the default stays a fresh key.
 */
const publishLive = async (
  url: string,
  events: ReadonlyArray<Issue31SignedNostrEvent>,
  authAs?: Uint8Array,
): Promise<void> => {
  const publisher = LocalKeySigner.fromPrivateKey(authAs ?? generateSecretKey());
  await withSocket(url, async (socket, next) => {
    const challengeFrame = await next();
    if (challengeFrame[0] !== "AUTH") throw new Error("expected an AUTH challenge");
    const auth = await publisher.signEvent({
      kind: 22242,
      created_at: Math.floor(Date.now() / 1_000),
      tags: [
        ["relay", url],
        ["challenge", challengeFrame[1] as string],
      ],
      content: "",
    });
    socket.send(JSON.stringify(["AUTH", auth]));
    const authOk = await next();
    if (authOk[0] !== "OK" || authOk[2] !== true) {
      throw new Error(`relay refused AUTH: ${JSON.stringify(authOk)}`);
    }
    for (const event of events) {
      socket.send(JSON.stringify(["EVENT", event]));
      const ok = await next();
      if (ok[0] !== "OK" || ok[1] !== event.id || ok[2] !== true) {
        throw new Error(`relay refused kind ${event.kind}: ${JSON.stringify(ok)}`);
      }
    }
  });
};

const liveClient = (
  url: string,
  cursorStore: Issue31RelayCursorStore,
  groupId: string,
  onSnapshot: (snapshot: Issue31NostrClientSnapshot) => void,
) =>
  createIssue31NostrClient({
    relayUrls: [url],
    signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
    webSocket: (class {
      constructor(socketUrl: string) {
        return openLiveSocket(socketUrl) as never;
      }
    } as unknown) as new (url: string) => Issue31WebSocketLike,
    admittedHostPublicKeys: [],
    communityGroupIds: [groupId],
    cursorStore,
    onSnapshot,
  });
