import type {
  CloudComputerCheckpointRestorePlan,
  PostgresCloudComputerCheckpointStore,
} from "./cloud-computer-checkpoint-store.js";
import type { SyncSql, SyncTransactionSql } from "./sql.js";

const REF = /^[a-z][a-z0-9._/-]{2,511}$/u;
const SHA256 = /^sha256:[0-9a-f]{64}$/u;
const LOCK = "openagents.cloud-computer.lifecycle";

export class CloudComputerLifecycleError extends Error {
  constructor(
    readonly code: "conflict" | "invalid" | "stale_generation",
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerLifecycleError";
  }
}

type Scope = Readonly<{
  computerRef: string;
  workspaceRef: string;
  ownerRef: string;
  tenantRef: string;
}>;

export interface CloudComputerLifecycleStateStore {
  beginStop(input: Scope & Readonly<{ generation: number; observedAt: string }>): Promise<void>;
  finishStop(
    input: Scope &
      Readonly<{
        generation: number;
        checkpointRef: string | null;
        outcome: "cold" | "failed";
        observedAt: string;
      }>,
  ): Promise<void>;
  beginRestore(
    input: Scope &
      Readonly<{
        expectedGeneration: number;
        nextGeneration: number;
        observedAt: string;
      }>,
  ): Promise<void>;
  finishRestore(
    input: Scope &
      Readonly<{
        generation: number;
        providerLeaseRef: string | null;
        outcome: "active" | "failed";
        observedAt: string;
      }>,
  ): Promise<void>;
  recordHostLoss(
    input: Scope &
      Readonly<{
        evidenceRef: string;
        generation: number;
        providerLeaseRef: string;
        evidenceDigest: string;
        observedAt: string;
      }>,
  ): Promise<void>;
}

const assertInput = (input: Scope, generation: number, observedAt: string): void => {
  for (const value of [input.computerRef, input.workspaceRef, input.ownerRef, input.tenantRef]) {
    if (!REF.test(value)) throw new CloudComputerLifecycleError("invalid", "scope ref is invalid");
  }
  if (
    !Number.isSafeInteger(generation) ||
    generation < 1 ||
    !Number.isFinite(Date.parse(observedAt))
  )
    throw new CloudComputerLifecycleError("invalid", "generation or timestamp is invalid");
};

export class PostgresCloudComputerLifecycleStateStore implements CloudComputerLifecycleStateStore {
  constructor(private readonly sql: SyncSql) {}

  private lock(tx: SyncTransactionSql, computerRef: string): Promise<unknown> {
    return tx`SELECT pg_advisory_xact_lock(hashtextextended(${`${LOCK}|${computerRef}`}, 3106))`;
  }

  async beginStop(input: Scope & Readonly<{ generation: number; observedAt: string }>) {
    assertInput(input, input.generation, input.observedAt);
    const rows: ReadonlyArray<{ computer_ref: string }> = await this.sql`
      UPDATE khala_sync_cloud_computers AS computer
      SET state = 'stopping', version = computer.version + 1, updated_at = ${input.observedAt}
      FROM khala_sync_cloud_computer_workspaces AS workspace
      WHERE computer.computer_ref = ${input.computerRef}
        AND workspace.workspace_ref = ${input.workspaceRef}
        AND workspace.computer_ref = computer.computer_ref
        AND computer.owner_ref = ${input.ownerRef} AND computer.tenant_ref = ${input.tenantRef}
        AND computer.generation = ${input.generation} AND computer.state = 'active'
        AND workspace.runtime_generation = computer.generation AND workspace.state = 'active'
      RETURNING computer.computer_ref
    `;
    if (rows.length !== 1) throw new CloudComputerLifecycleError("conflict", "stop CAS failed");
  }

  async finishStop(
    input: Scope &
      Readonly<{
        generation: number;
        checkpointRef: string | null;
        outcome: "cold" | "failed";
        observedAt: string;
      }>,
  ) {
    assertInput(input, input.generation, input.observedAt);
    if (input.outcome === "cold" && input.checkpointRef === null)
      throw new CloudComputerLifecycleError("invalid", "cold requires a checkpoint");
    const rows: ReadonlyArray<{ computer_ref: string }> = await this.sql`
      UPDATE khala_sync_cloud_computers
      SET state = ${input.outcome}, active_lease_ref = NULL, version = version + 1,
          updated_at = ${input.observedAt}
      WHERE computer_ref = ${input.computerRef} AND owner_ref = ${input.ownerRef}
        AND tenant_ref = ${input.tenantRef} AND generation = ${input.generation}
        AND state = 'stopping'
        AND (${input.outcome} = 'failed' OR latest_checkpoint_ref = ${input.checkpointRef})
      RETURNING computer_ref
    `;
    if (rows.length !== 1)
      throw new CloudComputerLifecycleError("conflict", "stop settlement CAS failed");
  }

  async beginRestore(
    input: Scope &
      Readonly<{ expectedGeneration: number; nextGeneration: number; observedAt: string }>,
  ) {
    assertInput(input, input.expectedGeneration, input.observedAt);
    if (
      !Number.isSafeInteger(input.nextGeneration) ||
      input.nextGeneration <= input.expectedGeneration
    )
      throw new CloudComputerLifecycleError("invalid", "next generation must advance");
    await this.sql.begin("isolation level serializable", async (tx) => {
      await this.lock(tx, input.computerRef);
      const rows: ReadonlyArray<{ computer_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computers AS computer
        SET generation = ${input.nextGeneration}, state = 'starting', active_lease_ref = NULL,
            version = computer.version + 1, updated_at = ${input.observedAt}
        FROM khala_sync_cloud_computer_workspaces AS workspace
        WHERE computer.computer_ref = ${input.computerRef}
          AND workspace.workspace_ref = ${input.workspaceRef}
          AND workspace.computer_ref = computer.computer_ref
          AND computer.owner_ref = ${input.ownerRef} AND computer.tenant_ref = ${input.tenantRef}
          AND computer.generation = ${input.expectedGeneration}
          AND computer.state IN ('cold', 'failed')
          AND workspace.runtime_generation = ${input.expectedGeneration}
          AND workspace.state = 'active'
        RETURNING computer.computer_ref
      `;
      if (rows.length !== 1)
        throw new CloudComputerLifecycleError("stale_generation", "restore generation CAS failed");
    });
  }

  async finishRestore(
    input: Scope &
      Readonly<{
        generation: number;
        providerLeaseRef: string | null;
        outcome: "active" | "failed";
        observedAt: string;
      }>,
  ) {
    assertInput(input, input.generation, input.observedAt);
    if ((input.outcome === "active") !== (input.providerLeaseRef !== null))
      throw new CloudComputerLifecycleError("invalid", "active outcome requires one lease");
    const rows: ReadonlyArray<{ computer_ref: string }> = await this.sql`
      UPDATE khala_sync_cloud_computers
      SET state = ${input.outcome}, active_lease_ref = ${input.providerLeaseRef},
          version = version + 1, updated_at = ${input.observedAt}
      WHERE computer_ref = ${input.computerRef} AND owner_ref = ${input.ownerRef}
        AND tenant_ref = ${input.tenantRef} AND generation = ${input.generation}
        AND state = 'starting'
      RETURNING computer_ref
    `;
    if (rows.length !== 1)
      throw new CloudComputerLifecycleError("stale_generation", "restore settlement CAS failed");
  }

  async recordHostLoss(
    input: Scope &
      Readonly<{
        evidenceRef: string;
        generation: number;
        providerLeaseRef: string;
        evidenceDigest: string;
        observedAt: string;
      }>,
  ) {
    assertInput(input, input.generation, input.observedAt);
    if (
      !REF.test(input.evidenceRef) ||
      !REF.test(input.providerLeaseRef) ||
      !SHA256.test(input.evidenceDigest)
    )
      throw new CloudComputerLifecycleError("invalid", "host-loss evidence is invalid");
    await this.sql.begin("isolation level serializable", async (tx) => {
      await this.lock(tx, input.computerRef);
      const prior: ReadonlyArray<{
        computer_ref: string;
        workspace_ref: string;
        runtime_generation: string | number;
        provider_lease_ref: string;
        evidence_digest: string;
      }> = await tx`
        SELECT computer_ref, workspace_ref, runtime_generation, provider_lease_ref,
               evidence_digest
        FROM khala_sync_cloud_computer_host_loss_evidence
        WHERE evidence_ref = ${input.evidenceRef}
        FOR UPDATE
      `;
      if (prior[0] !== undefined) {
        const evidence = prior[0];
        if (
          evidence.computer_ref !== input.computerRef ||
          evidence.workspace_ref !== input.workspaceRef ||
          Number(evidence.runtime_generation) !== input.generation ||
          evidence.provider_lease_ref !== input.providerLeaseRef ||
          evidence.evidence_digest !== input.evidenceDigest
        )
          throw new CloudComputerLifecycleError("conflict", "host-loss evidence differs");
        return;
      }
      const inserted: ReadonlyArray<{ evidence_ref: string }> = await tx`
        INSERT INTO khala_sync_cloud_computer_host_loss_evidence
          (evidence_ref, computer_ref, workspace_ref, owner_ref, tenant_ref,
           runtime_generation, provider_lease_ref, evidence_digest, observed_at)
        SELECT ${input.evidenceRef}, computer.computer_ref, workspace.workspace_ref,
               computer.owner_ref, computer.tenant_ref, computer.generation,
               ${input.providerLeaseRef}, ${input.evidenceDigest}, ${input.observedAt}
        FROM khala_sync_cloud_computers AS computer
        JOIN khala_sync_cloud_computer_workspaces AS workspace
          ON workspace.computer_ref = computer.computer_ref
        WHERE computer.computer_ref = ${input.computerRef}
          AND workspace.workspace_ref = ${input.workspaceRef}
          AND computer.owner_ref = ${input.ownerRef} AND computer.tenant_ref = ${input.tenantRef}
          AND computer.generation = ${input.generation}
          AND computer.active_lease_ref = ${input.providerLeaseRef}
          AND computer.state IN ('active', 'stopping')
        ON CONFLICT (evidence_ref) DO UPDATE SET evidence_ref = EXCLUDED.evidence_ref
        WHERE khala_sync_cloud_computer_host_loss_evidence.evidence_digest = EXCLUDED.evidence_digest
        RETURNING evidence_ref
      `;
      if (inserted.length !== 1)
        throw new CloudComputerLifecycleError("conflict", "host-loss evidence CAS failed");
      const updated: ReadonlyArray<{ computer_ref: string }> = await tx`
        UPDATE khala_sync_cloud_computers
        SET state = 'failed', active_lease_ref = NULL, version = version + 1,
            updated_at = ${input.observedAt}
        WHERE computer_ref = ${input.computerRef} AND generation = ${input.generation}
          AND active_lease_ref = ${input.providerLeaseRef} AND state IN ('active', 'stopping')
        RETURNING computer_ref
      `;
      if (updated.length !== 1)
        throw new CloudComputerLifecycleError("conflict", "host-loss state CAS failed");
    });
  }
}

type LifecycleCheckpointWriter = (
  input: Scope & Readonly<{ generation: number; boundary: "stop" }>,
) => Promise<Readonly<{ checkpointRef: string }>>;
type LifecycleRestorer = (
  input: Readonly<{
    plan: CloudComputerCheckpointRestorePlan;
    expectedBaseImageDigest: string;
  }>,
) => Promise<void>;

export class CloudComputerLifecycle {
  constructor(
    private readonly state: CloudComputerLifecycleStateStore,
    private readonly checkpoints: PostgresCloudComputerCheckpointStore,
  ) {}

  async stop(
    input: Scope & Readonly<{ generation: number; observedAt: string }>,
    writeCheckpoint: LifecycleCheckpointWriter,
  ): Promise<Readonly<{ state: "cold"; checkpointRef: string }>> {
    await this.state.beginStop(input);
    try {
      const receipt = await writeCheckpoint({ ...input, boundary: "stop" });
      await this.state.finishStop({
        ...input,
        checkpointRef: receipt.checkpointRef,
        outcome: "cold",
      });
      return { state: "cold", checkpointRef: receipt.checkpointRef };
    } catch (error) {
      await this.state.finishStop({ ...input, checkpointRef: null, outcome: "failed" });
      throw error;
    }
  }

  async resume(
    input: Scope &
      Readonly<{
        expectedGeneration: number;
        nextGeneration: number;
        providerLeaseRef: string;
        observedAt: string;
      }>,
    restore: LifecycleRestorer,
  ): Promise<Readonly<{ state: "active"; plan: CloudComputerCheckpointRestorePlan }>> {
    await this.state.beginRestore(input);
    try {
      await this.checkpoints.advanceRuntimeGeneration({
        workspaceRef: input.workspaceRef,
        ownerRef: input.ownerRef,
        tenantRef: input.tenantRef,
        expectedRuntimeGeneration: input.expectedGeneration,
        nextRuntimeGeneration: input.nextGeneration,
        observedAt: input.observedAt,
      });
      const plan = await this.checkpoints.restorePlan({
        workspaceRef: input.workspaceRef,
        ownerRef: input.ownerRef,
        tenantRef: input.tenantRef,
        expectedRuntimeGeneration: input.nextGeneration,
      });
      await restore({ plan, expectedBaseImageDigest: plan.baseImageDigest });
      await this.state.finishRestore({
        ...input,
        generation: input.nextGeneration,
        outcome: "active",
      });
      return { state: "active", plan };
    } catch (error) {
      await this.state.finishRestore({
        ...input,
        generation: input.nextGeneration,
        providerLeaseRef: null,
        outcome: "failed",
      });
      throw error;
    }
  }

  async replaceLostHost(
    input: Scope &
      Readonly<{
        evidenceRef: string;
        evidenceDigest: string;
        expectedGeneration: number;
        nextGeneration: number;
        lostProviderLeaseRef: string;
        replacementProviderLeaseRef: string;
        observedAt: string;
      }>,
    restore: LifecycleRestorer,
  ) {
    await this.state.recordHostLoss({
      ...input,
      generation: input.expectedGeneration,
      providerLeaseRef: input.lostProviderLeaseRef,
    });
    return this.resume({ ...input, providerLeaseRef: input.replacementProviderLeaseRef }, restore);
  }
}
