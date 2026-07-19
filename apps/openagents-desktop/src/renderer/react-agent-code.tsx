import { Alert, AlertDescription, AlertTitle } from "#components/ui/alert"
import { Badge } from "#components/ui/badge"
import { Button } from "#components/ui/button"
import { ComponentValueBinding, IntentRef, type IntentError, type IntentReporter, type JsonPayload } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"
import { Check, FileDiff, History, RotateCcw, ShieldCheck, X } from "lucide-react"
import { useEffect, useMemo, useState, type ReactElement } from "react"

import {
  projectDocumentGenerationForSource,
  type IdeAgentBacklink,
  type IdeAgentEvidenceFact,
  type IdeAgentProposal,
  type IdeAgentProposalOperation,
} from "../ide/agent-code-contract.ts"
import { PierreReviewAdapter } from "../ide/pierre-diffs-adapter.tsx"
import type { IdeReviewIntent } from "../ide/review-contract.ts"
import type { DesktopShellState } from "./shell.ts"
import { agentProposalReviewSource, selectedAgentProposal } from "./ide/agent-code-review.ts"

const dispatch = (report: IntentReporter, name: string, payload: JsonPayload = null): void => {
  void Effect.runPromise(report(
    payload === null ? IntentRef(name) : IntentRef(name, ComponentValueBinding()), payload,
  ) as Effect.Effect<void, IntentError>).catch((error: unknown) => {
    console.error("[openagents-desktop] agent-code intent failed", name, error instanceof Error ? error.message : "unknown_error")
  })
}


const operationPath = (operation: IdeAgentProposalOperation): string =>
  operation._tag === "Rename" ? `${operation.pathRef} → ${operation.targetPathRef}` : operation.pathRef

const evidenceLabel = (fact: IdeAgentEvidenceFact): string => {
  switch (fact.state._tag) {
    case "Requested": return `requested ${fact.state.requestedAt}`
    case "Running": return `running since ${fact.state.startedAt}`
    case "Passed": return `passed · ${fact.state.summary}`
    case "Failed": return `failed · ${fact.state.summary}`
    case "Unavailable": return `unavailable · ${fact.state.reason}`
    case "Stale": return `stale · ${fact.state.reason}`
  }
}

const backlinkLabel = (backlink: IdeAgentBacklink): string => {
  switch (backlink.resolution._tag) {
    case "Current": return `${backlink.resolution.pathRef} · current generation ${backlink.resolution.documentGeneration}`
    case "Historical": return `${backlink.resolution.pathRef} · historical checkpoint`
    case "Unavailable": return `unavailable · ${backlink.resolution.reason.replaceAll("_", " ")}`
  }
}

export const AgentProposalList = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement | null => state.agentCode.proposals.length === 0 ? null : <section className="oa-react-agent-proposal-list" aria-label="Agent proposals">
  <header><strong>Agent proposals</strong><Badge variant="outline">{state.agentCode.proposals.length}</Badge></header>
  {[...state.agentCode.proposals].reverse().map(proposal => <button type="button" key={proposal.proposalRef}
    aria-current={state.agentReviewProposalRef === proposal.proposalRef}
    onClick={() => dispatch(report, "DesktopAgentProposalSelected", proposal.proposalRef)}>
    <FileDiff aria-hidden="true" /><span><strong>{proposal.operations.length} {proposal.operations.length === 1 ? "operation" : "operations"}</strong><small>{proposal.lifecycle._tag} · attachment {proposal.attachment.attachmentGeneration}</small></span>
  </button>)}
</section>

export const AgentProposalReviewPanel = ({ state, report }: {
  readonly state: DesktopShellState
  readonly report: IntentReporter
}): ReactElement => {
  const proposal = selectedAgentProposal(state.agentCode, state.agentReviewProposalRef)
  const [layout, setLayout] = useState<"unified" | "split">("unified")
  const [contextLines, setContextLines] = useState(20)
  const [selectedRefs, setSelectedRefs] = useState<ReadonlyArray<string>>(() =>
    proposal?.operations.map(operation => operation.operationRef) ?? [])
  useEffect(() => setSelectedRefs(proposal?.operations.map(operation => operation.operationRef) ?? []), [proposal?.proposalRef])
  const currentGeneration = useMemo(() => {
    if (proposal?.operations.length !== 1) return null
    const sourceGeneration = state.workspaceEditor.tabs.find(tab => tab.pathRef === proposal.operations[0]?.pathRef)?.generation
    return sourceGeneration === undefined ? null : projectDocumentGenerationForSource(sourceGeneration)
  }, [proposal, state.workspaceEditor.tabs])
  const source = proposal === null ? null : agentProposalReviewSource(proposal, currentGeneration)
  const evidence = proposal === null ? [] : state.agentCode.evidence.filter(fact => fact.proposalRef === proposal.proposalRef)
  const backlinks = proposal === null ? [] : state.agentCode.backlinks.filter(link => link.proposalRef === proposal.proposalRef)
  const toggleOperation = (operationRef: string): void => setSelectedRefs(current =>
    current.includes(operationRef) ? current.filter(ref => ref !== operationRef) : [...current, operationRef])
  const decide = (disposition: "accept" | "reject"): void => {
    if (proposal === null || selectedRefs.length === 0) return
    dispatch(report, "DesktopAgentProposalDecisionRequested", { proposalRef: proposal.proposalRef, disposition, operationRefs: selectedRefs })
  }
  const onReviewIntent = (_intent: IdeReviewIntent): void => { /* Pierre selection remains review-local in IDE-08. */ }
  if (proposal === null) return <div className="oa-react-agent-proposal-empty"><FileDiff aria-hidden="true" /><h3>No agent proposal selected</h3><p>Agent output becomes an exact, reviewable proposal before it can change workspace files.</p></div>
  const lifecycle = proposal.lifecycle
  const undoExpired = lifecycle._tag === "Applied" && Date.now() > Date.parse(lifecycle.undoableUntil)
  return <section className="oa-react-agent-proposal-review" aria-label="Agent proposal review">
    <header className="oa-react-agent-proposal-heading">
      <div><Badge variant="secondary">{lifecycle._tag}</Badge><strong>{proposal.operations.length} proposed {proposal.operations.length === 1 ? "operation" : "operations"}</strong><small>worktree {proposal.attachment.worktreeRef} · attachment generation {proposal.attachment.attachmentGeneration}</small></div>
      <Button type="button" size="sm" variant="ghost" onClick={() => dispatch(report, "DesktopAgentProposalSelected", "")}>Repository changes</Button>
    </header>
    {state.agentCodeNotice === null ? null : <Alert><AlertTitle>Proposal status</AlertTitle><AlertDescription>{state.agentCodeNotice}</AlertDescription></Alert>}
    {lifecycle._tag === "RebaseRequired" ? <Alert variant="destructive"><AlertTitle>Explicit rebase required</AlertTitle><AlertDescription>The admitted base moved ({lifecycle.reason.replaceAll("_", " ")}; {lifecycle.conflictCount} conflicts). Current {lifecycle.currentPathRef} is {lifecycle.currentState}{lifecycle.currentDiskRevisionRef === null ? "" : ` at ${lifecycle.currentDiskRevisionRef}`}. The original base remains below; this proposal will not silently apply.</AlertDescription></Alert> : null}
    <fieldset className="oa-react-agent-operation-list" disabled={!(["Pending", "Reviewing"] as ReadonlyArray<string>).includes(lifecycle._tag)}>
      <legend>Select exact operations</legend>
      {proposal.operations.map(operation => <label key={operation.operationRef}>
        <input type="checkbox" checked={selectedRefs.includes(operation.operationRef)} onChange={() => toggleOperation(operation.operationRef)} />
        <span><strong>{operation._tag}</strong>{operationPath(operation)}<small>{operation.base.encoding} · {operation.base.lineEnding} · {operation.base.mode} · symlink {operation.policy.symlink}</small></span>
      </label>)}
    </fieldset>
    <div className="oa-react-review-toolbar" role="toolbar" aria-label="Agent proposal diff controls">
      <Button type="button" size="sm" variant={layout === "unified" ? "secondary" : "ghost"} aria-pressed={layout === "unified"} onClick={() => setLayout("unified")}>Unified</Button>
      <Button type="button" size="sm" variant={layout === "split" ? "secondary" : "ghost"} aria-pressed={layout === "split"} onClick={() => setLayout("split")}>Split</Button>
      <Button type="button" size="sm" variant="ghost" disabled={contextLines <= 5} onClick={() => setContextLines(value => Math.max(5, value - 5))}>Less context</Button>
      <span aria-live="polite">{contextLines} context lines</span>
      <Button type="button" size="sm" variant="ghost" disabled={contextLines >= 100} onClick={() => setContextLines(value => Math.min(100, value + 5))}>More context</Button>
    </div>
    {source === null ? <Alert variant="destructive"><AlertTitle>Proposal diff unavailable</AlertTitle><AlertDescription>The exact proposal exceeds the bounded diff policy.</AlertDescription></Alert>
      : <div className="oa-react-agent-proposal-diff"><PierreReviewAdapter source={source} options={{ mode: layout, contextLines, selection: null, annotations: [] }} onIntent={onReviewIntent} /></div>}
    <div className="oa-react-agent-proposal-actions" role="group" aria-label="Proposal decisions">
      {lifecycle._tag === "Pending" || lifecycle._tag === "Reviewing" ? <>
        <Button type="button" disabled={selectedRefs.length === 0} onClick={() => decide("accept")}><Check aria-hidden="true" />Accept selected</Button>
        <Button type="button" variant="destructive" disabled={selectedRefs.length === 0} onClick={() => decide("reject")}><X aria-hidden="true" />Reject selected</Button>
      </> : null}
      {lifecycle._tag === "Accepted" ? <Button type="button" onClick={() => dispatch(report, "DesktopAgentProposalApplyRequested", proposal.proposalRef)}><ShieldCheck aria-hidden="true" />Apply exact accepted proposal</Button> : null}
      {lifecycle._tag === "Applied" ? <Button type="button" variant="outline" disabled={undoExpired} onClick={() => dispatch(report, "DesktopAgentProposalUndoRequested", proposal.proposalRef)}><RotateCcw aria-hidden="true" />{undoExpired ? "Undo retention expired" : "Undo to checkpoint"}</Button> : null}
    </div>
    {proposal.lineage === null ? null : <section className="oa-react-agent-lineage" aria-label="ProductSpec lineage"><strong>ProductSpec lineage</strong><p>{proposal.lineage.criterionId} · {proposal.lineage.specRevisionRef} · packet {proposal.lineage.packetRef} · {proposal.lineage.terminalOutcome}</p></section>}
    <section className="oa-react-agent-evidence" aria-label="Post-apply evidence">
      <header><strong>Post-apply evidence</strong><small>Separate from harness completion</small></header>
      {evidence.length === 0 ? <p>No post-image evidence has been recorded.</p> : <ol>{evidence.map(fact => <li key={fact.evidenceRef}><Badge variant="outline">{fact.kind}</Badge><span>{evidenceLabel(fact)}</span><small>{fact.observedBy} · generation {fact.postImageGeneration}</small></li>)}</ol>}
    </section>
    <section className="oa-react-agent-backlinks" aria-label="Conversation code backlinks">
      <header><strong>Conversation ↔ code backlinks</strong>{proposal.conversationThreadRef === null
        ? <small>Creating conversation unavailable</small>
        : <Button type="button" size="sm" variant="ghost" onClick={() => dispatch(report, "DesktopAgentCreatingTurnOpened", proposal.proposalRef)}><History aria-hidden="true" />Open creating conversation</Button>}</header>
      {backlinks.length === 0 ? <p>No code backlinks were emitted.</p> : <ol>{backlinks.map(backlink => <li key={backlink.backlinkRef}><Button type="button" size="sm" variant="ghost" onClick={() => dispatch(report, "DesktopAgentBacklinkOpened", backlink.backlinkRef)}><History aria-hidden="true" />{backlinkLabel(backlink)}</Button></li>)}</ol>}
    </section>
  </section>
}
