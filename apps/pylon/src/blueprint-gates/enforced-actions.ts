import type { CommandSourceVerifiedResult } from "./command-execution-source-verified.js"
import type { DiagnosisGroundingResult } from "./diagnosis-grounding.js"
import type { FleetLivenessResult } from "./fleet-liveness.js"
import type { IssueCloseSafeResult } from "./issue-close-safe.js"
import type { MergeDeployGateResult } from "./merge-deploy-gate.js"

export type BlueprintGateId =
  | "fleet-liveness"
  | "diagnosis-grounding"
  | "issue-close-safe"
  | "command-source-verified"
  | "merge-deploy"

export type GateBlocked = Readonly<{
  ok: false
  gate: BlueprintGateId
  state: string
  reason: string
  missingEvidence: ReadonlyArray<string>
}>

export type GateAuthorized<TAction> = Readonly<{
  ok: true
  gate: BlueprintGateId
  state: string
  action: TAction
  evidenceRefs: ReadonlyArray<string>
}>

export type GateDecision<TAction> = GateAuthorized<TAction> | GateBlocked

export type FleetHealthyReportAction = Readonly<{
  kind: "report_fleet_healthy"
  reason: string
}>

export type DiagnosisRemediationAction = Readonly<{
  kind: "propose_remediation"
  remediation: string
}>

export type IssueCloseAction = Readonly<{
  kind: "emit_closes_keyword"
  issueNumber: number
  prNumber: number
}>

export type CommandProposalAction = Readonly<{
  kind: "propose_command"
  commandString: string
}>

export type MergeDeployLiveAction = Readonly<{
  kind: "report_deployment_live"
  prNumbers: ReadonlyArray<number>
}>

const blocked = (
  gate: BlueprintGateId,
  state: string,
  reason: string | null | undefined,
  missingEvidence: ReadonlyArray<string>,
): GateBlocked => ({
  ok: false,
  gate,
  state,
  reason: reason ?? "blueprint gate has not reached its terminal state",
  missingEvidence,
})

export const authorizeFleetHealthyReport = (
  gate: FleetLivenessResult,
): GateDecision<FleetHealthyReportAction> =>
  gate.canReportHealthy
    ? {
        ok: true,
        gate: "fleet-liveness",
        state: gate.gateState,
        action: { kind: "report_fleet_healthy", reason: gate.reason },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("fleet-liveness", gate.gateState, gate.reason, gate.missingEvidence)

export const authorizeDiagnosisRemediation = (
  gate: DiagnosisGroundingResult,
  remediation: string,
): GateDecision<DiagnosisRemediationAction> =>
  gate.canProposeRemediation
    ? {
        ok: true,
        gate: "diagnosis-grounding",
        state: gate.state,
        action: { kind: "propose_remediation", remediation },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("diagnosis-grounding", gate.state, gate.blockedReason, gate.missingEvidence)

export const authorizeIssueClose = (
  gate: IssueCloseSafeResult,
  issueNumber: number,
  prNumber: number,
): GateDecision<IssueCloseAction> =>
  gate.canClose
    ? {
        ok: true,
        gate: "issue-close-safe",
        state: gate.state,
        action: { kind: "emit_closes_keyword", issueNumber, prNumber },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("issue-close-safe", gate.state, gate.blockedReason, gate.missingEvidence)

export const authorizeCommandProposal = (
  gate: CommandSourceVerifiedResult,
  commandString: string,
): GateDecision<CommandProposalAction> =>
  gate.canPropose
    ? {
        ok: true,
        gate: "command-source-verified",
        state: gate.state,
        action: { kind: "propose_command", commandString },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("command-source-verified", gate.state, gate.blockedReason, gate.missingEvidence)

export const authorizeMergeDeployLiveReport = (
  gate: MergeDeployGateResult,
  prNumbers: ReadonlyArray<number>,
): GateDecision<MergeDeployLiveAction> =>
  gate.isLive
    ? {
        ok: true,
        gate: "merge-deploy",
        state: gate.state,
        action: { kind: "report_deployment_live", prNumbers },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("merge-deploy", gate.state, gate.blockedReason, gate.missingEvidence)
