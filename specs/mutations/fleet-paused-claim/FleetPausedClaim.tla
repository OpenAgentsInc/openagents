------------------------------- MODULE FleetPausedClaim -------------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Supervisors, WorkUnits

ASSUME Supervisors # {}
ASSUME WorkUnits # {}

None == "none"

VARIABLES runState, claims, holders, pausedClaimForged

vars == <<runState, claims, holders, pausedClaimForged>>

Init ==
  /\ runState = "running"
  /\ claims = [w \in WorkUnits |-> "none"]
  /\ holders = [w \in WorkUnits |-> None]
  /\ pausedClaimForged = FALSE

PauseRun ==
  /\ runState = "running"
  /\ runState' = "paused"
  /\ UNCHANGED <<claims, holders, pausedClaimForged>>

\* Mutation: the real spec requires runState = "running" before a claim.
ClaimWhilePaused(s, w) ==
  /\ runState = "paused"
  /\ claims[w] = "none"
  /\ claims' = [claims EXCEPT ![w] = "claimed"]
  /\ holders' = [holders EXCEPT ![w] = s]
  /\ pausedClaimForged' = TRUE
  /\ UNCHANGED runState

Next ==
  \/ PauseRun
  \/ \E s \in Supervisors, w \in WorkUnits : ClaimWhilePaused(s, w)

Spec == Init /\ [][Next]_vars

PausedRunsClaimNothing == pausedClaimForged = FALSE

================================================================================
