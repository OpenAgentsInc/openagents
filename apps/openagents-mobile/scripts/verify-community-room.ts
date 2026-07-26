/**
 * Read a seeded community room back off its relay and print what it projects to.
 *
 * ```sh
 * node --import tsx apps/openagents-mobile/scripts/verify-community-room.ts \
 *   /path/to/community-room.json
 * ```
 *
 * This is the check a simulator run makes implicitly when it is pointed at a
 * group id: that the room named by the file is still on the relay, still
 * readable with only the **public** keys the file carries, and still projects to
 * the same journey. It signs nothing and publishes nothing — every key in the
 * file is public, and a reader needs no more than that.
 *
 * A room can stop matching its file for an honest reason: the relay enforces
 * NIP-40, so units stop being served once their grants lapse. A shrinking record
 * count is that, not tampering.
 */
import { readFileSync } from "node:fs";

import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey } from "nostr-effect/pure";

import { projectIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";
import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";

interface RoomFile {
  readonly relayUrl: string;
  readonly groupId: string;
  readonly adminPubkeys: ReadonlyArray<string>;
  readonly scorerPubkeys: ReadonlyArray<string>;
  readonly ownerAppealPubkey: string;
  readonly rolePubkeys: Readonly<Record<string, string>>;
  readonly units: Readonly<Record<string, { readonly unitRef: string }>>;
  readonly expectedProducerExperiencePoints: number;
}

const NodeSocket = class implements Issue31WebSocketLike {
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

const main = async (): Promise<void> => {
  const path = process.argv[2];
  if (path === undefined) throw new Error("usage: verify-community-room.ts <community-room.json>");
  const room = JSON.parse(readFileSync(path, "utf8")) as RoomFile;

  let snapshot: Issue31NostrClientSnapshot | null = null;
  const client = createIssue31NostrClient({
    relayUrls: [room.relayUrl],
    signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
    webSocket: NodeSocket,
    admittedHostPublicKeys: [],
    communityGroupIds: [room.groupId],
    communityAuthors: [...room.scorerPubkeys],
    cursorStore: { load: async () => null, save: async () => {} },
    onSnapshot: (next) => {
      snapshot = next;
    },
  });
  await client.start();
  const deadline = Date.now() + 45_000;
  for (;;) {
    const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
    if ((current?.confirmedEvents.length ?? 0) >= 66 || Date.now() > deadline) break;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  client.close();
  const settled = ((): Issue31NostrClientSnapshot | null => snapshot)();
  if (settled === null) throw new Error("the client produced no snapshot");

  const now = Math.floor(Date.now() / 1_000);
  const configFor = (viewerPubkey: string) => ({
    groupId: room.groupId,
    adminPubkeys: [...room.adminPubkeys],
    scorerPubkeys: [...room.scorerPubkeys],
    ownerAppealPubkey: room.ownerAppealPubkey,
    viewerPubkey,
    nowUnixSeconds: now,
    transcriptLimit: 200,
  });
  const model = projectIssue31CommunityReadModel(
    settled,
    configFor(room.rolePubkeys["producerOperator"] ?? ""),
  );
  const revoked = projectIssue31CommunityReadModel(
    settled,
    configFor(room.rolePubkeys["departingOperator"] ?? ""),
  );
  const unit = (key: string) =>
    model.workUnits.find((row) => row.unitRef === room.units[key]?.unitRef);

  const lines = [
    `relay                 ${room.relayUrl}`,
    `group id (file)       ${room.groupId}`,
    `group id (projected)  ${model.groupId}`,
    `records off the wire  ${settled.confirmedEvents.length}`,
    `roster                ${model.roster.length} members`,
    `transcript            ${model.transcript.length} messages`,
    `work units            ${model.workUnits.length}`,
    `accepted quotes       ${unit("accepted")?.quotes.filter((row) => row.accepted).length}`,
    `independent verify    ${unit("accepted")?.verification?.operatorsAreIndependent}`,
    `self-verify refusal   ${unit("selfVerified")?.verification?.refusalReason}`,
    `refused deliveries    ${model.resultRefusals.map((row) => row.code).join(",")}`,
    `replayed grant        ${model.refusals.find((row) => row.code === "agent_key_burned")?.code}`,
    `revoked viewer        ${revoked.viewerRoleStatus}, ${revoked.controls.length} controls`,
    `experience            ${model.experience.recomputedTotalPoints} recomputed / ${model.experience.publishedRankPoints} published`,
  ];
  for (const line of lines) process.stdout.write(`${line}\n`);

  const agrees = room.groupId === model.groupId && model.roster.length > 0;
  process.stdout.write(`\nfile and room agree:  ${agrees}\n`);
  if (!agrees) process.exitCode = 1;
};

await main();
