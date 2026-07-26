/**
 * The omega#49 community journey, walked on a real relay.
 *
 * The previous lane closed this exit as blocked on two grounds: "no community
 * producer was stood up", and the deployed relay refusing `kind 9000` with
 * `restricted: group not found`. The second was a misread — the group had never
 * been created, and `kind 9007` create-group is admitted for an id that does not
 * exist yet. The first is what
 * `apps/openagents-mobile/scripts/community-room-producer.ts` now supplies.
 *
 * These tests do not construct records and hand them to a projection. They ask a
 * producer to publish a complete room to a relay, read it back **through the
 * shipped client's own subscription**, and project what came off the wire. A
 * record that the client's filters would not have requested cannot pass here,
 * because nothing hands it to the client.
 *
 * ## Two relays, one journey
 *
 * The same suite runs twice:
 *
 * - **`startTestRelay`** (always). A real relay speaking the real protocol
 *   in-process: real framing, real storage, real `#h` matching, real EOSE. It is
 *   not `MockRelayAdapter` and no fixture is involved. It keeps the journey out
 *   of the network's hands in CI.
 * - **the deployed relay** (`MOBILE_LIVE_RELAY_URL`). `wss://relay.openagents.com`
 *   with NIP-42 in front of it and a NIP-29 group policy behind it. This is the
 *   relay the app actually talks to.
 *
 * Every assertion below is made against both. Neither is a phone; the omega#49
 * exits that name a physical device still need one.
 *
 * ## What the relay is, and is not
 *
 * Probing the deployment showed it carries a group write from a key the admin
 * had already removed, exactly as it carries any other. That is recorded rather
 * than worked around: it is the contract's position stated by the transport. The
 * relay grants no membership, so it cannot withdraw one, and every admission,
 * acceptance, independence and refusal in this file is re-derived by the reader
 * from signatures. A relay `OK: true` is never an OpenAgents admission — the
 * seed contains four records the relay stores and the room must refuse, and each
 * is asserted refused below.
 */
import {
  buildCommunitySarahContext,
  isUntrustedCommunityContent,
} from "@openagentsinc/sarah/community";
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey } from "nostr-effect/pure";
import { startTestRelay } from "nostr-effect/relay/node";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  assertSeedReachedRelay,
  buildCommunityRoomSeed,
  publishCommunityRoomSeed,
  type CommunityPublishResult,
  type CommunityRoomSeed,
} from "../scripts/community-room-producer.ts";
import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";
import {
  issue31CommunityUntrustedBlocks,
  projectIssue31CommunityReadModel,
  type Issue31CommunityControl,
  type Issue31CommunityControlKind,
  type Issue31CommunityProjectionConfig,
  type Issue31CommunityReadModel,
} from "../src/workroom/issue31-community-read-model.ts";

const LIVE_RELAY_URL = process.env["MOBILE_LIVE_RELAY_URL"]?.trim();

/**
 * Node 24's browser-shaped global `WebSocket`, adapted to the client's
 * interface.
 *
 * `Issue31WebSocketLike` types its handler arguments as `unknown` so the client
 * cannot reach into DOM-specific event fields, which makes the global socket
 * non-assignable. That is a deliberate property of the interface, so this
 * adapts rather than widening the client's contract to make an assignment
 * compile.
 */
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

const waitFor = async (
  predicate: () => boolean,
  label: string,
  timeoutMs: number,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${label}`);
};

interface JourneyRun {
  readonly relayLabel: string;
  readonly seed: CommunityRoomSeed;
  readonly publishResults: ReadonlyArray<CommunityPublishResult>;
  readonly snapshot: Issue31NostrClientSnapshot;
}

/**
 * Seed one room and read it back through the client's own subscription.
 *
 * The read half matters as much as the write half. Nothing here injects a frame
 * into the client: it opens its own socket, sends its own `#h` and author
 * filters, and every record asserted on below is one it asked the relay for.
 */
const walkCommunityJourney = async (relayUrl: string, relayLabel: string): Promise<JourneyRun> => {
  const seed = buildCommunityRoomSeed();
  const publishResults = await publishCommunityRoomSeed({ relayUrl, seed });
  assertSeedReachedRelay(seed, publishResults);

  let snapshot: Issue31NostrClientSnapshot | null = null;
  const client = createIssue31NostrClient({
    relayUrls: [relayUrl],
    signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
    webSocket: NodeSocket,
    admittedHostPublicKeys: [],
    communityGroupIds: [seed.groupId],
    // Awards, rank and badges are author-scoped: only the admitted scorer key
    // publishes them, so a member labelling their own score is never stored.
    communityAuthors: [seed.cast.scorer.pubkey],
    cursorStore: memoryCursorStore(),
    onSnapshot: (next) => {
      snapshot = next;
    },
  });
  const expectedIds = new Set(seed.events.map((row) => row.event.id));
  try {
    await client.start();
    await waitFor(
      () => {
        const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
        const seen = new Set((current?.confirmedEvents ?? []).map((row) => row.event.id));
        for (const id of expectedIds) {
          if (!seen.has(id)) return false;
        }
        return true;
      },
      `${relayLabel} to return all ${expectedIds.size} seeded community records`,
      90_000,
    );
  } finally {
    client.close();
  }
  const settled = ((): Issue31NostrClientSnapshot | null => snapshot)();
  if (settled === null) throw new Error("the client produced no snapshot");
  return { relayLabel, seed, publishResults, snapshot: settled };
};

const controlKinds = (
  controls: ReadonlyArray<Issue31CommunityControl>,
): ReadonlyArray<Issue31CommunityControlKind> => controls.map((control) => control.kind);

const unitByRef = (model: Issue31CommunityReadModel, unitRef: string) => {
  const unit = model.workUnits.find((row) => row.unitRef === unitRef);
  if (unit === undefined) throw new Error(`the room did not render ${unitRef}`);
  return unit;
};

/**
 * Register the whole journey against one relay.
 *
 * The suite is a factory rather than two copies so a claim cannot hold on one
 * relay and quietly rot on the other.
 */
const registerCommunityJourney = (input: {
  readonly label: string;
  readonly skip: boolean;
  readonly setUp: () => Promise<string>;
  readonly tearDown?: () => Promise<void>;
}): void => {
  describe.skipIf(input.skip)(`the omega#49 community journey on ${input.label}`, () => {
    let run: JourneyRun;

    beforeAll(async () => {
      run = await walkCommunityJourney(await input.setUp(), input.label);
    }, 300_000);

    afterAll(async () => {
      await input.tearDown?.();
    });

    const config = (
      viewerPubkey: string | null,
      overrides: Partial<Issue31CommunityProjectionConfig> = {},
    ): Issue31CommunityProjectionConfig => ({
      groupId: run.seed.groupId,
      // Group admin authority is supplied out of band. The relay never gets a
      // vote on who admits whom, so it is not asked.
      adminPubkeys: [run.seed.cast.admin.pubkey],
      scorerPubkeys: [run.seed.cast.scorer.pubkey],
      ownerAppealPubkey: run.seed.cast.ownerAppeal.pubkey,
      viewerPubkey,
      nowUnixSeconds: run.seed.nowUnixSeconds,
      transcriptLimit: 200,
      ...overrides,
    });

    /** The same relay-returned records, read as of an earlier point in time. */
    const projectAsOf = (
      viewerPubkey: string | null,
      untilUnix: number,
    ): Issue31CommunityReadModel =>
      projectIssue31CommunityReadModel(
        {
          ...run.snapshot,
          confirmedEvents: run.snapshot.confirmedEvents.filter(
            (row) => row.event.created_at < untilUnix,
          ),
        },
        config(viewerPubkey, { nowUnixSeconds: untilUnix }),
      );

    test("the relay carried every record, and granted nothing by carrying it", () => {
      // The write half. Each of the 60 records — including the four the room
      // must refuse, and the one signed by an already-removed key — was stored.
      for (const result of run.publishResults) {
        expect(result.accepted, `${result.label} (kind ${result.kind}): ${result.message}`).toBe(
          true,
        );
      }
      // The read half. Nothing was handed to the client; it asked for all of it.
      const seen = new Set(run.snapshot.confirmedEvents.map((row) => row.event.id));
      for (const row of run.seed.events) {
        expect(seen.has(row.event.id), `${row.label} did not come back off the wire`).toBe(true);
      }
      // And the room refuses four of the things the relay happily carried.
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      expect(model.refusals.map((refusal) => refusal.code)).toContain("agent_key_burned");
      expect(
        unitByRef(model, run.seed.units.accepted.unitRef).quotes.filter((quote) => quote.accepted),
      ).toHaveLength(1);
      expect(
        unitByRef(model, run.seed.units.selfVerified.unitRef).verification?.refusalReason,
      ).toBe("self_dealing_operators");
      expect(
        projectIssue31CommunityReadModel(
          run.snapshot,
          config(run.seed.cast.departingOperator.pubkey),
        ).viewerRoleStatus,
      ).toBe("revoked");
    });

    test("the roster and the transcript render from what the relay returned", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      expect(model.groupId).toBe(run.seed.groupId);

      const roster = model.roster.map((row) => row.operatorPubkey);
      for (const operator of [
        run.seed.cast.sarah,
        run.seed.cast.producerOperator,
        run.seed.cast.verifierOperator,
        run.seed.cast.soloOperator,
        run.seed.cast.departingOperator,
      ]) {
        expect(roster).toContain(operator.pubkey);
      }
      // Every membership row came from an admin-signed record; nobody joined.
      expect(roster).not.toContain(run.seed.cast.admin.pubkey);

      // Agents are bound to operators by a NIP-OA signature the *operator*
      // made, verified off the persona rather than taken from the relay.
      const binding = (agentPubkey: string) =>
        model.agents.find((row) => row.agentPubkey === agentPubkey)?.operatorPubkey;
      expect(binding(run.seed.cast.producerAgent.pubkey)).toBe(
        run.seed.cast.producerOperator.pubkey,
      );
      expect(binding(run.seed.cast.verifierAgent.pubkey)).toBe(
        run.seed.cast.verifierOperator.pubkey,
      );
      expect(binding(run.seed.cast.soloAgentA.pubkey)).toBe(run.seed.cast.soloOperator.pubkey);
      expect(binding(run.seed.cast.soloAgentB.pubkey)).toBe(run.seed.cast.soloOperator.pubkey);

      const transcript = model.transcript.map((row) => row.displayText);
      for (const line of run.seed.transcriptLines) {
        expect(transcript).toContain(line);
      }
      expect(model.viewerRole).toBe("agent_operator");
      expect(controlKinds(model.controls)).toContain("post_message");
      expect(model.experienceOnlyCopy).toMatch(/experience points only/i);
    });

    test("the work unit renders with the exact bounds it was granted", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      const seeded = run.seed.units.accepted;
      const unit = unitByRef(model, seeded.unitRef);

      expect(unit.requestEventId).toBe(seeded.requestEventId);
      expect(unit.grantRef).toBe(seeded.grantRef);
      expect(unit.idempotencyRef).toBe(seeded.idempotencyRef);
      expect(unit.targetRefs).toEqual(seeded.targetRefs);
      // The permitted actions are the fence. Rendering a superset would be a
      // wider grant than the one that was signed.
      expect(unit.allowedActionRefs).toEqual(seeded.allowedActionRefs);
      expect(unit.expiresAtUnix).toBe(seeded.expiresAtUnix);
      expect(unit.expired).toBe(false);
      // v1 pays nothing, so the room shows an experience tier and never a price.
      expect(unit.experienceTierCopy).toMatch(/no payment in v1/i);
      expect(unit.experienceTierCopy).toMatch(/2 permitted actions/);
    });

    test("exactly one provider is accepted, and never by themselves", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      const unit = unitByRef(model, run.seed.units.accepted.unitRef);

      expect(unit.quotes).toHaveLength(2);
      const accepted = unit.quotes.filter((quote) => quote.accepted);
      expect(accepted).toHaveLength(1);
      expect(accepted[0]?.providerPubkey).toBe(run.seed.cast.producerAgent.pubkey);
      expect(unit.acceptedProviderPubkey).toBe(run.seed.cast.producerAgent.pubkey);

      // The rival quote was "accepted" by a kind-7000 the *provider* signed. The
      // relay stored it; acceptance is read only from the key that requested the
      // work, so it changed nothing.
      const rival = unit.quotes.find(
        (quote) => quote.providerPubkey === run.seed.cast.verifierAgent.pubkey,
      );
      expect(rival?.accepted).toBe(false);
    });

    test("a result is verified only by a distinct operator", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      const unit = unitByRef(model, run.seed.units.accepted.unitRef);

      expect(unit.result?.providerPubkey).toBe(run.seed.cast.producerAgent.pubkey);
      expect(unit.verification?.verifierSigned).toBe(true);
      expect(unit.verification?.operatorsAreIndependent).toBe(true);
      expect(unit.verification?.refusalReason).toBeNull();
      expect(unit.verification?.verdict).toBe("reproduced");
      // Both operators are resolved from the folded binding, never from the
      // verification's own tags.
      expect(unit.verification?.verifierOperatorPubkey).toBe(run.seed.cast.verifierOperator.pubkey);
      expect(unit.verification?.producerOperatorPubkey).toBe(run.seed.cast.producerOperator.pubkey);
      expect(unit.decision?.outcome).toBe("accepted");
      expect(unit.lifecycle).toBe("decided");
    });

    test("a self-verified result is refused, though every key differs", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.soloOperator.pubkey),
      );
      const unit = unitByRef(model, run.seed.units.selfVerified.unitRef);

      // Producer and verifier are different agent keys, so every key comparison
      // passes. They are the same operator, which only the record can say.
      expect(run.seed.cast.soloAgentA.pubkey).not.toBe(run.seed.cast.soloAgentB.pubkey);
      expect(unit.verification?.verifierSigned).toBe(true);
      expect(unit.verification?.operatorsAreIndependent).toBe(false);
      expect(unit.verification?.refusalReason).toBe("self_dealing_operators");
      // The unit does not advance on a refused verification.
      expect(unit.lifecycle).toBe("delivered");
      expect(unit.lifecycle).not.toBe("verified");
    });

    test("a replayed grant is refused and its replayed result verifies nothing", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.departingOperator.pubkey),
      );

      // The attacker re-used the operator's own NIP-OA signature — the exact
      // `auth` tag bytes that granted this agent — on a fresh persona after the
      // revocation. The burn set is monotonic, so the replay buys nothing.
      const replayRefusal = model.refusals.find((refusal) => refusal.code === "agent_key_burned");
      expect(replayRefusal).toBeDefined();
      expect(replayRefusal?.kind).toBe(30175);

      const agent = model.agents.find(
        (row) => row.agentPubkey === run.seed.cast.departingAgent.pubkey,
      );
      expect(agent?.status).toBe("revoked");
      expect(agent?.burned).toBe(true);
      expect(agent?.capabilityGrant).toBe("revoked");

      // The delivery was re-sent by that burned key. It is a re-send, not a
      // substitution — same provider, summary and idempotency ref — so the
      // original stands and nothing is refused for it.
      const unit = unitByRef(model, run.seed.units.revoked.unitRef);
      expect(unit.result?.sourceEventId).toBe(run.seed.revokedLane.standingResultEventId);
      expect(unit.result?.sourceEventId).not.toBe(run.seed.revokedLane.resentResultEventId);
      expect(unit.refusedResults).toEqual([]);

      // A genuinely independent verifier signed a verification of that delivery
      // after the revocation. It is still refused: the record no longer binds the
      // producing key to any operator, so independence is unknowable rather than
      // merely absent.
      expect(unit.verification?.operatorsAreIndependent).toBe(false);
      expect(unit.verification?.refusalReason).toBe("unknown_operator");
      expect(unit.lifecycle).not.toBe("verified");
    });

    test("a substituted result is refused, and a re-sent one is not", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      const unit = unitByRef(model, run.seed.units.substituted.unitRef);
      const swap = run.seed.substitution;

      // Nothing about this lane is revoked or expired: the grant is live and
      // the provider's key is unburned. The grant-layer burn set has no opinion
      // here, so only a check on the delivery itself can catch it.
      expect(unit.expired).toBe(false);
      expect(
        model.agents.find((row) => row.agentPubkey === run.seed.cast.producerAgent.pubkey)?.burned,
      ).toBe(false);

      // The first delivery stands. The relay stored a later, different one from
      // the same provider under the same grant, and it did not replace it.
      expect(unit.result?.sourceEventId).toBe(swap.standingResultEventId);
      expect(unit.result?.displaySummary).toBe(swap.standingSummary);
      expect(unit.result?.idempotencyRef).toBe(run.seed.units.substituted.idempotencyRef);

      // Two refusals, and only two. The byte-identical re-send is the same
      // delivery said twice — which a relay does on its own — and refusing it
      // would be its own defect.
      expect(unit.refusedResults).toHaveLength(2);
      const byId = new Map(unit.refusedResults.map((row) => [row.sourceEventId, row]));
      expect(
        unit.refusedResults.some((row) => row.sourceEventId === swap.redeliveredResultEventId),
      ).toBe(false);

      // The swap: same grant, different work.
      const substituted = byId.get(swap.substitutedResultEventId);
      expect(substituted?.code).toBe("result_replay");
      expect(substituted?.admittedResultEventId).toBe(swap.standingResultEventId);
      expect(substituted?.unitRef).toBe(run.seed.units.substituted.unitRef);
      expect(substituted?.detail).toMatch(/already delivered/i);

      // The lift: another unit's delivery re-bound to this one. The grant names
      // its own idempotency ref, so this is refused without trusting anything
      // the delivery says about itself.
      const misbound = byId.get(swap.misboundResultEventId);
      expect(misbound?.code).toBe("result_replay");
      expect(misbound?.idempotencyRef).toBe(swap.misboundIdempotencyRef);
      expect(misbound?.idempotencyRef).not.toBe(run.seed.units.substituted.idempotencyRef);
      expect(misbound?.detail).toMatch(/did not grant/i);
      // And the unit it was lifted from is untouched by the attempt.
      expect(unitByRef(model, run.seed.units.accepted.unitRef).refusedResults).toEqual([]);

      // It fails visibly rather than quietly: the room says so at room level too.
      expect(model.resultRefusals.map((row) => row.sourceEventId)).toContain(
        swap.substitutedResultEventId,
      );
      expect(model.status).toBe("gap");

      // The revocation lane re-sent an identical delivery too. Same rule, and it
      // is not counted as a substitution there either.
      expect(unitByRef(model, run.seed.units.revoked.unitRef).refusedResults).toEqual([]);
    });

    test("revocation removes room and work-unit access immediately", () => {
      const departing = run.seed.cast.departingOperator.pubkey;

      const before = projectAsOf(departing, run.seed.revocationAtUnix);
      expect(before.viewerRoleStatus).toBe("active");
      expect(before.viewerRole).toBe("agent_operator");
      expect(controlKinds(before.controls)).toContain("post_message");
      expect(before.controls.length).toBeGreaterThan(0);

      const after = projectIssue31CommunityReadModel(run.snapshot, config(departing));
      expect(after.viewerRoleStatus).toBe("revoked");
      expect(after.viewerRole).toBe("read_only");
      // Not "fewer controls". None, on the room and on every unit, including the
      // one this operator is still the accepted provider of.
      expect(after.controls).toEqual([]);
      for (const unit of after.workUnits) {
        expect(unit.controls).toEqual([]);
      }

      // The removed operator wrote again and the relay carried it. The message
      // renders — it is a real signed record — with the role the room derives,
      // which is no longer one that may act.
      const afterMessage = after.transcript.find((row) => row.authorPubkey === departing);
      expect(afterMessage?.sourceCreatedAt).toBeGreaterThan(run.seed.revocationAtUnix);
      expect(afterMessage?.authorRole).toBe("read_only");
    });

    test("an expired grant is not extended", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey, {
          nowUnixSeconds: run.seed.expiredUnitProjectionNowUnix,
        }),
      );
      const expired = unitByRef(model, run.seed.units.expired.unitRef);
      expect(expired.expiresAtUnix).toBe(run.seed.units.expired.expiresAtUnix);
      expect(expired.expired).toBe(true);
      expect(expired.controls).toEqual([]);

      // The same clock, a unit whose grant has not lapsed: the expiry is read
      // per grant rather than applied to the room.
      const live = unitByRef(model, run.seed.units.accepted.unitRef);
      expect(live.expired).toBe(false);
    });

    test("a typed rejection names its class and its appeal, and the owner rules", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      const unit = unitByRef(model, run.seed.units.rejected.unitRef);

      expect(unit.decision?.outcome).toBe("rejected");
      expect(unit.decision?.reasonClass).toBe("verification_failed");
      expect(unit.decision?.decidedByPubkey).toBe(run.seed.cast.sarah.pubkey);
      // A rejection is never a dead end.
      expect(unit.decision?.appealDestination).toContain(run.seed.cast.ownerAppeal.pubkey);
      expect(unit.appeal?.appellantPubkey).toBe(run.seed.cast.producerOperator.pubkey);
      expect(unit.appeal?.grounds).toBe("process_error");
      // Sarah decides acceptance; she cannot author the ruling on her own
      // decision, and the room checks who did.
      expect(unit.ruling?.authoredByAdmittedOwnerKey).toBe(true);
      expect(unit.ruling?.ownerAppealPubkey).toBe(run.seed.cast.ownerAppeal.pubkey);
      expect(unit.lifecycle).toBe("ruled");
    });

    test("the award stream is the authority for the experience total", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      expect(model.experience.recomputedTotalPoints).toBe(
        run.seed.expectedProducerExperiencePoints,
      );
      expect(model.experience.awardCount).toBe(1);
      // The scorer's NIP-85 rank agrees with the awards, and the room recomputed
      // it rather than reading it.
      expect(model.experience.publishedRankPoints).toBe(run.seed.expectedProducerExperiencePoints);
      expect(model.experience.publishedRankDisagreed).toBe(false);

      const badge = model.experience.badges.find((row) => row.badgeId === "first-accepted-unit");
      expect(badge?.source).toBe("awards_and_wire");
      expect(badge?.issuerPubkey).toBe(run.seed.cast.scorer.pubkey);
      expect(badge?.supportedByAwards).toBe(true);

      // The verifier's award belongs to the verifier, not to whoever is looking.
      const verifierView = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.verifierOperator.pubkey),
      );
      expect(verifierView.experience.recomputedTotalPoints).toBe(5);
    });

    test("nothing in the room is money, and no control offers to move any", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      expect(JSON.stringify(model)).not.toMatch(/earning|payout|wallet|escrow|settle/i);
      const offered = new Set(
        [...model.controls, ...model.workUnits.flatMap((unit) => unit.controls)].map(
          (control) => control.kind,
        ),
      );
      for (const forbidden of ["settle", "pay", "withdraw", "fund"]) {
        for (const kind of offered) {
          expect(kind).not.toContain(forbidden);
        }
      }
      expect(model.experienceOnlyCopy).toMatch(/pays no money/i);
    });

    test("member text is data, and it is the only form Sarah may read", () => {
      const model = projectIssue31CommunityReadModel(
        run.snapshot,
        config(run.seed.cast.producerOperator.pubkey),
      );
      const injection = run.seed.transcriptLines[3] as string;
      const row = model.transcript.find((entry) => entry.displayText === injection);
      expect(row).toBeDefined();
      expect(isUntrustedCommunityContent(row?.untrusted)).toBe(true);

      const blocks = issue31CommunityUntrustedBlocks(model);
      expect(blocks.every((block) => isUntrustedCommunityContent(block))).toBe(true);
      // The room has no other door into her context: the builder re-checks the
      // brand at runtime rather than trusting the caller's type.
      const context = buildCommunitySarahContext(blocks);
      expect(context).toContain(injection);
      expect(() => buildCommunitySarahContext([injection])).toThrow();
    });

    test("the community room and the owner-private room never merge", () => {
      // One relay, one device, two rooms. Every record here is community, and
      // none carries a private rumor, a private record, or a shared cursor.
      for (const row of run.snapshot.confirmedEvents) {
        expect(row.room).toBe("community");
        expect(row.privateRumorId).toBeNull();
        expect(row.privateRecord).toBeNull();
      }
      const replaySince = run.snapshot.relays[0]?.roomReplaySince ?? {};
      expect(Object.keys(replaySince).sort()).toEqual(["community", "discovery", "owner_private"]);
    });
  });
};

// The in-process relay. Real protocol, real storage, no network, always run.
let localRelay: Awaited<ReturnType<typeof startTestRelay>> | null = null;
registerCommunityJourney({
  label: "an in-process startTestRelay",
  skip: false,
  setUp: async () => {
    localRelay = await startTestRelay(41_000 + Math.floor(Math.random() * 4_000));
    return `ws://127.0.0.1:${localRelay.port}`;
  },
  tearDown: async () => {
    await Promise.resolve(localRelay?.stop());
    localRelay = null;
  },
});

// The deployed relay. Opt in, because it needs the network:
//
//   MOBILE_LIVE_RELAY_URL=wss://relay.openagents.com \
//     pnpm --dir apps/openagents-mobile exec vp test --run --root ../.. \
//       apps/openagents-mobile/tests/issue31-community-journey.test.ts
registerCommunityJourney({
  label: "the deployed relay",
  skip: LIVE_RELAY_URL === undefined || LIVE_RELAY_URL === "",
  setUp: async () => LIVE_RELAY_URL as string,
});
