----------------------------- MODULE ApprovalStaleForgery -----------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Requests

ASSUME Requests # {}

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

InterruptTurn(r) ==
  /\ requestState[r] = "requested"
  /\ requestState' = [requestState EXCEPT ![r] = "interrupted"]
  /\ response' = [response EXCEPT ![r] = "interrupt"]
  /\ epoch' = epoch + 1
  /\ UNCHANGED <<requestEpoch, forgedStaleResponse>>

\* Mutation: a stale approval after interruption is accepted as a response.
StaleApproveForged(r) ==
  /\ requestState[r] = "interrupted"
  /\ requestEpoch[r] # epoch
  /\ response' = [response EXCEPT ![r] = "approve"]
  /\ forgedStaleResponse' = TRUE
  /\ UNCHANGED <<requestState, epoch, requestEpoch>>

Next ==
  \/ \E r \in Requests : IssueRequest(r)
  \/ \E r \in Requests : InterruptTurn(r)
  \/ \E r \in Requests : StaleApproveForged(r)

Spec == Init /\ [][Next]_vars

NoStaleRequestForgery == forgedStaleResponse = FALSE

================================================================================
