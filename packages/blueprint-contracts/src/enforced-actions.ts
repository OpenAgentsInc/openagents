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
  claimedRootCause: string
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
  scriptPath: string
}>

export type MergeDeployLiveAction = Readonly<{
  kind: "report_deployment_live"
  prNumbers: ReadonlyArray<number>
  mergeCommitHashes: ReadonlyArray<string>
}>

export type FleetLivenessAuthorizerResult = Readonly<{
  canReportHealthy: boolean
  gateState: string
  reason: string
  missingEvidence: ReadonlyArray<string>
  satisfiedEvidence: ReadonlyArray<string>
}>

export type DiagnosisGroundingAuthorizerResult = Readonly<{
  canProposeRemediation: boolean
  state: string
  blockedReason: string | null
  missingEvidence: ReadonlyArray<string>
  satisfiedEvidence: ReadonlyArray<string>
  identity: Readonly<{ claimedRootCause: string }>
}>

export type IssueCloseAuthorizerResult = Readonly<{
  canClose: boolean
  state: string
  blockedReason: string | null
  missingEvidence: ReadonlyArray<string>
  satisfiedEvidence: ReadonlyArray<string>
  identity: Readonly<{ issueNumber: number; prNumber: number }>
}>

export type CommandProposalAuthorizerResult = Readonly<{
  canPropose: boolean
  state: string
  blockedReason: string | null
  missingEvidence: ReadonlyArray<string>
  satisfiedEvidence: ReadonlyArray<string>
  identity: Readonly<{ commandString: string; scriptPath: string }>
}>

export type MergeDeployAuthorizerResult = Readonly<{
  isLive: boolean
  state: string
  blockedReason: string | null
  missingEvidence: ReadonlyArray<string>
  satisfiedEvidence: ReadonlyArray<string>
  identity: Readonly<{
    prNumbers: ReadonlyArray<number>
    mergeCommitHashes: ReadonlyArray<string>
  }>
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
  gate: FleetLivenessAuthorizerResult,
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
  gate: DiagnosisGroundingAuthorizerResult,
  remediation: string,
): GateDecision<DiagnosisRemediationAction> =>
  gate.canProposeRemediation
    ? {
        ok: true,
        gate: "diagnosis-grounding",
        state: gate.state,
        action: {
          kind: "propose_remediation",
          claimedRootCause: gate.identity.claimedRootCause,
          remediation,
        },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("diagnosis-grounding", gate.state, gate.blockedReason, gate.missingEvidence)

export const authorizeIssueClose = (
  gate: IssueCloseAuthorizerResult,
): GateDecision<IssueCloseAction> =>
  gate.canClose
    ? {
        ok: true,
        gate: "issue-close-safe",
        state: gate.state,
        action: {
          kind: "emit_closes_keyword",
          issueNumber: gate.identity.issueNumber,
          prNumber: gate.identity.prNumber,
        },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("issue-close-safe", gate.state, gate.blockedReason, gate.missingEvidence)

export const authorizeCommandProposal = (
  gate: CommandProposalAuthorizerResult,
): GateDecision<CommandProposalAction> =>
  gate.canPropose
    ? {
        ok: true,
        gate: "command-source-verified",
        state: gate.state,
        action: {
          kind: "propose_command",
          commandString: gate.identity.commandString,
          scriptPath: gate.identity.scriptPath,
        },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("command-source-verified", gate.state, gate.blockedReason, gate.missingEvidence)

export const authorizeMergeDeployLiveReport = (
  gate: MergeDeployAuthorizerResult,
): GateDecision<MergeDeployLiveAction> =>
  gate.isLive
    ? {
        ok: true,
        gate: "merge-deploy",
        state: gate.state,
        action: {
          kind: "report_deployment_live",
          prNumbers: gate.identity.prNumbers,
          mergeCommitHashes: gate.identity.mergeCommitHashes,
        },
        evidenceRefs: gate.satisfiedEvidence,
      }
    : blocked("merge-deploy", gate.state, gate.blockedReason, gate.missingEvidence)
