/**
 * Seed one real community room on a real relay, and say what the relay said.
 *
 * ```sh
 * node --import tsx apps/openagents-mobile/scripts/seed-community-room.ts \
 *   --relay wss://relay.openagents.com \
 *   --out /tmp/community-room.json
 * ```
 *
 * The room this produces is the one a simulator run can be pointed at: the
 * written file carries the group id and every role's **public** key, and nothing
 * else. Secrets are generated per run, live only in this process, and are never
 * written, logged, or published — a room a stranger can read is the point, and a
 * room whose keys leaked would not be one.
 */
import { writeFileSync } from "node:fs";

import {
  assertSeedReachedRelay,
  buildCommunityRoomSeed,
  publishCommunityRoomSeed,
  type CommunityRoomSeed,
} from "./community-room-producer.ts";

const argOf = (name: string): string | undefined => {
  const index = process.argv.indexOf(`--${name}`);
  return index === -1 ? undefined : process.argv[index + 1];
};

const rolePubkeys = (seed: CommunityRoomSeed): Readonly<Record<string, string>> =>
  Object.fromEntries(Object.entries(seed.cast).map(([role, party]) => [role, party.pubkey]));

const main = async (): Promise<void> => {
  const relayUrl = argOf("relay") ?? "wss://relay.openagents.com";
  const outPath = argOf("out");
  const groupId = argOf("group");

  const seed = buildCommunityRoomSeed(groupId === undefined ? {} : { groupId });
  const results = await publishCommunityRoomSeed({ relayUrl, seed });

  for (const row of results) {
    const mark = row.accepted ? "ok " : "REFUSED";
    process.stdout.write(`${mark} ${row.label} (kind ${row.kind}) ${row.message}\n`);
  }
  assertSeedReachedRelay(seed, results);

  const record = {
    schema: "openagents.omega.issue49.community_room.v1",
    relayUrl,
    groupId: seed.groupId,
    // Public keys only. The out-of-band admin and scorer sets a reader must be
    // configured with are named here because a relay may never supply them.
    adminPubkeys: [seed.cast.admin.pubkey],
    scorerPubkeys: [seed.cast.scorer.pubkey],
    ownerAppealPubkey: seed.cast.ownerAppeal.pubkey,
    rolePubkeys: rolePubkeys(seed),
    units: seed.units,
    revocationAtUnix: seed.revocationAtUnix,
    expiredUnitProjectionNowUnix: seed.expiredUnitProjectionNowUnix,
    expectedProducerExperiencePoints: seed.expectedProducerExperiencePoints,
    experienceOnly: "v1 awards experience points only and pays no money.",
    seededAtUnix: seed.nowUnixSeconds,
    notes: [
      "Point a reader at groupId with adminPubkeys/scorerPubkeys/ownerAppealPubkey above; the relay supplies none of them and must not be asked to.",
      "The relay enforces NIP-40, so units.expired stops being served once expiresAtUnix passes, and every other unit once liveUntil passes.",
      "Probed on this deployment: a group write from a removed key is still carried. Revocation is enforced by the room's projection, never by the transport.",
    ],
  };
  const json = `${JSON.stringify(record, null, 2)}\n`;
  if (outPath !== undefined) writeFileSync(outPath, json);
  process.stdout.write(json);
};

await main();
