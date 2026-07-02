----------------------------- MODULE FleetDelegateSupervisor -----------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Supervisors, WorkUnits, Capacity, MaxRetries

ASSUME Supervisors # {}
ASSUME WorkUnits # {}
ASSUME Capacity \in Nat
ASSUME Capacity <= Cardinality(WorkUnits)
ASSUME MaxRetries \in Nat

Phases == {"idle", "ensure", "advertise", "select", "prepare", "dispatch", "verify", "done", "blocked"}
RunStates == {"running", "paused", "draining", "completed"}

VARIABLES phase, retry, active, claims, runState, supervisorLease, claimedWhilePaused

vars == << phase, retry, active, claims, runState, supervisorLease, claimedWhilePaused >>

Init ==
  /\ phase = [s \in Supervisors |-> "idle"]
  /\ retry = [s \in Supervisors |-> 0]
  /\ active = {}
  /\ claims = [w \in WorkUnits |-> {}]
  /\ runState = "running"
  /\ supervisorLease = {}
  /\ claimedWhilePaused = FALSE

LiveClaims == {w \in WorkUnits : claims[w] # {}}
Unclaimed == WorkUnits \ LiveClaims

ActiveAssignmentsNeverExceedAdvertisedCapacity ==
  Cardinality(active) <= Capacity

ClaimUniquenessUnderRacingSupervisors ==
  /\ Cardinality(supervisorLease) <= 1
  /\ \A w \in WorkUnits : Cardinality(claims[w]) <= 1

PausedRunsClaimNothing ==
  claimedWhilePaused = FALSE

StartSupervisor(s) ==
  /\ runState = "running"
  /\ phase[s] = "idle"
  /\ supervisorLease = {}
  /\ phase' = [phase EXCEPT ![s] = "ensure"]
  /\ retry' = retry
  /\ active' = active
  /\ claims' = claims
  /\ runState' = runState
  /\ supervisorLease' = {s}
  /\ claimedWhilePaused' = claimedWhilePaused

EnsurePylon(s) ==
  /\ phase[s] = "ensure"
  /\ phase' = [phase EXCEPT ![s] = "advertise"]
  /\ UNCHANGED << retry, active, claims, runState, supervisorLease, claimedWhilePaused >>

AdvertiseCapacity(s) ==
  /\ phase[s] = "advertise"
  /\ phase' = [phase EXCEPT ![s] = "select"]
  /\ UNCHANGED << retry, active, claims, runState, supervisorLease, claimedWhilePaused >>

SelectWork(s) ==
  /\ phase[s] = "select"
  /\ IF /\ runState = "running"
        /\ Unclaimed # {}
        /\ Cardinality(active) < Capacity
     THEN phase' = [phase EXCEPT ![s] = "prepare"]
          /\ retry' = retry
     ELSE IF retry[s] < MaxRetries
       THEN /\ phase' = [phase EXCEPT ![s] = "advertise"]
            /\ retry' = [retry EXCEPT ![s] = @ + 1]
       ELSE /\ phase' = [phase EXCEPT ![s] = "blocked"]
            /\ retry' = retry
  /\ active' = active
  /\ claims' = claims
  /\ runState' = runState
  /\ supervisorLease' = supervisorLease
  /\ claimedWhilePaused' = claimedWhilePaused

PrepareWork(s) ==
  /\ phase[s] = "prepare"
  /\ phase' = [phase EXCEPT ![s] = "dispatch"]
  /\ UNCHANGED << retry, active, claims, runState, supervisorLease, claimedWhilePaused >>

DispatchWork(s, w) ==
  /\ phase[s] = "dispatch"
  /\ runState = "running"
  /\ w \in Unclaimed
  /\ Cardinality(active) < Capacity
  /\ phase' = [phase EXCEPT ![s] = "verify"]
  /\ active' = active \cup {w}
  /\ claims' = [claims EXCEPT ![w] = {s}]
  /\ claimedWhilePaused' = claimedWhilePaused \/ (runState = "paused")
  /\ UNCHANGED << retry, runState, supervisorLease >>

CompleteAssignment(w) ==
  /\ w \in active
  /\ active' = active \ {w}
  /\ phase' = phase
  /\ retry' = retry
  /\ claims' = claims
  /\ runState' = runState
  /\ supervisorLease' = supervisorLease
  /\ claimedWhilePaused' = claimedWhilePaused

VerifyCloseout(s) ==
  /\ phase[s] = "verify"
  /\ phase' = [phase EXCEPT ![s] = "advertise"]
  /\ UNCHANGED << retry, active, claims, runState, supervisorLease, claimedWhilePaused >>

FinishSupervisor(s) ==
  /\ phase[s] \in {"advertise", "select", "verify"}
  /\ Unclaimed = {}
  /\ active = {}
  /\ phase' = [phase EXCEPT ![s] = "done"]
  /\ runState' = "completed"
  /\ supervisorLease' = supervisorLease \ {s}
  /\ UNCHANGED << retry, active, claims, claimedWhilePaused >>

PauseRun ==
  /\ runState = "running"
  /\ runState' = "paused"
  /\ UNCHANGED << phase, retry, active, claims, supervisorLease, claimedWhilePaused >>

ResumeRun ==
  /\ runState = "paused"
  /\ runState' = "running"
  /\ UNCHANGED << phase, retry, active, claims, supervisorLease, claimedWhilePaused >>

DrainRun ==
  /\ runState \in {"running", "paused"}
  /\ runState' = "draining"
  /\ UNCHANGED << phase, retry, active, claims, supervisorLease, claimedWhilePaused >>

DrainTerminates ==
  /\ runState = "draining"
  /\ active = {}
  /\ runState' = "completed"
  /\ phase' = [s \in Supervisors |-> IF s \in supervisorLease THEN "done" ELSE phase[s]]
  /\ supervisorLease' = {}
  /\ UNCHANGED << retry, active, claims, claimedWhilePaused >>

Next ==
  \/ \E s \in Supervisors : StartSupervisor(s)
  \/ \E s \in Supervisors : EnsurePylon(s)
  \/ \E s \in Supervisors : AdvertiseCapacity(s)
  \/ \E s \in Supervisors : SelectWork(s)
  \/ \E s \in Supervisors : PrepareWork(s)
  \/ \E s \in Supervisors, w \in WorkUnits : DispatchWork(s, w)
  \/ \E w \in WorkUnits : CompleteAssignment(w)
  \/ \E s \in Supervisors : VerifyCloseout(s)
  \/ \E s \in Supervisors : FinishSupervisor(s)
  \/ PauseRun
  \/ ResumeRun
  \/ DrainRun
  \/ DrainTerminates

DeadEndClassUnreachable ==
  \/ runState = "completed"
  \/ \E s \in Supervisors : phase[s] \in {"done", "blocked"}
  \/ ENABLED Next

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

TerminationUnderBoundedRetries ==
  <> (runState = "completed" \/ \A s \in Supervisors : phase[s] \in {"idle", "done", "blocked"})

DrainEventuallyTerminates ==
  [](runState = "draining" => <> (runState = "completed"))

================================================================================
