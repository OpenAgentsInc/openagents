import { createHash } from "node:crypto";

import {
  PORTABLE_CAPABILITY_BROKER_VERSION,
  type CapabilityBrokerPrivateDurableState,
} from "@openagentsinc/portable-session-contract";

import type { PortableGrantAuthorityBinding } from "./portable-capability-runtime-adapters.js";
import type {
  PortableCommandCapabilityGrantFact,
  PortableCommandCapabilityGrantFactScope,
  PortableCommandCapabilityGrantResolution,
} from "./portable-session-command-runner.js";
import type { SyncSql } from "./sql.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|"(?:token|apiKey|authorization|refreshToken|mnemonic|password|credential|secret|localPath|hostname|processId|providerSessionId|transportHandle|socket|pid|authHome)"\s*:/iu;

export type PortableCapabilityAuthorityFact = PortableGrantAuthorityBinding &
  Readonly<{
    status: "issued";
    expiresAt: string;
  }>;

export type PortableCapabilityGrantFactAuthority = Readonly<{
  resolve: (
    input: Readonly<{
      ownerUserId: string;
      grantRefs: ReadonlyArray<string>;
    }>,
  ) => Promise<ReadonlyArray<PortableCapabilityAuthorityFact>>;
}>;

type Fetch = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

export class HttpPortableCapabilityGrantFactAuthority implements PortableCapabilityGrantFactAuthority {
  private readonly fetch: Fetch;

  constructor(
    private readonly config: Readonly<{
      baseUrl: string;
      serviceBearer: string;
      fetch?: Fetch;
    }>,
  ) {
    if (!config.baseUrl.startsWith("https://") || config.serviceBearer.length < 8) {
      throw new PortableCommandCapabilityGrantResolverError(
        "grant fact authority config is invalid",
      );
    }
    this.fetch = config.fetch ?? globalThis.fetch;
  }

  async resolve(
    input: Readonly<{
      ownerUserId: string;
      grantRefs: ReadonlyArray<string>;
    }>,
  ): Promise<ReadonlyArray<PortableCapabilityAuthorityFact>> {
    let response: Response;
    try {
      response = await this.fetch(
        new URL("/api/portable-capability-grants/facts", this.config.baseUrl),
        {
          method: "POST",
          headers: {
            authorization: `Bearer ${this.config.serviceBearer}`,
            "content-type": "application/json",
          },
          body: JSON.stringify(input),
        },
      );
    } catch {
      throw new PortableCommandCapabilityGrantResolverError("grant fact authority is unavailable");
    }
    if (!response.ok) {
      throw new PortableCommandCapabilityGrantResolverError(
        "grant fact authority refused the scope",
      );
    }
    const envelope: unknown = await response.json();
    if (
      envelope === null ||
      typeof envelope !== "object" ||
      (envelope as { material?: unknown }).material !== "excluded" ||
      !Array.isArray((envelope as { facts?: unknown }).facts) ||
      FORBIDDEN_PRIVATE_MATERIAL.test(JSON.stringify(envelope))
    ) {
      throw new PortableCommandCapabilityGrantResolverError(
        "grant fact authority response is invalid",
      );
    }
    const facts = (envelope as { facts: ReadonlyArray<PortableCapabilityAuthorityFact> }).facts;
    if (
      facts.some(
        (fact) =>
          !SAFE_REF.test(fact.grantRef) ||
          !SAFE_REF.test(fact.ownerUserId) ||
          !["provider", "github"].includes(fact.kind) ||
          fact.status !== "issued" ||
          !Number.isFinite(Date.parse(fact.expiresAt)) ||
          (fact.kind === "provider" && fact.providerAccountRef === undefined) ||
          (fact.kind === "github" && fact.providerAccountRef !== undefined) ||
          (fact.providerAccountRef !== undefined && !SAFE_REF.test(fact.providerAccountRef)) ||
          (fact.runnerSessionId !== undefined && !SAFE_REF.test(fact.runnerSessionId)),
      )
    ) {
      throw new PortableCommandCapabilityGrantResolverError(
        "grant fact authority facts are invalid",
      );
    }
    return facts;
  }
}

export class PortableCommandCapabilityGrantResolverError extends Error {
  override readonly name = "PortableCommandCapabilityGrantResolverError";
}

type BrokerRow = Readonly<{
  state_json: unknown;
  claim_command_ref: string;
  active_move_ref: string | null;
  active_command_ref: string | null;
  active_source_attachment_ref: string | null;
  active_source_generation: string | number | null;
  active_destination_target_ref: string | null;
}>;

const parseState = (value: unknown): CapabilityBrokerPrivateDurableState => {
  const parsed = typeof value === "string" ? (JSON.parse(value) as unknown) : value;
  if (
    parsed === null ||
    typeof parsed !== "object" ||
    (parsed as CapabilityBrokerPrivateDurableState).schema !== PORTABLE_CAPABILITY_BROKER_VERSION ||
    (parsed as CapabilityBrokerPrivateDurableState).material !== "excluded" ||
    !Array.isArray((parsed as CapabilityBrokerPrivateDurableState).records) ||
    FORBIDDEN_PRIVATE_MATERIAL.test(JSON.stringify(parsed))
  )
    throw new PortableCommandCapabilityGrantResolverError("portable broker state is invalid");
  return parsed as CapabilityBrokerPrivateDurableState;
};

const destinationGrantRef = (
  scope: PortableCommandCapabilityGrantFactScope,
  sourceGrantRef: string,
) =>
  `grant.portable.${createHash("sha256")
    .update(
      [scope.commandExecutionClaimRef, sourceGrantRef, scope.destinationTargetRef].join("\u0000"),
    )
    .digest("hex")}`;

export class PostgresPortableCommandCapabilityGrantFactResolver {
  constructor(
    private readonly config: Readonly<{
      sql: SyncSql;
      authority: PortableCapabilityGrantFactAuthority;
      now?: () => string;
    }>,
  ) {}

  async resolve(
    scope: PortableCommandCapabilityGrantFactScope,
  ): Promise<PortableCommandCapabilityGrantResolution> {
    if (
      scope.sourceLeaseRefs.length === 0 ||
      scope.sourceLeaseRefs.some((ref) => !SAFE_REF.test(ref))
    ) {
      throw new PortableCommandCapabilityGrantResolverError(
        "portable source lease scope is invalid",
      );
    }
    const rows: BrokerRow[] = await this.config.sql`
      SELECT broker.state_json, claim.command_ref AS claim_command_ref,
             broker.active_move_ref, broker.active_command_ref,
             broker.active_source_attachment_ref, broker.active_source_generation,
             broker.active_destination_target_ref
      FROM khala_sync_portable_capability_brokers AS broker
      JOIN khala_sync_portable_command_executions AS claim
        ON claim.claim_ref = ${scope.commandExecutionClaimRef}
       AND claim.owner_user_id = broker.owner_user_id
       AND claim.session_ref = broker.session_ref
      WHERE broker.owner_user_id = ${scope.ownerRef}
        AND broker.session_ref = ${scope.sessionRef}
        AND claim.source_attachment_ref = ${scope.sourceAttachmentRef}
        AND claim.source_generation = ${scope.sourceGeneration}
        AND claim.executor_environment_ref = ${scope.sourceTargetRef}
        AND claim.destination_target_ref = ${scope.destinationTargetRef}
        AND claim.state = 'claimed'
    `;
    const row = rows[0];
    if (row === undefined)
      throw new PortableCommandCapabilityGrantResolverError("exact capability claim is absent");
    if (
      row.active_move_ref !== null &&
      (row.active_move_ref !== scope.commandExecutionClaimRef ||
        row.active_command_ref !== row.claim_command_ref ||
        row.active_source_attachment_ref !== scope.sourceAttachmentRef ||
        Number(row.active_source_generation) !== scope.sourceGeneration ||
        row.active_destination_target_ref !== scope.destinationTargetRef)
    )
      throw new PortableCommandCapabilityGrantResolverError("portable broker claim conflicts");
    const state = parseState(row.state_json);
    const records = scope.sourceLeaseRefs.map((leaseRef) => {
      const matches = state.records.filter((record) => record.lease.leaseRef === leaseRef);
      if (
        matches.length !== 1 ||
        matches[0]?.lease.ownerRef !== scope.ownerRef ||
        matches[0].lease.sessionRef !== scope.sessionRef ||
        matches[0].lease.attachmentRef !== scope.sourceAttachmentRef ||
        matches[0].lease.attachmentGeneration !== scope.sourceGeneration ||
        matches[0].lease.targetRef !== scope.sourceTargetRef ||
        !["issued", "redeemed"].includes(matches[0].lease.state) ||
        Date.parse(matches[0].lease.expiresAt) <=
          Date.parse(this.config.now?.() ?? new Date().toISOString()) ||
        matches[0].revokedAt !== undefined
      ) {
        throw new PortableCommandCapabilityGrantResolverError(
          "source lease does not match broker authority",
        );
      }
      return matches[0];
    });
    if (
      state.records
        .filter((record) => record.revokedAt === undefined)
        .some((record) => !scope.sourceLeaseRefs.includes(record.lease.leaseRef))
    ) {
      throw new PortableCommandCapabilityGrantResolverError(
        "source lease set differs from broker authority",
      );
    }
    const sourceGrantRefs = records.map((record) => record.sourceGrantRef);
    if (new Set(sourceGrantRefs).size !== sourceGrantRefs.length) {
      throw new PortableCommandCapabilityGrantResolverError("source grants are ambiguous");
    }
    const authorityFacts = await this.config.authority.resolve({
      ownerUserId: scope.ownerRef,
      grantRefs: sourceGrantRefs,
    });
    const byRef = new Map(authorityFacts.map((fact) => [fact.grantRef, fact]));
    const now = new Date(this.config.now?.() ?? new Date().toISOString());
    const bindings: PortableGrantAuthorityBinding[] = [];
    const facts: PortableCommandCapabilityGrantFact[] = records.map((record) => {
      const authority = byRef.get(record.sourceGrantRef);
      if (
        authority === undefined ||
        authority.ownerUserId !== scope.ownerRef ||
        authority.status !== "issued" ||
        new Date(authority.expiresAt) <= now
      )
        throw new PortableCommandCapabilityGrantResolverError(
          "source grant is not active for owner",
        );
      bindings.push({
        grantRef: authority.grantRef,
        ownerUserId: authority.ownerUserId,
        kind: authority.kind,
        ...(authority.providerAccountRef === undefined
          ? {}
          : { providerAccountRef: authority.providerAccountRef }),
        ...(authority.runnerSessionId === undefined
          ? {}
          : { runnerSessionId: authority.runnerSessionId }),
      });
      return {
        sourceLeaseRef: record.lease.leaseRef,
        destinationSourceGrantRef: destinationGrantRef(scope, authority.grantRef),
        expiresAt: new Date(
          Math.min(
            Date.parse(authority.expiresAt),
            Date.parse(record.lease.expiresAt),
            Date.parse(scope.commandLeaseExpiresAt),
          ),
        ).toISOString(),
      };
    });
    if (authorityFacts.length !== records.length) {
      throw new PortableCommandCapabilityGrantResolverError(
        "grant fact set differs from source grants",
      );
    }
    return { facts, bindings };
  }
}
