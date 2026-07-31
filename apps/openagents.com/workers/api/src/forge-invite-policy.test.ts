import { attachOwnerAttestation } from "@openagentsinc/sarah/community";
import { readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import * as nip19 from "nostr-effect/nip19";
import { hashPayloadBytes } from "nostr-effect/nip98";
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { describe, expect, test } from "vitest";

import {
  type ForgeInviteMembershipStore,
  makeD1ForgeInviteMembershipStore,
} from "./forge-invite-membership-store";
import {
  ForgeInvitePolicyError,
  decodeForgeNpub,
  makeForgeInvitePolicyAuthority,
  verifyForgeNip98Proof,
} from "./forge-invite-policy";
import {
  FORGE_GIT_TOKEN_PREFIX,
  type ForgeTenantGitAuthStore,
  makeD1ForgeTenantGitAuthStore,
} from "./forge-tenant-git-auth-store";

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = [];

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map((value) => (value === undefined ? null : value));
    return this;
  }

  async first<T = Record<string, unknown>>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ?? null) as T | null;
  }

  async all<T = Record<string, unknown>>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    };
  }

  async run(): Promise<{
    meta: { changes: number };
    results: [];
    success: true;
  }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]));
    return {
      meta: { changes: Number(result.changes) },
      results: [],
      success: true,
    };
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql);
  }

  async batch(statements: ReadonlyArray<SqliteD1Statement>): Promise<ReadonlyArray<unknown>> {
    const results: Array<unknown> = [];
    this.db.exec("BEGIN");
    try {
      for (const statement of statements) {
        results.push(await statement.run());
      }
      this.db.exec("COMMIT");
      return results;
    } catch (error) {
      this.db.exec("ROLLBACK");
      throw error;
    }
  }
}

const migration = (name: string): string =>
  readFileSync(new URL(`../migrations/${name}`, import.meta.url), "utf8");

const makeStores = async (): Promise<{
  db: DatabaseSync;
  membership: ForgeInviteMembershipStore;
  token: ForgeTenantGitAuthStore;
}> => {
  const db = new DatabaseSync(":memory:");
  db.exec("PRAGMA foreign_keys = ON");
  for (const name of [
    "0253_forge_tenant_git_access_tokens.sql",
    "0256_forge_tenant_isolation_posture.sql",
    "0316_forge_invite_membership.sql",
  ]) {
    db.exec(migration(name));
  }
  db.exec(
    "ALTER TABLE forge_git_access_tokens ADD COLUMN ref_restrictions_json TEXT NOT NULL DEFAULT '[]'",
  );
  const d1 = new SqliteD1(db) as unknown as D1Database;
  const token = makeD1ForgeTenantGitAuthStore(d1);
  await token.upsertTenant({
    displayName: "OpenAgents",
    nowIso,
    tenantRef,
  });
  return {
    db,
    membership: makeD1ForgeInviteMembershipStore(d1),
    token,
  };
};

const tenantRef = "tenant.openagents";
const repositoryRef = "repo.openagents.openagents";
const nowIso = "2026-07-25T20:00:30.000Z";
const eventTime = "2026-07-25T20:00:00.000Z";
const ownerBindingRef = "forge_actor.owner";
const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");

const bindHuman = async (
  membership: ForgeInviteMembershipStore,
  ownerSecret: Uint8Array,
  roleRefs: ReadonlyArray<string> = ["forge:member"],
) =>
  membership.bindHuman({
    acceptedAt: eventTime,
    accountRef: "github:owner",
    bindingEventCreatedAt: eventTime,
    bindingEventId: "event.binding.owner",
    bindingRef: ownerBindingRef,
    displayName: "Owner",
    expiresAt: "2026-07-26T20:00:00.000Z",
    inviteBindingRef: "forge_invite_binding.owner",
    inviteDigest: "a".repeat(64),
    inviteRef: "team_workspace_invite.owner",
    invitedSubjectRef: "github:owner",
    inviterBindingRef: "forge_actor.inviter",
    issuedAt: "2026-07-25T19:00:00.000Z",
    nostrPubkey: getPublicKey(ownerSecret),
    provenanceSourceRefs: ["github:OpenAgentsInc/openagents#9246"],
    roleRefs,
    teamRef: "team_openagents_core",
    tenantRef,
  });

const nip98Authorization = (
  input: Readonly<{
    body?: Uint8Array | undefined;
    createdAt?: string | undefined;
    method: string;
    secret: Uint8Array;
    url: string;
  }>,
): string => {
  const tags: Array<Array<string>> = [
    ["u", input.url],
    ["method", input.method.toUpperCase()],
  ];
  if (input.body !== undefined) {
    tags.push(["payload", hashPayloadBytes(input.body)]);
  }
  const event = finalizeEvent(
    {
      content: "",
      created_at: Math.floor(Date.parse(input.createdAt ?? eventTime) / 1_000),
      kind: 27_235,
      tags,
    },
    input.secret,
  );
  return `Nostr ${btoa(JSON.stringify(event))}`;
};

const transportRequest = (url: string, authorization: string, method = "GET"): Request =>
  new Request(url, {
    headers: { authorization },
    method,
  });

describe("Forge invite membership and transport policy", () => {
  test("binds one OpenAuth account to one canonical npub", async () => {
    const { membership } = await makeStores();
    const ownerSecret = generateSecretKey();
    const pubkey = getPublicKey(ownerSecret);
    const npub = nip19.npubEncode(pubkey);

    expect(decodeForgeNpub(npub)).toBe(pubkey);
    await expect(
      Promise.resolve().then(() => decodeForgeNpub(npub.toUpperCase())),
    ).rejects.toMatchObject({ code: "npub_invalid" });
    // Corrupt the final checksum character to something GUARANTEED different.
    // Hardcoding a replacement (e.g. always 'x') is a ~1-in-32 flake: the npub's
    // last character is drawn from the 32-symbol bech32 alphabet, so ~2.6% of
    // generated keys already end in that character, leaving the string unchanged
    // and still perfectly valid — and the rejection assertion then fails.
    const lastChar = npub.slice(-1);
    const corruptedNpub = `${npub.slice(0, -1)}${lastChar === "x" ? "y" : "x"}`;
    expect(corruptedNpub).not.toBe(npub);
    await expect(
      Promise.resolve().then(() => decodeForgeNpub(corruptedNpub)),
    ).rejects.toMatchObject({ code: "npub_invalid" });

    const binding = await bindHuman(membership, ownerSecret);
    expect(binding).toMatchObject({
      accountRef: "github:owner",
      bindingRef: ownerBindingRef,
      membershipState: "active",
      nostrPubkey: pubkey,
    });
    await expect(
      membership.bindHuman({
        acceptedAt: eventTime,
        accountRef: "email:owner@example.com",
        bindingEventCreatedAt: eventTime,
        bindingEventId: "event.binding.other",
        bindingRef: "forge_actor.other",
        displayName: "Other account",
        expiresAt: "2026-07-26T20:00:00.000Z",
        inviteBindingRef: "forge_invite_binding.other",
        inviteDigest: "b".repeat(64),
        inviteRef: "team_workspace_invite.other",
        invitedSubjectRef: "email:owner@example.com",
        inviterBindingRef: "forge_actor.inviter",
        issuedAt: eventTime,
        nostrPubkey: pubkey,
        provenanceSourceRefs: [],
        roleRefs: ["forge:member"],
        teamRef: "team_openagents_core",
        tenantRef,
      }),
    ).rejects.toMatchObject({ code: "binding_conflict" });
  });

  test("admits an owner-attested agent without replacing its identity", async () => {
    const { membership } = await makeStores();
    const ownerSecret = generateSecretKey();
    const agentSecret = generateSecretKey();
    const agentPubkey = getPublicKey(agentSecret);
    await bindHuman(membership, ownerSecret);
    const ownerAuthTag = attachOwnerAttestation({
      agentPubkey,
      operatorSeckeyHex: bytesToHex(ownerSecret),
    });

    const agent = await membership.attachAgent({
      accountRef: "agent:fixture",
      bindingEventCreatedAt: eventTime,
      bindingEventId: "event.binding.agent",
      bindingRef: "forge_actor.agent",
      displayName: "Fixture agent",
      nostrPubkey: agentPubkey,
      nowIso,
      ownerAuthTag,
      ownerBindingRef,
      sourceRefs: ["event.binding.agent"],
      tenantRef,
    });

    expect(agent).toMatchObject({
      actorKind: "agent",
      bindingRef: "forge_actor.agent",
      nostrPubkey: agentPubkey,
      ownerBindingRef,
    });
    expect(agent.accountRef).toBe("agent:fixture");

    await expect(
      membership.attachAgent({
        accountRef: "agent:unattested",
        bindingEventCreatedAt: eventTime,
        bindingEventId: "event.binding.unattested",
        bindingRef: "forge_actor.unattested",
        displayName: "Unattested agent",
        nostrPubkey: getPublicKey(generateSecretKey()),
        nowIso,
        ownerAuthTag,
        ownerBindingRef,
        sourceRefs: [],
        tenantRef,
      }),
    ).rejects.toMatchObject({ code: "owner_attestation_invalid" });
  });

  test("uses one membership authority for scoped tokens and NIP-98", async () => {
    const { membership, token } = await makeStores();
    const ownerSecret = generateSecretKey();
    const binding = await bindHuman(membership, ownerSecret);
    const rawToken = `${FORGE_GIT_TOKEN_PREFIX}owner_0000000000000000000000`;
    await token.mintGitAccessToken(
      {
        expiresAt: "2026-07-25T21:00:00.000Z",
        nowIso: eventTime,
        repositoryRef,
        scopes: ["git:receive-pack"],
        sourceRefs: ["github:OpenAgentsInc/openagents#9246"],
        subjectRef: binding.bindingRef,
        tenantRef,
        tokenRef: "forge_git_token.owner",
      },
      { makeToken: () => rawToken },
    );
    const authority = makeForgeInvitePolicyAuthority({
      policyStore: membership,
      tokenStore: token,
    });
    const url = `https://openagents.com/git/${tenantRef}/${repositoryRef}.git/info/refs?service=git-receive-pack`;
    const tokenSession = await authority.authorizeGitTransport({
      nowIso,
      repositoryRef,
      request: transportRequest(url, `Bearer ${rawToken}`),
      requiredScope: "git:receive-pack",
      tenantRef,
    });
    const nostrAuthorization = nip98Authorization({
      method: "GET",
      secret: ownerSecret,
      url,
    });
    const nostrSession = await authority.authorizeGitTransport({
      nowIso,
      repositoryRef,
      request: transportRequest(url, nostrAuthorization),
      requiredScope: "git:receive-pack",
      tenantRef,
    });

    expect(tokenSession.bindingRef).toBe(binding.bindingRef);
    expect(nostrSession.bindingRef).toBe(binding.bindingRef);
    expect(tokenSession.credentialMode).toBe("scoped_token");
    expect(nostrSession.credentialMode).toBe("nip98");

    await expect(
      authority.authorizeGitTransport({
        nowIso,
        repositoryRef,
        request: transportRequest(url, nostrAuthorization),
        requiredScope: "git:receive-pack",
        tenantRef,
      }),
    ).rejects.toMatchObject({ code: "nip98_replayed" });
  });

  test("rejects future, wrong-method, and wrong-body NIP-98 proofs", async () => {
    const secret = generateSecretKey();
    const url = "https://openagents.com/git/tenant/repo.git/git-receive-pack";
    const body = new TextEncoder().encode("pack");

    await expect(
      verifyForgeNip98Proof({
        authorization: nip98Authorization({
          body,
          createdAt: "2026-07-25T20:05:00.000Z",
          method: "POST",
          secret,
          url,
        }),
        body,
        method: "POST",
        nowIso,
        url,
      }),
    ).rejects.toMatchObject({ code: "credential_invalid" });

    const valid = nip98Authorization({
      body,
      method: "POST",
      secret,
      url,
    });
    await expect(
      verifyForgeNip98Proof({
        authorization: valid,
        body,
        method: "GET",
        nowIso,
        url,
      }),
    ).rejects.toMatchObject({ code: "credential_invalid" });
    await expect(
      verifyForgeNip98Proof({
        authorization: valid,
        body: new TextEncoder().encode("other"),
        method: "POST",
        nowIso,
        url,
      }),
    ).rejects.toMatchObject({ code: "credential_invalid" });
  });

  test("tombstones an owner, burns agent keys, and refuses credential replay", async () => {
    const { membership, token } = await makeStores();
    const ownerSecret = generateSecretKey();
    const agentSecret = generateSecretKey();
    const agentPubkey = getPublicKey(agentSecret);
    const owner = await bindHuman(membership, ownerSecret);
    const ownerAuthTag = attachOwnerAttestation({
      agentPubkey,
      operatorSeckeyHex: bytesToHex(ownerSecret),
    });
    const agent = await membership.attachAgent({
      accountRef: "agent:fixture",
      bindingEventCreatedAt: eventTime,
      bindingEventId: "event.binding.agent",
      bindingRef: "forge_actor.agent",
      displayName: "Fixture agent",
      nostrPubkey: agentPubkey,
      nowIso,
      ownerAuthTag,
      ownerBindingRef: owner.bindingRef,
      sourceRefs: [],
      tenantRef,
    });
    const rawToken = `${FORGE_GIT_TOKEN_PREFIX}agent_0000000000000000000000`;
    await token.mintGitAccessToken(
      {
        expiresAt: "2026-07-25T21:00:00.000Z",
        nowIso: eventTime,
        repositoryRef,
        scopes: ["git:admin"],
        sourceRefs: [],
        subjectRef: agent.bindingRef,
        tenantRef,
        tokenRef: "forge_git_token.agent",
      },
      { makeToken: () => rawToken },
    );
    const revoked = await membership.tombstoneMember({
      bindingRef: owner.bindingRef,
      burnReasonRef: "forge.member.revoked",
      nowIso,
      sourceRefs: ["event.revoke.owner"],
      tenantRef,
    });
    expect(revoked.map((item) => item.membershipState)).toEqual(["tombstoned", "tombstoned"]);
    expect(await membership.isBurnedKey(tenantRef, agentPubkey)).toBe(true);

    const authority = makeForgeInvitePolicyAuthority({
      policyStore: membership,
      tokenStore: token,
    });
    const url = `https://openagents.com/git/${tenantRef}/${repositoryRef}.git/git-receive-pack`;
    await expect(
      authority.authorizeGitTransport({
        nowIso,
        repositoryRef,
        request: transportRequest(url, `Bearer ${rawToken}`, "POST"),
        requiredScope: "git:receive-pack",
        tenantRef,
      }),
    ).rejects.toMatchObject({ code: "membership_tombstoned" });

    await expect(
      membership.attachAgent({
        accountRef: "agent:fixture-replay",
        bindingEventCreatedAt: eventTime,
        bindingEventId: "event.binding.agent.replay",
        bindingRef: "forge_actor.agent.replay",
        displayName: "Replayed agent",
        nostrPubkey: agentPubkey,
        nowIso,
        ownerAuthTag,
        ownerBindingRef: owner.bindingRef,
        sourceRefs: ["event.binding.agent"],
        tenantRef,
      }),
    ).rejects.toBeInstanceOf(ForgeInvitePolicyError);
  });

  test("grows after one observation and shrinks only after two successful absences", async () => {
    const { membership } = await makeStores();
    const owner = await bindHuman(membership, generateSecretKey());
    const present = await membership.reconcileMembership({
      bindingRef: owner.bindingRef,
      nowIso: "2026-07-25T20:01:00.000Z",
      observedPresent: true,
      querySucceeded: true,
      sourceMembershipGeneration: 1,
      sourceRefs: ["team_membership:owner"],
      teamRef: "team_openagents_core",
      tenantRef,
    });
    expect(present?.state).toBe("present");

    const firstAbsent = await membership.reconcileMembership({
      bindingRef: owner.bindingRef,
      nowIso: "2026-07-25T20:02:00.000Z",
      observedPresent: false,
      querySucceeded: true,
      sourceMembershipGeneration: 2,
      sourceRefs: ["team_membership:scan.2"],
      teamRef: "team_openagents_core",
      tenantRef,
    });
    expect(firstAbsent?.state).toBe("absence_pending");
    expect(
      (await membership.readActorBindingByRef(tenantRef, owner.bindingRef))?.membershipState,
    ).toBe("active");

    const staleAbsent = await membership.reconcileMembership({
      bindingRef: owner.bindingRef,
      nowIso: "2026-07-25T20:02:30.000Z",
      observedPresent: false,
      querySucceeded: true,
      sourceMembershipGeneration: 2,
      sourceRefs: ["team_membership:scan.2.replay"],
      teamRef: "team_openagents_core",
      tenantRef,
    });
    expect(staleAbsent).toEqual(firstAbsent);
    expect(
      (await membership.readActorBindingByRef(tenantRef, owner.bindingRef))?.membershipState,
    ).toBe("active");

    const failed = await membership.reconcileMembership({
      bindingRef: owner.bindingRef,
      nowIso: "2026-07-25T20:03:00.000Z",
      observedPresent: false,
      querySucceeded: false,
      sourceMembershipGeneration: 3,
      sourceRefs: ["team_membership:scan.failed"],
      teamRef: "team_openagents_core",
      tenantRef,
    });
    expect(failed).toEqual(firstAbsent);

    const confirmed = await membership.reconcileMembership({
      bindingRef: owner.bindingRef,
      nowIso: "2026-07-25T20:04:00.000Z",
      observedPresent: false,
      querySucceeded: true,
      sourceMembershipGeneration: 4,
      sourceRefs: ["team_membership:scan.4"],
      teamRef: "team_openagents_core",
      tenantRef,
    });
    expect(confirmed?.state).toBe("absence_confirmed");
    expect(
      (await membership.readActorBindingByRef(tenantRef, owner.bindingRef))?.membershipState,
    ).toBe("tombstoned");
  });
});
