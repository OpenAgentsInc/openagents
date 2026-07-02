------------------------------- MODULE ApprovalStaleForgery -------------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Requests

ASSUME Requests # {}

States == {"never", "requested", "approved", "rejected", "interrupted", "superseded"}
Responses == {"none", "approve", "reject", "interrupt", "supersede"}

VARIABLES requestState, response, epoch, requestEpoch

vars == <<requestState, response, epoch, requestEpoch>>

Init ==
  /\ requestState = [r \in Requests |-> "never"]
  /\ response = [r \in Requests |-> "none"]
  /\ epoch = 0
  /\ requestEpoch = [r \in Requests |-> 0]

IssueRequest(r) ==
  /\ requestState[r] = "never"
  /\ requestState' = [requestState EXCEPT ![r] = "requested"]
  /\ requestEpoch' = [requestEpoch EXCEPT ![r] = epoch]
  /\ UNCHANGED <<response, epoch>>

\* MUTATION: drops the `requestEpoch[r] = epoch` stale guard from Approve.
\* TLC must report the NoStaleApproveApplication action property violated.
Approve(r) ==
  /\ requestState[r] = "requested"
  /\ requestState' = [requestState EXCEPT ![r] = "approved"]
  /\ response' = [response EXCEPT ![r] = "approve"]
  /\ UNCHANGED <<epoch, requestEpoch>>

Reject(r) ==
  /\ requestState[r] = "requested"
  /\ requestEpoch[r] = epoch
  /\ requestState' = [requestState EXCEPT ![r] = "rejected"]
  /\ response' = [response EXCEPT ![r] = "reject"]
  /\ UNCHANGED <<epoch, requestEpoch>>

InterruptTurn(r) ==
  /\ requestState[r] = "requested"
  /\ requestState' = [requestState EXCEPT ![r] = "interrupted"]
  /\ response' = [response EXCEPT ![r] = "interrupt"]
  /\ epoch' = epoch + 1
  /\ UNCHANGED <<requestEpoch>>

Supersede(r) ==
  /\ requestState[r] = "requested"
  /\ requestState' = [requestState EXCEPT ![r] = "superseded"]
  /\ response' = [response EXCEPT ![r] = "supersede"]
  /\ epoch' = epoch + 1
  /\ UNCHANGED <<requestEpoch>>

\* A stale approve attempt: the harness retries an approve for a request
\* whose turn epoch has already advanced. The epoch guard on Approve is the
\* runtime defense; this action models the attempt so the action property
\* NoStaleApproveApplication below is falsifiable if that guard is dropped.
StaleApproveAttempt(r) ==
  /\ requestState[r] \in {"interrupted", "superseded"}
  /\ requestEpoch[r] # epoch
  /\ UNCHANGED vars

TerminalStable ==
  /\ \A r \in Requests : requestState[r] # "requested" /\ requestState[r] # "never"
  /\ UNCHANGED vars

Next ==
  \/ \E r \in Requests : IssueRequest(r)
  \/ \E r \in Requests : Approve(r)
  \/ \E r \in Requests : Reject(r)
  \/ \E r \in Requests : InterruptTurn(r)
  \/ \E r \in Requests : Supersede(r)
  \/ \E r \in Requests : StaleApproveAttempt(r)
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

\* Action property: an approve can only ever be applied to a request whose
\* recorded epoch is the current turn epoch. Falsifiable: dropping Approve's
\* `requestEpoch[r] = epoch` guard makes TLC report a violation. The naive
\* state-invariant form is wrong because epoch legitimately advances after
\* an approve.
NoStaleApproveApplication ==
  [][\A r \in Requests :
       (response[r] # "approve" /\ response'[r] = "approve") => requestEpoch[r] = epoch]_vars

AllIssuedRequestsEventuallyClose ==
  \A r \in Requests :
    (requestState[r] = "requested") ~> (requestState[r] # "requested")

================================================================================
