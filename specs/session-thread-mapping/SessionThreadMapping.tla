----------------------------- MODULE SessionThreadMapping -----------------------------
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Sessions, Threads

ASSUME Sessions # {}
ASSUME Threads # {}

None == "none"

VARIABLES sessionThread, threadSession, persisted, pendingPersist, crashed

vars == <<sessionThread, threadSession, persisted, pendingPersist, crashed>>

Init ==
  /\ sessionThread = [s \in Sessions |-> None]
  /\ threadSession = [t \in Threads |-> None]
  /\ persisted = {}
  /\ pendingPersist = {}
  /\ crashed = FALSE

Pair(s, t) == <<s, t>>
PersistedForSession(s) == {p \in persisted : p[1] = s}
PersistedForThread(t) == {p \in persisted : p[2] = t}

Bind(s, t) ==
  /\ ~crashed
  /\ sessionThread[s] = None
  /\ threadSession[t] = None
  /\ sessionThread' = [sessionThread EXCEPT ![s] = t]
  /\ threadSession' = [threadSession EXCEPT ![t] = s]
  /\ pendingPersist' = pendingPersist \cup {Pair(s, t)}
  /\ UNCHANGED <<persisted, crashed>>

PersistBinding(s, t) ==
  /\ ~crashed
  /\ Pair(s, t) \in pendingPersist
  /\ sessionThread[s] = t
  /\ threadSession[t] = s
  /\ persisted' = persisted \cup {Pair(s, t)}
  /\ pendingPersist' = pendingPersist \ {Pair(s, t)}
  /\ UNCHANGED <<sessionThread, threadSession, crashed>>

ArchiveSession(s) ==
  /\ ~crashed
  /\ sessionThread[s] # None
  /\ LET t == sessionThread[s] IN
     /\ sessionThread' = [sessionThread EXCEPT ![s] = None]
     /\ threadSession' = [threadSession EXCEPT ![t] = None]
     /\ persisted' = persisted \ {Pair(s, t)}
     /\ pendingPersist' = pendingPersist \ {Pair(s, t)}
  /\ UNCHANGED crashed

Crash ==
  /\ ~crashed
  /\ crashed' = TRUE
  /\ UNCHANGED <<sessionThread, threadSession, persisted, pendingPersist>>

Reload ==
  /\ crashed
  /\ \A s \in Sessions : Cardinality(PersistedForSession(s)) <= 1
  /\ \A t \in Threads : Cardinality(PersistedForThread(t)) <= 1
  /\ sessionThread' =
      [s \in Sessions |->
        IF PersistedForSession(s) # {}
        THEN (CHOOSE p \in PersistedForSession(s) : TRUE)[2]
        ELSE None]
  /\ threadSession' =
      [t \in Threads |->
        IF PersistedForThread(t) # {}
        THEN (CHOOSE p \in PersistedForThread(t) : TRUE)[1]
        ELSE None]
  /\ pendingPersist' = {}
  /\ crashed' = FALSE
  /\ UNCHANGED persisted

Next ==
  \/ \E s \in Sessions, t \in Threads : Bind(s, t)
  \/ \E s \in Sessions, t \in Threads : PersistBinding(s, t)
  \/ \E s \in Sessions : ArchiveSession(s)
  \/ Crash
  \/ Reload

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Reload)

NoOrphanThreadBinding ==
  /\ \A s \in Sessions : sessionThread[s] = None \/ threadSession[sessionThread[s]] = s
  /\ \A t \in Threads : threadSession[t] = None \/ sessionThread[threadSession[t]] = t

NoDoubleBind ==
  /\ \A s1, s2 \in Sessions :
      /\ s1 # s2
      /\ sessionThread[s1] # None
      => sessionThread[s1] # sessionThread[s2]
  /\ \A t1, t2 \in Threads :
      /\ t1 # t2
      /\ threadSession[t1] # None
      => threadSession[t1] # threadSession[t2]

PersistedMappingConsistent ==
  /\ \A p \in persisted :
      /\ p[1] \in Sessions
      /\ p[2] \in Threads
      /\ sessionThread[p[1]] = p[2]
      /\ threadSession[p[2]] = p[1]
  /\ \A s \in Sessions : Cardinality(PersistedForSession(s)) <= 1
  /\ \A t \in Threads : Cardinality(PersistedForThread(t)) <= 1

CrashReloadEventuallyRestoresBindings ==
  [](crashed => <> ~crashed)

================================================================================
