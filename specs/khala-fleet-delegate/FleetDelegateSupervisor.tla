----------------------------- MODULE FleetDelegateSupervisor -----------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Supervisors, WorkUnits, Capacity, ClaimTtl, MaxReleases

ASSUME Supervisors # {}
ASSUME WorkUnits # {}
ASSUME Capacity \in Nat
ASSUME Capacity > 0
ASSUME Capacity <= Cardinality(WorkUnits)
ASSUME ClaimTtl \in Nat
ASSUME ClaimTtl > 0
ASSUME MaxReleases \in Nat

None == "none"
LiveClaimStates == {"claimed", "in_progress", "closeout"}
ClaimStates == LiveClaimStates \cup {"none", "released", "expired"}
RunStates == {"running", "paused", "draining", "stopped", "completed"}
StateSources == {"none", "operator", "reconcile"}

VARIABLES
  runState,
  stateSource,
  claims,
  holders,
  ttl,
  releases,
  active,
  terminal,
  supervisorLease,
  pausedClaimForged,
  operatorRevived,
  operatorIntervened

vars == <<runState, stateSource, claims, holders, ttl, releases, active, terminal,
          supervisorLease, pausedClaimForged, operatorRevived, operatorIntervened>>

Init ==
  /\ runState = "running"
  /\ stateSource = "none"
  /\ claims = [w \in WorkUnits |-> "none"]
  /\ holders = [w \in WorkUnits |-> None]
  /\ ttl = [w \in WorkUnits |-> 0]
  /\ releases = [w \in WorkUnits |-> 0]
  /\ active = {}
  /\ terminal = {}
  /\ supervisorLease = {}
  /\ pausedClaimForged = FALSE
  /\ operatorRevived = FALSE
  /\ operatorIntervened = FALSE

LiveClaims == {w \in WorkUnits : claims[w] \in LiveClaimStates}
OpenClaims == {w \in WorkUnits : claims[w] \in {"claimed", "in_progress"}}
Claimable == {w \in WorkUnits : w \notin (LiveClaims \cup terminal) /\ releases[w] <= MaxReleases}
AutoRevivable ==
  (runState = "completed" \/ runState = "stopped") /\ stateSource # "operator"

ActiveAssignmentsNeverExceedAdvertisedCapacity ==
  Cardinality(active) <= Capacity

ClaimUniquenessUnderRacingSupervisors ==
  /\ Cardinality(supervisorLease) <= 1
  /\ \A w \in WorkUnits :
      claims[w] \in LiveClaimStates => holders[w] \in Supervisors

PausedRunsClaimNothing ==
  pausedClaimForged = FALSE

SupervisorReviveHonorsAutoRevivableGuard ==
  operatorRevived = FALSE

AcquireSupervisor(s) ==
  /\ runState = "running"
  /\ supervisorLease = {}
  /\ supervisorLease' = {s}
  /\ UNCHANGED <<runState, stateSource, claims, holders, ttl, releases, active, terminal,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

ReleaseSupervisor(s) ==
  /\ supervisorLease = {s}
  /\ runState \in {"completed", "stopped"}
  /\ supervisorLease' = {}
  /\ UNCHANGED <<runState, stateSource, claims, holders, ttl, releases, active, terminal,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

TryClaim(s, w) ==
  /\ supervisorLease = {s}
  /\ runState = "running"
  /\ w \in Claimable
  /\ Cardinality(OpenClaims) < Capacity
  /\ claims' = [claims EXCEPT ![w] = "claimed"]
  /\ holders' = [holders EXCEPT ![w] = s]
  /\ ttl' = [ttl EXCEPT ![w] = ClaimTtl]
  /\ pausedClaimForged' = pausedClaimForged \/ (runState = "paused")
  /\ UNCHANGED <<runState, stateSource, releases, active, terminal, supervisorLease,
                 operatorRevived, operatorIntervened>>

DispatchClaim(w) ==
  /\ claims[w] = "claimed"
  /\ runState = "running"
  /\ w \notin active
  /\ active' = active \cup {w}
  /\ claims' = [claims EXCEPT ![w] = "in_progress"]
  /\ UNCHANGED <<runState, stateSource, holders, ttl, releases, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

CompleteWork(w) ==
  /\ w \in active
  /\ active' = active \ {w}
  /\ terminal' = terminal \cup {w}
  /\ claims' = [claims EXCEPT ![w] = "closeout"]
  /\ ttl' = [ttl EXCEPT ![w] = 0]
  /\ UNCHANGED <<runState, stateSource, holders, releases, supervisorLease,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

ReleaseClaim(w) ==
  /\ claims[w] \in {"claimed", "in_progress"}
  /\ releases[w] < MaxReleases
  /\ active' = active \ {w}
  /\ claims' = [claims EXCEPT ![w] = "released"]
  /\ holders' = [holders EXCEPT ![w] = None]
  /\ ttl' = [ttl EXCEPT ![w] = 0]
  /\ releases' = [releases EXCEPT ![w] = @ + 1]
  /\ UNCHANGED <<runState, stateSource, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

ExpireClaim(w) ==
  /\ claims[w] \in {"claimed", "in_progress"}
  /\ ttl[w] = 0
  /\ active' = active \ {w}
  /\ claims' = [claims EXCEPT ![w] = "expired"]
  /\ holders' = [holders EXCEPT ![w] = None]
  /\ releases' = [releases EXCEPT ![w] = @ + 1]
  /\ UNCHANGED <<runState, stateSource, ttl, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

TickTime ==
  /\ \E w \in WorkUnits : ttl[w] > 0
  /\ ttl' = [w \in WorkUnits |-> IF ttl[w] > 0 THEN ttl[w] - 1 ELSE ttl[w]]
  /\ UNCHANGED <<runState, stateSource, claims, holders, releases, active, terminal,
                 supervisorLease, pausedClaimForged, operatorRevived, operatorIntervened>>

PauseRun ==
  /\ runState = "running"
  /\ runState' = "paused"
  /\ stateSource' = "operator"
  /\ operatorIntervened' = TRUE
  /\ UNCHANGED <<claims, holders, ttl, releases, active, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived>>

ResumeRun ==
  /\ runState = "paused"
  /\ runState' = "running"
  /\ stateSource' = "operator"
  /\ operatorIntervened' = TRUE
  /\ UNCHANGED <<claims, holders, ttl, releases, active, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived>>

DrainRun ==
  /\ runState \in {"running", "paused"}
  /\ runState' = "draining"
  /\ stateSource' = "operator"
  /\ operatorIntervened' = TRUE
  /\ UNCHANGED <<claims, holders, ttl, releases, active, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived>>

ReconcileClose ==
  /\ runState \in {"running", "draining"}
  /\ Claimable = {}
  /\ active = {}
  /\ runState' = IF terminal = WorkUnits THEN "completed" ELSE "stopped"
  /\ stateSource' = IF runState = "draining" THEN "operator" ELSE "reconcile"
  /\ UNCHANGED <<claims, holders, ttl, releases, active, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived, operatorIntervened>>

DrainComplete ==
  /\ runState = "draining"
  /\ active = {}
  /\ runState' = "stopped"
  /\ stateSource' = "operator"
  /\ operatorIntervened' = TRUE
  /\ UNCHANGED <<claims, holders, ttl, releases, active, terminal, supervisorLease,
                 pausedClaimForged, operatorRevived>>

ReviveForPlannerBacklog ==
  /\ AutoRevivable
  /\ Claimable # {}
  /\ runState' = "running"
  /\ stateSource' = "reconcile"
  /\ operatorRevived' = operatorRevived \/ (stateSource = "operator")
  /\ UNCHANGED <<claims, holders, ttl, releases, active, terminal, supervisorLease,
                 pausedClaimForged, operatorIntervened>>

TerminalStable ==
  /\ runState \in {"completed", "stopped"}
  /\ UNCHANGED vars

Next ==
  \/ \E s \in Supervisors : AcquireSupervisor(s)
  \/ \E s \in Supervisors : ReleaseSupervisor(s)
  \/ \E s \in Supervisors, w \in WorkUnits : TryClaim(s, w)
  \/ \E w \in WorkUnits : DispatchClaim(w)
  \/ \E w \in WorkUnits : CompleteWork(w)
  \/ \E w \in WorkUnits : ReleaseClaim(w)
  \/ \E w \in WorkUnits : ExpireClaim(w)
  \/ TickTime
  \/ PauseRun
  \/ ResumeRun
  \/ DrainRun
  \/ ReconcileClose
  \/ DrainComplete
  \/ ReviveForPlannerBacklog
  \/ TerminalStable

SafetySpec ==
  /\ Init
  /\ [][Next]_vars

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(\E s \in Supervisors : AcquireSupervisor(s))
  /\ WF_vars(\E s \in Supervisors, w \in WorkUnits : TryClaim(s, w))
  /\ WF_vars(\E w \in WorkUnits : DispatchClaim(w))
  /\ WF_vars(\E w \in WorkUnits : CompleteWork(w))
  /\ WF_vars(\E w \in WorkUnits : ExpireClaim(w))
  /\ WF_vars(TickTime)
  /\ WF_vars(ReconcileClose)
  /\ WF_vars(DrainComplete)
  /\ WF_vars(ReviveForPlannerBacklog)
  /\ WF_vars(\E s \in Supervisors : ReleaseSupervisor(s))

TerminationUnderBoundedClaims ==
  <> (operatorIntervened \/ (runState \in {"completed", "stopped"} /\ active = {} /\ OpenClaims = {}))

DrainEventuallyTerminates ==
  [](runState = "draining" => <> (runState \in {"completed", "stopped"}))

================================================================================
