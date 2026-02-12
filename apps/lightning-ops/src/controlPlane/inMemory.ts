import { Effect, Layer } from "effect";

import type {
  CompileDiagnostic,
  ControlPlaneCredentialRoleState,
  ControlPlaneInvoiceRecord,
  ControlPlaneOwnerSecurityControl,
  ControlPlanePaywall,
  ControlPlaneSecurityGlobal,
  ControlPlaneSettlementRecord,
  DeploymentIntentRecord,
  GatewayEventRecord,
  InvoiceLifecycleStatus,
} from "../contracts.js";
import { formatPaymentProofReference, normalizePreimageHex } from "../settlements/proof.js";

import {
  type CredentialRoleOperationInput,
  ControlPlaneService,
  type ControlPlaneSecurityState,
  type RecordDeploymentIntentInput,
  type RecordInvoiceLifecycleInput,
  type RecordSettlementInput,
  type RecordSettlementResult,
  type SetGlobalPauseInput,
  type SetOwnerKillSwitchInput,
} from "./service.js";

type ControlPlaneApi = Parameters<typeof ControlPlaneService.of>[0];

export type InMemoryControlPlaneState = {
  paywalls: Array<ControlPlanePaywall>;
  deployments: Array<DeploymentIntentRecord>;
  events: Array<GatewayEventRecord>;
  invoices: Array<ControlPlaneInvoiceRecord>;
  settlements: Array<ControlPlaneSettlementRecord>;
  globalSecurity: ControlPlaneSecurityGlobal;
  ownerSecurityControls: Array<ControlPlaneOwnerSecurityControl>;
  credentialRoles: Array<ControlPlaneCredentialRoleState>;
  writeCalls: Array<RecordDeploymentIntentInput>;
  invoiceWriteCalls: Array<RecordInvoiceLifecycleInput>;
  settlementWriteCalls: Array<RecordSettlementInput>;
  globalPauseWriteCalls: Array<SetGlobalPauseInput>;
  ownerKillSwitchWriteCalls: Array<SetOwnerKillSwitchInput>;
  credentialRotateCalls: Array<CredentialRoleOperationInput>;
  credentialActivateCalls: Array<CredentialRoleOperationInput>;
  credentialRevokeCalls: Array<Omit<CredentialRoleOperationInput, "fingerprint">>;
};

const invoiceRank: Record<InvoiceLifecycleStatus, number> = {
  open: 0,
  canceled: 1,
  expired: 1,
  settled: 2,
};

const chooseInvoiceStatus = (
  current: InvoiceLifecycleStatus,
  incoming: InvoiceLifecycleStatus,
): InvoiceLifecycleStatus => (invoiceRank[incoming] > invoiceRank[current] ? incoming : current);

const cloneDiagnostics = (diagnostics: ReadonlyArray<CompileDiagnostic>) =>
  diagnostics.map((diag) => ({ ...diag }));

const clonePaywall = (paywall: ControlPlanePaywall): ControlPlanePaywall => ({
  ...paywall,
  policy: {
    ...paywall.policy,
    allowedHosts: paywall.policy.allowedHosts ? [...paywall.policy.allowedHosts] : undefined,
    blockedHosts: paywall.policy.blockedHosts ? [...paywall.policy.blockedHosts] : undefined,
  },
  routes: paywall.routes.map((route) => ({ ...route })),
});

const cloneDeployment = (deployment: DeploymentIntentRecord): DeploymentIntentRecord => ({
  ...deployment,
  diagnostics: deployment.diagnostics,
});

const cloneInvoice = (invoice: ControlPlaneInvoiceRecord): ControlPlaneInvoiceRecord => ({
  ...invoice,
});

const cloneSettlement = (settlement: ControlPlaneSettlementRecord): ControlPlaneSettlementRecord => ({
  ...settlement,
  metadata: settlement.metadata,
});

const cloneGlobalSecurity = (global: ControlPlaneSecurityGlobal): ControlPlaneSecurityGlobal => ({
  ...global,
});

const cloneOwnerSecurityControl = (
  control: ControlPlaneOwnerSecurityControl,
): ControlPlaneOwnerSecurityControl => ({
  ...control,
});

const cloneCredentialRole = (role: ControlPlaneCredentialRoleState): ControlPlaneCredentialRoleState => ({
  ...role,
});

const cloneRecordInvoiceInput = (input: RecordInvoiceLifecycleInput): RecordInvoiceLifecycleInput => ({
  ...input,
});

const cloneRecordSettlementInput = (input: RecordSettlementInput): RecordSettlementInput => ({
  ...input,
  metadata: input.metadata,
});

export const makeInMemoryControlPlaneHarness = (input?: {
  readonly paywalls?: ReadonlyArray<ControlPlanePaywall>;
  readonly globalSecurity?: ControlPlaneSecurityGlobal;
  readonly ownerSecurityControls?: ReadonlyArray<ControlPlaneOwnerSecurityControl>;
  readonly credentialRoles?: ReadonlyArray<ControlPlaneCredentialRoleState>;
}) => {
  const defaultGlobalSecurity: ControlPlaneSecurityGlobal = {
    stateId: "global",
    globalPause: false,
    updatedAtMs: 0,
  };

  const state: InMemoryControlPlaneState = {
    paywalls: [...(input?.paywalls ?? [])].map(clonePaywall),
    deployments: [],
    events: [],
    invoices: [],
    settlements: [],
    globalSecurity: cloneGlobalSecurity(input?.globalSecurity ?? defaultGlobalSecurity),
    ownerSecurityControls: [...(input?.ownerSecurityControls ?? [])].map(cloneOwnerSecurityControl),
    credentialRoles: [...(input?.credentialRoles ?? [])].map(cloneCredentialRole),
    writeCalls: [],
    invoiceWriteCalls: [],
    settlementWriteCalls: [],
    globalPauseWriteCalls: [],
    ownerKillSwitchWriteCalls: [],
    credentialRotateCalls: [],
    credentialActivateCalls: [],
    credentialRevokeCalls: [],
  };
  let nextDeployment = 1;
  let nextEvent = 1;

  const writeInvoiceLifecycle = (
    args: RecordInvoiceLifecycleInput,
    options?: { readonly trackWrite?: boolean },
  ): ControlPlaneInvoiceRecord => {
    if (options?.trackWrite !== false) {
      state.invoiceWriteCalls.push(cloneRecordInvoiceInput(args));
    }

    const now = Date.now();
    const existingIndex = state.invoices.findIndex((invoice) => invoice.invoiceId === args.invoiceId);

    if (existingIndex < 0) {
      const created: ControlPlaneInvoiceRecord = {
        invoiceId: args.invoiceId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        amountMsats: args.amountMsats,
        status: args.status,
        ...(args.paymentHash ? { paymentHash: args.paymentHash } : {}),
        ...(args.paymentRequest ? { paymentRequest: args.paymentRequest } : {}),
        ...(args.paymentProofRef ? { paymentProofRef: args.paymentProofRef } : {}),
        ...(args.requestId ? { requestId: args.requestId } : {}),
        createdAtMs: now,
        updatedAtMs: now,
        ...(args.status === "settled" ? { settledAtMs: args.settledAtMs ?? now } : {}),
      };
      state.invoices.push(created);
      return cloneInvoice(created);
    }

    const existing = state.invoices[existingIndex]!;
    const nextStatus = chooseInvoiceStatus(existing.status, args.status);
    const nextPaymentHash = existing.paymentHash ?? args.paymentHash;
    const nextPaymentRequest = existing.paymentRequest ?? args.paymentRequest;
    const nextPaymentProofRef = existing.paymentProofRef ?? args.paymentProofRef;
    const nextRequestId = existing.requestId ?? args.requestId;
    const nextSettledAtMs =
      nextStatus === "settled" ? (existing.settledAtMs ?? args.settledAtMs ?? now) : existing.settledAtMs;

    const updated: ControlPlaneInvoiceRecord = {
      ...existing,
      paywallId: args.paywallId,
      ownerId: args.ownerId,
      amountMsats: args.amountMsats,
      status: nextStatus,
      ...(nextPaymentHash ? { paymentHash: nextPaymentHash } : {}),
      ...(nextPaymentRequest ? { paymentRequest: nextPaymentRequest } : {}),
      ...(nextPaymentProofRef ? { paymentProofRef: nextPaymentProofRef } : {}),
      ...(nextRequestId ? { requestId: nextRequestId } : {}),
      ...(nextSettledAtMs !== undefined ? { settledAtMs: nextSettledAtMs } : {}),
      updatedAtMs: now,
    };

    state.invoices[existingIndex] = updated;
    return cloneInvoice(updated);
  };

  const listPaywallsForCompile = () => Effect.sync(() => state.paywalls.map(clonePaywall));

  const recordDeploymentIntent = (args: RecordDeploymentIntentInput) =>
    Effect.sync(() => {
      state.writeCalls.push({ ...args, diagnostics: cloneDiagnostics(args.diagnostics) });
      const deploymentId = args.deploymentId ?? `dep_mem_${nextDeployment++}`;
      const now = Date.now();
      const existingIndex = state.deployments.findIndex((deployment) => deployment.deploymentId === deploymentId);
      const next: DeploymentIntentRecord = {
        deploymentId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        configHash: args.configHash,
        imageDigest: args.imageDigest,
        status: args.status,
        diagnostics: {
          diagnostics: cloneDiagnostics(args.diagnostics),
          metadata: args.metadata,
          requestId: args.requestId,
        },
        appliedAtMs: args.appliedAtMs,
        rolledBackFrom: args.rolledBackFrom,
        createdAtMs: existingIndex >= 0 ? state.deployments[existingIndex]!.createdAtMs : now,
        updatedAtMs: now,
      };

      if (existingIndex >= 0) {
        state.deployments[existingIndex] = next;
      } else {
        state.deployments.push(next);
      }

      return cloneDeployment(next);
    });

  const recordGatewayEvent: ControlPlaneApi["recordGatewayEvent"] = (args) =>
    Effect.sync(() => {
      const event: GatewayEventRecord = {
        eventId: `evt_mem_${nextEvent++}`,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        eventType: args.eventType,
        level: args.level,
        ...(args.requestId ? { requestId: args.requestId } : {}),
        ...(args.metadata !== undefined ? { metadata: args.metadata } : {}),
        createdAtMs: Date.now(),
      };
      state.events.push(event);
      return { ...event };
    });

  const recordInvoiceLifecycle: ControlPlaneApi["recordInvoiceLifecycle"] = (args) =>
    Effect.sync(() => writeInvoiceLifecycle(args, { trackWrite: true }));

  const recordSettlement: ControlPlaneApi["recordSettlement"] = (args) =>
    Effect.sync(() => {
      state.settlementWriteCalls.push(cloneRecordSettlementInput(args));
      const existing = state.settlements.find((settlement) => settlement.settlementId === args.settlementId);

      const upsertLinkedInvoice = () =>
        args.invoiceId
          ? writeInvoiceLifecycle(
              {
                invoiceId: args.invoiceId,
                paywallId: args.paywallId,
                ownerId: args.ownerId,
                amountMsats: args.amountMsats,
                status: "settled",
                ...(args.paymentHash ? { paymentHash: args.paymentHash } : {}),
                ...(args.requestId ? { requestId: args.requestId } : {}),
              },
              { trackWrite: true },
            )
          : undefined;

      if (existing) {
        const invoice = upsertLinkedInvoice();
        const result: RecordSettlementResult = {
          existed: true,
          settlement: cloneSettlement(existing),
          ...(invoice ? { invoice } : {}),
        };
        return result;
      }

      if (args.paymentProofType !== "lightning_preimage") {
        throw new Error("invalid_payment_proof_type");
      }

      const preimageHex = normalizePreimageHex(args.paymentProofValue);
      if (!preimageHex) {
        throw new Error("invalid_preimage");
      }

      const settlement: ControlPlaneSettlementRecord = {
        settlementId: args.settlementId,
        paywallId: args.paywallId,
        ownerId: args.ownerId,
        ...(args.invoiceId ? { invoiceId: args.invoiceId } : {}),
        amountMsats: args.amountMsats,
        paymentProofRef: formatPaymentProofReference(preimageHex),
        ...(args.requestId ? { requestId: args.requestId } : {}),
        ...(args.metadata !== undefined ? { metadata: args.metadata } : {}),
        createdAtMs: Date.now(),
      };
      state.settlements.push(settlement);

      const invoice = upsertLinkedInvoice();
      const result: RecordSettlementResult = {
        existed: false,
        settlement: cloneSettlement(settlement),
        ...(invoice ? { invoice } : {}),
      };
      return result;
    });

  const getSecurityState: ControlPlaneApi["getSecurityState"] = () =>
    Effect.sync((): ControlPlaneSecurityState => ({
      global: cloneGlobalSecurity(state.globalSecurity),
      ownerControls: state.ownerSecurityControls.map(cloneOwnerSecurityControl),
      credentialRoles: state.credentialRoles.map(cloneCredentialRole),
    }));

  const setGlobalPause: ControlPlaneApi["setGlobalPause"] = (args) =>
    Effect.sync(() => {
      state.globalPauseWriteCalls.push({ ...args });
      const now = Date.now();
      const next: ControlPlaneSecurityGlobal = {
        stateId: state.globalSecurity.stateId,
        globalPause: args.active,
        ...(args.active
          ? {
              denyReasonCode: "global_pause_active" as const,
              denyReason: args.reason ?? "Global paywall pause is active",
            }
          : {}),
        ...(args.updatedBy ? { updatedBy: args.updatedBy } : {}),
        updatedAtMs: now,
      };
      state.globalSecurity = next;
      return cloneGlobalSecurity(next);
    });

  const setOwnerKillSwitch: ControlPlaneApi["setOwnerKillSwitch"] = (args) =>
    Effect.sync(() => {
      state.ownerKillSwitchWriteCalls.push({ ...args });
      const now = Date.now();
      const existingIndex = state.ownerSecurityControls.findIndex((row) => row.ownerId === args.ownerId);
      const next: ControlPlaneOwnerSecurityControl = {
        ownerId: args.ownerId,
        killSwitch: args.active,
        ...(args.active
          ? {
              denyReasonCode: "owner_kill_switch_active" as const,
              denyReason: args.reason ?? "Owner kill switch is active",
            }
          : {}),
        ...(args.updatedBy ? { updatedBy: args.updatedBy } : {}),
        updatedAtMs: now,
      };
      if (existingIndex >= 0) {
        state.ownerSecurityControls[existingIndex] = next;
      } else {
        state.ownerSecurityControls.push(next);
      }
      return cloneOwnerSecurityControl(next);
    });

  const upsertCredentialRole = (inputRole: {
    role: "gateway_invoice" | "settlement_read" | "operator_admin";
    status: "active" | "rotating" | "revoked";
    fingerprint?: string;
    note?: string;
    versionResolver: (existing: ControlPlaneCredentialRoleState | null) => number;
    rotatedAtMs?: number;
    revokedAtMs?: number;
  }): ControlPlaneCredentialRoleState => {
    const now = Date.now();
    const existingIndex = state.credentialRoles.findIndex((row) => row.role === inputRole.role);
    const existing = existingIndex >= 0 ? state.credentialRoles[existingIndex]! : null;
    const next: ControlPlaneCredentialRoleState = {
      role: inputRole.role,
      status: inputRole.status,
      version: inputRole.versionResolver(existing),
      ...(inputRole.fingerprint ? { fingerprint: inputRole.fingerprint } : {}),
      ...(inputRole.note ? { note: inputRole.note } : {}),
      updatedAtMs: now,
      ...(inputRole.rotatedAtMs !== undefined ? { lastRotatedAtMs: inputRole.rotatedAtMs } : {}),
      ...(inputRole.revokedAtMs !== undefined ? { revokedAtMs: inputRole.revokedAtMs } : {}),
    };

    if (existingIndex >= 0) {
      state.credentialRoles[existingIndex] = next;
    } else {
      state.credentialRoles.push(next);
    }
    return cloneCredentialRole(next);
  };

  const rotateCredentialRole: ControlPlaneApi["rotateCredentialRole"] = (args) =>
    Effect.sync(() => {
      state.credentialRotateCalls.push({ ...args });
      return upsertCredentialRole({
        role: args.role,
        status: "rotating",
        ...(args.fingerprint ? { fingerprint: args.fingerprint } : {}),
        ...(args.note ? { note: args.note } : {}),
        versionResolver: (existing) => Math.max(1, (existing?.version ?? 0) + 1),
        rotatedAtMs: Date.now(),
      });
    });

  const activateCredentialRole: ControlPlaneApi["activateCredentialRole"] = (args) =>
    Effect.sync(() => {
      state.credentialActivateCalls.push({ ...args });
      return upsertCredentialRole({
        role: args.role,
        status: "active",
        ...(args.fingerprint ? { fingerprint: args.fingerprint } : {}),
        ...(args.note ? { note: args.note } : {}),
        versionResolver: (existing) => {
          if (!existing) return 1;
          if (existing.status === "rotating") return existing.version;
          return Math.max(1, existing.version + 1);
        },
        rotatedAtMs: Date.now(),
      });
    });

  const revokeCredentialRole: ControlPlaneApi["revokeCredentialRole"] = (args) =>
    Effect.sync(() => {
      state.credentialRevokeCalls.push({ ...args });
      return upsertCredentialRole({
        role: args.role,
        status: "revoked",
        ...(args.note ? { note: args.note } : {}),
        versionResolver: (existing) => Math.max(1, existing?.version ?? 1),
        revokedAtMs: Date.now(),
      });
    });

  const layer = Layer.succeed(
    ControlPlaneService,
    ControlPlaneService.of({
      listPaywallsForCompile,
      getSecurityState,
      recordDeploymentIntent,
      recordGatewayEvent,
      recordInvoiceLifecycle,
      recordSettlement,
      setGlobalPause,
      setOwnerKillSwitch,
      rotateCredentialRole,
      activateCredentialRole,
      revokeCredentialRole,
    }),
  );

  return {
    layer,
    state,
  };
};
