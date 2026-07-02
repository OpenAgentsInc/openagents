------------------------------- MODULE ApprovalProtocol -------------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Requests

ASSUME Requests # {}

States == {"never", "requested", "approved", "rejected", "interrupted", "superseded"}
Responses == {"none", "approve", "reject", "interrupt", "supersede"}

VARIABLES requestState, response, epoch, requestEpoch, forgedStaleResponse

vars == <<requestState, response, epoch, requestEpoch, forgedStaleResponse>>

Init ==
  /\ requestState = [r \in Requests |-> "never"]
  /\ response = [r \in Requests |-> "none"]
  /\ epoch = 0
  /\ requestEpoch = [r \in Requests |-> 0]
  /\ forgedStaleResponse = FALSE

IssueRequest(r) ==
  /\ requestState[r] = "never"
  /\ requestState' = [requestState EXCEPT ![r] = "requested"]
  /\ requestEpoch' = [requestEpoch EXCEPT ![r] = epoch]
  /\ UNCHANGED <<response, epoch, forgedStaleResponse>>

Approve(r) ==
  /\ requestState[r] = "requested"
  /\ requestEpoch[r] = epoch
  /\ requestState' = [requestState EXCEPT ![r] = "approved"]
  /\ response' = [response EXCEPT ![r] = "approve"]
  /\ UNCHANGED <<epoch, requestEpoch, forgedStaleResponse>>

Reject(r) ==
  /\ requestState[r] = "requested"
  /\ requestEpoch[r] = epoch
  /\ requestState' = [requestState EXCEPT ![r] = "rejected"]
  /\ response' = [response EXCEPT ![r] = "reject"]
  /\ UNCHANGED <<epoch, requestEpoch, forgedStaleResponse>>

InterruptTurn(r) ==
  /\ requestState[r] = "requested"
  /\ requestState' = [requestState EXCEPT ![r] = "interrupted"]
  /\ response' = [response EXCEPT ![r] = "interrupt"]
  /\ epoch' = epoch + 1
  /\ UNCHANGED <<requestEpoch, forgedStaleResponse>>

Supersede(r) ==
  /\ requestState[r] = "requested"
  /\ requestState' = [requestState EXCEPT ![r] = "superseded"]
  /\ response' = [response EXCEPT ![r] = "supersede"]
  /\ epoch' = epoch + 1
  /\ UNCHANGED <<requestEpoch, forgedStaleResponse>>

StaleApproveRejected(r) ==
  /\ requestState[r] \in {"interrupted", "superseded"}
  /\ requestEpoch[r] # epoch
  /\ forgedStaleResponse' = forgedStaleResponse \/ FALSE
  /\ UNCHANGED <<requestState, response, epoch, requestEpoch>>

TerminalStable ==
  /\ \A r \in Requests : requestState[r] # "requested" /\ requestState[r] # "never"
  /\ UNCHANGED vars

Next ==
  \/ \E r \in Requests : IssueRequest(r)
  \/ \E r \in Requests : Approve(r)
  \/ \E r \in Requests : Reject(r)
  \/ \E r \in Requests : InterruptTurn(r)
  \/ \E r \in Requests : Supersede(r)
  \/ \E r \in Requests : StaleApproveRejected(r)
  \/ TerminalStable

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(\E r \in Requests : IssueRequest(r))
  /\ WF_vars(\E r \in Requests : Approve(r) \/ Reject(r) \/ InterruptTurn(r) \/ Supersede(r))

NoLostApprovals ==
  \A r \in Requests :
    requestState[r] = "requested" \/ requestState[r] = "never" \/ response[r] # "none"

NoDuplicateApprovalResponses ==
  \A r \in Requests :
    response[r] = "none" \/ requestState[r] \in {"approved", "rejected", "interrupted", "superseded"}

NoStaleRequestForgery ==
  forgedStaleResponse = FALSE

AllIssuedRequestsEventuallyClose ==
  <> (\A r \in Requests : requestState[r] # "requested")

================================================================================
