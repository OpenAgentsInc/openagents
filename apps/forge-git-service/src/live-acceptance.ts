import { attachOwnerAttestation } from "@openagentsinc/sarah/community";
import { createHash, randomUUID } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import { generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { Cause, Config, Effect, Exit, Redacted } from "effect";
import postgres from "postgres";

import { makeD1ForgeInviteMembershipStore } from "../../openagents.com/workers/api/src/forge-invite-membership-store.js";
import { makeD1ForgeTenantGitAuthStore } from "../../openagents.com/workers/api/src/forge-tenant-git-auth-store.js";
import { makeKhalaSyncWritesDatabase } from "../../openagents.com/workers/api/src/khala-sync-domain-writes-database.js";
import { makeD1TeamWorkspaceInviteStore } from "../../openagents.com/workers/api/src/team-workspace-invites.js";

const git = (
  cwd: string,
  args: ReadonlyArray<string>,
  token?: string,
): Promise<Readonly<{ stderr: string; stdout: string }>> =>
  new Promise((resolve, reject) => {
    const child = spawn("git", [...args], {
      cwd,
      env:
        token === undefined
          ? process.env
          : {
              ...process.env,
              GIT_CONFIG_COUNT: "1",
              GIT_CONFIG_KEY_0: "http.extraHeader",
              GIT_CONFIG_VALUE_0: `Authorization: Bearer ${token}`,
              GIT_TERMINAL_PROMPT: "0",
            },
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout: Array<Buffer> = [];
    const stderr: Array<Buffer> = [];
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.once("error", reject);
    child.once("close", (code) => {
      const result = {
        stderr: Buffer.concat(stderr).toString("utf8"),
        stdout: Buffer.concat(stdout).toString("utf8"),
      };
      if (code === 0) {
        resolve(result);
      } else {
        reject(new Error(`git exited with code ${code ?? "unknown"}`));
      }
    });
  });

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const applyForgeMembershipMigration = async (
  databaseUrl: string,
): Promise<
  Readonly<{
    databaseName: string;
    forgeTenantReadWrite: boolean;
    userName: string;
  }>
> => {
  const migrationSql = await readFile(
    join(import.meta.dirname, "0316_forge_invite_membership.sql"),
    "utf8",
  );
  const sql = postgres(databaseUrl, {
    max: 1,
    prepare: false,
  });
  try {
    await sql.begin(async (transaction) => {
      await transaction.unsafe(migrationSql);
    });
    const [authority] = await sql<
      ReadonlyArray<{
        database_name: string;
        forge_tenant_read_write: boolean;
        user_name: string;
      }>
    >`
      SELECT current_database() AS database_name,
             current_user AS user_name,
             has_table_privilege(
               current_user,
               'public.forge_tenants',
               'SELECT,INSERT,UPDATE,DELETE'
             ) AS forge_tenant_read_write
    `;
    if (authority === undefined) {
      throw new Error("Acceptance database authority query returned no row");
    }
    return {
      databaseName: authority.database_name,
      forgeTenantReadWrite: authority.forge_tenant_read_write,
      userName: authority.user_name,
    };
  } finally {
    await sql.end({ timeout: 5 });
  }
};

interface AcceptanceOptions {
  readonly baseUrl: string;
  readonly databaseUrl: string;
  readonly sourceRevision: string;
}

const runAcceptance = async ({
  baseUrl: configuredBaseUrl,
  databaseUrl,
  sourceRevision,
}: AcceptanceOptions): Promise<void> => {
  const baseUrl = configuredBaseUrl.replace(/\/+$/, "");
  const databaseAuthority = await applyForgeMembershipMigration(databaseUrl);
  if (databaseAuthority.userName !== "khala_app" || !databaseAuthority.forgeTenantReadWrite) {
    throw new Error(
      `Acceptance database authority mismatch: user=${databaseAuthority.userName}, database=${databaseAuthority.databaseName}, forgeTenantReadWrite=${databaseAuthority.forgeTenantReadWrite}`,
    );
  }
  const now = new Date();
  const nowIso = now.toISOString();
  const expiresAt = new Date(now.getTime() + 8 * 60 * 60 * 1_000).toISOString();
  const runRef = randomUUID().replaceAll("-", "");
  const tenantRef = "tenant.openagents";
  const teamRef = "team_openagents_core";
  const repositoryRef = `repo.openagents.forge02-live-${runRef}`;
  const acceptanceRef = `forge02-live:${runRef}`;
  const humanAccountRef = `acceptance:human:${runRef}`;
  const agentAccountRef = `acceptance:agent:${runRef}`;
  const humanBindingRef = `forge_actor.human.${runRef}`;
  const agentBindingRef = `forge_actor.agent.${runRef}`;
  const humanTokenRef = `forge_git_token.human.${runRef}`;
  const agentTokenRef = `forge_git_token.agent.${runRef}`;
  const inviteRef = `team_workspace_invite_forge02_${runRef}`;
  const acceptanceEmail = `forge-02-${runRef}@acceptance.openagents.com`;

  const database = makeKhalaSyncWritesDatabase({
    KHALA_SYNC_DB: { connectionString: databaseUrl },
  });
  if (database === undefined) {
    throw new Error("The Cloud SQL compatibility database is unavailable");
  }
  const membership = makeD1ForgeInviteMembershipStore(database);
  const tokens = makeD1ForgeTenantGitAuthStore(database);
  const invites = makeD1TeamWorkspaceInviteStore(database, {
    nowIso: () => nowIso,
  });

  await tokens.upsertTenant({
    displayName: "OpenAgents",
    nowIso,
    tenantRef,
  });

  const inviteCreated = await invites.createOrRefreshInvite({
    email: acceptanceEmail,
    expiresAt,
    id: inviteRef,
    invitedByActorRef: "forge_actor.owner.acceptance",
    metadataJson: JSON.stringify({
      acceptanceRef,
      issueRef: "github:OpenAgentsInc/openagents#9244",
    }),
    role: "member",
    teamId: teamRef,
  });
  if (inviteCreated._tag !== "Created" && inviteCreated._tag !== "Refreshed") {
    throw new Error(`Invite creation failed: ${inviteCreated._tag}`);
  }
  const inviteAccepted = await invites.acceptInvite({
    sessionEmail: acceptanceEmail,
    token: inviteCreated.token,
    userId: humanAccountRef,
  });
  if (inviteAccepted._tag !== "Accepted" && inviteAccepted._tag !== "AlreadyAccepted") {
    throw new Error(`Invite acceptance failed: ${inviteAccepted._tag}`);
  }

  const humanSecret = generateSecretKey();
  const humanPubkey = getPublicKey(humanSecret);
  const human = await membership.bindHuman({
    acceptedAt: inviteAccepted.invite.acceptedAt ?? nowIso,
    accountRef: humanAccountRef,
    bindingEventCreatedAt: nowIso,
    bindingEventId: `forge_binding_event.human.${runRef}`,
    bindingRef: humanBindingRef,
    displayName: "FORGE-02 invited human acceptance",
    expiresAt: inviteAccepted.invite.expiresAt,
    inviteBindingRef: `forge_invite_binding.${inviteRef}`,
    inviteDigest: inviteAccepted.invite.tokenHash,
    inviteRef,
    invitedSubjectRef: humanAccountRef,
    inviterBindingRef: inviteAccepted.invite.invitedByActorRef,
    issuedAt: inviteAccepted.invite.createdAt,
    nostrPubkey: humanPubkey,
    provenanceSourceRefs: [
      `team_workspace_invite:${inviteRef}`,
      "github:OpenAgentsInc/openagents#9244",
    ],
    roleRefs: ["forge:member"],
    teamRef,
    tenantRef,
  });

  const agentSecret = generateSecretKey();
  const agentPubkey = getPublicKey(agentSecret);
  const agent = await membership.attachAgent({
    accountRef: agentAccountRef,
    bindingEventCreatedAt: nowIso,
    bindingEventId: `forge_binding_event.agent.${runRef}`,
    bindingRef: agentBindingRef,
    displayName: "FORGE-02 owner-attested agent acceptance",
    nostrPubkey: agentPubkey,
    nowIso,
    ownerAuthTag: attachOwnerAttestation({
      agentPubkey,
      operatorSeckeyHex: bytesToHex(humanSecret),
    }),
    ownerBindingRef: human.bindingRef,
    sourceRefs: ["github:OpenAgentsInc/openagents#9244"],
    tenantRef,
  });

  const requestedScopes = ["git:upload-pack", "git:receive-pack"] as const;
  const humanCredential = await tokens.mintGitAccessToken({
    expiresAt,
    nowIso,
    repositoryRef,
    scopes: requestedScopes,
    sourceRefs: [acceptanceRef, `forge_actor_binding:${human.bindingRef}`],
    subjectRef: human.bindingRef,
    tenantRef,
    tokenRef: humanTokenRef,
  });
  const agentCredential = await tokens.mintGitAccessToken({
    expiresAt,
    nowIso,
    repositoryRef,
    scopes: requestedScopes,
    sourceRefs: [acceptanceRef, `forge_actor_binding:${agent.bindingRef}`],
    subjectRef: agent.bindingRef,
    tenantRef,
    tokenRef: agentTokenRef,
  });

  const root = await mkdtemp(join(tmpdir(), "forge02-live-"));
  const source = join(root, "source");
  const humanClone = join(root, "human");
  const agentClone = join(root, "agent");
  const partialClone = join(root, "partial");
  const remote = `${baseUrl}/git/${encodeURIComponent(tenantRef)}/${encodeURIComponent(repositoryRef)}.git`;

  let receiptOutput: Readonly<Record<string, unknown>> | undefined;
  let receiptJson: string | undefined;
  let revokedAgentFetchRefused = false;
  try {
    await git(root, ["init", "--initial-branch=main", source]);
    await git(source, ["config", "user.email", "forge-02-human@acceptance.openagents.com"]);
    await git(source, ["config", "user.name", "FORGE-02 invited human"]);
    await writeFile(join(source, "README.md"), "FORGE-02 live transport acceptance\n");
    await git(source, ["add", "README.md"]);
    await git(source, ["commit", "-m", "Seed FORGE-02 live acceptance"]);
    const seedCommit = (await git(source, ["rev-parse", "HEAD"])).stdout.trim();
    await git(source, ["push", remote, "HEAD:refs/heads/main"], humanCredential.token);

    const advertisement = await fetch(`${remote}/info/refs?service=git-upload-pack`, {
      headers: { authorization: `Bearer ${humanCredential.token}` },
    });
    if (!advertisement.ok) {
      throw new Error(`Upload-pack advertisement failed: ${advertisement.status}`);
    }
    const capabilities = new TextDecoder().decode(await advertisement.arrayBuffer());
    for (const capability of ["filter", "allow-tip-sha1-in-want", "allow-reachable-sha1-in-want"]) {
      if (!capabilities.includes(capability)) {
        throw new Error(`Missing upload-pack capability: ${capability}`);
      }
    }

    await git(root, ["clone", remote, humanClone], humanCredential.token);
    await git(humanClone, ["config", "user.email", "forge-02-human@acceptance.openagents.com"]);
    await git(humanClone, ["config", "user.name", "FORGE-02 invited human"]);
    await writeFile(join(humanClone, "HUMAN.md"), "Invited human push passed\n");
    await git(humanClone, ["add", "HUMAN.md"]);
    await git(humanClone, ["commit", "-m", "Prove invited human push"]);
    await git(humanClone, ["push", "origin", "main"], humanCredential.token);
    const humanCommit = (await git(humanClone, ["rev-parse", "HEAD"])).stdout.trim();

    await git(root, ["clone", remote, agentClone], agentCredential.token);
    await git(agentClone, ["config", "user.email", "forge-02-agent@acceptance.openagents.com"]);
    await git(agentClone, ["config", "user.name", "FORGE-02 owner-attested agent"]);
    await writeFile(join(agentClone, "AGENT.md"), "Owner-attested agent push passed\n");
    await git(agentClone, ["add", "AGENT.md"]);
    await git(agentClone, ["commit", "-m", "Prove owner-attested agent push"]);
    await git(agentClone, ["push", "origin", "main"], agentCredential.token);
    const agentCommit = (await git(agentClone, ["rev-parse", "HEAD"])).stdout.trim();

    await git(
      root,
      ["clone", "--filter=blob:none", "--no-checkout", remote, partialClone],
      humanCredential.token,
    );
    const partialClonePromisor =
      (await git(partialClone, ["config", "--get", "remote.origin.promisor"])).stdout.trim() ===
      "true";
    if (!partialClonePromisor) {
      throw new Error("Partial clone did not record a promisor remote");
    }

    const tombstoned = await membership.tombstoneMember({
      bindingRef: agent.bindingRef,
      burnReasonRef: "forge.acceptance.revoked_agent",
      nowIso: new Date().toISOString(),
      sourceRefs: [acceptanceRef],
      tenantRef,
    });
    if (tombstoned.some((binding) => binding.membershipState !== "tombstoned")) {
      throw new Error("Agent tombstone did not persist");
    }
    try {
      await git(agentClone, ["fetch", "origin"], agentCredential.token);
    } catch {
      revokedAgentFetchRefused = true;
    }
    if (!revokedAgentFetchRefused) {
      throw new Error("Revoked agent fetch unexpectedly succeeded");
    }
    await git(humanClone, ["fetch", "origin"], humanCredential.token);
    if ((await readFile(join(humanClone, "README.md"), "utf8")).trim() === "") {
      throw new Error("Human clone content is empty");
    }

    const receipt = {
      schemaVersion: "openagents.forge_git.live_transport_receipt.v1",
      generatedAt: new Date().toISOString(),
      sourceRevision,
      scope: "private-cloud-run-real-stock-git",
      acceptanceRef,
      authority: {
        membership: "live-forge-policy",
        repository: "bare-repository",
        gcsMirrorRefAuthority: false,
      },
      actors: {
        humanBindingRef: human.bindingRef,
        agentBindingRef: agent.bindingRef,
        agentMembershipState: "tombstoned",
      },
      repositoryRef,
      tokenRefs: {
        human: humanTokenRef,
        agent: agentTokenRef,
      },
      checks: {
        humanClone: "passed",
        humanPush: "passed",
        agentClone: "passed",
        agentPush: "passed",
        revokedAgentFetchRefused,
        humanFetchAfterAgentRevocation: "passed",
        partialClonePromisor,
        advertisedCapabilities: [
          "filter",
          "allow-tip-sha1-in-want",
          "allow-reachable-sha1-in-want",
        ],
        seedCommit,
        humanCommit,
        agentCommit,
      },
      result: "passed",
    };
    receiptOutput = receipt;
    receiptJson = JSON.stringify(receiptOutput);
  } finally {
    const cleanupAt = new Date().toISOString();
    const cleanupOperations = [
      tokens.revokeGitAccessToken(tenantRef, humanTokenRef, cleanupAt),
      tokens.revokeGitAccessToken(tenantRef, agentTokenRef, cleanupAt),
      membership.tombstoneMember({
        bindingRef: humanBindingRef,
        burnReasonRef: "forge.acceptance.complete",
        nowIso: cleanupAt,
        sourceRefs: [acceptanceRef],
        tenantRef,
      }),
      membership.tombstoneMember({
        bindingRef: agentBindingRef,
        burnReasonRef: "forge.acceptance.complete",
        nowIso: cleanupAt,
        sourceRefs: [acceptanceRef],
        tenantRef,
      }),
    ];
    const cleanupResults = await Promise.allSettled(cleanupOperations);
    humanSecret.fill(0);
    agentSecret.fill(0);
    await rm(root, { force: true, recursive: true });
    const cleanupFailure = cleanupResults.find((result) => result.status === "rejected");
    if (cleanupFailure?.status === "rejected") {
      throw new Error("Acceptance actor cleanup failed", {
        cause: cleanupFailure.reason,
      });
    }
  }

  if (receiptJson === undefined || receiptOutput === undefined) {
    throw new Error("Acceptance receipt was not created");
  }
  console.log(
    JSON.stringify({
      ...receiptOutput,
      cleanup: "tokens-revoked-and-bindings-tombstoned",
      receiptSha256: sha256(receiptJson),
    }),
  );
};

const program = Effect.gen(function* () {
  const databaseUrl = yield* Config.redacted("FORGE_ACCEPTANCE_DATABASE_URL");
  const baseUrl = yield* Config.nonEmptyString("FORGE_ACCEPTANCE_BASE_URL");
  const sourceRevision = yield* Config.nonEmptyString("FORGE_ACCEPTANCE_SOURCE_REVISION").pipe(
    Config.withDefault("unknown"),
  );

  yield* Effect.tryPromise(() =>
    runAcceptance({
      baseUrl,
      databaseUrl: Redacted.value(databaseUrl),
      sourceRevision,
    }),
  );
});

const redactFailure = (value: string): string =>
  value
    .replace(/oa_forge_git_[A-Za-z0-9_-]+/g, "<redacted>")
    .replace(/(postgres(?:ql)?:\/\/[^:\s/]+:)[^@\s/]+(@)/gi, "$1<redacted>$2");

Effect.runPromiseExit(program).then((exit) => {
  if (Exit.isSuccess(exit)) {
    return;
  }
  console.error(
    JSON.stringify({
      schemaVersion: "openagents.forge_git.live_transport_receipt.v1",
      result: "failed",
      error: redactFailure(Cause.pretty(exit.cause)),
    }),
  );
  process.exitCode = 1;
});
