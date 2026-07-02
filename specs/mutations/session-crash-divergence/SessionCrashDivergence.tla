--------------------------- MODULE SessionCrashDivergence ---------------------------
EXTENDS FiniteSets, TLC

CONSTANTS Sessions, Threads

ASSUME Sessions # {}
ASSUME Threads # {}

None == "none"

VARIABLES sessionThread, threadSession, persisted, crashed

vars == <<sessionThread, threadSession, persisted, crashed>>

Init ==
  /\ sessionThread = [s \in Sessions |-> None]
  /\ threadSession = [t \in Threads |-> None]
  /\ persisted = {}
  /\ crashed = FALSE

Pair(s, t) == <<s, t>>
PersistedForSession(s) == {p \in persisted : p[1] = s}

BindAndPersist(s, t) ==
  /\ ~crashed
  /\ sessionThread[s] = None
  /\ threadSession[t] = None
  /\ sessionThread' = [sessionThread EXCEPT ![s] = t]
  /\ threadSession' = [threadSession EXCEPT ![t] = s]
  /\ persisted' = persisted \cup {Pair(s, t)}
  /\ UNCHANGED crashed

Crash ==
  /\ ~crashed
  /\ crashed' = TRUE
  /\ UNCHANGED <<sessionThread, threadSession, persisted>>

\* Mutation: reload restores only session -> thread, dropping the reverse map.
ReloadDropsReverseMap ==
  /\ crashed
  /\ sessionThread' =
      [s \in Sessions |->
        IF PersistedForSession(s) # {}
        THEN (CHOOSE p \in PersistedForSession(s) : TRUE)[2]
        ELSE None]
  /\ threadSession' = [t \in Threads |-> None]
  /\ crashed' = FALSE
  /\ UNCHANGED persisted

Next ==
  \/ \E s \in Sessions, t \in Threads : BindAndPersist(s, t)
  \/ Crash
  \/ ReloadDropsReverseMap

Spec == Init /\ [][Next]_vars

NoOrphanThreadBinding ==
  /\ \A s \in Sessions : sessionThread[s] = None \/ threadSession[sessionThread[s]] = s
  /\ \A t \in Threads : threadSession[t] = None \/ sessionThread[threadSession[t]] = t

================================================================================
